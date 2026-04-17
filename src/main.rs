//! cargo-cross: Cross-compilation tool for Rust projects

use cargo_cross::{
    cargo::{build_cargo_env, ensure_rust_src, ensure_target_installed, execute_cargo},
    cli::{parse_args, print_all_targets, print_version, ParseResult, SetupOutputFormat},
    color,
    config::{get_target_config, HostPlatform},
    error::{run_command, Result},
    platform::setup_cross_env,
    sanitize_cargo_env,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;
use tokio::process::Command as TokioCommand;

/// Format duration as human-readable string
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs >= 60 {
        let mins = secs / 60;
        let remaining_secs = secs % 60;
        format!("{mins}m {remaining_secs}s")
    } else {
        format!("{secs}s")
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    // Sanitize environment variables that could cause cargo errors
    sanitize_cargo_env();

    // Setup signal handlers for Ctrl+C and SIGTERM
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};

        // Handle SIGINT (Ctrl+C)
        tokio::spawn(async move {
            if let Ok(mut sigint) = signal(SignalKind::interrupt()) {
                sigint.recv().await;
                std::process::exit(130);
            }
        });

        // Handle SIGTERM
        tokio::spawn(async move {
            if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
                sigterm.recv().await;
                std::process::exit(143);
            }
        });
    }

    match run().await {
        Ok(code) => code,
        Err(e) => {
            color::log_error(&format!("Error: {e}"));
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<ExitCode> {
    match parse_args()? {
        ParseResult::Build(args) => run_cargo(*args).await,
        ParseResult::Setup(args) => run_setup(*args).await,
        ParseResult::Exec(args) => run_exec(*args).await,
        ParseResult::ShowTargets(format) => {
            print_all_targets(format);
            Ok(ExitCode::SUCCESS)
        }
        ParseResult::ShowVersion => {
            print_version();
            Ok(ExitCode::SUCCESS)
        }
    }
}

struct PreparedTarget {
    actual_target: String,
    skip_target_arg: bool,
    cross_env: cargo_cross::env::CrossEnv,
}

async fn run_cargo(args: cargo_cross::Args) -> Result<ExitCode> {
    let host = HostPlatform::detect();
    print_config(&args, &host);
    let total_targets = args.targets.len();
    let start_time = std::time::Instant::now();

    for (i, target) in args.targets.iter().enumerate() {
        color::log_success(&format!(
            "[{}/{}] Processing target: {}",
            color::yellow(&(i + 1).to_string()),
            color::yellow(&total_targets.to_string()),
            color::cyan(target)
        ));

        let target_start = std::time::Instant::now();
        let result = execute_target(target, &args, &host).await;
        let target_elapsed = target_start.elapsed();

        if let Err(e) = result {
            let command_cap = capitalize_command(args.command.as_str());
            color::log_error(&format!(
                "{command_cap} failed for target: {}",
                color::yellow(target)
            ));
            color::log_error(&format!("Error: {}", color::white(&e.to_string())));
            return Ok(ExitCode::FAILURE);
        }

        color::log_success(&format!(
            "Target {} completed (took {})",
            color::yellow(target),
            color::yellow(&format_duration(target_elapsed))
        ));
    }

    let elapsed = start_time.elapsed();
    color::print_separator();
    color::log_success(&format!(
        "All {} operations completed successfully!",
        color::cyan(args.command.as_str())
    ));
    color::log_success(&format!(
        "Total time: {}",
        color::yellow(&format_duration(elapsed))
    ));

    set_github_output(&args);

    Ok(ExitCode::SUCCESS)
}

async fn run_setup(setup: cargo_cross::cli::SetupArgs) -> Result<ExitCode> {
    if setup.args.targets.len() != 1 {
        return Err(cargo_cross::CrossError::InvalidArgument(
            "setup requires exactly one target; pass a single --target value".to_string(),
        ));
    }

    let host = HostPlatform::detect();
    let target = &setup.args.targets[0];
    let _guard = LogSilenceGuard::new();
    let prepared = prepare_target(target, &setup.args, &host).await?;
    let env = build_cargo_env(
        &prepared.actual_target,
        &setup.args,
        &prepared.cross_env,
        &host,
        prepared.skip_target_arg,
    );

    write_setup_github_env(&env)?;
    print_setup_env(&env, setup.format)?;
    set_github_output(&setup.args);
    Ok(ExitCode::SUCCESS)
}

async fn run_exec(exec: cargo_cross::cli::ExecArgs) -> Result<ExitCode> {
    let host = HostPlatform::detect();
    print_config(&exec.args, &host);
    println!(
        "{}",
        color::format_config("Exec command", &format_cli_command(&exec.command))
    );

    let total_targets = exec.args.targets.len();
    for (i, target) in exec.args.targets.iter().enumerate() {
        color::log_success(&format!(
            "[{}/{}] Processing target: {}",
            color::yellow(&(i + 1).to_string()),
            color::yellow(&total_targets.to_string()),
            color::cyan(target)
        ));

        if let Err(e) = execute_exec_target(target, &exec.args, &exec.command, &host).await {
            color::log_error(&format!(
                "Exec failed for target: {}",
                color::yellow(target)
            ));
            color::log_error(&format!("Error: {}", color::white(&e.to_string())));
            return Ok(ExitCode::FAILURE);
        }
    }

    set_github_output(&exec.args);
    Ok(ExitCode::SUCCESS)
}

async fn execute_target(target: &str, args: &cargo_cross::Args, host: &HostPlatform) -> Result<()> {
    color::print_separator();
    color::log_info(&format!(
        "Executing {} for {}...",
        color::magenta(args.command.as_str()),
        color::magenta(target)
    ));

    if args.clean_cache {
        color::log_info("Cleaning cache...");
        let _ = TokioCommand::new("cargo").arg("clean").status().await;
    }

    let prepared = prepare_target(target, args, host).await?;

    let status = execute_cargo(
        &prepared.actual_target,
        args,
        &prepared.cross_env,
        host,
        prepared.skip_target_arg,
    )
    .await?;

    if !status.success() {
        return Err(cargo_cross::CrossError::CargoFailed {
            code: status.code().unwrap_or(1),
        });
    }

    let command_cap = capitalize_command(args.command.as_str());
    color::log_success(&format!(
        "{command_cap} successful: {}",
        color::yellow(&prepared.actual_target)
    ));

    Ok(())
}

async fn execute_exec_target(
    target: &str,
    args: &cargo_cross::Args,
    command: &[String],
    host: &HostPlatform,
) -> Result<()> {
    color::print_separator();
    color::log_info(&format!(
        "Executing custom command for {}...",
        color::magenta(target)
    ));

    if args.clean_cache {
        color::log_info("Cleaning cache...");
        let _ = TokioCommand::new("cargo").arg("clean").status().await;
    }

    let prepared = prepare_target(target, args, host).await?;
    let build_env = build_cargo_env(
        &prepared.actual_target,
        args,
        &prepared.cross_env,
        host,
        prepared.skip_target_arg,
    );

    let actual_command = prepare_exec_command(
        command,
        &prepared.actual_target,
        !args.no_append_target && !prepared.skip_target_arg && !args.no_cargo_target,
    );

    let mut cmd = TokioCommand::new(&actual_command[0]);
    if actual_command.len() > 1 {
        cmd.args(&actual_command[1..]);
    }
    if let Some(ref cwd) = args.cargo_cwd {
        cmd.current_dir(cwd);
    }
    cmd.envs(&build_env);

    print_env_vars(&build_env);
    color::print_run_header();
    println!(
        "{}",
        color::format_command(&format_cli_command(&actual_command))
    );

    let status = run_command(&mut cmd, &actual_command[0]).await?;
    if !status.success() {
        return Err(cargo_cross::CrossError::CommandFailed {
            command: format_cli_command(&actual_command),
        });
    }

    color::log_success(&format!(
        "Exec successful: {}",
        color::yellow(&prepared.actual_target)
    ));

    Ok(())
}

fn prepare_exec_command(command: &[String], target: &str, inject_target: bool) -> Vec<String> {
    if !inject_target || !exec_cargo_subcommand_supports_target(command) {
        return command.to_vec();
    }

    let passthrough_index = command
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(command.len());
    if has_explicit_exec_target(&command[..passthrough_index]) {
        return command.to_vec();
    }

    let mut prepared = command[..passthrough_index].to_vec();
    prepared.push("--target".to_string());
    prepared.push(target.to_string());
    prepared.extend_from_slice(&command[passthrough_index..]);
    prepared
}

fn is_cargo_invocation(command: &[String]) -> bool {
    let Some(program) = command.first() else {
        return false;
    };

    Path::new(program)
        .file_stem()
        .and_then(|stem| stem.to_str())
        == Some("cargo")
}

fn exec_cargo_subcommand_supports_target(command: &[String]) -> bool {
    cargo_subcommand_for_exec(command).is_some_and(|subcommand| {
        matches!(
            subcommand,
            "build"
                | "b"
                | "check"
                | "c"
                | "clippy"
                | "doc"
                | "fix"
                | "run"
                | "r"
                | "test"
                | "t"
                | "bench"
                | "rustc"
                | "rustdoc"
        )
    })
}

fn cargo_subcommand_for_exec(command: &[String]) -> Option<&str> {
    if !is_cargo_invocation(command) {
        return None;
    }

    let passthrough_index = command
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(command.len());
    let mut index = 1;

    while index < passthrough_index {
        let arg = command[index].as_str();

        if index == 1 && arg.starts_with('+') {
            index += 1;
            continue;
        }

        if matches!(arg, "-C" | "-Z" | "--config" | "--color") {
            index += 2;
            continue;
        }

        if arg.starts_with("-C")
            || (arg.starts_with("-Z") && arg.len() > 2)
            || arg.starts_with("--config=")
            || arg.starts_with("--color=")
        {
            index += 1;
            continue;
        }

        if arg.starts_with('-') {
            index += 1;
            continue;
        }

        return Some(arg);
    }

    None
}

fn has_explicit_exec_target(args: &[String]) -> bool {
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--target" || arg.starts_with("--target=") {
            return true;
        }
        if arg == "-t" && iter.peek().is_some() {
            return true;
        }
        if arg.starts_with("-t") && arg.len() > 2 {
            return true;
        }
    }
    false
}

