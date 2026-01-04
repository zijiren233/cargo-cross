//! Windows cross-compilation setup (MinGW-w64 for GNU, native for MSVC)

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, Libc, TargetConfig};
use crate::env::{set_gcc_lib_paths, setup_sysroot_env, CrossEnv};
use crate::error::{CrossError, Result};
use crate::platform::{setup_cross_compile_prefix, setup_windows_host_cmake};
use crate::runner;

/// Setup Windows cross-compilation environment
///
/// - MSVC targets on Windows host: use native MSVC toolchain (skip setup)
/// - GNU targets (any host): use MinGW-w64 from cross-make
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let rust_target = target_config.target;

    // MSVC targets on Windows host use native toolchain
    if target_config.libc == Some(Libc::Msvc) {
        if host.is_windows() {
            color::log_success(&format!(
                "Using native MSVC toolchain for {}",
                color::yellow(rust_target)
            ));
            return Ok(CrossEnv::new());
        }
        // MSVC cross-compilation from non-Windows is not supported
        return Err(CrossError::CrossCompilationNotSupported {
            target_os: "windows-msvc".to_string(),
            host_os: host.os.to_string(),
        });
    }

    // GNU targets require MinGW-w64 toolchain
    setup_mingw(target_config, args, host).await
}

/// Setup MinGW-w64 toolchain for GNU targets
async fn setup_mingw(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let rust_target = target_config.target;

    // Validate architecture for MinGW
    if !matches!(arch, Arch::I686 | Arch::X86_64) {
        return Err(CrossError::UnsupportedArchitecture {
            arch: arch.as_str().to_string(),
            os: "windows-gnu".to_string(),
        });
    }

    // Setup MinGW-w64 toolchain (required even on Windows for GNU targets)
    let bin_prefix = format!("{}-w64-mingw32", arch.as_str());
    let cross_compiler_name = format!("{bin_prefix}-cross");
    let compiler_dir = args.cross_compiler_dir.join(format!(
        "{}-{}",
        cross_compiler_name, args.cross_deps_version
    ));

    // Determine executable extension and gcc name based on host
    let exe_ext = if host.is_windows() { ".exe" } else { "" };
    let gcc_name = format!("{bin_prefix}-gcc{exe_ext}");

    // Download compiler if not present
    let gcc_path = compiler_dir.join("bin").join(&gcc_name);
    if !gcc_path.exists() {
        let host_platform = host.download_platform();

        // Windows hosts use .zip, others use .tgz
        let (extension, format_hint) = if host.is_windows() {
            (".zip", Some(crate::download::ArchiveFormat::Zip))
        } else {
            (".tgz", Some(crate::download::ArchiveFormat::TarGz))
        };

        let download_url = format!(
            "https://github.com/zijiren233/cross-make/releases/download/{}-{}/{}{}",
            args.cross_deps_version, host_platform, cross_compiler_name, extension
        );
        crate::download::download_and_extract(
            &download_url,
            &compiler_dir,
            format_hint,
            args.github_proxy.as_deref(),
        )
        .await?;
    }

    let mut env = CrossEnv::new();
    let bin_dir = compiler_dir.join("bin");

    env.set_cc(&gcc_name);
    env.set_cxx(format!("{bin_prefix}-g++{exe_ext}"));
    env.set_ar(format!("{bin_prefix}-ar{exe_ext}"));
    env.set_linker(&gcc_name);
    env.add_path(&bin_dir);

    // Add library search paths from gcc to rustc
    set_gcc_lib_paths(&mut env, &compiler_dir, &bin_prefix);

    // Set BINDGEN_EXTRA_CLANG_ARGS for cross-compilation
    setup_sysroot_env(&mut env, &compiler_dir, &bin_prefix, rust_target);

    // Set CROSS_COMPILE prefix for cc crate and other build systems
    setup_cross_compile_prefix(&mut env, &bin_prefix);

    // On Windows, CMake defaults to Visual Studio which ignores CC/CXX
    // Force Ninja generator which respects CC/CXX env vars
    if host.is_windows() {
        setup_windows_host_cmake(&mut env);
    }

    // Setup Wine runner for cross-compiled Windows binaries (only on non-Windows hosts)
    if !host.is_windows() && args.command.needs_runner() {
        runner::setup_wine_runner(&mut env, rust_target);
    }

    color::log_success(&format!(
        "Configured MinGW-w64 toolchain for {}",
        color::yellow(rust_target)
    ));

    Ok(env)
}
