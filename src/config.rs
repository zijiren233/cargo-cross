//! Target configuration database for cargo-cross

use std::collections::HashMap;

/// Supported glibc versions
pub const SUPPORTED_GLIBC_VERSIONS: &[&str] = &[
    "2.28", "2.31", "2.32", "2.33", "2.34", "2.35", "2.36", "2.37", "2.38", "2.39", "2.40", "2.41",
    "2.42",
];

/// Default glibc version
pub const DEFAULT_GLIBC_VERSION: &str = "2.28";

/// Supported iPhone SDK versions (for Linux cross-compilation)
pub const SUPPORTED_IPHONE_SDK_VERSIONS: &[&str] = &[
    "17.0", "17.2", "17.4", "17.5", "18.0", "18.1", "18.2", "18.4", "18.5", "26.0", "26.1", "26.2",
];

/// Default iPhone SDK version
pub const DEFAULT_IPHONE_SDK_VERSION: &str = "26.2";

/// Supported macOS SDK versions (for Linux cross-compilation)
pub const SUPPORTED_MACOS_SDK_VERSIONS: &[&str] = &[
    "14.0", "14.2", "14.4", "14.5", "15.0", "15.1", "15.2", "15.4", "15.5", "26.0", "26.1", "26.2",
];

/// Default macOS SDK version
pub const DEFAULT_MACOS_SDK_VERSION: &str = "26.2";

/// Supported FreeBSD versions
pub const SUPPORTED_FREEBSD_VERSIONS: &[&str] = &["13", "14", "15"];

/// Default FreeBSD version
pub const DEFAULT_FREEBSD_VERSION: &str = "13";

/// Default cross-compiler dependencies version
pub const DEFAULT_CROSS_DEPS_VERSION: &str = "v0.7.4";

/// Default Android NDK version (LTS)
pub const DEFAULT_NDK_VERSION: &str = "r27d";

/// Default QEMU version
pub const DEFAULT_QEMU_VERSION: &str = "v10.2.0";

/// Format supported versions as comma-separated string
pub fn supported_glibc_versions_str() -> String {
    SUPPORTED_GLIBC_VERSIONS.join(", ")
}

/// Format supported FreeBSD versions as comma-separated string
pub fn supported_freebsd_versions_str() -> String {
    SUPPORTED_FREEBSD_VERSIONS.join(", ")
}

/// Format supported iPhone SDK versions as comma-separated string
pub fn supported_iphone_sdk_versions_str() -> String {
    SUPPORTED_IPHONE_SDK_VERSIONS.join(", ")
}

/// Format supported macOS SDK versions as comma-separated string
pub fn supported_macos_sdk_versions_str() -> String {
    SUPPORTED_MACOS_SDK_VERSIONS.join(", ")
}

/// Operating system type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Os {
    Linux,
    Windows,
    FreeBsd,
    Darwin,
    Ios,
    IosSim,
    Android,
}

impl Os {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::FreeBsd => "freebsd",
            Self::Darwin => "darwin",
            Self::Ios => "ios",
            Self::IosSim => "ios-sim",
            Self::Android => "android",
        }
    }
}

/// Architecture type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    Aarch64,
    Arm64e,
    Armv5,
    Armv6,
    Armv7,
    I586,
    I686,
    Loongarch64,
    Mips,
    Mipsel,
    Mips64,
    Mips64el,
    Powerpc64,
    Powerpc64le,
    Riscv64,
    S390x,
    X86_64,
    X86_64h,
}

impl Arch {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Aarch64 => "aarch64",
            Self::Arm64e => "arm64e",
            Self::Armv5 => "armv5",
            Self::Armv6 => "armv6",
            Self::Armv7 => "armv7",
            Self::I586 => "i586",
            Self::I686 => "i686",
            Self::Loongarch64 => "loongarch64",
            Self::Mips => "mips",
            Self::Mipsel => "mipsel",
            Self::Mips64 => "mips64",
            Self::Mips64el => "mips64el",
            Self::Powerpc64 => "powerpc64",
            Self::Powerpc64le => "powerpc64le",
            Self::Riscv64 => "riscv64",
            Self::S390x => "s390x",
            Self::X86_64 => "x86_64",
            Self::X86_64h => "x86_64h",
        }
    }

    /// Get the QEMU binary name for this architecture
    pub const fn qemu_binary_name(&self) -> Option<&'static str> {
        match self {
            Self::Aarch64 => Some("qemu-aarch64"),
            Self::Armv5 | Self::Armv6 | Self::Armv7 => Some("qemu-arm"),
            Self::I586 | Self::I686 => Some("qemu-i386"),
            Self::Loongarch64 => Some("qemu-loongarch64"),
            Self::Mips => Some("qemu-mips"),
            Self::Mipsel => Some("qemu-mipsel"),
            Self::Mips64 => Some("qemu-mips64"),
            Self::Mips64el => Some("qemu-mips64el"),
            Self::Powerpc64 => Some("qemu-ppc64"),
            Self::Powerpc64le => Some("qemu-ppc64le"),
            Self::Riscv64 => Some("qemu-riscv64"),
            Self::S390x => Some("qemu-s390x"),
            Self::X86_64 => Some("qemu-x86_64"),
            _ => None,
        }
    }
}