async fn prepare_target(
    target: &str,
    args: &cargo_cross::Args,
    host: &HostPlatform,
) -> Result<PreparedTarget> {
    let is_host_build = target == "host-tuple";
    let actual_target = if is_host_build { &host.triple } else { target };
    let target_config = get_target_config(actual_target);
    let auto_build_std = ensure_target_installed(actual_target, args.toolchain.as_deref()).await?;
    let mut cross_env = if is_host_build {
        color::log_info(&format!(
            "Building for host ({}), skipping toolchain setup",
            color::cyan(actual_target)
        ));
        cargo_cross::env::CrossEnv::new()
    } else if let Some(env) = check_preconfigured_env(actual_target, args) {
        color::log_success(&format!(
            "Using pre-configured compiler from environment variables for {}",
            color::yellow(actual_target)
        ));
        env
    } else if let Some(config) = target_config {
        setup_cross_env(config, args, host).await?
    } else {
        // Unknown target, use default environment
        color::log_warning(&format!(
            "No specific toolchain configuration for {}, using default",
            color::cyan(actual_target)
        ));
        cargo_cross::env::CrossEnv::new()
    };

    // Apply user-provided compiler overrides from CLI arguments
    // CLI args have highest priority: CLI > env vars > auto-config
    apply_user_overrides(&mut cross_env, args);

    // Enable build-std if auto-detected (target exists in rustc but not in rustup)
    if auto_build_std && args.build_std.is_none() && cross_env.build_std.is_none() {
        cross_env.build_std = Some("true".to_string());
    }

    // Handle build-std requirement
    let needs_build_std =
        args.build_std.is_some() || args.panic_immediate_abort || cross_env.build_std.is_some();

    if needs_build_std {
        ensure_rust_src(actual_target, args.toolchain.as_deref()).await?;
    }

    Ok(PreparedTarget {
        actual_target: actual_target.to_string(),
        skip_target_arg: is_host_build,
        cross_env,
    })
}

