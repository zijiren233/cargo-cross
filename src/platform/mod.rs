//! Platform-specific cross-compilation setup modules

pub mod android;
pub mod darwin;
pub mod freebsd;
pub mod ios;
pub mod linux;
pub mod windows;

use crate::cli::Args;
use crate::config::{Arch, HostPlatform, Libc, Os, TargetConfig};
use crate::env::CrossEnv;
use crate::error::Result;
use path_slash::PathExt as _;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Convert a path to CMake-compatible format (forward slashes)
///
/// `CMake` interprets backslashes as escape sequences (e.g., `\U` in `\Users`),
/// so all paths must use forward slashes. Uses the `path-slash` crate which
/// properly handles:
/// - Windows drive letters (C:\ -> C:/)
/// - UNC paths (\\server\share -> //server/share)
/// - Already forward-slashed paths (no-op)
#[must_use] 
pub fn to_cmake_path(path: &Path) -> String {
    path.to_slash_lossy().into_owned()
}

/// Setup cross-compilation environment for a target
pub async fn setup_cross_env(
    target_config: &TargetConfig,
    args: &Args,
    host: &HostPlatform,
) -> Result<CrossEnv> {
    // Skip toolchain setup if user wants to skip it
    if args.no_toolchain_setup {
        return Ok(CrossEnv::new());
    }

    match target_config.os {
        Os::Linux => linux::setup(target_config, args, host).await,
        Os::Windows => windows::setup(target_config, args, host).await,
        Os::FreeBsd => freebsd::setup(target_config, args, host).await,
        Os::Darwin => darwin::setup(target_config, args, host).await,
        Os::Ios | Os::IosSim => ios::setup(target_config, args, host).await,
        Os::Android => android::setup(target_config, args, host).await,
    }
}

/// Get the binary prefix for a Linux target
#[must_use] 
pub fn get_linux_bin_prefix(arch: Arch, libc: Libc, abi: Option<crate::config::Abi>) -> String {
    let arch_str = arch.as_str();

    // Special handling for gnu abi variants (gnusf, gnuspe, gnuabiv2, gnuabiv2hf)
    // These use combined libc+abi strings instead of separate libc and abi
    if let Some(abi_val) = abi {
        if abi_val.is_gnu_abi_variant() && libc == crate::config::Libc::Gnu {
            return format!("{arch_str}-linux-gnu{}", abi_val.as_str());
        }
    }

    let libc_str = libc.as_str();
    let abi_str = abi.map_or("", |a| a.as_str());

    format!("{arch_str}-linux-{libc_str}{abi_str}")
}

/// Get the cross-compiler folder name for a Linux target
#[must_use] 
pub fn get_linux_folder_name(
    arch: Arch,
    libc: Libc,
    abi: Option<crate::config::Abi>,
    glibc_version: &str,
    default_glibc_version: &str,
) -> String {
    let arch_str = arch.as_str();

    // Special handling for gnu abi variants (gnusf, gnuspe, gnuabiv2, gnuabiv2hf)
    if let Some(abi_val) = abi {
        if abi_val.is_gnu_abi_variant() && libc == crate::config::Libc::Gnu {
            let abi_suffix = abi_val.as_str();
            // For gnu libc, folder name includes glibc version suffix (except for default version)
            let folder_suffix = if glibc_version == default_glibc_version {
                format!("gnu{abi_suffix}")
            } else {
                format!("gnu{abi_suffix}-{glibc_version}")
            };
            return format!("{arch_str}-linux-{folder_suffix}-cross");
        }
    }

    let libc_str = libc.as_str();
    let abi_str = abi.map_or("", |a| a.as_str());

    // For gnu libc, folder name includes glibc version suffix (except for default version)
    let folder_suffix = if libc == Libc::Gnu && glibc_version != default_glibc_version {
        format!("{libc_str}{abi_str}-{glibc_version}")
    } else {
        format!("{libc_str}{abi_str}")
    };

    format!("{arch_str}-linux-{folder_suffix}-cross")
}


