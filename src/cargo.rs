//! Cargo command builder and executor

use crate::cli::Args;
use crate::color;
use crate::config::HostPlatform;
use crate::env::{get_build_std_config, CrossEnv};
use crate::error::{run_command, run_command_output, CrossError, Result};
use std::collections::HashMap;
use std::process::ExitStatus;
use tokio::process::Command as TokioCommand;

/// Build and execute cargo command for a target
pub async fn execute_cargo(
    target: &str,
    args: &Args,
    cross_env: &CrossEnv,
    host: &HostPlatform,
) -> Result<ExitStatus> {
    // Build environment variables
    let build_env = build_cargo_env(target, args, cross_env, host);

    // Build cargo command
    let mut cmd = build_cargo_command(target, args, cross_env);

    // Set environment variables
    cmd.envs(&build_env);

    // Print debug info
    print_env_vars(&build_env);
    color::print_run_header();
    println!("{}", color::format_command(&format_command_from_cmd(&cmd)));

    // Execute
    let status = run_command(&mut cmd, "cargo").await?;
    Ok(status)
}

/// Format command string from TokioCommand
fn format_command_from_cmd(cmd: &TokioCommand) -> String {
    let std_cmd = cmd.as_std();
    let mut parts = vec![std_cmd.get_program().to_string_lossy().into_owned()];
    for arg in std_cmd.get_args() {
        parts.push(arg.to_string_lossy().into_owned());
    }
    parts.join(" ")
}

/// Build environment variables for cargo execution
fn build_cargo_env(
    target: &str,
    args: &Args,
    cross_env: &CrossEnv,
    host: &HostPlatform,
) -> HashMap<String, String> {
    let target_lower = target.replace('-', "_");
    let mut env = cross_env.build_env(target, host);

    // Handle host config for same-target builds (only when --target is explicitly passed)
    // When no_cargo_target is true, we don't pass --target to cargo, so these aren't needed
    if !args.no_cargo_target && target == host.triple {
        add_host_config_env(&mut env);
    }

    // Build RUSTFLAGS
    let rustflags = build_rustflags(args, cross_env);
    if !rustflags.is_empty() {
        env.insert("RUSTFLAGS".to_string(), rustflags);
    }

    // Add sccache/rustc wrapper
    add_wrapper_env(&mut env, args);

    // Add sccache options
    add_sccache_env(&mut env, args);

    // Add CC crate environment
    add_cc_crate_env(&mut env, args);

    // Add user-provided compiler flags
    add_compiler_flags_env(&mut env, args, &target_lower);

    // Add other environment variables
    if let Some(ref trim_paths) = args.cargo_trim_paths {
        env.insert("CARGO_TRIM_PATHS".to_string(), trim_paths.clone());
    }
    if let Some(ref bootstrap) = args.rustc_bootstrap {
        env.insert("RUSTC_BOOTSTRAP".to_string(), bootstrap.clone());
    }

    env
}

/// Build RUSTFLAGS string
fn build_rustflags(args: &Args, cross_env: &CrossEnv) -> String {
    let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();

    // Add cross_env rustflags
    if let Some(ref flags) = cross_env.rustflags_string() {
        append_flag(&mut rustflags, flags);
    }

    // Add CRT static flag
    if let Some(crt_static) = args.crt_static {
        let flag = if crt_static {
            "-C target-feature=+crt-static"
        } else {
            "-C target-feature=-crt-static"
        };
        append_flag(&mut rustflags, flag);
    }

    // Add panic=immediate-abort flag
    if args.panic_immediate_abort {
        append_flag(&mut rustflags, "-Zunstable-options -Cpanic=immediate-abort");
    }

    // Add fmt-debug flag
    if let Some(ref fmt_debug) = args.fmt_debug {
        append_flag(&mut rustflags, &format!("-Zfmt-debug={fmt_debug}"));
    }

    // Add location-detail flag
    if let Some(ref location_detail) = args.location_detail {
        append_flag(
            &mut rustflags,
            &format!("-Zlocation-detail={location_detail}"),
        );
    }

    // Add additional rustflags from command line
    for flag in &args.rustflags {
        append_flag(&mut rustflags, flag);
    }

    rustflags
}