/// Check for pre-configured compiler environment variables
/// Returns Some(CrossEnv) if CC_<target> or generic CC/CXX are set
fn check_preconfigured_env(
    target: &str,
    args: &cargo_cross::Args,
) -> Option<cargo_cross::env::CrossEnv> {
    // Skip if user explicitly wants to skip toolchain setup
    if args.no_toolchain_setup {
        return None;
    }

    let target_lower = target.replace('-', "_");
    let target_upper = target.to_uppercase().replace('-', "_");

    // Check target-specific CC_<target> first
    let cc_target_var = format!("CC_{target_lower}");
    let cxx_target_var = format!("CXX_{target_lower}");
    let ar_target_var = format!("AR_{target_lower}");
    let linker_var = format!("CARGO_TARGET_{target_upper}_LINKER");
    let runner_var = format!("CARGO_TARGET_{target_upper}_RUNNER");

    // Helper to get non-empty env var
    let get_env = |name: &str| std::env::var(name).ok().filter(|s| !s.is_empty());

    // Check if target-specific CC is set
    if let Some(cc) = get_env(&cc_target_var) {
        let mut env = cargo_cross::env::CrossEnv::new();
        env.set_cc(&cc);

        if let Some(cxx) = get_env(&cxx_target_var) {
            env.set_cxx(&cxx);
        }
        if let Some(ar) = get_env(&ar_target_var) {
            env.set_ar(&ar);
        }
        if let Some(linker) = get_env(&linker_var) {
            env.set_linker(&linker);
        }
        if let Some(runner) = get_env(&runner_var) {
            env.set_runner(&runner);
        }
        return Some(env);
    }

    // Check generic CC/CXX environment variables
    // Only use if both CC and CXX are set (matching cross.sh behavior)
    if let (Some(cc), Some(cxx)) = (get_env("CC"), get_env("CXX")) {
        let mut env = cargo_cross::env::CrossEnv::new();
        env.set_cc(&cc);
        env.set_cxx(&cxx);

        // AR defaults to CC with -gcc suffix replaced by -ar
        if let Some(ar) = get_env("AR") {
            env.set_ar(&ar);
        } else if cc.ends_with("-gcc") {
            env.set_ar(format!("{}-ar", cc.trim_end_matches("-gcc")));
        }

        // Linker defaults to CC
        if let Some(linker) = get_env("LINKER") {
            env.set_linker(&linker);
        } else {
            env.set_linker(&cc);
        }

        // RUNNER support
        if let Some(runner) = get_env("RUNNER") {
            env.set_runner(&runner);
        }

        return Some(env);
    }

    None
}

