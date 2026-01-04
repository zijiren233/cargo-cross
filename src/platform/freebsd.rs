//! FreeBSD cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, TargetConfig};
use crate::env::{set_gcc_lib_paths, setup_sysroot_env, CrossEnv};
use crate::error::{CrossError, Result};
use crate::platform::{setup_cross_compile_prefix, setup_windows_host_cmake};

/// Setup FreeBSD cross-compilation environment
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let rust_target = target_config.target;
    let freebsd_version = &args.freebsd_version;

    // Validate architecture
    if !matches!(
        arch,
        Arch::X86_64 | Arch::Aarch64 | Arch::Powerpc64 | Arch::Powerpc64le | Arch::Riscv64
    ) {
        return Err(CrossError::UnsupportedArchitecture {
            arch: arch.as_str().to_string(),
            os: "freebsd".to_string(),
        });
    }

    let bin_prefix = format!("{}-unknown-freebsd{}", arch.as_str(), freebsd_version);
    let cross_compiler_name = format!("{bin_prefix}-cross");

    // Add .exe extension on Windows
    let exe_ext = if host.is_windows() { ".exe" } else { "" };
    let gcc_name = format!("{bin_prefix}-gcc{exe_ext}");
    let compiler_dir = args.cross_compiler_dir.join(format!(
        "{}-{}",
        cross_compiler_name, args.cross_deps_version
    ));

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
    if host.is_windows() {
        setup_windows_host_cmake(&mut env);
    }

    color::log_success(&format!(
        "Configured FreeBSD {} toolchain for {}",
        color::yellow(freebsd_version),
        color::yellow(rust_target)
    ));

    Ok(env)
}
