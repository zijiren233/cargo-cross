//! cargo-cross: Cross-compilation tool for Rust projects

use cargo_cross::{
    cargo::{ensure_rust_src, ensure_target_installed, execute_cargo},
    cli::parse_args,
    color,
    config::{get_target_config, HostPlatform},
    error::Result,
    platform::setup_cross_env,
};
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
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
    let args = parse_args()?;

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
            i + 1,
            total_targets,
            target
        ));

        // Execute build for this target
        let result = execute_target(target, &args, &host).await;

        if let Err(e) = result {
            let command = args.command.as_str();
            let command_cap = format!(
                "{}{}",
                command.chars().next().unwrap().to_uppercase(),
                &command[1..]
            );
            color::log_error(&format!("{command_cap} failed for target: {target}"));
            color::log_error(&format!("Error: {e}"));
            return Ok(ExitCode::FAILURE);
        }
    }

    let elapsed = start_time.elapsed();
    color::print_separator();
    color::log_success(&format!(
        "All {} operations completed successfully!",
        args.command.as_str()
    ));
    color::log_success(&format!("Total time: {}s", elapsed.as_secs()));

    // Set GitHub output if in GitHub Actions
    set_github_output(&args);

    Ok(ExitCode::SUCCESS)
}

async fn execute_target(target: &str, args: &cargo_cross::Args, host: &HostPlatform) -> Result<()> {
    color::print_separator();
    color::log_info(&format!(
        "Executing {} for {}...",
        args.command.as_str(),
        target
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

    // Setup cross-compilation environment
    let mut cross_env = if let Some(config) = target_config {
        setup_cross_env(config, args, host).await?
    } else {
        // Unknown target, use default environment
        color::log_warning(&format!(
            "No specific toolchain configuration for {target}, using default"
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
    color::log_success(&format!("{command_cap} successful: {target}"));

    Ok(())
}

fn print_config(args: &cargo_cross::Args, _host: &HostPlatform) {
    color::print_config_header();
    println!("{}", color::format_config("Command", args.command.as_str()));
    println!(
        "{}",
        color::format_config(
            "Source directory",
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