/// C library type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Libc {
    Musl,
    Gnu,
    Msvc,
}

impl Libc {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Musl => "musl",
            Self::Gnu => "gnu",
            Self::Msvc => "msvc",
        }
    }
}

/// ABI type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Abi {
    Eabi,
    Eabihf,
}

impl Abi {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Eabi => "eabi",
            Self::Eabihf => "eabihf",
        }
    }
}

/// Target configuration
#[derive(Debug, Clone)]
pub struct TargetConfig {
    pub target: &'static str,
    pub os: Os,
    pub arch: Arch,
    pub libc: Option<Libc>,
    pub abi: Option<Abi>,
}

impl TargetConfig {
    const fn new(target: &'static str, os: Os, arch: Arch) -> Self {
        Self {
            target,
            os,
            arch,
            libc: None,
            abi: None,
        }
    }

    const fn with_libc(mut self, libc: Libc) -> Self {
        self.libc = Some(libc);
        self
    }

    const fn with_abi(mut self, abi: Abi) -> Self {
        self.abi = Some(abi);
        self
    }
}

/// All supported target configurations
pub static TARGETS: std::sync::LazyLock<HashMap<&'static str, TargetConfig>> =
    std::sync::LazyLock::new(|| {
        let configs = vec![
            // Linux musl targets
            TargetConfig::new("aarch64-unknown-linux-musl", Os::Linux, Arch::Aarch64)
                .with_libc(Libc::Musl),
            TargetConfig::new("arm-unknown-linux-musleabi", Os::Linux, Arch::Armv6)
                .with_libc(Libc::Musl)
                .with_abi(Abi::Eabi),
            TargetConfig::new("arm-unknown-linux-musleabihf", Os::Linux, Arch::Armv6)
                .with_libc(Libc::Musl)
                .with_abi(Abi::Eabihf),
            TargetConfig::new("armv5te-unknown-linux-musleabi", Os::Linux, Arch::Armv5)
                .with_libc(Libc::Musl)
                .with_abi(Abi::Eabi),
            TargetConfig::new("armv7-unknown-linux-musleabi", Os::Linux, Arch::Armv7)
                .with_libc(Libc::Musl)
                .with_abi(Abi::Eabi),
            TargetConfig::new("armv7-unknown-linux-musleabihf", Os::Linux, Arch::Armv7)
                .with_libc(Libc::Musl)
                .with_abi(Abi::Eabihf),
            TargetConfig::new("i586-unknown-linux-musl", Os::Linux, Arch::I586)
                .with_libc(Libc::Musl),
            TargetConfig::new("i686-unknown-linux-musl", Os::Linux, Arch::I686)
                .with_libc(Libc::Musl),
            TargetConfig::new(
                "loongarch64-unknown-linux-musl",
                Os::Linux,
                Arch::Loongarch64,
            )
            .with_libc(Libc::Musl),
            TargetConfig::new("mips-unknown-linux-musl", Os::Linux, Arch::Mips)
                .with_libc(Libc::Musl),
            TargetConfig::new("mipsel-unknown-linux-musl", Os::Linux, Arch::Mipsel)
                .with_libc(Libc::Musl),
            TargetConfig::new("mips64-unknown-linux-muslabi64", Os::Linux, Arch::Mips64)
                .with_libc(Libc::Musl),
            TargetConfig::new("mips64-openwrt-linux-musl", Os::Linux, Arch::Mips64)
                .with_libc(Libc::Musl),
            TargetConfig::new(
                "mips64el-unknown-linux-muslabi64",
                Os::Linux,
                Arch::Mips64el,
            )
            .with_libc(Libc::Musl),
            TargetConfig::new("powerpc64-unknown-linux-musl", Os::Linux, Arch::Powerpc64)
                .with_libc(Libc::Musl),
            TargetConfig::new(
                "powerpc64le-unknown-linux-musl",
                Os::Linux,
                Arch::Powerpc64le,
            )
            .with_libc(Libc::Musl),
            TargetConfig::new("riscv64gc-unknown-linux-musl", Os::Linux, Arch::Riscv64)
                .with_libc(Libc::Musl),
            TargetConfig::new("s390x-unknown-linux-musl", Os::Linux, Arch::S390x)
                .with_libc(Libc::Musl),
            TargetConfig::new("x86_64-unknown-linux-musl", Os::Linux, Arch::X86_64)
                .with_libc(Libc::Musl),
            // Linux gnu targets
            TargetConfig::new("aarch64-unknown-linux-gnu", Os::Linux, Arch::Aarch64)
                .with_libc(Libc::Gnu),
            TargetConfig::new("arm-unknown-linux-gnueabi", Os::Linux, Arch::Armv6)
                .with_libc(Libc::Gnu)
                .with_abi(Abi::Eabi),
            TargetConfig::new("arm-unknown-linux-gnueabihf", Os::Linux, Arch::Armv6)
                .with_libc(Libc::Gnu)
                .with_abi(Abi::Eabihf),
            TargetConfig::new("armv5te-unknown-linux-gnueabi", Os::Linux, Arch::Armv5)
                .with_libc(Libc::Gnu)
                .with_abi(Abi::Eabi),
            TargetConfig::new("armv7-unknown-linux-gnueabi", Os::Linux, Arch::Armv7)
                .with_libc(Libc::Gnu)
                .with_abi(Abi::Eabi),
            TargetConfig::new("armv7-unknown-linux-gnueabihf", Os::Linux, Arch::Armv7)
                .with_libc(Libc::Gnu)
                .with_abi(Abi::Eabihf),
            TargetConfig::new("i586-unknown-linux-gnu", Os::Linux, Arch::I586).with_libc(Libc::Gnu),
            TargetConfig::new("i686-unknown-linux-gnu", Os::Linux, Arch::I686).with_libc(Libc::Gnu),
            TargetConfig::new(
                "loongarch64-unknown-linux-gnu",
                Os::Linux,
                Arch::Loongarch64,
            )
            .with_libc(Libc::Gnu),
            TargetConfig::new("mips-unknown-linux-gnu", Os::Linux, Arch::Mips).with_libc(Libc::Gnu),
            TargetConfig::new("mipsel-unknown-linux-gnu", Os::Linux, Arch::Mipsel)
                .with_libc(Libc::Gnu),
            TargetConfig::new("mips64-unknown-linux-gnuabi64", Os::Linux, Arch::Mips64)
                .with_libc(Libc::Gnu),
            TargetConfig::new("mips64el-unknown-linux-gnuabi64", Os::Linux, Arch::Mips64el)
                .with_libc(Libc::Gnu),
            TargetConfig::new("powerpc64-unknown-linux-gnu", Os::Linux, Arch::Powerpc64)
                .with_libc(Libc::Gnu),
            TargetConfig::new(
                "powerpc64le-unknown-linux-gnu",
                Os::Linux,
                Arch::Powerpc64le,
            )
            .with_libc(Libc::Gnu),
            TargetConfig::new("riscv64gc-unknown-linux-gnu", Os::Linux, Arch::Riscv64)
                .with_libc(Libc::Gnu),
            TargetConfig::new("s390x-unknown-linux-gnu", Os::Linux, Arch::S390x)
                .with_libc(Libc::Gnu),
            TargetConfig::new("x86_64-unknown-linux-gnu", Os::Linux, Arch::X86_64)
                .with_libc(Libc::Gnu),
            // Windows GNU targets
            TargetConfig::new("i686-pc-windows-gnu", Os::Windows, Arch::I686).with_libc(Libc::Gnu),
            TargetConfig::new("x86_64-pc-windows-gnu", Os::Windows, Arch::X86_64)
                .with_libc(Libc::Gnu),
            // FreeBSD targets
            TargetConfig::new("x86_64-unknown-freebsd", Os::FreeBsd, Arch::X86_64),
            TargetConfig::new("aarch64-unknown-freebsd", Os::FreeBsd, Arch::Aarch64),
            TargetConfig::new("powerpc64-unknown-freebsd", Os::FreeBsd, Arch::Powerpc64),
            TargetConfig::new(
                "powerpc64le-unknown-freebsd",
                Os::FreeBsd,
                Arch::Powerpc64le,
            ),
            TargetConfig::new("riscv64gc-unknown-freebsd", Os::FreeBsd, Arch::Riscv64),
            // Darwin (macOS) targets
            TargetConfig::new("x86_64-apple-darwin", Os::Darwin, Arch::X86_64),
            TargetConfig::new("x86_64h-apple-darwin", Os::Darwin, Arch::X86_64h),
            TargetConfig::new("aarch64-apple-darwin", Os::Darwin, Arch::Aarch64),
            TargetConfig::new("arm64e-apple-darwin", Os::Darwin, Arch::Arm64e),
            // iOS targets
            TargetConfig::new("x86_64-apple-ios", Os::Ios, Arch::X86_64),
            TargetConfig::new("aarch64-apple-ios", Os::Ios, Arch::Aarch64),
            TargetConfig::new("aarch64-apple-ios-sim", Os::IosSim, Arch::Aarch64),
            // Android targets
            TargetConfig::new("aarch64-linux-android", Os::Android, Arch::Aarch64),
            TargetConfig::new("arm-linux-androideabi", Os::Android, Arch::Armv7),
            TargetConfig::new("armv7-linux-androideabi", Os::Android, Arch::Armv7),
            TargetConfig::new("i686-linux-android", Os::Android, Arch::I686),
            TargetConfig::new("riscv64-linux-android", Os::Android, Arch::Riscv64),
            TargetConfig::new("x86_64-linux-android", Os::Android, Arch::X86_64),
        ];

        configs.into_iter().map(|c| (c.target, c)).collect()
    });