/// Add host config environment variables for same-target builds
/// These are needed when explicitly passing --target that matches the host
fn add_host_config_env(env: &mut HashMap<String, String>) {
    env.insert("CARGO_UNSTABLE_HOST_CONFIG".to_string(), "true".to_string());
    env.insert(
        "CARGO_UNSTABLE_TARGET_APPLIES_TO_HOST".to_string(),
        "true".to_string(),
    );
    env.insert(
        "CARGO_TARGET_APPLIES_TO_HOST".to_string(),
        "false".to_string(),
    );
}

/// Add wrapper environment (sccache or rustc_wrapper)
fn add_wrapper_env(env: &mut HashMap<String, String>, args: &Args) {
    if args.enable_sccache {
        env.insert("RUSTC_WRAPPER".to_string(), "sccache".to_string());
    } else if let Some(ref wrapper) = args.rustc_wrapper {
        env.insert("RUSTC_WRAPPER".to_string(), wrapper.display().to_string());
    }
}

/// Add sccache environment variables
fn add_sccache_env(env: &mut HashMap<String, String>, args: &Args) {
    // First, pass through all SCCACHE_* environment variables from current environment
    for (key, val) in std::env::vars() {
        if key.starts_with("SCCACHE_") && !val.is_empty() {
            env.insert(key, val);
        }
    }

    // Then, override with args-based settings (args have higher priority than env vars)
    if let Some(ref dir) = args.sccache_dir {
        env.insert("SCCACHE_DIR".to_string(), dir.display().to_string());
    }
    if let Some(ref size) = args.sccache_cache_size {
        env.insert("SCCACHE_CACHE_SIZE".to_string(), size.clone());
    }
    if let Some(ref timeout) = args.sccache_idle_timeout {
        env.insert("SCCACHE_IDLE_TIMEOUT".to_string(), timeout.clone());
    }
    if let Some(ref log) = args.sccache_log {
        env.insert("SCCACHE_LOG".to_string(), log.clone());
    }
    if args.sccache_no_daemon {
        env.insert("SCCACHE_NO_DAEMON".to_string(), "1".to_string());
    }
    if args.sccache_direct {
        env.insert("SCCACHE_DIRECT".to_string(), "true".to_string());
    }
}

/// Add CC crate environment variables
fn add_cc_crate_env(env: &mut HashMap<String, String>, args: &Args) {
    if args.cc_no_defaults {
        env.insert("CRATE_CC_NO_DEFAULTS".to_string(), "1".to_string());
    }
    if args.cc_shell_escaped_flags {
        env.insert("CC_SHELL_ESCAPED_FLAGS".to_string(), "1".to_string());
    }
    if args.cc_enable_debug || args.verbose_level > 0 {
        env.insert("CC_ENABLE_DEBUG_OUTPUT".to_string(), "1".to_string());
    }

    // Pass through additional CC crate environment variables
    let passthrough_vars = ["CC_FORCE_DISABLE", "CC_KNOWN_WRAPPER_CUSTOM"];
    for var in passthrough_vars {
        if let Ok(val) = std::env::var(var) {
            if !val.is_empty() {
                env.insert(var.to_string(), val);
            }
        }
    }
}

