//! Linux cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{HostPlatform, Libc, TargetConfig, DEFAULT_GLIBC_VERSION};
use crate::download::download_cross_compiler;
use crate::env::{set_gcc_lib_paths, setup_sysroot_env, CrossEnv};
use crate::error::Result;
use crate::platform::{get_linux_bin_prefix, get_linux_folder_name};
use crate::runner;

/// Setup Linux cross-compilation environment
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let libc = target_config.libc.expect("Linux target must have libc");
    let abi = target_config.abi;
    let rust_target = target_config.target;

    // Binary names never include glibc version (binaries are in separate versioned folders)
    let bin_prefix = get_linux_bin_prefix(arch, libc, abi);

    // For gnu libc, folder name includes glibc version suffix (except for default version)
    let cross_compiler_name =
        get_linux_folder_name(arch, libc, abi, &args.glibc_version, DEFAULT_GLIBC_VERSION);

    let gcc_name = format!("{bin_prefix}-gcc");
    let compiler_dir = args.cross_compiler_dir.join(format!(
        "{}-{}",
        cross_compiler_name, args.cross_deps_version
    ));

    // Download compiler if not present
    let gcc_path = compiler_dir.join("bin").join(&gcc_name);
    if !gcc_path.exists() {
        let host_platform = host.download_platform();
        let download_url = format!(
            "https://github.com/zijiren233/cross-make/releases/download/{}-{}/{}.tgz",
            args.cross_deps_version, host_platform, cross_compiler_name
        );
        download_cross_compiler(&compiler_dir, &download_url, args.github_proxy.as_deref()).await?;
    }

    let mut env = CrossEnv::new();

    // Set compiler paths
    env.set_cc(&gcc_name);
    env.set_cxx(format!("{bin_prefix}-g++"));
    env.set_ar(format!("{bin_prefix}-ar"));
    env.set_linker(&gcc_name);
    env.add_path(compiler_dir.join("bin"));

    // Add library search paths from gcc to rustc
    set_gcc_lib_paths(&mut env, &compiler_dir, &bin_prefix);

    // Set BINDGEN_EXTRA_CLANG_ARGS and C_INCLUDE_PATH for cross-compilation
    setup_sysroot_env(&mut env, &compiler_dir, &bin_prefix, rust_target);

    // Setup runner only if the command needs to execute binaries
    if args.command.needs_runner() {
        if host.is_darwin() {
            runner::setup_docker_qemu_runner(
                &mut env,
                arch,
                &bin_prefix,
                &compiler_dir,
                libc.as_str(),
                args,
                host,
            )
            .await?;
        } else if host.is_linux() {
            runner::setup_qemu_runner(&mut env, arch, &bin_prefix, &compiler_dir, args, host)
                .await?;
        }
    }

    let libc_display = if libc == Libc::Gnu && args.glibc_version != DEFAULT_GLIBC_VERSION {
        format!("{} {}", libc.as_str(), args.glibc_version)
    } else {
        libc.as_str().to_string()
    };

    color::log_success(&format!(
        "Configured Linux {libc_display} toolchain for {rust_target}"
    ));

    Ok(env)
}
