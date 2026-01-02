//! Command-line argument parsing for cargo-cross
//!
//! This module provides a custom argument parser that matches the exact behavior
//! of the original bash script, supporting:
//! - `+toolchain` as the first argument
//! - Optional value arguments (e.g., `--crt-static`, `--crt-static=true`, `--crt-static true`)
//! - Combined short flags (e.g., `-vvv`)
//! - Short options with attached values (e.g., `-j4`, `-Ffoo`)
//! - Commands anywhere in the argument list
//! - `--` for passthrough arguments

use crate::config::{
    self, DEFAULT_CROSS_DEPS_VERSION, DEFAULT_FREEBSD_VERSION, DEFAULT_GLIBC_VERSION,
    DEFAULT_IPHONE_SDK_VERSION, DEFAULT_MACOS_SDK_VERSION, DEFAULT_NDK_VERSION,
    DEFAULT_QEMU_VERSION, SUPPORTED_FREEBSD_VERSIONS, SUPPORTED_GLIBC_VERSIONS,
    SUPPORTED_IPHONE_SDK_VERSIONS, SUPPORTED_MACOS_SDK_VERSIONS,
};
use crate::error::{CrossError, Result};
use std::path::PathBuf;

/// Supported cargo commands
const SUPPORTED_COMMANDS: &[&str] = &["b", "build", "check", "c", "run", "r", "test", "t", "bench"];

/// Cargo command to execute
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Command {
    #[default]
    Build,
    Check,
    Run,
    Test,
    Bench,
}

impl Command {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "b" | "build" => Some(Self::Build),
            "c" | "check" => Some(Self::Check),
            "r" | "run" => Some(Self::Run),
            "t" | "test" => Some(Self::Test),
            "bench" => Some(Self::Bench),
            _ => None,
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Check => "check",
            Self::Run => "run",
            Self::Test => "test",
            Self::Bench => "bench",
        }
    }

    /// Check if this command needs a runner (executes compiled binaries)
    pub const fn needs_runner(&self) -> bool {
        matches!(self, Self::Run | Self::Test | Self::Bench)
    }
}

/// Parsed command-line arguments
#[derive(Debug, Clone)]
pub struct Args {
    // Toolchain and command
    pub toolchain: Option<String>,
    pub command: Command,

    // Profile and features
    pub profile: String,
    pub features: Option<String>,
    pub no_default_features: bool,
    pub all_features: bool,

    // Target configuration
    pub targets: Vec<String>,
    pub use_default_linker: bool,
    pub no_cargo_target: bool,

    // Version options
    pub glibc_version: String,
    pub iphone_sdk_version: String,
    pub iphone_sdk_path: Option<PathBuf>,
    pub iphone_simulator_sdk_path: Option<PathBuf>,
    pub macos_sdk_version: String,
    pub macos_sdk_path: Option<PathBuf>,
    pub freebsd_version: String,
    pub ndk_version: String,
    pub qemu_version: String,
    pub cross_deps_version: String,

    // Directories
    pub cross_compiler_dir: PathBuf,
    pub cargo_target_dir: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,

    // Package and target selection
    pub package: Option<String>,
    pub workspace: bool,
    pub exclude: Option<String>,
    pub bin_target: Option<String>,
    pub build_bins: bool,
    pub build_lib: bool,
    pub example_target: Option<String>,
    pub build_examples: bool,
    pub test_target: Option<String>,
    pub build_tests: bool,
    pub bench_target: Option<String>,
    pub build_benches: bool,
    pub build_all_targets: bool,
    pub manifest_path: Option<PathBuf>,

    // Compiler options
    pub cc: Option<PathBuf>,
    pub cxx: Option<PathBuf>,
    pub ar: Option<PathBuf>,
    pub linker: Option<PathBuf>,
    pub cflags: Option<String>,
    pub cxxflags: Option<String>,
    pub cxxstdlib: Option<String>,
    pub rustflags: Vec<String>,
    pub rustc_wrapper: Option<PathBuf>,

    // Sccache options
    pub enable_sccache: bool,
    pub sccache_dir: Option<PathBuf>,
    pub sccache_cache_size: Option<String>,
    pub sccache_idle_timeout: Option<String>,
    pub sccache_log: Option<String>,
    pub sccache_no_daemon: bool,
    pub sccache_direct: bool,

    // CC crate options
    pub cc_no_defaults: bool,
    pub cc_shell_escaped_flags: bool,
    pub cc_enable_debug: bool,

    // Build options
    pub crt_static: Option<bool>,
    pub panic_immediate_abort: bool,
    pub fmt_debug: Option<String>,
    pub location_detail: Option<String>,
    pub build_std: Option<String>,
    pub build_std_features: Option<String>,
    pub cargo_trim_paths: Option<String>,
    pub no_embed_metadata: bool,
    pub rustc_bootstrap: Option<String>,

    // Output options
    pub verbose_level: u8,
    pub quiet: bool,
    pub message_format: Option<String>,
    pub color: Option<String>,
    pub build_plan: bool,
    pub timings: Option<String>,

    // Dependency options
    pub ignore_rust_version: bool,
    pub locked: bool,
    pub offline: bool,
    pub frozen: bool,
    pub lockfile_path: Option<PathBuf>,

    // Build configuration
    pub jobs: Option<String>,
    pub keep_going: bool,
    pub future_incompat_report: bool,

    // Additional cargo arguments
    pub cargo_args: Option<String>,
    pub cargo_z_flags: Vec<String>,
    pub cargo_config: Vec<String>,
    pub cargo_cwd: Option<PathBuf>,
    pub passthrough_args: Vec<String>,

    // Proxy
    pub github_proxy: Option<String>,

    // Misc
    pub clean_cache: bool,
}

/// Helper to get non-empty environment variable
fn get_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// Helper to parse bool from env var ("true" or "1" = true)
fn get_env_bool(name: &str) -> bool {
    get_env(name).map(|v| v == "true" || v == "1").unwrap_or(false)
}