/// Add user-provided compiler flags
fn add_compiler_flags_env(env: &mut HashMap<String, String>, args: &Args, target_lower: &str) {
    if let Some(ref cflags) = args.cflags {
        let existing = env
            .get(&format!("CFLAGS_{target_lower}"))
            .cloned()
            .unwrap_or_default();
        let new_flags = if existing.is_empty() {
            cflags.clone()
        } else {
            format!("{existing} {cflags}")
        };
        env.insert(format!("CFLAGS_{target_lower}"), new_flags.clone());
        env.insert("CFLAGS".to_string(), new_flags);
    }

    if let Some(ref cxxflags) = args.cxxflags {
        let existing = env
            .get(&format!("CXXFLAGS_{target_lower}"))
            .cloned()
            .unwrap_or_default();
        let new_flags = if existing.is_empty() {
            cxxflags.clone()
        } else {
            format!("{existing} {cxxflags}")
        };
        env.insert(format!("CXXFLAGS_{target_lower}"), new_flags.clone());
        env.insert("CXXFLAGS".to_string(), new_flags);
    }

    if let Some(ref ldflags) = args.ldflags {
        let existing = env
            .get(&format!("LDFLAGS_{target_lower}"))
            .cloned()
            .unwrap_or_default();
        let new_flags = if existing.is_empty() {
            ldflags.clone()
        } else {
            format!("{existing} {ldflags}")
        };
        env.insert(format!("LDFLAGS_{target_lower}"), new_flags.clone());
        env.insert("LDFLAGS".to_string(), new_flags);
    }

    if let Some(ref cxxstdlib) = args.cxxstdlib {
        env.insert(format!("CXXSTDLIB_{target_lower}"), cxxstdlib.clone());
        env.insert("CXXSTDLIB".to_string(), cxxstdlib.clone());
    }
}

/// Build the cargo command with all arguments
fn build_cargo_command(target: &str, args: &Args, cross_env: &CrossEnv) -> TokioCommand {
    let mut cmd = TokioCommand::new("cargo");

    // Toolchain
    if let Some(ref toolchain) = args.toolchain {
        cmd.arg(format!("+{toolchain}"));
    }

    // Command
    cmd.arg(args.command.as_str());

    // Working directory
    if let Some(ref cwd) = args.cargo_cwd {
        cmd.arg("-C").arg(cwd);
    }

    // -Z flags
    for flag in &args.cargo_z_flags {
        cmd.arg("-Z").arg(flag);
    }

    // --config flags
    for config in &args.cargo_config {
        cmd.arg("--config").arg(config);
    }

    // Target
    if !args.no_cargo_target {
        cmd.arg("--target").arg(target);
    }

    // Profile
    add_profile_args(&mut cmd, args);

    // Features
    add_feature_args(&mut cmd, args);

    // Package and target selection
    add_package_args(&mut cmd, args);

    // Build-std
    add_build_std_args(&mut cmd, args, cross_env);

    // Verbosity
    add_verbosity_args(&mut cmd, args);

    // Output options
    add_output_args(&mut cmd, args);

    // Dependency options
    add_dependency_args(&mut cmd, args);

    // Build configuration
    add_build_config_args(&mut cmd, args);

    // Additional cargo args
    for arg in &args.cargo_args {
        cmd.arg(arg);
    }

    // Passthrough arguments
    if !args.passthrough_args.is_empty() {
        cmd.arg("--");
        for arg in &args.passthrough_args {
            cmd.arg(arg);
        }
    }

    cmd
}

/// Add profile arguments
fn add_profile_args(cmd: &mut TokioCommand, args: &Args) {
    if args.profile == "release" {
        cmd.arg("--release");
    } else if args.profile != "debug" {
        cmd.arg("--profile").arg(&args.profile);
    }
}

/// Add feature arguments
fn add_feature_args(cmd: &mut TokioCommand, args: &Args) {
    if let Some(ref features) = args.features {
        cmd.arg("--features").arg(features);
    }
    if args.no_default_features {
        cmd.arg("--no-default-features");
    }
    if args.all_features {
        cmd.arg("--all-features");
    }
}

