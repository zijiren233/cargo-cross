//! Darwin (macOS) cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, TargetConfig};
use crate::download::download_and_extract;
use crate::env::CrossEnv;
use crate::error::{CrossError, Result};
use crate::platform::setup_cmake;
use crate::runner;

/// Setup Darwin cross-compilation environment
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let rust_target = target_config.target;

    if host.is_darwin() {
        setup_native(arch, rust_target, args, host).await
    } else if host.is_linux() {
        setup_osxcross(arch, rust_target, args, host).await
    } else {
        Err(CrossError::CrossCompilationNotSupported {
            target_os: "darwin".to_string(),
            host_os: host.os.to_string(),
        })
    }
}

/// Setup native Darwin compilation (on macOS host)
async fn setup_native(
    arch: Arch,
    rust_target: &str,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let mut env = CrossEnv::new();

    // Setup Rosetta runner for x86_64 targets on ARM macOS
    if args.command.needs_runner() {
        runner::setup_rosetta_runner(&mut env, arch, rust_target, host);
    }

    // Priority: MACOS_SDK_PATH > MACOS_SDK_VERSION > system default
    let sdk_path = if let Some(ref path) = args.macos_sdk_path {
        if !path.exists() {
            return Err(CrossError::SdkPathNotExist { path: path.clone() });
        }
        Some(path.clone())
    } else {
        super::find_apple_sdk(super::AppleSdkType::MacOS, &args.macos_sdk_version).await
    };

    if let Some(ref sdk) = sdk_path {
        env.set_sdkroot(sdk);
        env.add_rustflag(format!("-C link-arg=--sysroot={}", sdk.display()));
        color::log_success(&format!(
            "Using macOS SDK at {}",
            color::cyan(&sdk.display().to_string())
        ));
    }

    // Setup CMake generator if specified
    setup_cmake(&mut env, args.cmake_generator.as_deref(), host.is_windows());

    color::log_success(&format!(
        "Using native macOS toolchain for {}",
        color::yellow(rust_target)
    ));

    Ok(env)
}

/// Setup osxcross for cross-compilation from Linux
async fn setup_osxcross(
    arch: Arch,
    rust_target: &str,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    // Map host architecture
    let host_arch_name = match host.arch {
        "x86_64" | "amd64" => "amd64",
        "aarch64" | "arm64" => "aarch64",
        _ => {
            return Err(CrossError::CrossCompilationNotSupported {
                target_os: "darwin".to_string(),
                host_os: format!("{}/{}", host.os, host.arch),
            });
        }
    };

    let osxcross_version = "v0.2.6";
    let macos_sdk_suffix = args.macos_sdk_version.replace('.', "-");
    let osxcross_dir = args.cross_compiler_dir.join(format!(
        "osxcross-{macos_sdk_suffix}-{host_arch_name}-{osxcross_version}"
    ));

    // Download osxcross if not present
    if !osxcross_dir.join("bin").exists() {
        let ubuntu_version = super::get_ubuntu_version()
            .await
            .unwrap_or_else(|| "20.04".to_string());
        let url_arch = if host_arch_name == "amd64" {
            "x86_64"
        } else {
            host_arch_name
        };

        let download_url = format!(
            "https://github.com/zijiren233/osxcross/releases/download/{osxcross_version}/osxcross-{macos_sdk_suffix}-linux-{url_arch}-gnu-ubuntu-{ubuntu_version}.tar.gz"
        );

        download_and_extract(
            &download_url,
            &osxcross_dir,
            None,
            args.github_proxy.as_deref(),
        )
        .await?;
    }

    let mut env = CrossEnv::new();

    // Setup library path for linker to find its shared libraries
    super::setup_darwin_linker_library_path(&mut env, &osxcross_dir);

    // Set osxcross environment
    env.set_env("OSXCROSS_MP_INC", "1");
    env.set_env("MACOSX_DEPLOYMENT_TARGET", "10.12");

    // Enable osxcross debug output in verbose mode
    if args.verbose_level > 0 {
        env.set_env("OCDEBUG", "1");
    }

    // Find the clang binary using wildcard pattern
    let clang_pattern = format!("{}-apple-darwin*-clang", arch.as_str());
    let clang_path = super::find_file_by_pattern(&osxcross_dir.join("bin"), &clang_pattern)
        .await
        .ok_or_else(|| CrossError::CompilerNotFound {
            path: osxcross_dir.join("bin"),
        })?;

    // Extract tool prefix (e.g., aarch64-apple-darwin25.2)
    let tool_prefix = clang_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix("-clang"))
        .ok_or_else(|| CrossError::CompilerNotFound {
            path: clang_path.clone(),
        })?
        .to_string();

    // Set compiler paths
    env.set_cc(format!("{tool_prefix}-clang"));
    env.set_cxx(format!("{tool_prefix}-clang++"));
    env.set_ar(format!("{tool_prefix}-ar"));
    env.set_linker(format!("{tool_prefix}-clang"));
    env.add_path(osxcross_dir.join("bin"));
    env.add_path(osxcross_dir.join("clang/bin"));

    // Set COMPILER_PATH for cc crate
    env.set_env(
        "COMPILER_PATH",
        osxcross_dir.join("bin").display().to_string(),
    );

    // Set linker flags
    let linker_path = osxcross_dir.join("bin").join(format!("{tool_prefix}-ld"));
    env.add_ldflag(format!("-fuse-ld={}", linker_path.display()));
    env.add_rustflag(format!("-C link-arg=-fuse-ld={}", linker_path.display()));

    // Set SDKROOT from osxcross SDK directory
    let sdk_dir = osxcross_dir.join("SDK");
    if sdk_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&sdk_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("MacOSX") {
                    let sdk_path = entry.path();
                    env.set_sdkroot(&sdk_path);
                    env.add_rustflag(format!("-C link-arg=--sysroot={}", sdk_path.display()));
                    break;
                }
            }
        }
    }

    // Setup CMake generator if specified
    setup_cmake(&mut env, args.cmake_generator.as_deref(), host.is_windows());

    color::log_success(&format!(
        "Configured osxcross toolchain (SDK {}) for {}",
        color::cyan(&args.macos_sdk_version),
        color::yellow(rust_target)
    ));

    Ok(env)
}