/// Apply user-provided compiler overrides from CLI arguments
/// CLI arguments have the highest priority and override both env vars and auto-config
fn apply_user_overrides(env: &mut cargo_cross::env::CrossEnv, args: &cargo_cross::Args) {
    if let Some(ref cc) = args.cc {
        let cc_str = cc.display().to_string();
        if !cc_str.is_empty() {
            env.set_cc(cc_str);
        }
    }
    if let Some(ref cxx) = args.cxx {
        let cxx_str = cxx.display().to_string();
        if !cxx_str.is_empty() {
            env.set_cxx(cxx_str);
        }
    }
    if let Some(ref ar) = args.ar {
        let ar_str = ar.display().to_string();
        if !ar_str.is_empty() {
            env.set_ar(ar_str);
        }
    }
    if let Some(ref linker) = args.linker {
        let linker_str = linker.display().to_string();
        if !linker_str.is_empty() {
            env.set_linker(linker_str);
        }
    }
}

fn print_config(args: &cargo_cross::Args, _host: &HostPlatform) {
    color::print_config_header();
    println!("{}", color::format_config("Command", args.command.as_str()));
    println!(
        "{}",
        color::format_config(
            "Working directory",
            &std::env::current_dir().map_or_else(|_| ".".to_string(), |p| p.display().to_string())
        )
    );

    if let Some(ref package) = args.package {
        println!("{}", color::format_config("Package", package));
    }
    if let Some(ref bin) = args.bin_target {
        println!("{}", color::format_config("Binary target", bin));
    }
    if args.build_bins {
        println!("{}", color::format_config("Build all binaries", "true"));
    }
    if args.build_lib {
        println!("{}", color::format_config("Build library", "true"));
    }
    if args.build_all_targets {
        println!("{}", color::format_config("Build all targets", "true"));
    }
    if args.workspace {
        println!("{}", color::format_config("Building workspace", "true"));
    }

    println!("{}", color::format_config("Profile", &args.profile));

    if let Some(ref toolchain) = args.toolchain {
        println!("{}", color::format_config("Toolchain", toolchain));
    }

    let targets_str = args.targets.join(", ");
    println!("{}", color::format_config("Targets", &targets_str));

    if args.glibc_version != cargo_cross::config::DEFAULT_GLIBC_VERSION {
        println!(
            "{}",
            color::format_config("Glibc version", &args.glibc_version)
        );
    }

    if let Some(ref features) = args.features {
        println!("{}", color::format_config("Features", features));
    }
    if args.no_default_features {
        println!("{}", color::format_config("No default features", "true"));
    }
    if args.all_features {
        println!("{}", color::format_config("All features", "true"));
    }

    if !args.rustflags.is_empty() {
        println!(
            "{}",
            color::format_config("Additional rustflags", &args.rustflags.join(" "))
        );
    }

    if let Some(ref build_std) = args.build_std {
        let display = if build_std == "true" {
            "true".to_string()
        } else {
            build_std.clone()
        };
        println!("{}", color::format_config("Build std", &display));
    }
}