/// Add package and target selection arguments
fn add_package_args(cmd: &mut TokioCommand, args: &Args) {
    if let Some(ref package) = args.package {
        cmd.arg("--package").arg(package);
    }
    if args.workspace {
        cmd.arg("--workspace");
    }
    if let Some(ref exclude) = args.exclude {
        cmd.arg("--exclude").arg(exclude);
    }
    if let Some(ref bin) = args.bin_target {
        cmd.arg("--bin").arg(bin);
    }
    if args.build_bins {
        cmd.arg("--bins");
    }
    if args.build_lib {
        cmd.arg("--lib");
    }
    if let Some(ref example) = args.example_target {
        cmd.arg("--example").arg(example);
    }
    if args.build_examples {
        cmd.arg("--examples");
    }
    if let Some(ref test) = args.test_target {
        cmd.arg("--test").arg(test);
    }
    if args.build_tests {
        cmd.arg("--tests");
    }
    if let Some(ref bench) = args.bench_target {
        cmd.arg("--bench").arg(bench);
    }
    if args.build_benches {
        cmd.arg("--benches");
    }
    if args.build_all_targets {
        cmd.arg("--all-targets");
    }
    if let Some(ref manifest) = args.manifest_path {
        cmd.arg("--manifest-path").arg(manifest);
    }
}

/// Add build-std arguments
fn add_build_std_args(cmd: &mut TokioCommand, args: &Args, cross_env: &CrossEnv) {
    let build_std_value = args
        .build_std
        .as_ref()
        .or(cross_env.build_std.as_ref())
        .map(|s| {
            if s == "true" {
                get_build_std_config().to_string()
            } else {
                s.clone()
            }
        });

    if let Some(ref crates) = build_std_value {
        cmd.arg(format!("-Zbuild-std={crates}"));
    }

    if let Some(ref features) = args.build_std_features {
        cmd.arg(format!("-Zbuild-std-features={features}"));
    }
}

/// Add verbosity arguments
fn add_verbosity_args(cmd: &mut TokioCommand, args: &Args) {
    if args.verbose_level > 0 {
        let v_flag = format!("-{}", "v".repeat(args.verbose_level as usize));
        cmd.arg(&v_flag);
    }
    if args.quiet {
        cmd.arg("--quiet");
    }
}

/// Add output option arguments
fn add_output_args(cmd: &mut TokioCommand, args: &Args) {
    if let Some(ref format) = args.message_format {
        cmd.arg("--message-format").arg(format);
    }
    if let Some(ref color) = args.color {
        cmd.arg("--color").arg(color);
    }
    if args.build_plan {
        cmd.arg("--build-plan");
    }
    if let Some(ref timings) = args.timings {
        if timings == "true" {
            cmd.arg("--timings");
        } else {
            cmd.arg(format!("--timings={timings}"));
        }
    }
}

/// Add dependency option arguments
fn add_dependency_args(cmd: &mut TokioCommand, args: &Args) {
    if args.ignore_rust_version {
        cmd.arg("--ignore-rust-version");
    }
    if args.locked {
        cmd.arg("--locked");
    }
    if args.offline {
        cmd.arg("--offline");
    }
    if args.frozen {
        cmd.arg("--frozen");
    }
    if let Some(ref lockfile) = args.lockfile_path {
        cmd.arg("--lockfile-path").arg(lockfile);
    }
}

/// Add build configuration arguments
fn add_build_config_args(cmd: &mut TokioCommand, args: &Args) {
    if let Some(ref jobs) = args.jobs {
        cmd.arg("--jobs").arg(jobs);
    }
    if args.keep_going {
        cmd.arg("--keep-going");
    }
    if args.future_incompat_report {
        cmd.arg("--future-incompat-report");
    }
    if args.no_embed_metadata {
        cmd.arg("-Zno-embed-metadata");
    }
    if let Some(ref target_dir) = args.cargo_target_dir {
        cmd.arg("--target-dir").arg(target_dir);
    }
    if let Some(ref artifact_dir) = args.artifact_dir {
        cmd.arg("--artifact-dir").arg(artifact_dir);
    }
}

/// Helper to append a flag to a space-separated string
fn append_flag(flags: &mut String, flag: &str) {
    if !flags.is_empty() {
        flags.push(' ');
    }
    flags.push_str(flag);
}

/// Print environment variables
fn print_env_vars(env: &HashMap<String, String>) {
    if env.is_empty() {
        return;
    }

    color::print_env_header();
    let mut keys: Vec<_> = env.keys().collect();
    keys.sort();

    for key in keys {
        if let Some(value) = env.get(key) {
            println!("{}", color::format_env(key, value));
        }
    }
}

