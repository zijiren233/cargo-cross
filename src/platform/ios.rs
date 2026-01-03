//! iOS cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, Os, TargetConfig};
use crate::download::download_and_extract;
use crate::env::CrossEnv;
use crate::error::{CrossError, Result};

/// Setup iOS cross-compilation environment
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let rust_target = target_config.target;
    let is_simulator = matches!(target_config.os, Os::IosSim) || arch == Arch::X86_64;

    if host.is_darwin() {
        setup_native(rust_target, args, is_simulator).await
    } else if host.is_linux() {
        setup_ioscross(arch, rust_target, args, host, is_simulator).await
    } else {
        Err(CrossError::CrossCompilationNotSupported {
            target_os: "ios".to_string(),
            host_os: host.os.to_string(),
        })
    }
}

/// Setup native iOS compilation (on macOS host)
async fn setup_native(rust_target: &str, args: &Args, is_simulator: bool) -> Result<CrossEnv> {
    let mut env = CrossEnv::new();

    let sdk_type = if is_simulator {
        super::AppleSdkType::IPhoneSimulator
    } else {
        super::AppleSdkType::IPhoneOS
    };

    // Check custom SDK path first
    let sdk_path = if is_simulator {
        if let Some(ref path) = args.iphone_simulator_sdk_path {
            if !path.exists() {
                return Err(CrossError::SdkPathNotExist { path: path.clone() });
            }
            Some(path.clone())
        } else {
            super::find_apple_sdk(sdk_type, &args.iphone_sdk_version).await
        }
    } else if let Some(ref path) = args.iphone_sdk_path {
        if !path.exists() {
            return Err(CrossError::SdkPathNotExist { path: path.clone() });
        }
        Some(path.clone())
    } else {
        super::find_apple_sdk(sdk_type, &args.iphone_sdk_version).await
    };

    if let Some(ref sdk) = sdk_path {
        env.set_sdkroot(sdk);
        env.add_rustflag(format!("-C link-arg=--sysroot={}", sdk.display()));
        color::log_success(&format!(
            "Using iPhone SDK at {}",
            color::cyan(&sdk.display().to_string())
        ));
    }

    // Set deployment target to match Rust's minimum iOS version
    // This ensures C code (e.g., aws-lc-sys) is compiled with the same minimum version
    // as the Rust target, avoiding symbol mismatches like ___chkstk_darwin
    let deployment_target = if is_simulator {
        "IPHONE_SIMULATOR_DEPLOYMENT_TARGET"
    } else {
        "IPHONEOS_DEPLOYMENT_TARGET"
    };
    // Use iOS 12.0 as minimum - this is a reasonable baseline that has ___chkstk_darwin
    // and is compatible with modern Rust iOS targets
    env.set_env(deployment_target, "12.0");

    color::log_success(&format!(
        "Using native macOS toolchain for {}",
        color::yellow(rust_target)
    ));

    Ok(env)
}

/// Setup ioscross for cross-compilation from Linux
async fn setup_ioscross(
    arch: Arch,
    rust_target: &str,
    args: &Args,
    host: &HostPlatform,
    is_simulator: bool,
) -> Result<CrossEnv> {
    // Map architecture
    let arch_prefix = match arch {
        Arch::Aarch64 => "arm64",
        Arch::X86_64 => "x86_64",
        _ => {
            return Err(CrossError::UnsupportedArchitecture {
                arch: arch.as_str().to_string(),
                os: "ios".to_string(),
            });
        }
    };

    let cctools_version = "v0.1.9";
    let iphone_sdk_suffix = args.iphone_sdk_version.replace('.', "-");

    let mut cross_compiler_name = format!("ios-{arch_prefix}-cross");
    if is_simulator {
        cross_compiler_name.push_str("-simulator");
    }
    cross_compiler_name.push_str(&format!("-{cctools_version}-{iphone_sdk_suffix}"));

    let clang_name = format!("{arch_prefix}-apple-darwin11-clang");
    let compiler_dir = args.cross_compiler_dir.join(&cross_compiler_name);

    // Download compiler if not present
    if !compiler_dir.join("bin").join(&clang_name).exists() {
        let host_platform = host.download_platform();
        let ubuntu_version = super::get_ubuntu_version()
            .await
            .unwrap_or_else(|| "20.04".to_string());

        let ios_sdk_type = if is_simulator {
            "iPhoneSimulator"
        } else {
            "iPhoneOS"
        };

        let download_url = format!(
            "https://github.com/zijiren233/cctools-port/releases/download/{cctools_version}/ioscross-{ios_sdk_type}{iphone_sdk_suffix}-{arch_prefix}-{host_platform}-gnu-ubuntu-{ubuntu_version}.tar.gz"
        );

        download_and_extract(
            &download_url,
            &compiler_dir,
            None,
            args.github_proxy.as_deref(),
        )
        .await?;
    }

    let mut env = CrossEnv::new();

    // Setup library path for linker to find its shared libraries
    super::setup_darwin_linker_library_path(&mut env, &compiler_dir);

    // Set compiler paths
    env.set_cc(format!("{arch_prefix}-apple-darwin11-clang"));
    env.set_cxx(format!("{arch_prefix}-apple-darwin11-clang++"));
    env.set_ar(format!("{arch_prefix}-apple-darwin11-ar"));
    env.set_linker(format!("{arch_prefix}-apple-darwin11-clang"));
    env.add_path(compiler_dir.join("bin"));
    env.add_path(compiler_dir.join("clang/bin"));

    // Set linker flags
    let linker_path = compiler_dir
        .join("bin")
        .join(format!("{arch_prefix}-apple-darwin11-ld"));
    env.add_ldflag(format!("-fuse-ld={}", linker_path.display()));
    env.add_rustflag(format!("-C link-arg=-fuse-ld={}", linker_path.display()));

    // Set SDKROOT from SDK directory
    let sdk_dir = compiler_dir.join("SDK");
    if sdk_dir.exists() {
        if let Ok(mut entries) = tokio::fs::read_dir(&sdk_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() {
                    env.set_sdkroot(&path);
                    break;
                }
            }
        }
    }

    // Set deployment target to ensure C code uses compatible minimum version
    let deployment_target = if is_simulator {
        "IPHONE_SIMULATOR_DEPLOYMENT_TARGET"
    } else {
        "IPHONEOS_DEPLOYMENT_TARGET"
    };
    env.set_env(deployment_target, "12.0");

    color::log_success(&format!(
        "Configured iOS toolchain for {}",
        color::yellow(rust_target)
    ));

    Ok(env)
}
