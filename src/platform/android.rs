//! Android NDK cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, TargetConfig};
use crate::download::download_and_extract;
use crate::env::CrossEnv;
use crate::error::{CrossError, Result};
use tokio::fs;

/// Setup Android cross-compilation environment
pub async fn setup(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    let arch = target_config.arch;
    let rust_target = target_config.target;

    let ndk_dir = args
        .cross_compiler_dir
        .join(format!("android-ndk-{}-{}", host.os, args.ndk_version));

    let clang_base_dir = ndk_dir
        .join("toolchains/llvm/prebuilt")
        .join(format!("{}-x86_64", host.os))
        .join("bin");

    // Download NDK if not present
    if !ndk_dir.exists() || !clang_base_dir.exists() {
        let ndk_url = format!(
            "https://dl.google.com/android/repository/android-ndk-{}-{}.zip",
            args.ndk_version, host.os
        );

        download_and_extract(
            &ndk_url,
            &ndk_dir,
            Some(crate::download::ArchiveFormat::Zip),
            args.github_proxy.as_deref(),
        )
        .await?;

        // Move contents from nested directory if present
        let nested_dir = ndk_dir.join(format!("android-ndk-{}", args.ndk_version));
        if nested_dir.exists() {
            // Move all contents from nested directory to ndk_dir
            let mut entries = fs::read_dir(&nested_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let dest = ndk_dir.join(entry.file_name());
                fs::rename(entry.path(), &dest).await?;
            }
            fs::remove_dir(&nested_dir).await.ok();
        }
    }

    // Map architecture to Android target prefix
    let (clang_prefix, android_abi) = match arch {
        Arch::Armv7 => ("armv7a-linux-androideabi24", "armeabi-v7a"),
        Arch::Aarch64 => ("aarch64-linux-android24", "arm64-v8a"),
        Arch::I686 => ("i686-linux-android24", "x86"),
        Arch::X86_64 => ("x86_64-linux-android24", "x86_64"),
        Arch::Riscv64 => ("riscv64-linux-android35", "riscv64"),
        _ => {
            return Err(CrossError::UnsupportedArchitecture {
                arch: arch.as_str().to_string(),
                os: "android".to_string(),
            });
        }
    };

    let mut env = CrossEnv::new();

    // Set compiler paths
    env.set_cc(format!("{clang_prefix}-clang"));
    env.set_cxx(format!("{clang_prefix}-clang++"));
    env.set_ar("llvm-ar".to_string());
    env.set_linker(format!("{clang_prefix}-clang"));
    env.add_path(&clang_base_dir);

    // Create wrapper toolchain file for cmake
    let wrapper_toolchain_dir = ndk_dir.join("build/cmake/wrappers");
    let wrapper_toolchain_file = wrapper_toolchain_dir.join(format!("android-{android_abi}.cmake"));
    let ndk_toolchain_file = ndk_dir.join("build/cmake/android.toolchain.cmake");

    if !wrapper_toolchain_file.exists() {
        fs::create_dir_all(&wrapper_toolchain_dir).await?;

        let toolchain_content = format!(
            r#"# Auto-generated Android toolchain wrapper
set(ANDROID_ABI "{}")
set(ANDROID_PLATFORM "android-24")
set(ANDROID_NDK "{}")
include("{}")
"#,
            android_abi,
            ndk_dir.display(),
            ndk_toolchain_file.display()
        );

        fs::write(&wrapper_toolchain_file, toolchain_content).await?;
    }

    // Set CMAKE_TOOLCHAIN_FILE for CMake-based builds
    env.set_env(
        "CMAKE_TOOLCHAIN_FILE",
        wrapper_toolchain_file.display().to_string(),
    );

    // Set LIBCLANG_PATH for bindgen
    let ndk_llvm_base = ndk_dir
        .join("toolchains/llvm/prebuilt")
        .join(format!("{}-x86_64", host.os));

    for lib_dir in &["lib", "lib64", "musl/lib"] {
        let libclang_path = ndk_llvm_base.join(lib_dir);
        let libclang_so = libclang_path.join("libclang.so");
        if libclang_so.exists() {
            env.set_env("LIBCLANG_PATH", libclang_path.display().to_string());
            break;
        }
    }

    color::log_success(&format!(
        "Configured Android toolchain for {}",
        color::yellow(rust_target)
    ));

    Ok(env)
}