impl Default for Args {
    fn default() -> Self {
        let temp_dir = std::env::temp_dir();
        let default_cross_compiler_dir = temp_dir.join("rust-cross-compiler");

        // Read from environment variables (for GitHub Action compatibility)
        let cross_compiler_dir = get_env("CROSS_COMPILER_DIR")
            .map(PathBuf::from)
            .unwrap_or(default_cross_compiler_dir);

        // Parse command from env
        let command = get_env("COMMAND")
            .and_then(|s| Command::parse(&s))
            .unwrap_or(Command::Build);

        // Parse targets from env (comma or newline separated)
        let mut targets = Vec::new();
        if let Some(env_targets) = get_env("TARGETS") {
            for target in env_targets.split([',', '\n']) {
                let target = target.trim();
                if !target.is_empty() {
                    targets.push(target.to_string());
                }
            }
        }

        // Parse rustflags from env
        let mut rustflags = Vec::new();
        if let Some(flags) = get_env("ADDITIONAL_RUSTFLAGS") {
            rustflags.push(flags);
        }

        // Parse passthrough args from env
        let mut passthrough_args = Vec::new();
        if let Some(args) = get_env("CARGO_PASSTHROUGH_ARGS") {
            // Format is "-- arg1 arg2 ..." - strip the "--" prefix
            let args = args.strip_prefix("--").unwrap_or(&args).trim();
            for arg in args.split_whitespace() {
                passthrough_args.push(arg.to_string());
            }
        }

        // Parse verbose level from env
        let verbose_level = get_env("VERBOSE_LEVEL")
            .and_then(|v| {
                if v == "true" {
                    Some(1)
                } else {
                    v.parse().ok()
                }
            })
            .unwrap_or(0);

        // Parse crt_static from env
        let crt_static = get_env("CRT_STATIC").and_then(|v| match v.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        });

        // Parse panic_immediate_abort from env
        let panic_immediate_abort = get_env("PANIC_IMMEDIATE_ABORT")
            .map(|v| v == "true")
            .unwrap_or(false);

        // Parse build_std from env
        let build_std = get_env("BUILD_STD").and_then(|v| {
            if v == "false" {
                None
            } else if v == "true" {
                Some("true".to_string())
            } else {
                Some(v)
            }
        });

        Self {
            toolchain: get_env("TOOLCHAIN"),
            command,
            profile: get_env("PROFILE").unwrap_or_else(|| "release".to_string()),
            features: get_env("FEATURES"),
            no_default_features: get_env_bool("NO_DEFAULT_FEATURES"),
            all_features: get_env_bool("ALL_FEATURES"),
            targets,
            use_default_linker: get_env_bool("USE_DEFAULT_LINKER"),
            no_cargo_target: false,
            glibc_version: get_env("GLIBC_VERSION")
                .unwrap_or_else(|| DEFAULT_GLIBC_VERSION.to_string()),
            iphone_sdk_version: get_env("IPHONE_SDK_VERSION")
                .unwrap_or_else(|| DEFAULT_IPHONE_SDK_VERSION.to_string()),
            iphone_sdk_path: get_env("IPHONE_SDK_PATH").map(PathBuf::from),
            iphone_simulator_sdk_path: get_env("IPHONE_SIMULATOR_SDK_PATH").map(PathBuf::from),
            macos_sdk_version: get_env("MACOS_SDK_VERSION")
                .unwrap_or_else(|| DEFAULT_MACOS_SDK_VERSION.to_string()),
            macos_sdk_path: get_env("MACOS_SDK_PATH").map(PathBuf::from),
            freebsd_version: get_env("FREEBSD_VERSION")
                .unwrap_or_else(|| DEFAULT_FREEBSD_VERSION.to_string()),
            ndk_version: get_env("NDK_VERSION").unwrap_or_else(|| DEFAULT_NDK_VERSION.to_string()),
            qemu_version: get_env("QEMU_VERSION")
                .unwrap_or_else(|| DEFAULT_QEMU_VERSION.to_string()),
            cross_deps_version: DEFAULT_CROSS_DEPS_VERSION.to_string(),
            cross_compiler_dir,
            cargo_target_dir: get_env("CARGO_TARGET_DIR").map(PathBuf::from),
            artifact_dir: get_env("ARTIFACT_DIR").map(PathBuf::from),
            package: get_env("PACKAGE"),
            workspace: get_env_bool("BUILD_WORKSPACE"),
            exclude: get_env("EXCLUDE"),
            bin_target: get_env("BIN_TARGET"),
            build_bins: get_env_bool("BUILD_BINS"),
            build_lib: get_env_bool("BUILD_LIB"),
            example_target: get_env("EXAMPLE_TARGET"),
            build_examples: get_env_bool("BUILD_EXAMPLES"),
            test_target: get_env("TEST_TARGET"),
            build_tests: get_env_bool("BUILD_TESTS"),
            bench_target: get_env("BENCH_TARGET"),
            build_benches: get_env_bool("BUILD_BENCHES"),
            build_all_targets: get_env_bool("BUILD_ALL_TARGETS"),
            manifest_path: get_env("MANIFEST_PATH").map(PathBuf::from),
            cc: get_env("CC").map(PathBuf::from),
            cxx: get_env("CXX").map(PathBuf::from),
            ar: get_env("AR").map(PathBuf::from),
            linker: get_env("LINKER").map(PathBuf::from),
            cflags: get_env("CFLAGS"),
            cxxflags: get_env("CXXFLAGS"),
            cxxstdlib: get_env("CXXSTDLIB"),
            rustflags,
            rustc_wrapper: get_env("RUSTC_WRAPPER").map(PathBuf::from),
            enable_sccache: get_env_bool("ENABLE_SCCACHE"),
            sccache_dir: get_env("SCCACHE_DIR").map(PathBuf::from),
            sccache_cache_size: get_env("SCCACHE_CACHE_SIZE"),
            sccache_idle_timeout: get_env("SCCACHE_IDLE_TIMEOUT"),
            sccache_log: get_env("SCCACHE_LOG"),
            sccache_no_daemon: get_env_bool("SCCACHE_NO_DAEMON"),
            sccache_direct: get_env_bool("SCCACHE_DIRECT"),
            cc_no_defaults: get_env_bool("CRATE_CC_NO_DEFAULTS"),
            cc_shell_escaped_flags: get_env_bool("CC_SHELL_ESCAPED_FLAGS"),
            cc_enable_debug: get_env_bool("CC_ENABLE_DEBUG_OUTPUT"),
            crt_static,
            panic_immediate_abort,
            fmt_debug: None,
            location_detail: None,
            build_std,
            build_std_features: get_env("BUILD_STD_FEATURES"),
            cargo_trim_paths: get_env("CARGO_TRIM_PATHS"),
            no_embed_metadata: get_env_bool("NO_EMBED_METADATA"),
            rustc_bootstrap: get_env("RUSTC_BOOTSTRAP"),
            verbose_level,
            quiet: get_env_bool("QUIET"),
            message_format: get_env("MESSAGE_FORMAT"),
            color: get_env("COLOR"),
            build_plan: get_env_bool("BUILD_PLAN"),
            timings: get_env("TIMINGS"),
            ignore_rust_version: get_env_bool("IGNORE_RUST_VERSION"),
            locked: get_env_bool("LOCKED"),
            offline: get_env_bool("OFFLINE"),
            frozen: get_env_bool("FROZEN"),
            lockfile_path: get_env("LOCKFILE_PATH").map(PathBuf::from),
            jobs: get_env("JOBS"),
            keep_going: get_env_bool("KEEP_GOING"),
            future_incompat_report: get_env_bool("FUTURE_INCOMPAT_REPORT"),
            cargo_args: get_env("CARGO_ARGS"),
            cargo_z_flags: Vec::new(),
            cargo_config: Vec::new(),
            cargo_cwd: get_env("CARGO_CWD").map(PathBuf::from),
            passthrough_args,
            github_proxy: get_env("GH_PROXY"),
            clean_cache: get_env_bool("CLEAN_CACHE"),
        }
    }
}