/// Get target configuration by name
pub fn get_target_config(target: &str) -> Option<&'static TargetConfig> {
    TARGETS.get(target)
}

/// Get all supported targets
pub fn all_targets() -> impl Iterator<Item = &'static str> {
    TARGETS.keys().copied()
}

/// Expand target patterns
///
/// Supports:
/// - `all` - all targets
/// - Glob patterns (using globset):
///   - `*` - matches any sequence of characters
///   - `?` - matches any single character
///   - `[abc]` - matches any character in the set
///   - `[a-z]` - matches any character in the range
///   - `[!abc]` - matches any character NOT in the set
///   - `{a,b,c}` - matches any of the alternatives
///   - Examples: `*-linux-musl`, `aarch64-*`, `{x86_64,aarch64}-*-linux-*`
/// - Regex patterns (prefix with `~`): `~.*linux.*(musl|gnu)`, `~^x86_64-.*`
/// - Direct target name: `x86_64-unknown-linux-gnu`
pub fn expand_targets(pattern: &str) -> Vec<&'static str> {
    let pattern = pattern.trim();

    let mut targets: Vec<&'static str> = if pattern == "all" {
        TARGETS.keys().copied().collect()
    } else if let Some(regex_pattern) = pattern.strip_prefix('~') {
        // Regex mode: prefix with ~
        regex::Regex::new(regex_pattern).map_or_else(
            |_| vec![],
            |re| TARGETS.keys().copied().filter(|t| re.is_match(t)).collect(),
        )
    } else if pattern.contains('*')
        || pattern.contains('?')
        || pattern.contains('[')
        || pattern.contains('{')
    {
        // Use globset for glob pattern matching
        globset::Glob::new(pattern).map_or_else(
            |_| vec![],
            |glob| {
                let matcher = glob.compile_matcher();
                TARGETS
                    .keys()
                    .copied()
                    .filter(|t| matcher.is_match(t))
                    .collect()
            },
        )
    } else {
        // Direct target name - lookup to get the static reference
        TARGETS
            .get(pattern)
            .map_or_else(std::vec::Vec::new, |config| vec![config.target])
    };

    // Sort targets for consistent output
    targets.sort_unstable();
    targets
}