/// Setup `CMake` generator for cross-compilation
///
/// If `cmake_generator` is specified, uses it directly.
/// On Windows, auto-detects if not specified (VS ignores CC/CXX).
/// On other platforms, only sets if explicitly specified.
pub fn setup_cmake(env: &mut CrossEnv, cmake_generator: Option<&str>, is_windows: bool) {
    // User specified generator - use it on any platform
    if let Some(g) = cmake_generator {
        env.extra_env
            .insert("CMAKE_GENERATOR".to_string(), g.to_string());
        return;
    }

    // On non-Windows, don't override CMake's default
    if !is_windows {
        return;
    }

    // Auto-detect on Windows: Ninja > MinGW Makefiles > Unix Makefiles
    let generator = if which::which("ninja").is_ok() {
        "Ninja"
    } else if which::which("mingw32-make").is_ok() {
        "MinGW Makefiles"
    } else {
        "Unix Makefiles"
    };
    env.extra_env
        .insert("CMAKE_GENERATOR".to_string(), generator.to_string());
}

/// Setup `CROSS_COMPILE` prefix for cc crate and other build systems
///
/// `CROSS_COMPILE` is a common convention used by:
/// - Linux kernel builds
/// - cc crate (Rust)
/// - Many autoconf/automake projects
///   Note: `bin_dir` should already be in PATH, so we use prefix directly.
pub fn setup_cross_compile_prefix(env: &mut CrossEnv, bin_prefix: &str) {
    // CROSS_COMPILE should be the prefix including trailing dash
    // e.g., "armv7-linux-gnueabihf-" so tools become "${CROSS_COMPILE}gcc"
    env.extra_env
        .insert("CROSS_COMPILE".to_string(), format!("{bin_prefix}-"));
}

/// Setup library path for Darwin/iOS linker binaries
///
/// The Darwin/iOS linker binaries from cross-compilation toolchains need to find their
/// shared libraries at runtime. This function adds the compiler's lib directory to
/// the library path (`LD_LIBRARY_PATH` on Linux, `DYLD_LIBRARY_PATH` on macOS).
pub fn setup_darwin_linker_library_path(env: &mut CrossEnv, compiler_dir: &Path) {
    let lib_dir = compiler_dir.join("lib");
    if lib_dir.exists() {
        env.add_library_path(&lib_dir);
    }
}

/// Get Ubuntu version from `lsb_release` (used for Linux cross-compilation downloads)
pub async fn get_ubuntu_version() -> Option<String> {
    let output = Command::new("lsb_release").arg("-rs").output().await.ok()?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if version.contains('.') {
            return Some(version);
        }
    }
    None
}

/// Find an Apple SDK by version using xcrun and xcode-select
pub async fn find_apple_sdk(sdk_type: AppleSdkType, version: &str) -> Option<PathBuf> {
    let (sdk_name, platform_name) = sdk_type.names(version);

    // Try xcrun first
    if let Some(path) = try_xcrun_sdk(&sdk_name).await {
        return Some(path);
    }

    // Try xcode-select path
    if let Some(path) = try_xcode_select_sdk(platform_name, version).await {
        return Some(path);
    }

    // Search in /Applications/Xcode*.app
    search_xcode_apps_for_sdk(platform_name, version)
}

/// Apple SDK type
#[derive(Debug, Clone, Copy)]
pub enum AppleSdkType {
    MacOS,
    IPhoneOS,
    IPhoneSimulator,
}

impl AppleSdkType {
    /// Get SDK name and platform name for this SDK type
    fn names(&self, version: &str) -> (String, &'static str) {
        match self {
            Self::MacOS => (format!("macosx{version}"), "MacOSX"),
            Self::IPhoneOS => (format!("iphoneos{version}"), "iPhoneOS"),
            Self::IPhoneSimulator => (format!("iphonesimulator{version}"), "iPhoneSimulator"),
        }
    }
}