/// Argument parser state
struct ArgParser {
    args: Vec<String>,
    pos: usize,
    result: Args,
    command_found: bool,
}

impl ArgParser {
    fn new(args: Vec<String>) -> Self {
        Self {
            args,
            pos: 0,
            result: Args::default(),
            command_found: false,
        }
    }

    /// Current argument
    fn current(&self) -> Option<&str> {
        self.args.get(self.pos).map(std::string::String::as_str)
    }

    /// Peek at next argument
    fn peek(&self) -> Option<&str> {
        self.args.get(self.pos + 1).map(std::string::String::as_str)
    }

    /// Advance to next argument
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// Check if next argument looks like an option or command
    fn is_next_arg_option_or_command(&self) -> bool {
        match self.peek() {
            None => true,
            Some(next) => {
                // Check if it's a command (only if command not already found)
                if !self.command_found && SUPPORTED_COMMANDS.contains(&next) {
                    return true;
                }
                // Check if it starts with -
                next.starts_with('-')
            }
        }
    }

    /// Get the next argument value, consuming it
    fn take_value(&mut self, option: &str) -> Result<String> {
        self.advance();
        match self.current() {
            Some(v) if !v.starts_with('-') => Ok(v.to_string()),
            _ => Err(CrossError::MissingValue(option.to_string())),
        }
    }

    /// Get optional value - returns Some if next arg exists and doesn't look like option
    fn take_optional_value(&mut self, default: &str) -> String {
        if self.is_next_arg_option_or_command() {
            default.to_string()
        } else {
            self.advance();
            self.current().unwrap_or(default).to_string()
        }
    }

    /// Get optional boolean value
    fn take_optional_bool(&mut self, default: bool) -> bool {
        if self.is_next_arg_option_or_command() {
            default
        } else {
            self.advance();
            match self.current() {
                Some("true") => true,
                Some("false") => false,
                _ => default,
            }
        }
    }