/// Host platform information
#[derive(Debug, Clone)]
pub struct HostPlatform {
    pub os: &'static str,
    pub arch: &'static str,
    pub triple: String,
}

impl HostPlatform {
    /// Detect current host platform
    pub fn detect() -> Self {
        let os = if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else if cfg!(target_os = "freebsd") {
            "freebsd"
        } else {
            "unknown"
        };

        let arch = if cfg!(target_arch = "x86_64") {
            "x86_64"
        } else if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else if cfg!(target_arch = "arm") {
            "armv7"
        } else if cfg!(target_arch = "x86") {
            "i686"
        } else if cfg!(target_arch = "s390x") {
            "s390x"
        } else if cfg!(target_arch = "riscv64") {
            "riscv64"
        } else if cfg!(target_arch = "loongarch64") {
            "loongarch64"
        } else if cfg!(all(target_arch = "powerpc64", target_endian = "big")) {
            "powerpc64"
        } else if cfg!(all(target_arch = "powerpc64", target_endian = "little")) {
            "powerpc64le"
        } else if cfg!(all(target_arch = "mips64", target_endian = "big")) {
            "mips64"
        } else if cfg!(all(target_arch = "mips64", target_endian = "little")) {
            "mips64el"
        } else {
            "unknown"
        };

        // Get the host triple from rustc
        let triple = std::process::Command::new("rustc")
            .args(["-vV"])
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout).ok().and_then(|s| {
                    s.lines()
                        .find(|line| line.starts_with("host:"))
                        .map(|line| line.trim_start_matches("host:").trim().to_string())
                })
            })
            .unwrap_or_else(|| format!("{arch}-unknown-{os}"));

        Self { os, arch, triple }
    }

    /// Get platform string for downloads (e.g., "linux-x86_64")
    pub fn download_platform(&self) -> String {
        format!("{}-{}", self.os, self.arch)
    }

    /// Check if host can natively run the target architecture
    pub fn can_run_natively(&self, target_arch: Arch) -> bool {
        match self.arch {
            "x86_64" => matches!(target_arch, Arch::X86_64 | Arch::I686 | Arch::I586),
            "aarch64" => matches!(
                target_arch,
                Arch::Aarch64 | Arch::Armv5 | Arch::Armv6 | Arch::Armv7
            ),
            "i686" | "i586" => matches!(target_arch, Arch::I686 | Arch::I586),
            _ => self.arch == target_arch.as_str(),
        }
    }

    /// Check if running on Windows
    pub fn is_windows(&self) -> bool {
        self.os == "windows"
    }

    /// Check if running on macOS/Darwin
    pub fn is_darwin(&self) -> bool {
        self.os == "darwin"
    }

    /// Check if running on Linux
    pub fn is_linux(&self) -> bool {
        self.os == "linux"
    }

    /// Get PATH separator for this platform
    pub fn path_separator(&self) -> &'static str {
        if self.is_windows() {
            ";"
        } else {
            ":"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_lookup() {
        assert!(get_target_config("x86_64-unknown-linux-musl").is_some());
        assert!(get_target_config("invalid-target").is_none());
    }

    #[test]
    fn test_expand_all() {
        let targets = expand_targets("all");
        assert!(targets.len() > 50);
    }

    #[test]
    fn test_expand_pattern() {
        let targets = expand_targets("*-linux-musl");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.ends_with("-linux-musl") || target.contains("-linux-musl"));
        }
    }

    #[test]
    fn test_expand_direct() {
        let targets = expand_targets("x86_64-unknown-linux-gnu");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0], "x86_64-unknown-linux-gnu");
    }

    #[test]
    fn test_glibc_versions() {
        assert!(SUPPORTED_GLIBC_VERSIONS.contains(&"2.28"));
        assert!(SUPPORTED_GLIBC_VERSIONS.contains(&"2.31"));
    }

    #[test]
    fn test_os_as_str() {
        assert_eq!(Os::Linux.as_str(), "linux");
        assert_eq!(Os::Windows.as_str(), "windows");
        assert_eq!(Os::Darwin.as_str(), "darwin");
        assert_eq!(Os::FreeBsd.as_str(), "freebsd");
        assert_eq!(Os::Ios.as_str(), "ios");
        assert_eq!(Os::Android.as_str(), "android");
    }

    #[test]
    fn test_arch_as_str() {
        assert_eq!(Arch::X86_64.as_str(), "x86_64");
        assert_eq!(Arch::Aarch64.as_str(), "aarch64");
        assert_eq!(Arch::Armv7.as_str(), "armv7");
        assert_eq!(Arch::Riscv64.as_str(), "riscv64");
    }

    #[test]
    fn test_arch_qemu_binary_name() {
        assert_eq!(Arch::Aarch64.qemu_binary_name(), Some("qemu-aarch64"));
        assert_eq!(Arch::X86_64.qemu_binary_name(), Some("qemu-x86_64"));
        assert_eq!(Arch::Armv7.qemu_binary_name(), Some("qemu-arm"));
        assert_eq!(Arch::Riscv64.qemu_binary_name(), Some("qemu-riscv64"));
    }

    #[test]
    fn test_libc_as_str() {
        assert_eq!(Libc::Musl.as_str(), "musl");
        assert_eq!(Libc::Gnu.as_str(), "gnu");
    }

    #[test]
    fn test_abi_as_str() {
        assert_eq!(Abi::Eabi.as_str(), "eabi");
        assert_eq!(Abi::Eabihf.as_str(), "eabihf");
    }

    #[test]
    fn test_expand_invalid_target() {
        let targets = expand_targets("totally-invalid-target-name");
        assert!(targets.is_empty());
    }

    #[test]
    fn test_expand_freebsd_pattern() {
        let targets = expand_targets("*-freebsd");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.ends_with("-freebsd"));
        }
    }

    #[test]
    fn test_expand_darwin_pattern() {
        let targets = expand_targets("*-apple-darwin");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.ends_with("-apple-darwin"));
        }
    }

    #[test]
    fn test_expand_regex_pattern() {
        // Regex mode with ~ prefix
        let targets = expand_targets("~.*linux.*(musl|gnu)");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.contains("linux"));
            assert!(target.contains("musl") || target.contains("gnu"));
        }
    }

    #[test]
    fn test_expand_regex_anchored() {
        // Anchored regex
        let targets = expand_targets("~^x86_64-.*-linux-musl$");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.starts_with("x86_64-"));
            assert!(target.ends_with("-linux-musl"));
        }
    }

    #[test]
    fn test_expand_glob_question_mark() {
        // ? matches single character
        let targets = expand_targets("x86_64-unknown-linux-???");
        assert!(targets.contains(&"x86_64-unknown-linux-gnu"));
        // Test with architecture - ?686 matches i686
        let targets2 = expand_targets("?686-unknown-linux-gnu");
        assert!(targets2.contains(&"i686-unknown-linux-gnu"));
    }

    #[test]
    fn test_expand_glob_brackets() {
        // [abc] character class
        let targets = expand_targets("[xi]86_64-unknown-linux-gnu");
        assert!(targets.contains(&"x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn test_expand_glob_braces() {
        // {a,b} alternatives
        let targets = expand_targets("{x86_64,aarch64}-unknown-linux-gnu");
        assert!(targets.contains(&"x86_64-unknown-linux-gnu"));
        assert!(targets.contains(&"aarch64-unknown-linux-gnu"));
        assert_eq!(targets.len(), 2);
    }

    #[test]
    fn test_expand_glob_braces_with_wildcard() {
        // Combine braces with wildcards
        let targets = expand_targets("{x86_64,aarch64}-*-linux-musl");
        assert!(!targets.is_empty());
        for target in &targets {
            assert!(target.starts_with("x86_64-") || target.starts_with("aarch64-"));
            assert!(target.ends_with("-linux-musl"));
        }
    }

    #[test]
    fn test_all_targets_count() {
        let count = all_targets().count();
        assert!(count > 50, "Expected more than 50 targets, got {count}");
    }

    #[test]
    fn test_target_config_linux_musl() {
        let config = get_target_config("aarch64-unknown-linux-musl").unwrap();
        assert_eq!(config.os, Os::Linux);
        assert_eq!(config.arch, Arch::Aarch64);
        assert_eq!(config.libc, Some(Libc::Musl));
    }

    #[test]
    fn test_target_config_windows() {
        let config = get_target_config("x86_64-pc-windows-gnu").unwrap();
        assert_eq!(config.os, Os::Windows);
        assert_eq!(config.arch, Arch::X86_64);
    }

    #[test]
    fn test_target_config_darwin() {
        let config = get_target_config("aarch64-apple-darwin").unwrap();
        assert_eq!(config.os, Os::Darwin);
        assert_eq!(config.arch, Arch::Aarch64);
    }

    #[test]
    fn test_target_config_android() {
        let config = get_target_config("aarch64-linux-android").unwrap();
        assert_eq!(config.os, Os::Android);
        assert_eq!(config.arch, Arch::Aarch64);
    }
}
