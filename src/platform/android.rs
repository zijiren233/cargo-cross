//! Android NDK cross-compilation setup

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform, TargetConfig};
use crate::download::download_and_extract;
use crate::env::CrossEnv;
use crate::error::{CrossError, Result};
use crate::platform::to_cmake_path;
use std::path::PathBuf;
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

    // Use nested joins to ensure native path separators on Windows
    let prebuilt_dir = ndk_dir.join("toolchains").join("llvm").join("prebuilt");

    // Download NDK if not present
    if !ndk_dir.exists() {
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

    // Detect available prebuilt directory after download
    let clang_base_dir = find_prebuilt_bin_dir(&prebuilt_dir, host).await?;

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
    // On Windows, Android NDK provides .cmd wrappers (not .exe) for clang
    // These .cmd scripts set up the environment and call the real clang
    // We must use .cmd extension because Windows won't execute extensionless files
    let clang_ext = if host.is_windows() { ".cmd" } else { "" };
    env.set_cc(format!("{clang_prefix}-clang{clang_ext}"));
    env.set_cxx(format!("{clang_prefix}-clang++{clang_ext}"));
    env.set_ar(format!(
        "llvm-ar{}",
        if host.is_windows() { ".exe" } else { "" }
    ));
    env.set_linker(format!("{clang_prefix}-clang{clang_ext}"));
    env.add_path(&clang_base_dir);

    // Create wrapper toolchain file for cmake
    // Use nested joins to ensure native path separators on Windows
    let wrapper_toolchain_dir = ndk_dir.join("build").join("cmake").join("wrappers");
    let wrapper_toolchain_file = wrapper_toolchain_dir.join(format!("android-{android_abi}.cmake"));
    let ndk_toolchain_file = ndk_dir
        .join("build")
        .join("cmake")
        .join("android.toolchain.cmake");

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
            to_cmake_path(&ndk_dir),
            to_cmake_path(&ndk_toolchain_file)
        );

        fs::write(&wrapper_toolchain_file, toolchain_content).await?;
    }

    // Set CMAKE_TOOLCHAIN_FILE for CMake-based builds
    env.set_env(
        "CMAKE_TOOLCHAIN_FILE",
        to_cmake_path(&wrapper_toolchain_file),
    );

    // Setup CMake generator (auto-detect on Windows, use specified on any platform)
    crate::platform::setup_cmake(&mut env, args.cmake_generator.as_deref(), host.is_windows());

    // Set LIBCLANG_PATH for bindgen
    let ndk_llvm_base = clang_base_dir.parent().unwrap_or(&clang_base_dir);
    let libclang_name = if host.is_windows() {
        "libclang.dll"
    } else {
        "libclang.so"
    };

    // Check common library directories for libclang
    let lib_candidates = [
        ndk_llvm_base.join("lib"),
        ndk_llvm_base.join("lib64"),
        ndk_llvm_base.join("musl").join("lib"),
    ];
    for libclang_path in &lib_candidates {
        if libclang_path.join(libclang_name).exists() {
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

/// Find the prebuilt bin directory in the NDK
/// Tries multiple possible directory names for cross-platform compatibility
async fn find_prebuilt_bin_dir(prebuilt_dir: &PathBuf, host: &HostPlatform) -> Result<PathBuf> {
    // Possible prebuilt directory names in order of preference
    let candidates = if host.os == "darwin" {
        // macOS: try arch-specific first, then generic
        vec![
            format!("darwin-{}", host.arch), // darwin-aarch64 or darwin-x86_64
            "darwin-x86_64".to_string(),     // Rosetta fallback for ARM Mac
            "darwin".to_string(),            // Generic (some NDK versions)
        ]
    } else {
        // Linux/Windows: typically os-x86_64
        vec![
            format!("{}-{}", host.os, host.arch),
            format!("{}-x86_64", host.os),
        ]
    };

    for candidate in &candidates {
        let bin_dir = prebuilt_dir.join(candidate).join("bin");
        if bin_dir.exists() {
            return Ok(bin_dir);
        }
    }

    // If no known directory found, try to find any directory in prebuilt
    if prebuilt_dir.exists() {
        let mut entries = fs::read_dir(prebuilt_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let bin_dir = entry.path().join("bin");
            if bin_dir.exists() {
                return Ok(bin_dir);
            }
        }
    }

    Err(CrossError::CompilerNotFound {
        path: prebuilt_dir.clone(),
    })
}