    /// Parse a single argument
    fn parse_arg(&mut self) -> Result<bool> {
        let arg = match self.current() {
            Some(a) => a.to_string(),
            None => return Ok(false),
        };

        // Handle -- passthrough
        if arg == "--" {
            self.advance();
            while let Some(a) = self.current() {
                self.result.passthrough_args.push(a.to_string());
                self.advance();
            }
            return Ok(false);
        }

        // Handle -vvv style verbose flags
        if arg.starts_with('-') && !arg.starts_with("--") && arg.chars().skip(1).all(|c| c == 'v') {
            self.result.verbose_level += (arg.len() - 1) as u8;
            self.advance();
            return Ok(true);
        }

        // Check for command
        if !self.command_found {
            if let Some(cmd) = Command::parse(&arg) {
                self.result.command = cmd;
                self.command_found = true;
                self.advance();
                return Ok(true);
            }
        }

        // Parse options
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            "--show-all-targets" => {
                print_all_targets();
                std::process::exit(0);
            }

            // Profile
            s if s.starts_with("--profile=") => {
                self.result.profile = s.trim_start_matches("--profile=").to_string();
            }
            "--profile" => {
                self.result.profile = self.take_value("--profile")?;
            }

            // Release shorthand
            "-r" | "--release" => {
                self.result.profile = "release".to_string();
            }

            // Features
            s if s.starts_with("--features=") || s.starts_with("-F=") => {
                self.result.features = Some(s.split_once('=').unwrap().1.to_string());
            }
            s if s.starts_with("-F") && s.len() > 2 => {
                self.result.features = Some(s[2..].to_string());
            }
            "-F" | "--features" => {
                self.result.features = Some(self.take_value("--features")?);
            }
            "--no-default-features" => {
                self.result.no_default_features = true;
            }
            "--all-features" => {
                self.result.all_features = true;
            }

            // Target
            s if s.starts_with("--target=")
                || s.starts_with("--targets=")
                || s.starts_with("-t=") =>
            {
                let value = s.split_once('=').unwrap().1;
                self.add_targets(value);
            }
            s if s.starts_with("-t") && s.len() > 2 => {
                self.add_targets(&s[2..]);
            }
            "-t" | "--target" | "--targets" => {
                let value = self.take_value("--target")?;
                self.add_targets(&value);
            }

            // Cross compiler directory
            s if s.starts_with("--cross-compiler-dir=") => {
                self.result.cross_compiler_dir =
                    PathBuf::from(s.trim_start_matches("--cross-compiler-dir="));
            }
            "--cross-compiler-dir" => {
                self.result.cross_compiler_dir =
                    PathBuf::from(self.take_value("--cross-compiler-dir")?);
            }

            // GitHub proxy
            s if s.starts_with("--github-proxy-mirror=") => {
                self.result.github_proxy =
                    Some(s.trim_start_matches("--github-proxy-mirror=").to_string());
            }
            "--github-proxy-mirror" => {
                self.result.github_proxy = Some(self.take_value("--github-proxy-mirror")?);
            }

            // NDK version
            s if s.starts_with("--ndk-version=") => {
                self.result.ndk_version = s.trim_start_matches("--ndk-version=").to_string();
            }
            "--ndk-version" => {
                self.result.ndk_version = self.take_value("--ndk-version")?;
            }

            // Glibc version
            s if s.starts_with("--glibc-version=") => {
                self.result.glibc_version = s.trim_start_matches("--glibc-version=").to_string();
            }
            "--glibc-version" => {
                self.result.glibc_version = self.take_value("--glibc-version")?;
            }

            // iPhone SDK version
            s if s.starts_with("--iphone-sdk-version=") => {
                self.result.iphone_sdk_version =
                    s.trim_start_matches("--iphone-sdk-version=").to_string();
            }
            "--iphone-sdk-version" => {
                self.result.iphone_sdk_version = self.take_value("--iphone-sdk-version")?;
            }

            // iPhone SDK path
            s if s.starts_with("--iphone-sdk-path=") => {
                self.result.iphone_sdk_path =
                    Some(PathBuf::from(s.trim_start_matches("--iphone-sdk-path=")));
            }
            "--iphone-sdk-path" => {
                self.result.iphone_sdk_path =
                    Some(PathBuf::from(self.take_value("--iphone-sdk-path")?));
            }

            // iPhone simulator SDK path
            s if s.starts_with("--iphone-simulator-sdk-path=") => {
                self.result.iphone_simulator_sdk_path = Some(PathBuf::from(
                    s.trim_start_matches("--iphone-simulator-sdk-path="),
                ));
            }
            "--iphone-simulator-sdk-path" => {
                self.result.iphone_simulator_sdk_path = Some(PathBuf::from(
                    self.take_value("--iphone-simulator-sdk-path")?,
                ));
            }

            // macOS SDK version
            s if s.starts_with("--macos-sdk-version=") => {
                self.result.macos_sdk_version =
                    s.trim_start_matches("--macos-sdk-version=").to_string();
            }
            "--macos-sdk-version" => {
                self.result.macos_sdk_version = self.take_value("--macos-sdk-version")?;
            }

            // macOS SDK path
            s if s.starts_with("--macos-sdk-path=") => {
                self.result.macos_sdk_path =
                    Some(PathBuf::from(s.trim_start_matches("--macos-sdk-path=")));
            }
            "--macos-sdk-path" => {
                self.result.macos_sdk_path =
                    Some(PathBuf::from(self.take_value("--macos-sdk-path")?));
            }

            // FreeBSD version
            s if s.starts_with("--freebsd-version=") => {
                self.result.freebsd_version =
                    s.trim_start_matches("--freebsd-version=").to_string();
            }
            "--freebsd-version" => {
                self.result.freebsd_version = self.take_value("--freebsd-version")?;
            }

            // Package selection
            s if s.starts_with("--package=") || s.starts_with("-p=") => {
                self.result.package = Some(s.split_once('=').unwrap().1.to_string());
            }
            s if s.starts_with("-p") && s.len() > 2 => {
                self.result.package = Some(s[2..].to_string());
            }
            "-p" | "--package" => {
                self.result.package = Some(self.take_value("--package")?);
            }

            "--workspace" => {
                self.result.workspace = true;
            }

            s if s.starts_with("--exclude=") => {
                self.result.exclude = Some(s.trim_start_matches("--exclude=").to_string());
            }
            "--exclude" => {
                self.result.exclude = Some(self.take_value("--exclude")?);
            }

            // Binary targets
            s if s.starts_with("--bin=") => {
                self.result.bin_target = Some(s.trim_start_matches("--bin=").to_string());
            }
            "--bin" => {
                self.result.bin_target = Some(self.take_value("--bin")?);
            }
            "--bins" => {
                self.result.build_bins = true;
            }
            "--lib" => {
                self.result.build_lib = true;
            }

            // Example targets
            s if s.starts_with("--example=") => {
                self.result.example_target = Some(s.trim_start_matches("--example=").to_string());
            }
            "--example" => {
                self.result.example_target = Some(self.take_value("--example")?);
            }
            "--examples" => {
                self.result.build_examples = true;
            }

            // Test targets
            s if s.starts_with("--test=") => {
                self.result.test_target = Some(s.trim_start_matches("--test=").to_string());
            }
            "--test" => {
                self.result.test_target = Some(self.take_value("--test")?);
            }
            "--tests" => {
                self.result.build_tests = true;
            }

            // Bench targets
            s if s.starts_with("--bench=") => {
                self.result.bench_target = Some(s.trim_start_matches("--bench=").to_string());
            }
            "--bench" => {
                self.result.bench_target = Some(self.take_value("--bench")?);
            }
            "--benches" => {
                self.result.build_benches = true;
            }

            "--all-targets" => {
                self.result.build_all_targets = true;
            }

            // Manifest path
            s if s.starts_with("--manifest-path=") => {
                self.result.manifest_path =
                    Some(PathBuf::from(s.trim_start_matches("--manifest-path=")));
            }
            "--manifest-path" => {
                self.result.manifest_path =
                    Some(PathBuf::from(self.take_value("--manifest-path")?));
            }

            // Use default linker
            "--use-default-linker" => {
                self.result.use_default_linker = true;
            }

            // Compiler paths
            s if s.starts_with("--cc=") => {
                self.result.cc = Some(PathBuf::from(s.trim_start_matches("--cc=")));
            }
            "--cc" => {
                self.result.cc = Some(PathBuf::from(self.take_value("--cc")?));
            }
            s if s.starts_with("--cxx=") => {
                self.result.cxx = Some(PathBuf::from(s.trim_start_matches("--cxx=")));
            }
            "--cxx" => {
                self.result.cxx = Some(PathBuf::from(self.take_value("--cxx")?));
            }
            s if s.starts_with("--ar=") => {
                self.result.ar = Some(PathBuf::from(s.trim_start_matches("--ar=")));
            }
            "--ar" => {
                self.result.ar = Some(PathBuf::from(self.take_value("--ar")?));
            }
            s if s.starts_with("--linker=") => {
                self.result.linker = Some(PathBuf::from(s.trim_start_matches("--linker=")));
            }
            "--linker" => {
                self.result.linker = Some(PathBuf::from(self.take_value("--linker")?));
            }

            // Compiler flags
            s if s.starts_with("--cflags=") => {
                self.result.cflags = Some(s.trim_start_matches("--cflags=").to_string());
            }
            "--cflags" => {
                self.result.cflags = Some(self.take_value("--cflags")?);
            }
            s if s.starts_with("--cxxflags=") => {
                self.result.cxxflags = Some(s.trim_start_matches("--cxxflags=").to_string());
            }
            "--cxxflags" => {
                self.result.cxxflags = Some(self.take_value("--cxxflags")?);
            }
            s if s.starts_with("--cxxstdlib=") => {
                self.result.cxxstdlib = Some(s.trim_start_matches("--cxxstdlib=").to_string());
            }
            "--cxxstdlib" => {
                self.result.cxxstdlib = Some(self.take_value("--cxxstdlib")?);
            }

            // Rustflags (can be specified multiple times)
            s if s.starts_with("--rustflags=") => {
                self.result
                    .rustflags
                    .push(s.trim_start_matches("--rustflags=").to_string());
            }
            "--rustflags" => {
                let value = self.take_value("--rustflags")?;
                self.result.rustflags.push(value);
            }

            // Rustc wrapper
            s if s.starts_with("--rustc-wrapper=") => {
                self.result.rustc_wrapper =
                    Some(PathBuf::from(s.trim_start_matches("--rustc-wrapper=")));
            }
            "--rustc-wrapper" => {
                self.result.rustc_wrapper =
                    Some(PathBuf::from(self.take_value("--rustc-wrapper")?));
            }

            // Sccache options
            "--enable-sccache" => {
                self.result.enable_sccache = true;
            }
            s if s.starts_with("--sccache-dir=") => {
                self.result.sccache_dir =
                    Some(PathBuf::from(s.trim_start_matches("--sccache-dir=")));
            }
            "--sccache-dir" => {
                self.result.sccache_dir = Some(PathBuf::from(self.take_value("--sccache-dir")?));
            }
            s if s.starts_with("--sccache-cache-size=") => {
                self.result.sccache_cache_size =
                    Some(s.trim_start_matches("--sccache-cache-size=").to_string());
            }
            "--sccache-cache-size" => {
                self.result.sccache_cache_size = Some(self.take_value("--sccache-cache-size")?);
            }
            s if s.starts_with("--sccache-idle-timeout=") => {
                self.result.sccache_idle_timeout =
                    Some(s.trim_start_matches("--sccache-idle-timeout=").to_string());
            }
            "--sccache-idle-timeout" => {
                self.result.sccache_idle_timeout = Some(self.take_value("--sccache-idle-timeout")?);
            }
            s if s.starts_with("--sccache-log=") => {
                self.result.sccache_log = Some(s.trim_start_matches("--sccache-log=").to_string());
            }
            "--sccache-log" => {
                self.result.sccache_log = Some(self.take_value("--sccache-log")?);
            }
            "--sccache-no-daemon" => {
                self.result.sccache_no_daemon = true;
            }
            "--sccache-direct" => {
                self.result.sccache_direct = true;
            }

            // CC crate options
            "--cc-no-defaults" => {
                self.result.cc_no_defaults = true;
            }
            "--cc-shell-escaped-flags" => {
                self.result.cc_shell_escaped_flags = true;
            }
            "--cc-enable-debug" => {
                self.result.cc_enable_debug = true;
            }

            // CRT static (optional value)
            s if s.starts_with("--crt-static=") || s.starts_with("--static-crt=") => {
                let value = s.split_once('=').unwrap().1;
                self.result.crt_static = Some(value != "false");
            }
            "--crt-static" | "--static-crt" => {
                self.result.crt_static = Some(self.take_optional_bool(true));
            }

            // Panic immediate abort
            "--panic-immediate-abort" => {
                self.result.panic_immediate_abort = true;
            }

            // Fmt debug
            s if s.starts_with("--fmt-debug=") => {
                self.result.fmt_debug = Some(s.trim_start_matches("--fmt-debug=").to_string());
            }
            "--fmt-debug" => {
                self.result.fmt_debug = Some(self.take_value("--fmt-debug")?);
            }

            // Location detail
            s if s.starts_with("--location-detail=") => {
                self.result.location_detail =
                    Some(s.trim_start_matches("--location-detail=").to_string());
            }
            "--location-detail" => {
                self.result.location_detail = Some(self.take_value("--location-detail")?);
            }

            // Build std (optional value)
            s if s.starts_with("--build-std=") => {
                let value = s.trim_start_matches("--build-std=");
                self.result.build_std = Some(if value.is_empty() {
                    "true".to_string()
                } else {
                    value.to_string()
                });
            }
            "--build-std" => {
                self.result.build_std = Some(self.take_optional_value("true"));
            }

            // Build std features
            s if s.starts_with("--build-std-features=") => {
                self.result.build_std_features =
                    Some(s.trim_start_matches("--build-std-features=").to_string());
            }
            "--build-std-features" => {
                self.result.build_std_features = Some(self.take_value("--build-std-features")?);
            }

            // Cargo args
            s if s.starts_with("--cargo-args=") || s.starts_with("--args=") => {
                self.result.cargo_args = Some(s.split_once('=').unwrap().1.to_string());
            }
            "--cargo-args" | "--args" => {
                self.result.cargo_args = Some(self.take_value("--cargo-args")?);
            }

            // Toolchain
            s if s.starts_with("--toolchain=") => {
                self.result.toolchain = Some(s.trim_start_matches("--toolchain=").to_string());
            }
            "--toolchain" => {
                self.result.toolchain = Some(self.take_value("--toolchain")?);
            }

            // Cargo trim paths (optional value)
            s if s.starts_with("--cargo-trim-paths=") || s.starts_with("--trim-paths=") => {
                self.result.cargo_trim_paths = Some(s.split_once('=').unwrap().1.to_string());
            }
            "--cargo-trim-paths" | "--trim-paths" => {
                self.result.cargo_trim_paths = Some(self.take_optional_value("true"));
            }

            // No embed metadata
            "--no-embed-metadata" => {
                self.result.no_embed_metadata = true;
            }

            // RUSTC_BOOTSTRAP (optional value)
            s if s.starts_with("--rustc-bootstrap=") => {
                let value = s.trim_start_matches("--rustc-bootstrap=");
                self.result.rustc_bootstrap = Some(if value.is_empty() {
                    "1".to_string()
                } else {
                    value.to_string()
                });
            }
            "--rustc-bootstrap" => {
                self.result.rustc_bootstrap = Some(self.take_optional_value("1"));
            }

            // Target dir
            s if s.starts_with("--target-dir=") => {
                self.result.cargo_target_dir =
                    Some(PathBuf::from(s.trim_start_matches("--target-dir=")));
            }
            "--target-dir" => {
                self.result.cargo_target_dir =
                    Some(PathBuf::from(self.take_value("--target-dir")?));
            }

            // Artifact dir
            s if s.starts_with("--artifact-dir=") => {
                self.result.artifact_dir =
                    Some(PathBuf::from(s.trim_start_matches("--artifact-dir=")));
            }
            "--artifact-dir" => {
                self.result.artifact_dir = Some(PathBuf::from(self.take_value("--artifact-dir")?));
            }

            // Color
            s if s.starts_with("--color=") => {
                self.result.color = Some(s.trim_start_matches("--color=").to_string());
            }
            "--color" => {
                self.result.color = Some(self.take_value("--color")?);
            }

            // Build plan
            "--build-plan" => {
                self.result.build_plan = true;
            }

            // Timings (optional value)
            s if s.starts_with("--timings=") => {
                self.result.timings = Some(s.trim_start_matches("--timings=").to_string());
            }
            "--timings" => {
                self.result.timings = Some(self.take_optional_value("true"));
            }

            // Lockfile path
            s if s.starts_with("--lockfile-path=") => {
                self.result.lockfile_path =
                    Some(PathBuf::from(s.trim_start_matches("--lockfile-path=")));
            }
            "--lockfile-path" => {
                self.result.lockfile_path =
                    Some(PathBuf::from(self.take_value("--lockfile-path")?));
            }

            // Config (can be specified multiple times)
            s if s.starts_with("--config=") => {
                self.result
                    .cargo_config
                    .push(s.trim_start_matches("--config=").to_string());
            }
            "--config" => {
                let value = self.take_value("--config")?;
                self.result.cargo_config.push(value);
            }

            // -C (cargo working directory)
            s if s.starts_with("-C=") => {
                self.result.cargo_cwd = Some(PathBuf::from(s.trim_start_matches("-C=")));
            }
            s if s.starts_with("-C") && s.len() > 2 => {
                self.result.cargo_cwd = Some(PathBuf::from(&s[2..]));
            }
            "-C" => {
                self.result.cargo_cwd = Some(PathBuf::from(self.take_value("-C")?));
            }

            // -Z flags (can be specified multiple times)
            s if s.starts_with("-Z=") => {
                self.result
                    .cargo_z_flags
                    .push(s.trim_start_matches("-Z=").to_string());
            }
            s if s.starts_with("-Z") && s.len() > 2 => {
                self.result.cargo_z_flags.push(s[2..].to_string());
            }
            "-Z" => {
                let value = self.take_value("-Z")?;
                self.result.cargo_z_flags.push(value);
            }

            // Output options
            "-q" | "--quiet" => {
                self.result.quiet = true;
            }
            "--verbose" => {
                self.result.verbose_level += 1;
            }

            // Message format
            s if s.starts_with("--message-format=") => {
                self.result.message_format =
                    Some(s.trim_start_matches("--message-format=").to_string());
            }
            "--message-format" => {
                self.result.message_format = Some(self.take_value("--message-format")?);
            }

            // Dependency options
            "--ignore-rust-version" => {
                self.result.ignore_rust_version = true;
            }
            "--locked" => {
                self.result.locked = true;
            }
            "--offline" => {
                self.result.offline = true;
            }
            "--frozen" => {
                self.result.frozen = true;
            }

            // Jobs
            s if s.starts_with("--jobs=") || s.starts_with("-j=") => {
                self.result.jobs = Some(s.split_once('=').unwrap().1.to_string());
            }
            s if s.starts_with("-j") && s.len() > 2 => {
                self.result.jobs = Some(s[2..].to_string());
            }
            "-j" | "--jobs" => {
                self.result.jobs = Some(self.take_value("--jobs")?);
            }

            // Keep going
            "--keep-going" => {
                self.result.keep_going = true;
            }

            // Future incompat report
            "--future-incompat-report" => {
                self.result.future_incompat_report = true;
            }

            // Clean cache
            "--clean-cache" => {
                self.result.clean_cache = true;
            }

            // Unknown option
            s if s.starts_with('-') => {
                return Err(CrossError::UnknownOption(s.to_string()));
            }

            // Unknown positional argument
            _ => {
                return Err(CrossError::InvalidArgument(arg));
            }
        }