fn set_github_output(args: &cargo_cross::Args) {
    if let Ok(github_output) = std::env::var("GITHUB_OUTPUT") {
        // Convert targets to JSON array
        let json_array = serde_json::to_string(&args.targets).unwrap_or_else(|_| "[]".to_string());

        if let Ok(mut file) = std::fs::OpenOptions::new()
            .append(true)
            .open(&github_output)
        {
            use std::io::Write;
            let _ = writeln!(file, "targets={json_array}");
        }
    }
}

fn capitalize_command(command: &str) -> String {
    let mut chars = command.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
        None => "Command".to_string(),
    }
}

fn format_cli_command(command: &[String]) -> String {
    command
        .iter()
        .map(|part| shell_escape(part))
        .collect::<Vec<_>>()
        .join(" ")
}

fn print_env_vars(env: &std::collections::HashMap<String, String>) {
    if env.is_empty() {
        return;
    }

    color::print_env_header();
    for (key, value) in sorted_env(env) {
        println!("{}", color::format_env(&key, &value));
    }
}

fn print_setup_env(
    env: &std::collections::HashMap<String, String>,
    format: SetupOutputFormat,
) -> Result<()> {
    let rendered = render_setup_env(env, format)?;
    if !rendered.is_empty() {
        println!("{rendered}");
    }
    Ok(())
}