/// Install Rust target if needed
/// Returns Ok(true) if build-std is required, Ok(false) otherwise
pub async fn ensure_target_installed(target: &str, toolchain: Option<&str>) -> Result<bool> {
    // Check if target is installed
    let mut cmd = TokioCommand::new("rustup");
    cmd.arg("target").arg("list").arg("--installed");

    if let Some(tc) = toolchain {
        cmd.arg("--toolchain").arg(tc);
    }

    let output = run_command_output(&mut cmd, "rustup").await?;
    let installed = String::from_utf8_lossy(&output.stdout);

    if installed.lines().any(|line| line.trim() == target) {
        return Ok(false);
    }

    // Check if target is available
    let mut cmd = TokioCommand::new("rustup");
    cmd.arg("target").arg("list");

    if let Some(tc) = toolchain {
        cmd.arg("--toolchain").arg(tc);
    }

    let output = run_command_output(&mut cmd, "rustup").await?;
    let available = String::from_utf8_lossy(&output.stdout);

    if available
        .lines()
        .any(|line| line.trim().starts_with(target))
    {
        // Install target
        color::log_info(&format!(
            "Installing Rust target: {}",
            color::yellow(target)
        ));

        let mut cmd = TokioCommand::new("rustup");
        cmd.arg("target").arg("add").arg(target);

        if let Some(tc) = toolchain {
            cmd.arg("--toolchain").arg(tc);
        }

        let status = run_command(&mut cmd, "rustup").await?;
        if !status.success() {
            return Err(CrossError::TargetInstallFailed {
                target: target.to_string(),
            });
        }
        return Ok(false);
    }

    // Check if target exists in rustc (requires build-std)
    let mut cmd = TokioCommand::new("rustc");
    cmd.args(["--print=target-list"]);
    let output = run_command_output(&mut cmd, "rustc").await?;

    let targets = String::from_utf8_lossy(&output.stdout);
    if targets.lines().any(|line| line.trim() == target) {
        color::log_info(&format!(
            "Target {} not available in rustup but exists in rustc, using build-std",
            color::yellow(target)
        ));
        return Ok(true);
    }

    Err(CrossError::BuildStdRequired {
        target: target.to_string(),
    })
}

/// Add rust-src component if needed for build-std
pub async fn ensure_rust_src(target: &str, toolchain: Option<&str>) -> Result<()> {
    let toolchain_info = toolchain
        .map(|t| format!(" and toolchain: {}", color::yellow(t)))
        .unwrap_or_default();
    color::log_info(&format!(
        "Adding rust-src component for target: {}{}",
        color::yellow(target),
        toolchain_info
    ));

    let mut cmd = TokioCommand::new("rustup");
    cmd.arg("component")
        .arg("add")
        .arg("rust-src")
        .arg("--target")
        .arg(target);

    if let Some(tc) = toolchain {
        cmd.arg("--toolchain").arg(tc);
    }

    let status = run_command(&mut cmd, "rustup").await?;
    if !status.success() {
        color::log_warning("Failed to add rust-src component, build-std may not work");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_flag_empty() {
        let mut flags = String::new();
        append_flag(&mut flags, "-C opt-level=3");
        assert_eq!(flags, "-C opt-level=3");
    }

    #[test]
    fn test_append_flag_non_empty() {
        let mut flags = String::from("-C opt-level=3");
        append_flag(&mut flags, "-C target-feature=+crt-static");
        assert_eq!(flags, "-C opt-level=3 -C target-feature=+crt-static");
    }

    #[test]
    fn test_append_flag_multiple() {
        let mut flags = String::new();
        append_flag(&mut flags, "-C opt-level=3");
        append_flag(&mut flags, "-C target-feature=+crt-static");
        append_flag(&mut flags, "-Z build-std");
        assert_eq!(
            flags,
            "-C opt-level=3 -C target-feature=+crt-static -Z build-std"
        );
    }
}