        self.advance();
        Ok(true)
    }

    /// Add targets from a comma-separated string
    fn add_targets(&mut self, value: &str) {
        for target in value.split(',') {
            let target = target.trim();
            if !target.is_empty() {
                // Expand patterns
                let expanded = config::expand_targets(target);
                if expanded.is_empty() {
                    // Keep original if not a known pattern (might be a custom target)
                    self.result.targets.push(target.to_string());
                } else {
                    for t in expanded {
                        if !self.result.targets.contains(&t.to_string()) {
                            self.result.targets.push(t.to_string());
                        }
                    }
                }
            }
        }
    }

    /// Parse all arguments
    fn parse(mut self) -> Result<Args> {
        while self.pos < self.args.len() {
            if !self.parse_arg()? {
                break;
            }
        }
        Ok(self.result)
    }
}

/// Parse command-line arguments
pub fn parse_args() -> Result<Args> {
    let args: Vec<String> = std::env::args().collect();
    parse_args_from(args)
}

/// Parse arguments from a vector (for testing)
pub fn parse_args_from(args: Vec<String>) -> Result<Args> {
    let mut args = args;

    // Skip program name
    if !args.is_empty() {
        args.remove(0);
    }

    // When invoked as `cargo cross`, skip the "cross" argument
    if !args.is_empty()
        && std::env::var("CARGO").is_ok()
        && std::env::var("CARGO_HOME").is_ok()
        && args.first().map(|s| s.as_str()) == Some("cross")
    {
        args.remove(0);
    }
    // Handle +toolchain as first argument
    let mut result = Args::default();
    if let Some(first) = args.first() {
        if let Some(toolchain) = first.strip_prefix('+') {
            result.toolchain = Some(toolchain.to_string());
            args.remove(0);
        }
    }

    // Parse remaining arguments
    let mut parser = ArgParser::new(args);
    parser.result = result;
    let mut args = parser.parse()?;

    // Validate versions
    validate_versions(&args)?;

    // Handle empty targets - default to host
    if args.targets.is_empty() {
        let host = config::HostPlatform::detect();
        args.targets.push(host.triple);
        args.use_default_linker = true;
        args.no_cargo_target = true;
    }

    // Handle RELEASE environment variable
    if std::env::var("RELEASE")
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        args.profile = "release".to_string();
    }

    Ok(args)
}

