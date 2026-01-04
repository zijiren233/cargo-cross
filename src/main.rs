//! cargo-cross: Cross-compilation tool for Rust projects

use cargo_cross::{
    cargo::{ensure_rust_src, ensure_target_installed, execute_cargo},
    cli::{parse_args, print_all_targets, print_version, ParseResult},
    color,
    config::{get_target_config, HostPlatform},
    error::Result,
    platform::setup_cross_env,
    sanitize_cargo_env,
};
use std::process::ExitCode;
use std::time::Duration;

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
    // Parse command-line arguments
    let args = match parse_args()? {
        ParseResult::Build(args) => *args,
        ParseResult::ShowTargets(format) => {
            print_all_targets(format);
            return Ok(ExitCode::SUCCESS);
        }
        ParseResult::ShowVersion => {
            print_version();
            return Ok(ExitCode::SUCCESS);
        }
    };

    // Detect host platform
    let host = HostPlatform::detect();

    // Print execution configuration
    print_config(&args, &host);

    // Process each target
    let total_targets = args.targets.len();
    let start_time = std::time::Instant::now();

    for (i, target) in args.targets.iter().enumerate() {
        color::log_success(&format!(
            "[{}/{}] Processing target: {}",
            color::yellow(&(i + 1).to_string()),
            color::yellow(&total_targets.to_string()),
            color::cyan(target)
        ));

        // Execute build for this target with timing
        let target_start = std::time::Instant::now();
        let result = execute_target(target, &args, &host).await;
        let target_elapsed = target_start.elapsed();

        if let Err(e) = result {
            let command = args.command.as_str();
            let command_cap = format!(
                "{}{}",
                command.chars().next().unwrap().to_uppercase(),
                &command[1..]
            );
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

    // Set GitHub output if in GitHub Actions
    set_github_output(&args);

    Ok(ExitCode::SUCCESS)
}

async fn execute_target(target: &str, args: &cargo_cross::Args, host: &HostPlatform) -> Result<()> {
    color::print_separator();
    color::log_info(&format!(
        "Executing {} for {}...",
        color::magenta(args.command.as_str()),
        color::magenta(target)
    ));

    // Clean cache if requested
    if args.clean_cache {
        color::log_info("Cleaning cache...");
        let _ = tokio::process::Command::new("cargo")
            .arg("clean")
            .status()
            .await;
    }

    // Get target configuration
    let target_config = get_target_config(target);

    // Ensure target is installed and check if build-std is required
    let auto_build_std = ensure_target_installed(target, args.toolchain.as_deref()).await?;

    // Check for pre-configured compiler environment variables first
    // This allows users to skip toolchain download if they have their own compiler setup
    let mut cross_env = if let Some(env) = check_preconfigured_env(target, args) {
        color::log_success(&format!(
            "Using pre-configured compiler from environment variables for {}",
            color::yellow(target)
        ));
        env
    } else if let Some(config) = target_config {
        setup_cross_env(config, args, host).await?
    } else {
        // Unknown target, use default environment
        color::log_warning(&format!(
            "No specific toolchain configuration for {}, using default",
            color::cyan(target)
        ));
        cargo_cross::env::CrossEnv::new()
    };

    // Enable build-std if auto-detected (target exists in rustc but not in rustup)
    if auto_build_std && args.build_std.is_none() && cross_env.build_std.is_none() {
        cross_env.build_std = Some("true".to_string());
    }

    // Handle build-std requirement
    let needs_build_std =
        args.build_std.is_some() || args.panic_immediate_abort || cross_env.build_std.is_some();

    if needs_build_std {
        ensure_rust_src(target, args.toolchain.as_deref()).await?;
    }

    // Execute cargo
    let status = execute_cargo(target, args, &cross_env, host).await?;

    if !status.success() {
        return Err(cargo_cross::CrossError::CargoFailed {
            code: status.code().unwrap_or(1),
        });
    }

    let command = args.command.as_str();
    let command_cap = format!(
        "{}{}",
        command.chars().next().unwrap().to_uppercase(),
        &command[1..]
    );
    color::log_success(&format!(
        "{command_cap} successful: {}",
        color::yellow(target)
    ));

    Ok(())
}

/// Check for pre-configured compiler environment variables
/// Returns Some(CrossEnv) if CC_<target> or generic CC/CXX are set
fn check_preconfigured_env(
    target: &str,
    args: &cargo_cross::Args,
) -> Option<cargo_cross::env::CrossEnv> {
    // Skip if user explicitly wants default linker
    if args.use_default_linker {
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