/// Try to find SDK using xcrun
async fn try_xcrun_sdk(sdk_name: &str) -> Option<PathBuf> {
    let output = Command::new("xcrun")
        .args(["--sdk", sdk_name, "--show-sdk-path"])
        .output()
        .await
        .ok()?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let path = PathBuf::from(&path);
        if path.exists() {
            return Some(path);
        }
    }
    None
}

/// Try to find SDK using xcode-select path
async fn try_xcode_select_sdk(platform_name: &str, version: &str) -> Option<PathBuf> {
    let output = Command::new("xcode-select").arg("-p").output().await.ok()?;

    if output.status.success() {
        let xcode_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let sdk_path = PathBuf::from(&xcode_path)
            .join(format!("Platforms/{platform_name}.platform/Developer/SDKs"))
            .join(format!("{platform_name}{version}.sdk"));
        if sdk_path.exists() {
            return Some(sdk_path);
        }
    }
    None
}

/// Search for SDK in /Applications/Xcode*.app directories
fn search_xcode_apps_for_sdk(platform_name: &str, version: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir("/Applications").ok()?;

    for entry in entries.filter_map(std::result::Result::ok) {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with("Xcode") && name_str.ends_with(".app") {
            let sdk_path = entry
                .path()
                .join(format!(
                    "Contents/Developer/Platforms/{platform_name}.platform/Developer/SDKs"
                ))
                .join(format!("{platform_name}{version}.sdk"));
            if sdk_path.exists() {
                return Some(sdk_path);
            }
        }
    }
    None
}

/// Find a file matching a glob pattern in a directory
///
/// Pattern uses glob syntax where `*` matches any sequence of characters.
/// The pattern must match the entire filename, not just a substring.
pub async fn find_file_by_pattern(dir: &Path, pattern: &str) -> Option<PathBuf> {
    let matcher = globset::Glob::new(pattern).ok()?.compile_matcher();

    let mut entries = tokio::fs::read_dir(dir).await.ok()?;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        if matcher.is_match(&*name.to_string_lossy()) {
            return Some(entry.path());
        }
    }

    None
}