/// Validate version options
fn validate_versions(args: &Args) -> Result<()> {
    // Validate glibc version
    if !SUPPORTED_GLIBC_VERSIONS.contains(&args.glibc_version.as_str()) {
        return Err(CrossError::UnsupportedGlibcVersion {
            version: args.glibc_version.clone(),
            supported: SUPPORTED_GLIBC_VERSIONS.join(", "),
        });
    }

    // Validate iPhone SDK version (only for non-macOS cross-compilation)
    let host = config::HostPlatform::detect();
    if !host.is_darwin()
        && !SUPPORTED_IPHONE_SDK_VERSIONS.contains(&args.iphone_sdk_version.as_str())
    {
        return Err(CrossError::UnsupportedIphoneSdkVersion {
            version: args.iphone_sdk_version.clone(),
            supported: SUPPORTED_IPHONE_SDK_VERSIONS.join(", "),
        });
    }

    // Validate macOS SDK version (only for non-macOS cross-compilation)
    if !host.is_darwin() && !SUPPORTED_MACOS_SDK_VERSIONS.contains(&args.macos_sdk_version.as_str())
    {
        return Err(CrossError::UnsupportedMacosSdkVersion {
            version: args.macos_sdk_version.clone(),
            supported: SUPPORTED_MACOS_SDK_VERSIONS.join(", "),
        });
    }

    // Validate FreeBSD version
    if !SUPPORTED_FREEBSD_VERSIONS.contains(&args.freebsd_version.as_str()) {
        return Err(CrossError::UnsupportedFreebsdVersion {
            version: args.freebsd_version.clone(),
            supported: SUPPORTED_FREEBSD_VERSIONS.join(", "),
        });
    }

    Ok(())
}