fn render_setup_env(
    env: &std::collections::HashMap<String, String>,
    format: SetupOutputFormat,
) -> Result<String> {
    let mut rendered = Vec::new();

    match resolve_setup_output_format(format) {
        SetupOutputFormat::Auto => unreachable!("setup output format is resolved before printing"),
        SetupOutputFormat::Bash | SetupOutputFormat::Zsh => {
            for (key, value) in sorted_env(env) {
                rendered.push(format!("export {key}={}", shell_escape(&value)));
            }
        }
        SetupOutputFormat::Fish => {
            for (key, value) in sorted_env(env) {
                rendered.push(format!("set -gx {key} -- {};", fish_escape(&value)));
            }
        }
        SetupOutputFormat::Powershell => {
            for (key, value) in sorted_env(env) {
                rendered.push(format!("$Env:{key} = {}", powershell_escape(&value)));
            }
        }
        SetupOutputFormat::Cmd => {
            for (key, value) in sorted_env(env) {
                rendered.push(format!("set \"{key}={}\"", cmd_escape(&value)));
            }
        }
        SetupOutputFormat::Json => {
            return Ok(serde_json::to_string_pretty(&sorted_env(env))?);
        }
    }

    Ok(rendered.join("\n"))
}

fn write_setup_github_env(env: &std::collections::HashMap<String, String>) -> Result<()> {
    let Ok(github_env) = std::env::var("GITHUB_ENV") else {
        return Ok(());
    };

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(github_env)?;

    use std::io::Write;
    for (key, value) in sorted_env(env) {
        writeln!(file, "{key}<<__CARGO_CROSS_EOF__")?;
        writeln!(file, "{value}")?;
        writeln!(file, "__CARGO_CROSS_EOF__")?;
    }

    Ok(())
}

fn sorted_env(env: &std::collections::HashMap<String, String>) -> BTreeMap<String, String> {
    env.iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn resolve_setup_output_format(format: SetupOutputFormat) -> SetupOutputFormat {
    resolve_setup_output_format_with_shells(
        format,
        std::env::var_os("SHELL"),
        std::env::var_os("COMSPEC"),
    )
}

fn resolve_setup_output_format_with_shells(
    format: SetupOutputFormat,
    shell: Option<std::ffi::OsString>,
    comspec: Option<std::ffi::OsString>,
) -> SetupOutputFormat {
    match format {
        SetupOutputFormat::Auto => {
            detect_setup_shell(shell, comspec).unwrap_or(SetupOutputFormat::Bash)
        }
        other => other,
    }
}

fn detect_setup_shell(
    shell: Option<std::ffi::OsString>,
    comspec: Option<std::ffi::OsString>,
) -> Option<SetupOutputFormat> {
    shell
        .and_then(detect_shell_from_os_str)
        .or_else(|| comspec.and_then(detect_shell_from_os_str))
}

fn detect_shell_from_os_str(shell: std::ffi::OsString) -> Option<SetupOutputFormat> {
    let shell = shell.to_string_lossy();
    let shell = shell.rsplit(['/', '\\']).next()?;
    let name = shell
        .strip_suffix(".exe")
        .unwrap_or(shell)
        .to_ascii_lowercase();

    match name.as_str() {
        "bash" | "sh" => Some(SetupOutputFormat::Bash),
        "zsh" => Some(SetupOutputFormat::Zsh),
        "fish" => Some(SetupOutputFormat::Fish),
        "powershell" | "pwsh" => Some(SetupOutputFormat::Powershell),
        "cmd" => Some(SetupOutputFormat::Cmd),
        _ => None,
    }
}

fn shell_escape(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }

    if value.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
    }) {
        return value.to_string();
    }

    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

fn fish_escape(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('\n', "\\n");
    format!("\"{escaped}\"")
}

fn powershell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn cmd_escape(value: &str) -> String {
    value
        .replace('^', "^^")
        .replace('&', "^&")
        .replace('|', "^|")
        .replace('<', "^<")
        .replace('>', "^>")
        .replace('%', "%%")
        .replace('"', "^\"")
}

struct LogSilenceGuard {
    previous: Option<OsString>,
}