/// Check if a filename matches a glob pattern (for testing)
#[cfg(test)]
fn glob_matches(pattern: &str, filename: &str) -> bool {
    globset::Glob::new(pattern)
        .map(|g| g.compile_matcher().is_match(filename))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Abi;

    // Tests for CMake path conversion (using path-slash crate)

    #[test]
    fn test_to_cmake_path_unix() {
        // Unix paths should pass through unchanged
        let path = Path::new("/home/user/project");
        assert_eq!(to_cmake_path(path), "/home/user/project");
    }

    #[test]
    fn test_to_cmake_path_relative() {
        // Relative paths should work
        let path = Path::new("src/main.rs");
        assert_eq!(to_cmake_path(path), "src/main.rs");
    }

    #[test]
    fn test_to_cmake_path_with_dots() {
        // Paths with . and .. should be preserved
        let path = Path::new("../project/./src");
        assert_eq!(to_cmake_path(path), "../project/./src");
    }

    // Note: Windows-specific path tests (C:\, UNC paths) would only work correctly
    // when compiled and run on Windows. The path-slash crate handles these cases
    // properly on Windows by converting backslashes to forward slashes.

    #[test]
    fn test_linux_bin_prefix_musl() {
        let prefix = get_linux_bin_prefix(Arch::Aarch64, Libc::Musl, None);
        assert_eq!(prefix, "aarch64-linux-musl");
    }

    #[test]
    fn test_linux_bin_prefix_gnu() {
        let prefix = get_linux_bin_prefix(Arch::X86_64, Libc::Gnu, None);
        assert_eq!(prefix, "x86_64-linux-gnu");
    }

    #[test]
    fn test_linux_bin_prefix_with_abi() {
        let prefix = get_linux_bin_prefix(Arch::Armv7, Libc::Musl, Some(Abi::Eabihf));
        assert_eq!(prefix, "armv7-linux-musleabihf");
    }

    #[test]
    fn test_linux_folder_name_musl() {
        let name = get_linux_folder_name(Arch::Aarch64, Libc::Musl, None, "2.28", "2.28");
        assert_eq!(name, "aarch64-linux-musl-cross");
    }

    #[test]
    fn test_linux_folder_name_gnu_default() {
        let name = get_linux_folder_name(Arch::X86_64, Libc::Gnu, None, "2.28", "2.28");
        assert_eq!(name, "x86_64-linux-gnu-cross");
    }

    #[test]
    fn test_linux_folder_name_gnu_custom_version() {
        let name = get_linux_folder_name(Arch::X86_64, Libc::Gnu, None, "2.31", "2.28");
        assert_eq!(name, "x86_64-linux-gnu-2.31-cross");
    }

    #[test]
    fn test_linux_folder_name_with_abi() {
        let name = get_linux_folder_name(Arch::Armv7, Libc::Gnu, Some(Abi::Eabihf), "2.28", "2.28");
        assert_eq!(name, "armv7-linux-gnueabihf-cross");
    }

    // Tests for glob pattern matching (verifying the fix for -libc++ suffix issue)

    #[test]
    fn test_glob_matches_clang_exact() {
        // Should match the exact clang binary
        assert!(glob_matches(
            "x86_64-apple-darwin*-clang",
            "x86_64-apple-darwin25.2-clang"
        ));
    }

    #[test]
    fn test_glob_does_not_match_clang_plus_plus() {
        // Should NOT match clang++ when looking for clang
        // This was the bug: regex "x86_64-apple-darwin.*-clang" would match
        // "x86_64-apple-darwin25.2-clang++" because "clang" is a substring
        assert!(!glob_matches(
            "x86_64-apple-darwin*-clang",
            "x86_64-apple-darwin25.2-clang++"
        ));
    }

    #[test]
    fn test_glob_does_not_match_clang_with_libc_suffix() {
        // Should NOT match clang++-libc++ when looking for clang
        // This was the exact bug reported: finding "clang++-libc++" instead of "clang"
        assert!(!glob_matches(
            "x86_64-apple-darwin*-clang",
            "x86_64-apple-darwin25.2-clang++-libc++"
        ));
    }

    #[test]
    fn test_glob_matches_clang_plus_plus_exact() {
        // Should match clang++ when pattern is for clang++
        assert!(glob_matches(
            "x86_64-apple-darwin*-clang++",
            "x86_64-apple-darwin25.2-clang++"
        ));
    }

    #[test]
    fn test_glob_does_not_match_clang_plus_plus_with_suffix() {
        // Should NOT match clang++-libc++ when looking for clang++
        assert!(!glob_matches(
            "x86_64-apple-darwin*-clang++",
            "x86_64-apple-darwin25.2-clang++-libc++"
        ));
    }

    #[test]
    fn test_glob_matches_aarch64_darwin_clang() {
        assert!(glob_matches(
            "aarch64-apple-darwin*-clang",
            "aarch64-apple-darwin25.2-clang"
        ));
        assert!(!glob_matches(
            "aarch64-apple-darwin*-clang",
            "aarch64-apple-darwin25.2-clang++"
        ));
    }

    #[test]
    fn test_glob_matches_different_darwin_versions() {
        let pattern = "x86_64-apple-darwin*-clang";
        assert!(glob_matches(pattern, "x86_64-apple-darwin24.0-clang"));
        assert!(glob_matches(pattern, "x86_64-apple-darwin25.2-clang"));
        assert!(glob_matches(pattern, "x86_64-apple-darwin26.0-clang"));
        // Should not match clang++ variants
        assert!(!glob_matches(pattern, "x86_64-apple-darwin24.0-clang++"));
        assert!(!glob_matches(pattern, "x86_64-apple-darwin25.2-clang++"));
    }

    #[test]
    fn test_glob_matches_ios_compiler() {
        // iOS uses darwin11 prefix
        assert!(glob_matches(
            "arm64-apple-darwin*-clang",
            "arm64-apple-darwin11-clang"
        ));
        assert!(!glob_matches(
            "arm64-apple-darwin*-clang",
            "arm64-apple-darwin11-clang++"
        ));
    }

    #[test]
    fn test_x32_folder_names() {
        use crate::config::{Abi, Arch, Libc};

        // Test x32 gnu with glibc version
        let folder = get_linux_folder_name(
            Arch::X86_64,
            Libc::Gnu,
            Some(Abi::X32),
            "2.17",
            "",
        );
        assert_eq!(folder, "x86_64-linux-gnux32-2.17-cross");

        // Test x32 gnu with default (empty) version
        let folder = get_linux_folder_name(
            Arch::X86_64,
            Libc::Gnu,
            Some(Abi::X32),
            "",
            "",
        );
        assert_eq!(folder, "x86_64-linux-gnux32-cross");
    }

    #[test]
    fn test_x32_bin_prefix() {
        use crate::config::{Abi, Arch, Libc};

        // Test x32 gnu bin prefix
        let bin_prefix = get_linux_bin_prefix(Arch::X86_64, Libc::Gnu, Some(Abi::X32));
        assert_eq!(bin_prefix, "x86_64-linux-gnux32");
    }

    #[test]
    fn test_aarch64_be_targets() {
        use crate::config::{Arch, Libc};

        // Test aarch64_be musl
        let bin_prefix = get_linux_bin_prefix(Arch::Aarch64Be, Libc::Musl, None);
        assert_eq!(bin_prefix, "aarch64_be-linux-musl");

        let folder = get_linux_folder_name(Arch::Aarch64Be, Libc::Musl, None, "", "");
        assert_eq!(folder, "aarch64_be-linux-musl-cross");

        // Test aarch64_be gnu with version
        let bin_prefix = get_linux_bin_prefix(Arch::Aarch64Be, Libc::Gnu, None);
        assert_eq!(bin_prefix, "aarch64_be-linux-gnu");

        let folder = get_linux_folder_name(Arch::Aarch64Be, Libc::Gnu, None, "2.17", "");
        assert_eq!(folder, "aarch64_be-linux-gnu-2.17-cross");
    }

    #[test]
    fn test_m68k_targets() {
        use crate::config::{Arch, Libc};

        // Test m68k musl
        let bin_prefix = get_linux_bin_prefix(Arch::M68k, Libc::Musl, None);
        assert_eq!(bin_prefix, "m68k-linux-musl");

        let folder = get_linux_folder_name(Arch::M68k, Libc::Musl, None, "", "");
        assert_eq!(folder, "m68k-linux-musl-cross");

        // Test m68k gnu with version
        let bin_prefix = get_linux_bin_prefix(Arch::M68k, Libc::Gnu, None);
        assert_eq!(bin_prefix, "m68k-linux-gnu");

        let folder = get_linux_folder_name(Arch::M68k, Libc::Gnu, None, "2.17", "");
        assert_eq!(folder, "m68k-linux-gnu-2.17-cross");
    }

    #[test]
    fn test_csky_targets() {
        use crate::config::{Abi, Arch, Libc};

        // Test csky gnuabiv2
        let bin_prefix = get_linux_bin_prefix(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2));
        assert_eq!(bin_prefix, "csky-linux-gnuabiv2");

        let folder = get_linux_folder_name(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2), "", "");
        assert_eq!(folder, "csky-linux-gnuabiv2-cross");

        let folder = get_linux_folder_name(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2), "2.17", "");
        assert_eq!(folder, "csky-linux-gnuabiv2-2.17-cross");

        // Test csky gnuabiv2hf
        let bin_prefix = get_linux_bin_prefix(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2Hf));
        assert_eq!(bin_prefix, "csky-linux-gnuabiv2hf");

        let folder = get_linux_folder_name(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2Hf), "", "");
        assert_eq!(folder, "csky-linux-gnuabiv2hf-cross");

        let folder = get_linux_folder_name(Arch::Csky, Libc::Gnu, Some(Abi::GnuAbiv2Hf), "2.17", "");
        assert_eq!(folder, "csky-linux-gnuabiv2hf-2.17-cross");
    }
}