/// Print help message
fn print_help() {
    use colored::Colorize;

    println!(
        "{} {}",
        "Usage:".bright_green(),
        "[+toolchain] [OPTIONS] [COMMAND]".bright_cyan()
    );
    println!();
    println!("{}", "Commands:".bright_green());
    println!(
        "  {}, {}    Compile the package (default)",
        "b".bright_cyan(),
        "build".bright_cyan()
    );
    println!(
        "  {}, {}    Analyze the package and report errors",
        "c".bright_cyan(),
        "check".bright_cyan()
    );
    println!(
        "  {}, {}      Run a binary or example of the package",
        "r".bright_cyan(),
        "run".bright_cyan()
    );
    println!(
        "  {}, {}     Run the tests",
        "t".bright_cyan(),
        "test".bright_cyan()
    );
    println!("  {}       Run the benchmarks", "bench".bright_cyan());
    println!();
    println!("{}", "Options:".bright_green());
    println!(
        "      {} {}               Set the build profile (debug/release)",
        "--profile".bright_cyan(),
        "<PROFILE>".bright_cyan()
    );
    println!(
        "      {} {}        Specify the cross compiler directory",
        "--cross-compiler-dir".bright_cyan(),
        "<DIR>".bright_cyan()
    );
    println!(
        "  {}, {} {}             Space or comma separated list of features",
        "-F".bright_cyan(),
        "--features".bright_cyan(),
        "<FEATURES>".bright_cyan()
    );
    println!(
        "      {}             Do not activate default features",
        "--no-default-features".bright_cyan()
    );
    println!(
        "      {}                    Activate all available features",
        "--all-features".bright_cyan()
    );
    println!(
        "  {}, {} {}                 Rust target triple(s)",
        "-t".bright_cyan(),
        "--target".bright_cyan(),
        "<TRIPLE>".bright_cyan()
    );
    println!(
        "      {}                Display all supported target triples",
        "--show-all-targets".bright_cyan()
    );
    println!(
        "      {} {}       Use a GitHub proxy mirror",
        "--github-proxy-mirror".bright_cyan(),
        "<URL>".bright_cyan()
    );
    println!(
        "      {} {}           Specify the Android NDK version",
        "--ndk-version".bright_cyan(),
        "<VERSION>".bright_cyan()
    );
    println!(
        "      {} {}         Specify glibc version for gnu targets (default: {})",
        "--glibc-version".bright_cyan(),
        "<VERSION>".bright_cyan(),
        DEFAULT_GLIBC_VERSION
    );
    println!(
        "      {} {}    Specify iPhone SDK version (default: {})",
        "--iphone-sdk-version".bright_cyan(),
        "<VERSION>".bright_cyan(),
        DEFAULT_IPHONE_SDK_VERSION
    );
    println!(
        "      {} {}     Specify macOS SDK version (default: {})",
        "--macos-sdk-version".bright_cyan(),
        "<VERSION>".bright_cyan(),
        DEFAULT_MACOS_SDK_VERSION
    );
    println!(
        "      {} {}      Specify FreeBSD version (default: {})",
        "--freebsd-version".bright_cyan(),
        "<VERSION>".bright_cyan(),
        DEFAULT_FREEBSD_VERSION
    );
    println!(
        "  {}, {} {}                  Package to build",
        "-p".bright_cyan(),
        "--package".bright_cyan(),
        "<SPEC>".bright_cyan()
    );
    println!(
        "      {}                       Build all workspace members",
        "--workspace".bright_cyan()
    );
    println!(
        "      {} {}                  Exclude packages from the build",
        "--exclude".bright_cyan(),
        "<SPEC>".bright_cyan()
    );
    println!(
        "      {} {}                      Binary target to build",
        "--bin".bright_cyan(),
        "<NAME>".bright_cyan()
    );
    println!(
        "      {}                            Build all binary targets",
        "--bins".bright_cyan()
    );
    println!(
        "      {}                             Build only the library target",
        "--lib".bright_cyan()
    );
    println!(
        "  {}, {}                         Build optimized artifacts with the release profile",
        "-r".bright_cyan(),
        "--release".bright_cyan()
    );
    println!(
        "  {}, {}                           Do not print cargo log messages",
        "-q".bright_cyan(),
        "--quiet".bright_cyan()
    );
    println!(
        "      {}              Use system default linker",
        "--use-default-linker".bright_cyan()
    );
    println!(
        "      {} {}               Additional rustflags",
        "--rustflags".bright_cyan(),
        "<FLAGS>".bright_cyan()
    );
    println!(
        "      {}{}       Add -C target-feature=+crt-static",
        "--crt-static".bright_cyan(),
        "[=<true|false>]".bright_cyan()
    );
    println!(
        "      {}           Enable panic=immediate-abort",
        "--panic-immediate-abort".bright_cyan()
    );
    println!(
        "      {}{}            Use -Zbuild-std",
        "--build-std".bright_cyan(),
        "[=<CRATES>]".bright_cyan()
    );
    println!(
        "      {} {}               Additional arguments to pass to cargo",
        "--cargo-args".bright_cyan(),
        "<ARGS>".bright_cyan()
    );
    println!(
        "      {} {}           Rust toolchain to use",
        "--toolchain".bright_cyan(),
        "<TOOLCHAIN>".bright_cyan()
    );
    println!(
        "  {} {}                              Change current working directory",
        "-C".bright_cyan(),
        "<DIR>".bright_cyan()
    );
    println!(
        "  {} {}                             Unstable (nightly-only) flags to Cargo",
        "-Z".bright_cyan(),
        "<FLAG>".bright_cyan()
    );
    println!(
        "  {}, {}                         Use verbose output",
        "-v".bright_cyan(),
        "--verbose".bright_cyan()
    );
    println!(
        "  {}, {}                            Display this help message",
        "-h".bright_cyan(),
        "--help".bright_cyan()
    );
}