impl LogSilenceGuard {
    fn new() -> Self {
        let previous = std::env::var_os("CARGO_CROSS_SILENT");
        std::env::set_var("CARGO_CROSS_SILENT", "1");
        Self { previous }
    }
}

impl Drop for LogSilenceGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            std::env::set_var("CARGO_CROSS_SILENT", previous);
        } else {
            std::env::remove_var("CARGO_CROSS_SILENT");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cargo_subcommand_for_exec, detect_setup_shell, prepare_exec_command, render_setup_env,
        resolve_setup_output_format_with_shells, write_setup_github_env,
    };
    use cargo_cross::cli::SetupOutputFormat;
    use std::collections::HashMap;
    use std::ffi::OsString;

    #[test]
    fn prepare_exec_command_injects_target_for_cargo() {
        let command = vec![
            "cargo".to_string(),
            "clippy".to_string(),
            "--workspace".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(
            prepared,
            vec![
                "cargo",
                "clippy",
                "--workspace",
                "--target",
                "x86_64-pc-windows-gnu",
            ]
        );
    }

    #[test]
    fn prepare_exec_command_respects_existing_target() {
        let command = vec![
            "cargo".to_string(),
            "clippy".to_string(),
            "--target".to_string(),
            "aarch64-apple-darwin".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn prepare_exec_command_respects_existing_short_target() {
        let command = vec![
            "cargo".to_string(),
            "clippy".to_string(),
            "-t".to_string(),
            "aarch64-apple-darwin".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn prepare_exec_command_respects_existing_short_concat_target() {
        let command = vec![
            "cargo".to_string(),
            "clippy".to_string(),
            "-taarch64-apple-darwin".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn prepare_exec_command_inserts_target_before_passthrough_separator() {
        let command = vec![
            "cargo".to_string(),
            "test".to_string(),
            "--".to_string(),
            "--nocapture".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(
            prepared,
            vec![
                "cargo",
                "test",
                "--target",
                "x86_64-pc-windows-gnu",
                "--",
                "--nocapture",
            ]
        );
    }

    #[test]
    fn prepare_exec_command_ignores_target_after_passthrough_separator() {
        let command = vec![
            "cargo".to_string(),
            "test".to_string(),
            "--".to_string(),
            "--target".to_string(),
            "aarch64-apple-darwin".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(
            prepared,
            vec![
                "cargo",
                "test",
                "--target",
                "x86_64-pc-windows-gnu",
                "--",
                "--target",
                "aarch64-apple-darwin",
            ]
        );
    }

    #[test]
    fn prepare_exec_command_skips_non_cargo_commands() {
        let command = vec!["env".to_string(), "FOO=bar".to_string()];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn prepare_exec_command_skips_non_target_cargo_subcommands() {
        let command = vec!["cargo".to_string(), "metadata".to_string()];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn prepare_exec_command_skips_fmt_passthrough_subcommands() {
        let command = vec![
            "cargo".to_string(),
            "fmt".to_string(),
            "--".to_string(),
            "--check".to_string(),
        ];
        let prepared = prepare_exec_command(&command, "x86_64-pc-windows-gnu", true);
        assert_eq!(prepared, command);
    }

    #[test]
    fn cargo_subcommand_for_exec_skips_toolchain_and_global_options() {
        let command = vec![
            "cargo".to_string(),
            "+nightly".to_string(),
            "-q".to_string(),
            "--config".to_string(),
            "build.jobs=4".to_string(),
            "clippy".to_string(),
        ];
        assert_eq!(cargo_subcommand_for_exec(&command), Some("clippy"));
    }

    #[test]
    fn resolve_setup_output_format_falls_back_to_bash() {
        assert_eq!(
            resolve_setup_output_format_with_shells(SetupOutputFormat::Auto, None, None),
            SetupOutputFormat::Bash
        );
    }

    #[test]
    fn detect_current_setup_shell_prefers_current_shell() {
        assert_eq!(
            detect_setup_shell(
                Some(OsString::from("/bin/fish")),
                Some(OsString::from("C:\\Windows\\System32\\cmd.exe"))
            ),
            Some(SetupOutputFormat::Fish)
        );
    }

    #[test]
    fn detect_current_setup_shell_falls_back_to_comspec() {
        assert_eq!(
            detect_setup_shell(
                None,
                Some(OsString::from(
                    "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
                ))
            ),
            Some(SetupOutputFormat::Powershell)
        );
    }

    #[test]
    fn write_setup_github_env_appends_multiline_entries() {
        let github_env =
            std::env::temp_dir().join(format!("cargo-cross-github-env-{}.txt", std::process::id()));
        let original = std::env::var_os("GITHUB_ENV");
        std::env::set_var("GITHUB_ENV", &github_env);

        let mut env = HashMap::new();
        env.insert("CC".to_string(), "x86_64-w64-mingw32-gcc".to_string());
        env.insert(
            "PATH".to_string(),
            "/tmp/toolchain/bin:/usr/bin".to_string(),
        );
        write_setup_github_env(&env).unwrap();

        let contents = std::fs::read_to_string(&github_env).unwrap();
        assert!(contents.contains("CC<<__CARGO_CROSS_EOF__"));
        assert!(contents.contains("x86_64-w64-mingw32-gcc"));
        assert!(contents.contains("PATH<<__CARGO_CROSS_EOF__"));
        assert!(contents.contains("/tmp/toolchain/bin:/usr/bin"));

        let _ = std::fs::remove_file(&github_env);
        restore_env_var("GITHUB_ENV", original);
    }

    #[test]
    fn render_setup_env_supports_bash() {
        let mut env = HashMap::new();
        env.insert("CC".to_string(), "clang".to_string());
        env.insert(
            "PATH".to_string(),
            "/tmp/toolchain/bin:/usr/bin".to_string(),
        );

        let rendered = render_setup_env(&env, SetupOutputFormat::Bash).unwrap();
        assert!(rendered.contains("export CC=clang"));
        assert!(rendered.contains("export PATH=/tmp/toolchain/bin:/usr/bin"));
    }

    #[test]
    fn render_setup_env_supports_fish() {
        let mut env = HashMap::new();
        env.insert(
            "PATH".to_string(),
            "/tmp/toolchain/bin:/usr/bin".to_string(),
        );

        let rendered = render_setup_env(&env, SetupOutputFormat::Fish).unwrap();
        assert_eq!(rendered, "set -gx PATH -- \"/tmp/toolchain/bin:/usr/bin\";");
    }

    #[test]
    fn render_setup_env_supports_json() {
        let mut env = HashMap::new();
        env.insert(
            "PATH".to_string(),
            "/tmp/toolchain/bin:/usr/bin".to_string(),
        );

        let rendered = render_setup_env(&env, SetupOutputFormat::Json).unwrap();
        assert!(rendered.contains("\"PATH\": \"/tmp/toolchain/bin:/usr/bin\""));
    }

    #[test]
    fn render_setup_env_supports_powershell() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "C:\\toolchain\\bin".to_string());

        let rendered = render_setup_env(&env, SetupOutputFormat::Powershell).unwrap();
        assert_eq!(rendered, "$Env:PATH = 'C:\\toolchain\\bin'");
    }

    #[test]
    fn render_setup_env_supports_cmd() {
        let mut env = HashMap::new();
        env.insert(
            "PATH".to_string(),
            "C:\\toolchain\\bin;%USERPROFILE%".to_string(),
        );

        let rendered = render_setup_env(&env, SetupOutputFormat::Cmd).unwrap();
        assert_eq!(rendered, "set \"PATH=C:\\toolchain\\bin;%%USERPROFILE%%\"");
    }

    fn restore_env_var(name: &str, value: Option<OsString>) {
        if let Some(value) = value {
            std::env::set_var(name, value);
        } else {
            std::env::remove_var(name);
        }
    }
}