/// Print all supported targets
fn print_all_targets() {
    use colored::Colorize;

    println!("{}", "Supported Rust targets:".bright_green());
    let mut targets: Vec<_> = config::all_targets().collect();
    targets.sort_unstable();
    for target in targets {
        println!("  {}", target.bright_cyan());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty() {
        let args = parse_args_from(vec!["cargo-cross".to_string()]).unwrap();
        assert_eq!(args.command, Command::Build);
        assert_eq!(args.profile, "release");
    }

    #[test]
    fn test_parse_toolchain() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "+nightly".to_string()]).unwrap();
        assert_eq!(args.toolchain, Some("nightly".to_string()));
    }

    #[test]
    fn test_parse_command() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "check".to_string()]).unwrap();
        assert_eq!(args.command, Command::Check);
    }

    #[test]
    fn test_parse_target() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "-t".to_string(),
            "x86_64-unknown-linux-musl".to_string(),
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_parse_target_short() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "-tx86_64-unknown-linux-musl".to_string(),
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_parse_verbose() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "-vvv".to_string()]).unwrap();
        assert_eq!(args.verbose_level, 3);
    }

    #[test]
    fn test_parse_crt_static_flag() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--crt-static".to_string()]).unwrap();
        assert_eq!(args.crt_static, Some(true));
    }

    #[test]
    fn test_parse_crt_static_value() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--crt-static=false".to_string(),
        ])
        .unwrap();
        assert_eq!(args.crt_static, Some(false));
    }

    #[test]
    fn test_parse_build_std() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--build-std".to_string()]).unwrap();
        assert_eq!(args.build_std, Some("true".to_string()));
    }

    #[test]
    fn test_parse_build_std_value() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--build-std=core,alloc".to_string(),
        ])
        .unwrap();
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
    }

    #[test]
    fn test_parse_jobs() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "-j4".to_string()]).unwrap();
        assert_eq!(args.jobs, Some("4".to_string()));
    }

    #[test]
    fn test_parse_passthrough() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--".to_string(),
            "--test-arg".to_string(),
            "value".to_string(),
        ])
        .unwrap();
        assert_eq!(args.passthrough_args, vec!["--test-arg", "value"]);
    }

    #[test]
    fn test_parse_z_flag() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "-Zbuild-std".to_string()]).unwrap();
        assert_eq!(args.cargo_z_flags, vec!["build-std"]);
    }

    #[test]
    fn test_parse_features() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "-F".to_string(),
            "foo,bar".to_string(),
        ])
        .unwrap();
        assert_eq!(args.features, Some("foo,bar".to_string()));
    }

    #[test]
    fn test_parse_features_long() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--features=foo,bar".to_string(),
        ])
        .unwrap();
        assert_eq!(args.features, Some("foo,bar".to_string()));
    }

    #[test]
    fn test_parse_all_features() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--all-features".to_string(),
        ])
        .unwrap();
        assert!(args.all_features);
    }

    #[test]
    fn test_parse_no_default_features() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--no-default-features".to_string(),
        ])
        .unwrap();
        assert!(args.no_default_features);
    }

    #[test]
    fn test_parse_profile() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--profile".to_string(),
            "dev".to_string(),
        ])
        .unwrap();
        assert_eq!(args.profile, "dev");
    }

    #[test]
    fn test_parse_package() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "-p".to_string(),
            "my-package".to_string(),
        ])
        .unwrap();
        assert_eq!(args.package, Some("my-package".to_string()));
    }

    #[test]
    fn test_parse_workspace() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--workspace".to_string()]).unwrap();
        assert!(args.workspace);
    }

    #[test]
    fn test_parse_locked() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--locked".to_string()]).unwrap();
        assert!(args.locked);
    }

    #[test]
    fn test_parse_offline() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--offline".to_string()]).unwrap();
        assert!(args.offline);
    }

    #[test]
    fn test_parse_frozen() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--frozen".to_string()]).unwrap();
        assert!(args.frozen);
    }

    #[test]
    fn test_parse_multiple_targets() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "-t".to_string(),
            "x86_64-unknown-linux-musl".to_string(),
            "-t".to_string(),
            "aarch64-unknown-linux-musl".to_string(),
        ])
        .unwrap();
        assert_eq!(
            args.targets,
            vec!["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
        );
    }

    #[test]
    fn test_parse_target_with_equals() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--target=x86_64-unknown-linux-musl".to_string(),
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_parse_bin() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--bin".to_string(),
            "my-bin".to_string(),
        ])
        .unwrap();
        assert_eq!(args.bin_target, Some("my-bin".to_string()));
    }

    #[test]
    fn test_parse_bins() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "--bins".to_string()]).unwrap();
        assert!(args.build_bins);
    }

    #[test]
    fn test_parse_lib() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "--lib".to_string()]).unwrap();
        assert!(args.build_lib);
    }

    #[test]
    fn test_parse_all_targets() {
        let args =
            parse_args_from(vec!["cargo-cross".to_string(), "--all-targets".to_string()]).unwrap();
        assert!(args.build_all_targets);
    }

    #[test]
    fn test_parse_test_command() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "test".to_string()]).unwrap();
        assert_eq!(args.command, Command::Test);
    }

    #[test]
    fn test_parse_run_command() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "run".to_string()]).unwrap();
        assert_eq!(args.command, Command::Run);
    }

    #[test]
    fn test_parse_bench_command() {
        let args = parse_args_from(vec!["cargo-cross".to_string(), "bench".to_string()]).unwrap();
        assert_eq!(args.command, Command::Bench);
    }

    #[test]
    fn test_command_needs_runner() {
        assert!(!Command::Build.needs_runner());
        assert!(!Command::Check.needs_runner());
        assert!(Command::Run.needs_runner());
        assert!(Command::Test.needs_runner());
        assert!(Command::Bench.needs_runner());
    }

    #[test]
    fn test_command_as_str() {
        assert_eq!(Command::Build.as_str(), "build");
        assert_eq!(Command::Check.as_str(), "check");
        assert_eq!(Command::Run.as_str(), "run");
        assert_eq!(Command::Test.as_str(), "test");
        assert_eq!(Command::Bench.as_str(), "bench");
    }

    #[test]
    fn test_command_parse() {
        assert_eq!(Command::parse("b"), Some(Command::Build));
        assert_eq!(Command::parse("build"), Some(Command::Build));
        assert_eq!(Command::parse("c"), Some(Command::Check));
        assert_eq!(Command::parse("check"), Some(Command::Check));
        assert_eq!(Command::parse("r"), Some(Command::Run));
        assert_eq!(Command::parse("run"), Some(Command::Run));
        assert_eq!(Command::parse("t"), Some(Command::Test));
        assert_eq!(Command::parse("test"), Some(Command::Test));
        assert_eq!(Command::parse("bench"), Some(Command::Bench));
        assert_eq!(Command::parse("invalid"), None);
    }

    #[test]
    fn test_parse_sccache() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--enable-sccache".to_string(),
        ])
        .unwrap();
        assert!(args.enable_sccache);
    }

    #[test]
    fn test_parse_glibc_version() {
        let args = parse_args_from(vec![
            "cargo-cross".to_string(),
            "--glibc-version=2.31".to_string(),
        ])
        .unwrap();
        assert_eq!(args.glibc_version, "2.31");
    }
}
