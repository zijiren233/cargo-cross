//! Command-line argument parsing for cargo-cross using clap

use crate::config::{
    self, DEFAULT_CROSS_DEPS_VERSION, DEFAULT_FREEBSD_VERSION, DEFAULT_GLIBC_VERSION,
    DEFAULT_IPHONE_SDK_VERSION, DEFAULT_MACOS_SDK_VERSION, DEFAULT_NDK_VERSION,
    DEFAULT_QEMU_VERSION, SUPPORTED_FREEBSD_VERSIONS, SUPPORTED_GLIBC_VERSIONS,
    SUPPORTED_IPHONE_SDK_VERSIONS, SUPPORTED_MACOS_SDK_VERSIONS,
};
use crate::error::{CrossError, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Args as ClapArgs, Parser, Subcommand, ValueHint};
use std::path::PathBuf;

// ============================================================================
// CLI Styles
// ============================================================================

/// Custom styles for CLI help output
fn cli_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::BrightCyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::BrightCyan.on_default() | Effects::BOLD)
        .literal(AnsiColor::BrightGreen.on_default())
        .placeholder(AnsiColor::BrightMagenta.on_default())
        .valid(AnsiColor::BrightGreen.on_default())
        .invalid(AnsiColor::BrightRed.on_default())
        .error(AnsiColor::BrightRed.on_default() | Effects::BOLD)
}

// ============================================================================
// CLI Structure
// ============================================================================

/// Cross-compilation tool for Rust projects
#[derive(Parser, Debug)]
#[command(name = "cargo-cross", version)]
#[command(about = "Cross-compilation tool for Rust projects, no Docker required")]
#[command(long_about = "\
Cross-compilation tool for Rust projects.

This tool provides cross-compilation support for Rust projects across multiple
platforms including Linux (musl/gnu), Windows, macOS, FreeBSD, iOS, and Android.
It automatically downloads and configures the appropriate cross-compiler toolchains.")]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
#[command(styles = cli_styles())]
#[command(override_usage = "cargo-cross [+toolchain] <COMMAND> [OPTIONS]")]
#[command(after_help = "\
Use 'cargo-cross <COMMAND> --help' for more information about a command.

TOOLCHAIN:
    If the first argument begins with +, it will be interpreted as a Rust toolchain
    name (such as +nightly, +stable, or +1.75.0). This follows the same convention
    as rustup and cargo.

EXAMPLES:
    cargo-cross build -t x86_64-unknown-linux-musl
    cargo-cross +nightly build -t aarch64-unknown-linux-gnu --profile release
    cargo-cross build -t '*-linux-musl' --crt-static true
    cargo-cross test -t x86_64-unknown-linux-musl -- --nocapture")]
pub struct Cli {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Compile the current package
    #[command(visible_alias = "b")]
    #[command(long_about = "\
Compile the current package and all of its dependencies.

When no target selection options are given, cargo-cross will build all binary
and library targets of the selected packages.")]
    #[command(
        override_usage = "cargo-cross [+toolchain] build [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
    )]
    Build(BuildArgs),

    /// Analyze the current package and report errors, but don't build object files
    #[command(visible_alias = "c")]
    #[command(long_about = "\
Check the current package and all of its dependencies for errors.

This will essentially compile packages without performing the final step of
code generation, which is faster than running build.")]
    #[command(
        override_usage = "cargo-cross [+toolchain] check [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
    )]
    Check(BuildArgs),

    /// Run a binary or example of the current package
    #[command(visible_alias = "r")]
    #[command(long_about = "\
Run a binary or example of the local package.

For cross-compilation targets, QEMU user-mode emulation is used to run the binary.")]
    #[command(
        override_usage = "cargo-cross [+toolchain] run [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
    )]
    Run(BuildArgs),

    /// Run the tests
    #[command(visible_alias = "t")]
    #[command(long_about = "\
Execute all unit and integration tests and build examples of a local package.

For cross-compilation targets, QEMU user-mode emulation is used to run tests.")]
    #[command(
        override_usage = "cargo-cross [+toolchain] test [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
    )]
    Test(BuildArgs),

    /// Run the benchmarks
    #[command(long_about = "\
Execute all benchmarks of a local package.

For cross-compilation targets, QEMU user-mode emulation is used to run benchmarks.")]
    #[command(
        override_usage = "cargo-cross [+toolchain] bench [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
    )]
    Bench(BuildArgs),

    /// Display all supported cross-compilation targets
    #[command(long_about = "\
Display all supported cross-compilation targets.

You can also use glob patterns with --target to match multiple targets,
for example: --target '*-linux-musl' or --target 'aarch64-*'")]
    Targets(TargetsArgs),

    /// Print version information
    Version,
}

// ============================================================================
// Targets Arguments
// ============================================================================

/// Output format for targets command
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable colored text (default)
    #[default]
    Text,
    /// JSON array format
    Json,
    /// Plain text, one target per line
    Plain,
}

#[derive(ClapArgs, Debug, Clone, Default)]
pub struct TargetsArgs {
    /// Output format
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value = "text",
        help = "Output format (text, json, plain)"
    )]
    pub format: OutputFormat,
}

// ============================================================================
// Build Arguments
// ============================================================================

#[derive(ClapArgs, Debug, Clone, Default)]
#[command(next_help_heading = "Target Selection")]
pub struct BuildArgs {
    // ===== Target Selection =====
    /// Build for the target triple(s)
    #[arg(
        short = 't',
        long = "target",
        visible_alias = "targets",
        value_delimiter = ',',
        env = "TARGETS",
        value_name = "TRIPLE",
        help = "Build for the target triple(s), comma-separated",
        long_help = "\
Build for the specified target architecture. This flag may be specified multiple
times or with comma-separated values. Supports glob patterns like '*-linux-musl'.

The general format of the triple is <arch><sub>-<vendor>-<sys>-<abi>.

Examples:
  -t x86_64-unknown-linux-musl
  -t aarch64-unknown-linux-gnu,armv7-unknown-linux-gnueabihf
  -t '*-linux-musl'

Use 'cargo-cross targets' to see all supported targets."
    )]
    pub targets: Vec<String>,

    // ===== Feature Selection =====
    /// Space or comma separated list of features to activate
    #[arg(
        short = 'F',
        long,
        env = "FEATURES",
        value_name = "FEATURES",
        conflicts_with = "all_features",
        help_heading = "Feature Selection",
        long_help = "\
Space or comma separated list of features to activate.

Features of workspace members may be enabled with package-name/feature-name syntax.
This flag may be specified multiple times, which enables all specified features."
    )]
    pub features: Option<String>,

    /// Do not activate the `default` feature of the selected packages
    #[arg(long, env = "NO_DEFAULT_FEATURES", help_heading = "Feature Selection")]
    pub no_default_features: bool,

    /// Activate all available features of all selected packages
    #[arg(
        long,
        env = "ALL_FEATURES",
        conflicts_with = "features",
        help_heading = "Feature Selection"
    )]
    pub all_features: bool,

    // ===== Profile =====
    /// Build artifacts in release mode, with optimizations
    #[arg(
        short = 'r',
        long = "release",
        env = "RELEASE",
        conflicts_with = "profile",
        help_heading = "Profile",
        long_help = "\
Build artifacts in release mode, with optimizations.

This is equivalent to --profile=release.
This flag is provided for compatibility with cargo's -r/--release option."
    )]
    pub release: bool,

    /// Build artifacts with the specified profile
    #[arg(
        long,
        default_value = "release",
        env = "PROFILE",
        value_name = "PROFILE-NAME",
        conflicts_with = "release",
        help_heading = "Profile",
        long_help = "\
Build artifacts with the specified profile.

Built-in profiles: dev, release, test, bench.
Custom profiles can be defined in Cargo.toml.
Default is 'release' for cross-compilation (differs from cargo's default of 'dev')."
    )]
    pub profile: String,

    // ===== Package Selection =====
    /// Package to build (see `cargo help pkgid`)
    #[arg(
        short = 'p',
        long,
        env = "PACKAGE",
        value_name = "SPEC",
        help_heading = "Package Selection",
        long_help = "\
Build only the specified packages. This flag may be specified multiple times
and supports common Unix glob patterns like *, ?, and []."
    )]
    pub package: Option<String>,

    /// Build all members in the workspace
    #[arg(
        long,
        visible_alias = "all",
        env = "BUILD_WORKSPACE",
        help_heading = "Package Selection"
    )]
    pub workspace: bool,

    /// Exclude packages from the build (must be used with --workspace)
    #[arg(
        long,
        env = "EXCLUDE",
        value_name = "SPEC",
        requires = "workspace",
        help_heading = "Package Selection",
        long_help = "\
Exclude the specified packages. Must be used in conjunction with the --workspace flag.
This flag may be specified multiple times and supports common Unix glob patterns."
    )]
    pub exclude: Option<String>,

    /// Build only the specified binary
    #[arg(
        long = "bin",
        env = "BIN_TARGET",
        value_name = "NAME",
        help_heading = "Package Selection",
        long_help = "\
Build the specified binary. This flag may be specified multiple times
and supports common Unix glob patterns."
    )]
    pub bin_target: Option<String>,

    /// Build all binary targets
    #[arg(long = "bins", env = "BUILD_BINS", help_heading = "Package Selection")]
    pub build_bins: bool,

    /// Build only this package's library
    #[arg(long = "lib", env = "BUILD_LIB", help_heading = "Package Selection")]
    pub build_lib: bool,

    /// Build only the specified example
    #[arg(
        long = "example",
        env = "EXAMPLE_TARGET",
        value_name = "NAME",
        help_heading = "Package Selection",
        long_help = "\
Build the specified example. This flag may be specified multiple times
and supports common Unix glob patterns."
    )]
    pub example_target: Option<String>,

    /// Build all example targets
    #[arg(
        long = "examples",
        env = "BUILD_EXAMPLES",
        help_heading = "Package Selection"
    )]
    pub build_examples: bool,

    /// Build only the specified test target
    #[arg(
        long = "test",
        env = "TEST_TARGET",
        value_name = "NAME",
        help_heading = "Package Selection",
        long_help = "\
Build the specified integration test. This flag may be specified multiple times
and supports common Unix glob patterns."
    )]
    pub test_target: Option<String>,

    /// Build all test targets (includes unit tests from lib/bins)
    #[arg(
        long = "tests",
        env = "BUILD_TESTS",
        help_heading = "Package Selection",
        long_help = "\
Build all targets that have the test = true manifest flag set. By default this
includes the library and binaries built as unittests, and integration tests."
    )]
    pub build_tests: bool,

    /// Build only the specified bench target
    #[arg(
        long = "bench",
        env = "BENCH_TARGET",
        value_name = "NAME",
        help_heading = "Package Selection",
        long_help = "\
Build the specified benchmark. This flag may be specified multiple times
and supports common Unix glob patterns."
    )]
    pub bench_target: Option<String>,

    /// Build all bench targets
    #[arg(
        long = "benches",
        env = "BUILD_BENCHES",
        help_heading = "Package Selection",
        long_help = "\
Build all targets that have the bench = true manifest flag set. By default this
includes the library and binaries built as benchmarks, and bench targets."
    )]
    pub build_benches: bool,

    /// Build all targets (equivalent to --lib --bins --tests --benches --examples)
    #[arg(
        long = "all-targets",
        env = "BUILD_ALL_TARGETS",
        help_heading = "Package Selection"
    )]
    pub build_all_targets: bool,

    /// Path to Cargo.toml
    #[arg(long, env = "MANIFEST_PATH", value_name = "PATH",
          value_hint = ValueHint::FilePath, help_heading = "Package Selection",
          long_help = "\
Path to Cargo.toml. By default, Cargo searches for the Cargo.toml file
in the current directory or any parent directory.")]
    pub manifest_path: Option<PathBuf>,

    // ===== Version Options =====
    /// Glibc version for Linux GNU targets
    #[arg(long, default_value = DEFAULT_GLIBC_VERSION, env = "GLIBC_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify glibc version for GNU libc targets.

Supported versions: 2.28, 2.31, 2.32, 2.33, 2.34, 2.35, 2.36, 2.37, 2.38, 2.39, 2.40, 2.41, 2.42

The glibc version determines the minimum Linux kernel version required to run the built binary.
Lower versions provide better compatibility with older systems.")]
    pub glibc_version: String,

    /// iPhone SDK version for iOS targets
    #[arg(long, default_value = DEFAULT_IPHONE_SDK_VERSION, env = "IPHONE_SDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify iPhone SDK version for iOS targets.

On Linux (cross-compilation): Uses pre-built SDK from releases.
On macOS (native): Uses installed Xcode SDK (warns if version not found).

Supported versions on Linux: 17.0, 17.2, 17.4, 17.5, 18.0, 18.1, 18.2, 18.4, 18.5, 26.0, 26.1, 26.2")]
    pub iphone_sdk_version: String,

    /// Override iPhoneOS SDK path (skips version lookup)
    #[arg(long, env = "IPHONE_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override iPhoneOS SDK path for device targets (skips version lookup).

Use this option to specify a custom SDK location instead of the version-based lookup.")]
    pub iphone_sdk_path: Option<PathBuf>,

    /// Override iPhoneSimulator SDK path
    #[arg(long, env = "IPHONE_SIMULATOR_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override iPhoneSimulator SDK path for simulator targets.

Use this option to specify a custom SDK location for iOS simulator builds.")]
    pub iphone_simulator_sdk_path: Option<PathBuf>,

    /// macOS SDK version for Darwin targets
    #[arg(long, default_value = DEFAULT_MACOS_SDK_VERSION, env = "MACOS_SDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify macOS SDK version for Darwin targets.

On Linux (cross-compilation): Uses osxcross with pre-built SDK from releases.
On macOS (native): Uses installed Xcode SDK (warns if version not found).

Supported versions on Linux: 14.0, 14.2, 14.4, 14.5, 15.0, 15.1, 15.2, 15.4, 15.5, 26.0, 26.1, 26.2")]
    pub macos_sdk_version: String,

    /// Override macOS SDK path (skips version lookup)
    #[arg(long, env = "MACOS_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override macOS SDK path directly (skips version lookup).

Use this option to specify a custom SDK location instead of the version-based lookup.")]
    pub macos_sdk_path: Option<PathBuf>,

    /// FreeBSD version for FreeBSD targets
    #[arg(long, default_value = DEFAULT_FREEBSD_VERSION, env = "FREEBSD_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify FreeBSD version for FreeBSD targets.

Supported versions: 13, 14")]
    pub freebsd_version: String,

    /// Android NDK version
    #[arg(long, default_value = DEFAULT_NDK_VERSION, env = "NDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify Android NDK version for Android targets.

The NDK will be automatically downloaded from Google's official repository.")]
    pub ndk_version: String,

    /// QEMU version for user-mode emulation
    #[arg(long, default_value = DEFAULT_QEMU_VERSION, env = "QEMU_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify QEMU version for user-mode emulation.

QEMU is used to run cross-compiled binaries during test/run/bench commands.")]
    pub qemu_version: String,

    // ===== Directories =====
    /// Directory for cross-compiler toolchains
    #[arg(long, env = "CROSS_COMPILER_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Specify the directory where cross-compiler toolchains will be downloaded and stored.

Defaults to a temporary directory. Set this to reuse downloaded toolchains across builds.")]
    pub cross_compiler_dir: Option<PathBuf>,

    /// Directory for all generated artifacts
    #[arg(long, visible_alias = "target-dir", env = "CARGO_TARGET_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Directory for all generated artifacts and intermediate files.

Defaults to 'target' in the root of the workspace.")]
    pub cargo_target_dir: Option<PathBuf>,

    /// Copy final artifacts to this directory (unstable)
    #[arg(long, env = "ARTIFACT_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Copy final artifacts to this directory.

This option is unstable and requires the nightly toolchain.")]
    pub artifact_dir: Option<PathBuf>,

    // ===== Compiler Options =====
    /// Override C compiler path
    #[arg(long, env = "CC", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the C compiler path.

By default, cargo-cross automatically configures the appropriate cross-compiler
for the target. Use this option to specify a custom C compiler.")]
    pub cc: Option<PathBuf>,

    /// Override C++ compiler path
    #[arg(long, env = "CXX", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the C++ compiler path.

By default, cargo-cross automatically configures the appropriate cross-compiler
for the target. Use this option to specify a custom C++ compiler.")]
    pub cxx: Option<PathBuf>,

    /// Override archiver (ar) path
    #[arg(long, env = "AR", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the archiver (ar) path.

By default, cargo-cross automatically configures the appropriate archiver
for the target. Use this option to specify a custom archiver.")]
    pub ar: Option<PathBuf>,

    /// Override linker path
    #[arg(long, env = "LINKER", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath,
          conflicts_with = "use_default_linker", help_heading = "Compiler Options",
          long_help = "\
Override the linker path.

By default, cargo-cross uses the cross-compiler as the linker.
Use this option to specify a custom linker (e.g., lld, mold).")]
    pub linker: Option<PathBuf>,

    /// Additional flags for C compilation
    #[arg(
        long,
        env = "CFLAGS",
        value_name = "FLAGS",
        allow_hyphen_values = true,
        help_heading = "Compiler Options",
        long_help = "\
Additional flags to pass to the C compiler.

These flags are appended to the default CFLAGS for the target.
Example: --cflags '-O2 -Wall -march=native'"
    )]
    pub cflags: Option<String>,

    /// Additional flags for C++ compilation
    #[arg(
        long,
        env = "CXXFLAGS",
        value_name = "FLAGS",
        allow_hyphen_values = true,
        help_heading = "Compiler Options",
        long_help = "\
Additional flags to pass to the C++ compiler.

These flags are appended to the default CXXFLAGS for the target.
Example: --cxxflags '-O2 -Wall -std=c++17'"
    )]
    pub cxxflags: Option<String>,

    /// Additional flags for linking
    #[arg(
        long,
        env = "LDFLAGS",
        value_name = "FLAGS",
        allow_hyphen_values = true,
        help_heading = "Compiler Options",
        long_help = "\
Additional flags to pass to the linker.

These flags are appended to the default LDFLAGS for the target.
Example: --ldflags '-L/usr/local/lib -static'"
    )]
    pub ldflags: Option<String>,

    /// C++ standard library to use
    #[arg(
        long,
        env = "CXXSTDLIB",
        value_name = "LIB",
        help_heading = "Compiler Options",
        long_help = "\
Specify the C++ standard library to use.

Common values: libc++, libstdc++
This affects which C++ standard library implementation is linked."
    )]
    pub cxxstdlib: Option<String>,

    /// Additional RUSTFLAGS (can be repeated)
    #[arg(long = "rustflag", visible_alias = "rustflags", value_name = "FLAG",
          env = "ADDITIONAL_RUSTFLAGS", allow_hyphen_values = true,
          action = clap::ArgAction::Append, help_heading = "Compiler Options",
          long_help = "\
Additional flags to pass to rustc via RUSTFLAGS.

This option can be specified multiple times.
Example: --rustflag '-C target-cpu=native' --rustflag '-C lto=thin'")]
    pub rustflags: Vec<String>,

    /// Rustc wrapper program (e.g., sccache, cachepot)
    #[arg(long, env = "RUSTC_WRAPPER", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath,
          conflicts_with = "enable_sccache", help_heading = "Compiler Options",
          long_help = "\
Specify a rustc wrapper program.

The wrapper will be invoked instead of rustc directly.
Common wrappers include sccache and cachepot for compilation caching.")]
    pub rustc_wrapper: Option<PathBuf>,

    /// Use the default system linker instead of cross-compiler
    #[arg(
        long,
        env = "USE_DEFAULT_LINKER",
        conflicts_with = "linker",
        help_heading = "Compiler Options",
        long_help = "\
Use the default system linker instead of the cross-compiler linker.

This is useful when building for the host target or when you have
a custom linker setup."
    )]
    pub use_default_linker: bool,

    // ===== Sccache Options =====
    /// Enable sccache for compilation caching
    #[arg(
        long,
        env = "ENABLE_SCCACHE",
        conflicts_with = "rustc_wrapper",
        help_heading = "Sccache Options",
        long_help = "\
Enable sccache as the rustc wrapper for compilation caching.

sccache is a compiler caching tool that speeds up compilation by caching
previous compilations and detecting when the same compilation is being done again."
    )]
    pub enable_sccache: bool,

    /// Directory for sccache local disk cache
    #[arg(long, env = "SCCACHE_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Sccache Options",
          long_help = "\
Specify the directory for sccache's local disk cache.

Defaults to $HOME/.cache/sccache on Linux/macOS.")]
    pub sccache_dir: Option<PathBuf>,

    /// Maximum cache size (e.g., '10G', '500M')
    #[arg(
        long,
        env = "SCCACHE_CACHE_SIZE",
        value_name = "SIZE",
        help_heading = "Sccache Options",
        long_help = "\
Maximum size of the local disk cache.

Accepts values like '10G' (10 gigabytes), '500M' (500 megabytes).
Default is 10GB."
    )]
    pub sccache_cache_size: Option<String>,

    /// Idle timeout in seconds for sccache server
    #[arg(
        long,
        env = "SCCACHE_IDLE_TIMEOUT",
        value_name = "SECONDS",
        help_heading = "Sccache Options",
        long_help = "\
Idle timeout in seconds for the sccache server.

The server will shut down after being idle for this duration.
Set to 0 to run indefinitely."
    )]
    pub sccache_idle_timeout: Option<String>,

    /// Log level for sccache (error, warn, info, debug, trace)
    #[arg(
        long,
        env = "SCCACHE_LOG",
        value_name = "LEVEL",
        help_heading = "Sccache Options",
        long_help = "\
Set the log level for sccache.

Valid values: error, warn, info, debug, trace"
    )]
    pub sccache_log: Option<String>,

    /// Run sccache without the daemon (single process mode)
    #[arg(
        long,
        env = "SCCACHE_NO_DAEMON",
        help_heading = "Sccache Options",
        long_help = "\
Run sccache without the background daemon.

This runs sccache in single-process mode, which may be slower but
avoids daemon startup issues in some environments."
    )]
    pub sccache_no_daemon: bool,

    /// Enable sccache direct mode (bypass preprocessor)
    #[arg(
        long,
        env = "SCCACHE_DIRECT",
        help_heading = "Sccache Options",
        long_help = "\
Enable sccache direct mode.

Direct mode caches based on source file content directly,
bypassing the preprocessor for potentially faster cache lookups."
    )]
    pub sccache_direct: bool,

    // ===== CC Crate Options =====
    /// Disable CC crate default compiler flags
    #[arg(
        long,
        env = "CRATE_CC_NO_DEFAULTS",
        hide = true,
        help_heading = "CC Crate Options"
    )]
    pub cc_no_defaults: bool,

    /// Use shell-escaped flags for CC crate
    #[arg(
        long,
        env = "CC_SHELL_ESCAPED_FLAGS",
        hide = true,
        help_heading = "CC Crate Options"
    )]
    pub cc_shell_escaped_flags: bool,

    /// Enable CC crate debug output
    #[arg(
        long,
        env = "CC_ENABLE_DEBUG_OUTPUT",
        hide = true,
        help_heading = "CC Crate Options"
    )]
    pub cc_enable_debug: bool,

    // ===== Build Options =====
    /// Link the C runtime statically
    #[arg(long, value_parser = parse_optional_bool, env = "CRT_STATIC",
          value_name = "BOOL", num_args = 1, help_heading = "Build Options",
          long_help = "\
Control whether the C runtime is statically linked.

  --crt-static true   Link C runtime statically (larger binary, more portable)
  --crt-static false  Link C runtime dynamically (smaller binary, requires libc)

For musl targets, static linking is the default.
For glibc targets, dynamic linking is the default.")]
    pub crt_static: Option<bool>,

    /// Abort immediately on panic (smaller binary, implies --build-std)
    #[arg(
        long,
        env = "PANIC_IMMEDIATE_ABORT",
        help_heading = "Build Options",
        long_help = "\
Use panic=abort and remove panic formatting code.

This produces smaller binaries by eliminating panic message formatting.
Requires the nightly toolchain and implies --build-std.

Note: Stack traces and panic messages will not be available."
    )]
    pub panic_immediate_abort: bool,

    /// Debug formatting mode (full, shallow, none) - requires nightly
    #[arg(long, value_name = "MODE", hide = true, help_heading = "Build Options")]
    pub fmt_debug: Option<String>,

    /// Location detail mode - requires nightly
    #[arg(long, value_name = "MODE", hide = true, help_heading = "Build Options")]
    pub location_detail: Option<String>,

    /// Build the standard library from source
    #[arg(long, value_parser = parse_build_std, env = "BUILD_STD",
          value_name = "CRATES", help_heading = "Build Options",
          long_help = "\
Build the standard library from source (requires nightly).

  --build-std true          Build std, core, alloc, and proc_macro
  --build-std core,alloc    Build only specified crates

This is required for targets not supported by pre-built std,
or when using panic=abort or other std-modifying options.")]
    pub build_std: Option<String>,

    /// Features to enable when building std
    #[arg(
        long,
        env = "BUILD_STD_FEATURES",
        value_name = "FEATURES",
        requires = "build_std",
        help_heading = "Build Options",
        long_help = "\
Space-separated list of features to enable for the standard library.

Example: --build-std core,alloc --build-std-features panic_immediate_abort

Common features:
  panic_immediate_abort  - Abort without formatting panic messages
  optimize_for_size      - Optimize std for binary size"
    )]
    pub build_std_features: Option<String>,

    /// Trim paths in compiler output for reproducible builds
    #[arg(
        long,
        visible_alias = "trim-paths",
        env = "CARGO_TRIM_PATHS",
        value_name = "VALUE",
        help_heading = "Build Options",
        long_help = "\
Control how paths are trimmed in compiler output.

This helps with reproducible builds by removing local path prefixes.

Valid values: true, macro, diagnostics, object, all, none
Default: false (no trimming)"
    )]
    pub cargo_trim_paths: Option<String>,

    /// Disable metadata embedding (requires nightly)
    #[arg(
        long,
        env = "NO_EMBED_METADATA",
        hide = true,
        help_heading = "Build Options"
    )]
    pub no_embed_metadata: bool,

    /// Set RUSTC_BOOTSTRAP for using nightly features on stable
    #[arg(
        long,
        env = "RUSTC_BOOTSTRAP",
        value_name = "VALUE",
        hide = true,
        help_heading = "Build Options"
    )]
    pub rustc_bootstrap: Option<String>,

    // ===== Output Options =====
    /// Use verbose output (-v, -vv for very verbose)
    #[arg(short = 'v', long = "verbose", action = clap::ArgAction::Count,
          env = "VERBOSE_LEVEL", conflicts_with = "quiet",
          help_heading = "Output Options",
          long_help = "\
Use verbose output. May be specified twice for 'very verbose' output.

  -v   Show compilation commands and warnings
  -vv  Show dependency warnings and build script output
  -vvv Maximum verbosity")]
    pub verbose_level: u8,

    /// Do not print cargo log messages
    #[arg(
        short = 'q',
        long,
        env = "QUIET",
        conflicts_with = "verbose_level",
        help_heading = "Output Options",
        long_help = "\
Do not print cargo log messages.

This silences cargo's informational output, showing only errors and warnings."
    )]
    pub quiet: bool,

    /// Diagnostic message format
    #[arg(
        long,
        env = "MESSAGE_FORMAT",
        value_name = "FMT",
        help_heading = "Output Options",
        long_help = "\
The output format for diagnostic messages.

Valid values:
  human (default)  - Human-readable text format
  short            - Shorter, human-readable text messages
  json             - Emit JSON messages to stdout"
    )]
    pub message_format: Option<String>,

    /// Control when colored output is used
    #[arg(
        long,
        env = "COLOR",
        value_name = "WHEN",
        help_heading = "Output Options",
        long_help = "\
Control when colored output is used.

Valid values:
  auto (default)  - Automatically detect if color support is available
  always          - Always display colors
  never           - Never display colors"
    )]
    pub color: Option<String>,

    /// Output the build plan in JSON (requires nightly)
    #[arg(long, env = "BUILD_PLAN", hide = true, help_heading = "Output Options")]
    pub build_plan: bool,

    /// Timing output formats (html, json)
    #[arg(long, env = "TIMINGS", value_name = "FMTS",
          num_args = 0..=1, default_missing_value = "true",
          help_heading = "Output Options",
          long_help = "\
Output information about how long each compilation takes.

  --timings        Output timing report as HTML (default)
  --timings=json   Output machine-readable JSON (requires -Z unstable-options)

The HTML report is saved to target/cargo-timings/.")]
    pub timings: Option<String>,

    // ===== Dependency Options =====
    /// Ignore `rust-version` specification in packages
    #[arg(
        long,
        env = "IGNORE_RUST_VERSION",
        help_heading = "Dependency Options",
        long_help = "\
Ignore rust-version specification in packages.

This allows building with a Rust version older than what the package specifies."
    )]
    pub ignore_rust_version: bool,

    /// Assert that Cargo.lock will remain unchanged
    #[arg(
        long,
        env = "LOCKED",
        help_heading = "Dependency Options",
        long_help = "\
Assert that the exact same dependencies and versions are used as when
the existing Cargo.lock file was originally generated.

Cargo will exit with an error if Cargo.lock is missing or needs updating.
Use in CI pipelines for deterministic builds."
    )]
    pub locked: bool,

    /// Run without accessing the network
    #[arg(
        long,
        env = "OFFLINE",
        help_heading = "Dependency Options",
        long_help = "\
Prevents Cargo from accessing the network for any reason.

Cargo will attempt to proceed with locally cached data.
May result in different dependency resolution than online mode.

Use 'cargo fetch' to download dependencies before going offline."
    )]
    pub offline: bool,

    /// Require Cargo.lock and cache are up to date (implies --locked --offline)
    #[arg(
        long,
        env = "FROZEN",
        help_heading = "Dependency Options",
        long_help = "\
Equivalent to specifying both --locked and --offline.

Requires that both Cargo.lock and the dependency cache are up to date."
    )]
    pub frozen: bool,

    /// Path to Cargo.lock (unstable)
    #[arg(long, env = "LOCKFILE_PATH", value_name = "PATH",
          value_hint = ValueHint::FilePath, help_heading = "Dependency Options",
          long_help = "\
Changes the path of the lockfile from the default (<workspace_root>/Cargo.lock).

This option requires the nightly toolchain.")]
    pub lockfile_path: Option<PathBuf>,

    // ===== Build Configuration =====
    /// Number of parallel jobs to run
    #[arg(
        short = 'j',
        long,
        env = "JOBS",
        value_name = "N",
        help_heading = "Build Configuration",
        long_help = "\
Number of parallel jobs to run.

Defaults to the number of logical CPUs.
If negative, sets max jobs to (logical CPUs + N).
Use 'default' to reset to the default value."
    )]
    pub jobs: Option<String>,

    /// Build as many crates as possible, rather than aborting on first error
    #[arg(
        long,
        env = "KEEP_GOING",
        help_heading = "Build Configuration",
        long_help = "\
Build as many crates in the dependency graph as possible.

Rather than aborting the build on the first crate that fails to build,
cargo-cross will continue building other crates in the dependency graph."
    )]
    pub keep_going: bool,

    /// Output a future incompatibility report after the build
    #[arg(
        long,
        env = "FUTURE_INCOMPAT_REPORT",
        help_heading = "Build Configuration",
        long_help = "\
Displays a future-incompat report for any future-incompatible warnings
produced during execution of this command.

See 'cargo report' for more information."
    )]
    pub future_incompat_report: bool,

    // ===== Additional Cargo Arguments =====
    /// Additional arguments to pass to cargo
    #[arg(
        long,
        visible_alias = "args",
        env = "CARGO_ARGS",
        value_name = "ARGS",
        hide = true,
        allow_hyphen_values = true,
        help_heading = "Additional Options"
    )]
    pub cargo_args: Option<String>,

    /// Unstable (nightly-only) flags to Cargo
    #[arg(short = 'Z', value_name = "FLAG",
          action = clap::ArgAction::Append, help_heading = "Additional Options",
          long_help = "\
Unstable (nightly-only) flags to Cargo.

Run 'cargo -Z help' for details on available flags.
Common flags: build-std, unstable-options")]
    pub cargo_z_flags: Vec<String>,

    /// Override a Cargo configuration value
    #[arg(long = "config", value_name = "KEY=VALUE",
          action = clap::ArgAction::Append, help_heading = "Additional Options",
          long_help = "\
Override a Cargo configuration value.

The argument should be in TOML syntax of KEY=VALUE.
This flag may be specified multiple times.

Example: --config 'build.jobs=4' --config 'profile.release.lto=true'")]
    pub cargo_config: Vec<String>,

    /// Change to directory before doing anything
    #[arg(short = 'C', long = "directory", env = "CARGO_CWD",
          value_name = "DIR", value_hint = ValueHint::DirPath,
          help_heading = "Additional Options",
          long_help = "\
Changes the current working directory before executing any specified operations.

This affects where cargo looks for the project manifest (Cargo.toml),
as well as the directories searched for .cargo/config.toml.")]
    pub cargo_cwd: Option<PathBuf>,

    /// Rust toolchain to use (alternative to +toolchain syntax)
    #[arg(
        long = "toolchain",
        env = "TOOLCHAIN",
        value_name = "TOOLCHAIN",
        help_heading = "Additional Options",
        long_help = "\
Specify the Rust toolchain to use for compilation.

This is an alternative to the +toolchain syntax (e.g., +nightly).
Examples: --toolchain nightly, --toolchain stable, --toolchain 1.75.0"
    )]
    pub toolchain_option: Option<String>,

    /// GitHub mirror URL for downloading toolchains
    #[arg(long, visible_alias = "github-proxy-mirror", env = "GH_PROXY", value_name = "URL",
          value_hint = ValueHint::Url, hide_env = true,
          help_heading = "Additional Options",
          long_help = "\
Specify a GitHub mirror/proxy URL for downloading cross-compiler toolchains.

Useful in regions where GitHub access is slow or restricted.
Example: --github-proxy 'https://ghproxy.com/'")]
    pub github_proxy: Option<String>,

    /// Clean the target directory before building
    #[arg(
        long,
        env = "CLEAN_CACHE",
        help_heading = "Additional Options",
        long_help = "\
Clean the target directory before building.

Equivalent to running 'cargo clean' before the build."
    )]
    pub clean_cache: bool,

    /// Arguments passed through to cargo (after --)
    #[arg(
        last = true,
        allow_hyphen_values = true,
        value_name = "ARGS",
        help = "Arguments passed through to the underlying cargo command",
        long_help = "\
Arguments passed through to the underlying cargo command.

Everything after -- is passed directly to cargo/test runner.
For test command, these are passed to the test binary.

Examples:
  cargo-cross test -- --nocapture --test-threads=1
  cargo-cross run -- --arg1 --arg2"
    )]
    pub passthrough_args: Vec<String>,
}

// ============================================================================
// BuildArgs impl
// ============================================================================

impl BuildArgs {
    /// Create default BuildArgs with proper version defaults
    pub fn default_for_host() -> Self {
        Self {
            profile: "release".to_string(),
            glibc_version: DEFAULT_GLIBC_VERSION.to_string(),
            iphone_sdk_version: DEFAULT_IPHONE_SDK_VERSION.to_string(),
            macos_sdk_version: DEFAULT_MACOS_SDK_VERSION.to_string(),
            freebsd_version: DEFAULT_FREEBSD_VERSION.to_string(),
            ndk_version: DEFAULT_NDK_VERSION.to_string(),
            qemu_version: DEFAULT_QEMU_VERSION.to_string(),
            ..Default::default()
        }
    }
}

// ============================================================================
// Custom Parsers
// ============================================================================

/// Parse optional bool value (true/false)
fn parse_optional_bool(s: &str) -> std::result::Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("invalid bool value: {s}")),
    }
}

/// Parse build-std value
fn parse_build_std(s: &str) -> std::result::Result<String, String> {
    if s == "false" || s == "0" {
        Err("build-std disabled".to_string())
    } else if s == "true" || s == "1" {
        Ok("true".to_string())
    } else {
        Ok(s.to_string())
    }
}

// ============================================================================
// Command Enum (for internal use)
// ============================================================================

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
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Build => "build",
            Self::Check => "check",
            Self::Run => "run",
            Self::Test => "test",
            Self::Bench => "bench",
        }
    }

    pub const fn needs_runner(&self) -> bool {
        matches!(self, Self::Run | Self::Test | Self::Bench)
    }
}

// ============================================================================
// Args (converted from BuildArgs)
// ============================================================================

/// Parsed and validated arguments
#[derive(Debug, Clone)]
pub struct Args {
    /// Rust toolchain to use (e.g., "nightly", "stable")
    pub toolchain: Option<String>,
    /// Cargo command to execute
    pub command: Command,
    /// Expanded target list (after glob pattern expansion)
    pub targets: Vec<String>,
    /// Skip passing --target to cargo (for host builds)
    pub no_cargo_target: bool,
    /// Cross-deps version for toolchain downloads
    pub cross_deps_version: String,
    /// Directory for cross-compiler toolchains
    pub cross_compiler_dir: PathBuf,
    /// All build arguments from CLI
    pub build: BuildArgs,
}

impl std::ops::Deref for Args {
    type Target = BuildArgs;

    fn deref(&self) -> &Self::Target {
        &self.build
    }
}

impl std::ops::DerefMut for Args {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.build
    }
}

impl Args {
    /// Create Args from BuildArgs and Command
    fn from_build_args(b: BuildArgs, command: Command, toolchain: Option<String>) -> Self {
        let cross_compiler_dir = b
            .cross_compiler_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("rust-cross-compiler"));
        let targets = expand_target_list(&b.targets);

        Self {
            toolchain,
            command,
            targets,
            no_cargo_target: false,
            cross_deps_version: DEFAULT_CROSS_DEPS_VERSION.to_string(),
            cross_compiler_dir,
            build: b,
        }
    }
}

// ============================================================================
// Parse Result
// ============================================================================

/// Result of parsing CLI arguments
pub enum ParseResult {
    /// Normal build/check/run/test/bench command
    Build(Box<Args>),
    /// Show targets command
    ShowTargets(OutputFormat),
    /// Show version
    ShowVersion,
}

// ============================================================================
// Environment Sanitization
// ============================================================================

/// Remove empty environment variables that clap would incorrectly treat as having values.
/// Clap's `env = "VAR"` attribute treats empty strings as valid values, which causes
/// parsing errors for PathBuf and other types that don't accept empty strings.
fn sanitize_clap_env() {
    // Remove all empty environment variables - this is safe because empty string
    // env vars are almost never meaningful and would cause clap parsing errors
    let empty_vars: Vec<_> = std::env::vars()
        .filter(|(_, v)| v.is_empty())
        .map(|(k, _)| k)
        .collect();

    for var in empty_vars {
        std::env::remove_var(&var);
    }
}

// ============================================================================
// Entry Point
// ============================================================================

/// Parse command-line arguments
pub fn parse_args() -> Result<ParseResult> {
    let args: Vec<String> = std::env::args().collect();
    parse_args_from(args)
}

/// Parse arguments from a vector (for testing)
pub fn parse_args_from(args: Vec<String>) -> Result<ParseResult> {
    use std::env;

    // Remove empty environment variables that clap would incorrectly treat as having values
    // This must be done before clap parses, as clap reads from env vars with `env = "VAR"`
    sanitize_clap_env();

    let mut toolchain: Option<String> = None;

    // When invoked as `cargo cross`, cargo sets the CARGO env var and passes
    // args as ["cargo-cross", "cross", ...]. We need to skip both.
    // When invoked directly as `cargo-cross`, only skip the program name.
    let is_cargo_subcommand = env::var("CARGO").is_ok()
        && env::var("CARGO_HOME").is_ok()
        && args.get(1).map(String::as_str) == Some("cross");

    let skip_count = if is_cargo_subcommand { 2 } else { 1 };
    let mut args: Vec<String> = args.iter().skip(skip_count).cloned().collect();

    // Extract +toolchain from args (can appear at the beginning)
    // e.g., cargo-cross +nightly build -t x86_64-unknown-linux-musl
    if let Some(tc) = args.first().and_then(|a| a.strip_prefix('+')) {
        toolchain = Some(tc.to_string());
        args.remove(0);
    }

    // Prepend program name for clap
    args.insert(0, "cargo-cross".to_string());

    // Try to parse with clap
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            // For help/version/missing subcommand, let clap print and exit
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayVersion
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
            ) {
                e.exit();
            }
            // For argument errors, return Err so tests can catch them
            return Err(CrossError::ClapError(e.render().to_string()));
        }
    };

    process_cli(cli, toolchain)
}

fn process_cli(cli: Cli, toolchain: Option<String>) -> Result<ParseResult> {
    match cli.command {
        CliCommand::Build(args) => {
            let args = finalize_args(args, Command::Build, toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Check(args) => {
            let args = finalize_args(args, Command::Check, toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Run(args) => {
            let args = finalize_args(args, Command::Run, toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Test(args) => {
            let args = finalize_args(args, Command::Test, toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Bench(args) => {
            let args = finalize_args(args, Command::Bench, toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Targets(args) => Ok(ParseResult::ShowTargets(args.format)),
        CliCommand::Version => Ok(ParseResult::ShowVersion),
    }
}

/// Expand target list, handling glob patterns
fn expand_target_list(targets: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for target in targets {
        // Split by comma or newline to support multiple delimiters
        for part in target.split([',', '\n']) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let expanded = config::expand_targets(part);
            if expanded.is_empty() {
                if !result.contains(&part.to_string()) {
                    result.push(part.to_string());
                }
            } else {
                for t in expanded {
                    let t = t.to_string();
                    if !result.contains(&t) {
                        result.push(t);
                    }
                }
            }
        }
    }
    result
}

fn finalize_args(
    mut build_args: BuildArgs,
    command: Command,
    toolchain: Option<String>,
) -> Result<Args> {
    // Handle --release flag: set profile to "release"
    if build_args.release {
        build_args.profile = "release".to_string();
    }

    // Merge toolchain: +toolchain syntax takes precedence over --toolchain option
    let final_toolchain = toolchain.or_else(|| build_args.toolchain_option.clone());

    let mut args = Args::from_build_args(build_args, command, final_toolchain);

    // Validate versions
    validate_versions(&args)?;

    // Handle empty targets - default to host
    if args.targets.is_empty() {
        let host = config::HostPlatform::detect();
        args.targets.push(host.triple);
        args.use_default_linker = true;
        args.no_cargo_target = true;
    }

    Ok(args)
}

/// Validate version options
fn validate_versions(args: &Args) -> Result<()> {
    if !SUPPORTED_GLIBC_VERSIONS.contains(&args.glibc_version.as_str()) {
        return Err(CrossError::UnsupportedGlibcVersion {
            version: args.glibc_version.clone(),
            supported: SUPPORTED_GLIBC_VERSIONS.join(", "),
        });
    }

    let host = config::HostPlatform::detect();
    if !host.is_darwin()
        && !SUPPORTED_IPHONE_SDK_VERSIONS.contains(&args.iphone_sdk_version.as_str())
    {
        return Err(CrossError::UnsupportedIphoneSdkVersion {
            version: args.iphone_sdk_version.clone(),
            supported: SUPPORTED_IPHONE_SDK_VERSIONS.join(", "),
        });
    }

    if !host.is_darwin() && !SUPPORTED_MACOS_SDK_VERSIONS.contains(&args.macos_sdk_version.as_str())
    {
        return Err(CrossError::UnsupportedMacosSdkVersion {
            version: args.macos_sdk_version.clone(),
            supported: SUPPORTED_MACOS_SDK_VERSIONS.join(", "),
        });
    }

    if !SUPPORTED_FREEBSD_VERSIONS.contains(&args.freebsd_version.as_str()) {
        return Err(CrossError::UnsupportedFreebsdVersion {
            version: args.freebsd_version.clone(),
            supported: SUPPORTED_FREEBSD_VERSIONS.join(", "),
        });
    }

    Ok(())
}

/// Print all supported targets
pub fn print_all_targets(format: OutputFormat) {
    let mut targets: Vec<_> = config::all_targets().collect();
    targets.sort_unstable();

    match format {
        OutputFormat::Text => {
            use colored::Colorize;
            println!("{}", "Supported Rust targets:".bright_green());
            for target in &targets {
                println!("  {}", target.bright_cyan());
            }
        }
        OutputFormat::Json => {
            let json_array = serde_json::to_string(&targets).unwrap_or_else(|_| "[]".to_string());
            println!("{json_array}");
        }
        OutputFormat::Plain => {
            for target in &targets {
                println!("{target}");
            }
        }
    }

    // Output to GITHUB_OUTPUT if running in GitHub Actions
    if let Ok(github_output) = std::env::var("GITHUB_OUTPUT") {
        let json_array = serde_json::to_string(&targets).unwrap_or_else(|_| "[]".to_string());
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .append(true)
            .open(&github_output)
        {
            use std::io::Write;
            let _ = writeln!(file, "all-targets={json_array}");
        }
    }
}

/// Print version information
pub fn print_version() {
    use colored::Colorize;

    let version = env!("CARGO_PKG_VERSION");
    let name = env!("CARGO_PKG_NAME");
    println!("{} {}", name.bright_green(), version.bright_cyan());
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args> {
        let args: Vec<String> = args.iter().map(std::string::ToString::to_string).collect();
        match parse_args_from(args)? {
            ParseResult::Build(args) => Ok(*args),
            ParseResult::ShowTargets(_) => panic!("unexpected ShowTargets"),
            ParseResult::ShowVersion => panic!("unexpected ShowVersion"),
        }
    }

    // Note: test_parse_empty_requires_subcommand removed because MissingSubcommand
    // now calls exit() which cannot be tested

    #[test]
    fn test_parse_build_command() {
        let args = parse(&["cargo-cross", "build"]).unwrap();
        assert_eq!(args.command, Command::Build);
    }

    #[test]
    fn test_parse_check_command() {
        let args = parse(&["cargo-cross", "check"]).unwrap();
        assert_eq!(args.command, Command::Check);
    }

    #[test]
    fn test_parse_target() {
        let args = parse(&["cargo-cross", "build", "-t", "x86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_parse_multiple_targets() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl,aarch64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(
            args.targets,
            vec!["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
        );
    }

    #[test]
    fn test_parse_verbose() {
        let args = parse(&["cargo-cross", "build", "-vvv"]).unwrap();
        assert_eq!(args.verbose_level, 3);
    }

    #[test]
    fn test_parse_crt_static_flag() {
        let args = parse(&["cargo-cross", "build", "--crt-static", "true"]).unwrap();
        assert_eq!(args.crt_static, Some(true));
    }

    #[test]
    fn test_parse_crt_static_false() {
        let args = parse(&["cargo-cross", "build", "--crt-static", "false"]).unwrap();
        assert_eq!(args.crt_static, Some(false));
    }

    #[test]
    fn test_parse_build_std() {
        let args = parse(&["cargo-cross", "build", "--build-std", "true"]).unwrap();
        assert_eq!(args.build_std, Some("true".to_string()));
    }

    #[test]
    fn test_parse_build_std_crates() {
        let args = parse(&["cargo-cross", "build", "--build-std", "core,alloc"]).unwrap();
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
    }

    #[test]
    fn test_parse_features() {
        let args = parse(&["cargo-cross", "build", "--features", "foo,bar"]).unwrap();
        assert_eq!(args.features, Some("foo,bar".to_string()));
    }

    #[test]
    fn test_parse_no_default_features() {
        let args = parse(&["cargo-cross", "build", "--no-default-features"]).unwrap();
        assert!(args.no_default_features);
    }

    #[test]
    fn test_parse_profile() {
        let args = parse(&["cargo-cross", "build", "--profile", "dev"]).unwrap();
        assert_eq!(args.profile, "dev");
    }

    #[test]
    fn test_parse_jobs() {
        let args = parse(&["cargo-cross", "build", "-j", "4"]).unwrap();
        assert_eq!(args.jobs, Some("4".to_string()));
    }

    #[test]
    fn test_parse_passthrough_args() {
        let args = parse(&["cargo-cross", "build", "--", "--foo", "--bar"]).unwrap();
        assert_eq!(args.passthrough_args, vec!["--foo", "--bar"]);
    }

    #[test]
    fn test_parse_z_flag() {
        let args = parse(&["cargo-cross", "build", "-Z", "build-std"]).unwrap();
        assert_eq!(args.cargo_z_flags, vec!["build-std"]);
    }

    #[test]
    fn test_parse_config_flag() {
        let args = parse(&["cargo-cross", "build", "--config", "opt-level=3"]).unwrap();
        assert_eq!(args.cargo_config, vec!["opt-level=3"]);
    }

    #[test]
    fn test_targets_subcommand() {
        let args: Vec<String> = vec!["cargo-cross".to_string(), "targets".to_string()];
        match parse_args_from(args).unwrap() {
            ParseResult::ShowTargets(format) => {
                assert_eq!(format, OutputFormat::Text);
            }
            _ => panic!("expected ShowTargets"),
        }
    }

    #[test]
    fn test_targets_json_format() {
        let args: Vec<String> = vec![
            "cargo-cross".to_string(),
            "targets".to_string(),
            "--format".to_string(),
            "json".to_string(),
        ];
        match parse_args_from(args).unwrap() {
            ParseResult::ShowTargets(format) => {
                assert_eq!(format, OutputFormat::Json);
            }
            _ => panic!("expected ShowTargets"),
        }
    }

    #[test]
    fn test_targets_plain_format() {
        let args: Vec<String> = vec![
            "cargo-cross".to_string(),
            "targets".to_string(),
            "-f".to_string(),
            "plain".to_string(),
        ];
        match parse_args_from(args).unwrap() {
            ParseResult::ShowTargets(format) => {
                assert_eq!(format, OutputFormat::Plain);
            }
            _ => panic!("expected ShowTargets"),
        }
    }

    #[test]
    fn test_parse_toolchain() {
        let args = parse(&["cargo-cross", "+nightly", "build"]).unwrap();
        assert_eq!(args.toolchain, Some("nightly".to_string()));
        assert_eq!(args.command, Command::Build);
    }

    #[test]
    fn test_parse_toolchain_with_target() {
        let args = parse(&[
            "cargo-cross",
            "+nightly",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.toolchain, Some("nightly".to_string()));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    // =========================================================================
    // Equals syntax vs space syntax tests
    // =========================================================================

    #[test]
    fn test_equals_syntax_target() {
        let args = parse(&["cargo-cross", "build", "-t=x86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_equals_syntax_long_target() {
        let args = parse(&["cargo-cross", "build", "--target=x86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_equals_syntax_profile() {
        let args = parse(&["cargo-cross", "build", "--profile=dev"]).unwrap();
        assert_eq!(args.profile, "dev");
    }

    #[test]
    fn test_equals_syntax_features() {
        let args = parse(&["cargo-cross", "build", "--features=foo,bar"]).unwrap();
        assert_eq!(args.features, Some("foo,bar".to_string()));
    }

    #[test]
    fn test_equals_syntax_short_features() {
        let args = parse(&["cargo-cross", "build", "-F=foo,bar"]).unwrap();
        assert_eq!(args.features, Some("foo,bar".to_string()));
    }

    #[test]
    fn test_equals_syntax_jobs() {
        let args = parse(&["cargo-cross", "build", "-j=8"]).unwrap();
        assert_eq!(args.jobs, Some("8".to_string()));
    }

    #[test]
    fn test_equals_syntax_crt_static() {
        let args = parse(&["cargo-cross", "build", "--crt-static=true"]).unwrap();
        assert_eq!(args.crt_static, Some(true));
    }

    // =========================================================================
    // Mixed flags and options tests
    // =========================================================================

    #[test]
    fn test_mixed_crt_static_then_flag() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--crt-static",
            "true",
            "--no-default-features",
        ])
        .unwrap();
        assert_eq!(args.crt_static, Some(true));
        assert!(args.no_default_features);
    }

    #[test]
    fn test_mixed_crt_static_then_short_option() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--crt-static",
            "false",
            "-F",
            "serde",
        ])
        .unwrap();
        assert_eq!(args.crt_static, Some(false));
        assert_eq!(args.features, Some("serde".to_string()));
    }

    #[test]
    fn test_mixed_crt_static_with_target() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--crt-static",
            "true",
            "-t",
            "x86_64-unknown-linux-musl",
            "--profile",
            "release",
        ])
        .unwrap();
        assert_eq!(args.crt_static, Some(true));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile, "release");
    }

    #[test]
    fn test_mixed_flag_then_crt_static() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--no-default-features",
            "--crt-static",
            "true",
        ])
        .unwrap();
        assert!(args.no_default_features);
        assert_eq!(args.crt_static, Some(true));
    }

    #[test]
    fn test_mixed_multiple_flags_and_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "aarch64-unknown-linux-musl",
            "--no-default-features",
            "-F",
            "serde,json",
            "--crt-static",
            "true",
            "--profile",
            "release",
            "-vv",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["aarch64-unknown-linux-musl"]);
        assert!(args.no_default_features);
        assert_eq!(args.features, Some("serde,json".to_string()));
        assert_eq!(args.crt_static, Some(true));
        assert_eq!(args.profile, "release");
        assert_eq!(args.verbose_level, 2);
    }

    // =========================================================================
    // Complex option ordering tests
    // =========================================================================

    #[test]
    fn test_options_before_command_style() {
        // Options can come in any order
        let args = parse(&[
            "cargo-cross",
            "build",
            "--profile",
            "dev",
            "-t",
            "x86_64-unknown-linux-musl",
            "--features",
            "foo",
        ])
        .unwrap();
        assert_eq!(args.profile, "dev");
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.features, Some("foo".to_string()));
    }

    #[test]
    fn test_interleaved_short_and_long_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
            "--profile",
            "release",
            "-F",
            "foo",
            "--no-default-features",
            "-j",
            "4",
            "--locked",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile, "release");
        assert_eq!(args.features, Some("foo".to_string()));
        assert!(args.no_default_features);
        assert_eq!(args.jobs, Some("4".to_string()));
        assert!(args.locked);
    }

    // =========================================================================
    // Verbose flag variations
    // =========================================================================

    #[test]
    fn test_verbose_single() {
        let args = parse(&["cargo-cross", "build", "-v"]).unwrap();
        assert_eq!(args.verbose_level, 1);
    }

    #[test]
    fn test_verbose_double() {
        let args = parse(&["cargo-cross", "build", "-vv"]).unwrap();
        assert_eq!(args.verbose_level, 2);
    }

    #[test]
    fn test_verbose_triple() {
        let args = parse(&["cargo-cross", "build", "-vvv"]).unwrap();
        assert_eq!(args.verbose_level, 3);
    }

    #[test]
    fn test_verbose_separate() {
        let args = parse(&["cargo-cross", "build", "-v", "-v", "-v"]).unwrap();
        assert_eq!(args.verbose_level, 3);
    }

    #[test]
    fn test_verbose_long_form() {
        let args = parse(&["cargo-cross", "build", "--verbose", "--verbose"]).unwrap();
        assert_eq!(args.verbose_level, 2);
    }

    #[test]
    fn test_verbose_mixed_with_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-v",
            "-t",
            "x86_64-unknown-linux-musl",
            "-v",
        ])
        .unwrap();
        assert_eq!(args.verbose_level, 2);
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    // =========================================================================
    // Timings option (optional value) tests
    // =========================================================================

    #[test]
    fn test_timings_without_value() {
        let args = parse(&["cargo-cross", "build", "--timings"]).unwrap();
        assert_eq!(args.timings, Some("true".to_string()));
    }

    #[test]
    fn test_timings_with_value() {
        let args = parse(&["cargo-cross", "build", "--timings=html"]).unwrap();
        assert_eq!(args.timings, Some("html".to_string()));
    }

    #[test]
    fn test_timings_followed_by_flag() {
        let args = parse(&["cargo-cross", "build", "--timings", "--locked"]).unwrap();
        assert_eq!(args.timings, Some("true".to_string()));
        assert!(args.locked);
    }

    #[test]
    fn test_timings_followed_by_option() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--timings",
            "-t",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.timings, Some("true".to_string()));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    // =========================================================================
    // Multiple values / repeated options tests
    // =========================================================================

    #[test]
    fn test_multiple_targets_comma_separated() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl,aarch64-unknown-linux-musl,armv7-unknown-linux-musleabihf",
        ])
        .unwrap();
        assert_eq!(args.targets.len(), 3);
        assert_eq!(args.targets[0], "x86_64-unknown-linux-musl");
        assert_eq!(args.targets[1], "aarch64-unknown-linux-musl");
        assert_eq!(args.targets[2], "armv7-unknown-linux-musleabihf");
    }

    #[test]
    fn test_multiple_targets_repeated_option() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
            "-t",
            "aarch64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.targets.len(), 2);
    }

    #[test]
    fn test_multiple_rustflags() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--rustflag",
            "-C opt-level=3",
            "--rustflag",
            "-C lto=thin",
        ])
        .unwrap();
        assert_eq!(args.rustflags.len(), 2);
        assert_eq!(args.rustflags[0], "-C opt-level=3");
        assert_eq!(args.rustflags[1], "-C lto=thin");
    }

    #[test]
    fn test_multiple_config_flags() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--config",
            "build.jobs=4",
            "--config",
            "profile.release.lto=true",
        ])
        .unwrap();
        assert_eq!(args.cargo_config.len(), 2);
    }

    #[test]
    fn test_multiple_z_flags() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-Z",
            "build-std",
            "-Z",
            "unstable-options",
        ])
        .unwrap();
        assert_eq!(args.cargo_z_flags.len(), 2);
    }

    // =========================================================================
    // Hyphen values tests (for compiler flags)
    // =========================================================================

    #[test]
    fn test_cflags_with_hyphen() {
        let args = parse(&["cargo-cross", "build", "--cflags", "-O2 -Wall"]).unwrap();
        assert_eq!(args.cflags, Some("-O2 -Wall".to_string()));
    }

    #[test]
    fn test_ldflags_with_hyphen() {
        let args = parse(&["cargo-cross", "build", "--ldflags", "-L/usr/local/lib"]).unwrap();
        assert_eq!(args.ldflags, Some("-L/usr/local/lib".to_string()));
    }

    #[test]
    fn test_rustflag_with_hyphen() {
        let args = parse(&["cargo-cross", "build", "--rustflag", "-C target-cpu=native"]).unwrap();
        assert_eq!(args.rustflags, vec!["-C target-cpu=native"]);
    }

    // =========================================================================
    // Passthrough arguments tests
    // =========================================================================

    #[test]
    fn test_passthrough_single() {
        let args = parse(&["cargo-cross", "build", "--", "--nocapture"]).unwrap();
        assert_eq!(args.passthrough_args, vec!["--nocapture"]);
    }

    #[test]
    fn test_passthrough_multiple() {
        let args = parse(&[
            "cargo-cross",
            "test",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .unwrap();
        assert_eq!(
            args.passthrough_args,
            vec!["--nocapture", "--test-threads=1"]
        );
    }

    #[test]
    fn test_passthrough_with_hyphen_values() {
        let args = parse(&["cargo-cross", "build", "--", "-v", "--foo", "-bar"]).unwrap();
        assert_eq!(args.passthrough_args, vec!["-v", "--foo", "-bar"]);
    }

    #[test]
    fn test_passthrough_after_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
            "--profile",
            "release",
            "--",
            "--foo",
            "--bar",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile, "release");
        assert_eq!(args.passthrough_args, vec!["--foo", "--bar"]);
    }

    // =========================================================================
    // Alias tests
    // =========================================================================

    #[test]
    fn test_alias_targets() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--targets",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_alias_workspace_all() {
        let args = parse(&["cargo-cross", "build", "--all"]).unwrap();
        assert!(args.workspace);
    }

    #[test]
    fn test_alias_rustflags() {
        let args = parse(&["cargo-cross", "build", "--rustflags", "-C lto"]).unwrap();
        assert_eq!(args.rustflags, vec!["-C lto"]);
    }

    #[test]
    fn test_alias_trim_paths() {
        let args = parse(&["cargo-cross", "build", "--trim-paths", "all"]).unwrap();
        assert_eq!(args.cargo_trim_paths, Some("all".to_string()));
    }

    // =========================================================================
    // Command alias tests
    // =========================================================================

    #[test]
    fn test_command_alias_b() {
        let args = parse(&["cargo-cross", "b"]).unwrap();
        assert_eq!(args.command, Command::Build);
    }

    #[test]
    fn test_command_alias_c() {
        let args = parse(&["cargo-cross", "c"]).unwrap();
        assert_eq!(args.command, Command::Check);
    }

    #[test]
    fn test_command_alias_r() {
        let args = parse(&["cargo-cross", "r"]).unwrap();
        assert_eq!(args.command, Command::Run);
    }

    #[test]
    fn test_command_alias_t() {
        let args = parse(&["cargo-cross", "t"]).unwrap();
        assert_eq!(args.command, Command::Test);
    }

    // =========================================================================
    // Requires relationship tests
    // =========================================================================

    #[test]
    fn test_requires_exclude_needs_workspace() {
        let result = parse(&["cargo-cross", "build", "--exclude", "foo"]);
        assert!(
            result.is_err() || {
                // clap exits on error, so we might not get here
                false
            }
        );
    }

    #[test]
    fn test_requires_exclude_with_workspace() {
        let args = parse(&["cargo-cross", "build", "--workspace", "--exclude", "foo"]).unwrap();
        assert!(args.workspace);
        assert_eq!(args.exclude, Some("foo".to_string()));
    }

    #[test]
    fn test_requires_build_std_features_with_build_std() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--build-std",
            "core,alloc",
            "--build-std-features",
            "panic_immediate_abort",
        ])
        .unwrap();
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
        assert_eq!(
            args.build_std_features,
            Some("panic_immediate_abort".to_string())
        );
    }

    // =========================================================================
    // Conflicts relationship tests
    // =========================================================================

    #[test]
    fn test_conflicts_quiet_verbose() {
        let result = parse(&["cargo-cross", "build", "--quiet", "--verbose"]);
        // This should fail due to conflict
        assert!(
            result.is_err() || {
                // clap exits, checking we don't panic
                false
            }
        );
    }

    #[test]
    fn test_conflicts_features_all_features() {
        let result = parse(&[
            "cargo-cross",
            "build",
            "--features",
            "foo",
            "--all-features",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_conflicts_linker_use_default_linker() {
        let result = parse(&[
            "cargo-cross",
            "build",
            "--linker",
            "/usr/bin/ld",
            "--use-default-linker",
        ]);
        assert!(result.is_err());
    }

    // =========================================================================
    // Complex real-world scenario tests
    // =========================================================================

    #[test]
    fn test_real_world_linux_musl_build() {
        let args = parse(&[
            "cargo-cross",
            "+nightly",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
            "--profile",
            "release",
            "--crt-static",
            "true",
            "--no-default-features",
            "-F",
            "serde,json",
            "-j",
            "8",
            "--locked",
        ])
        .unwrap();
        assert_eq!(args.toolchain, Some("nightly".to_string()));
        assert_eq!(args.command, Command::Build);
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile, "release");
        assert_eq!(args.crt_static, Some(true));
        assert!(args.no_default_features);
        assert_eq!(args.features, Some("serde,json".to_string()));
        assert_eq!(args.jobs, Some("8".to_string()));
        assert!(args.locked);
    }

    #[test]
    fn test_real_world_multi_target_build() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl,aarch64-unknown-linux-musl",
            "--profile",
            "release",
            "--build-std",
            "core,alloc",
            "--build-std-features",
            "panic_immediate_abort",
            "-vv",
        ])
        .unwrap();
        assert_eq!(args.targets.len(), 2);
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
        assert_eq!(
            args.build_std_features,
            Some("panic_immediate_abort".to_string())
        );
        assert_eq!(args.verbose_level, 2);
    }

    #[test]
    fn test_real_world_test_with_passthrough() {
        let args = parse(&[
            "cargo-cross",
            "test",
            "-t",
            "x86_64-unknown-linux-musl",
            "--",
            "--nocapture",
            "--test-threads=1",
        ])
        .unwrap();
        assert_eq!(args.command, Command::Test);
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(
            args.passthrough_args,
            vec!["--nocapture", "--test-threads=1"]
        );
    }

    #[test]
    fn test_real_world_with_compiler_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "aarch64-unknown-linux-musl",
            "--cc",
            "/opt/cross/bin/aarch64-linux-musl-gcc",
            "--cxx",
            "/opt/cross/bin/aarch64-linux-musl-g++",
            "--ar",
            "/opt/cross/bin/aarch64-linux-musl-ar",
            "--cflags",
            "-O2 -march=armv8-a",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["aarch64-unknown-linux-musl"]);
        assert!(args.cc.is_some());
        assert!(args.cxx.is_some());
        assert!(args.ar.is_some());
        assert_eq!(args.cflags, Some("-O2 -march=armv8-a".to_string()));
    }

    #[test]
    fn test_real_world_sccache_build() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "x86_64-unknown-linux-musl",
            "--enable-sccache",
            "--sccache-dir",
            "/tmp/sccache",
            "--sccache-cache-size",
            "10G",
        ])
        .unwrap();
        assert!(args.enable_sccache);
        assert_eq!(args.sccache_dir, Some(PathBuf::from("/tmp/sccache")));
        assert_eq!(args.sccache_cache_size, Some("10G".to_string()));
    }

    #[test]
    fn test_real_world_workspace_build() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--workspace",
            "--exclude",
            "test-crate",
            "-t",
            "x86_64-unknown-linux-musl",
            "--profile",
            "release",
        ])
        .unwrap();
        assert!(args.workspace);
        assert_eq!(args.exclude, Some("test-crate".to_string()));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    // =========================================================================
    // Edge case tests
    // =========================================================================

    #[test]
    fn test_edge_case_equals_in_value() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--config",
            "build.rustflags=['-C', 'opt-level=3']",
        ])
        .unwrap();
        assert_eq!(
            args.cargo_config,
            vec!["build.rustflags=['-C', 'opt-level=3']"]
        );
    }

    #[test]
    fn test_edge_case_empty_passthrough() {
        let args = parse(&["cargo-cross", "build", "--"]).unwrap();
        assert!(args.passthrough_args.is_empty());
    }

    #[test]
    fn test_edge_case_target_with_numbers() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "armv7-unknown-linux-musleabihf",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["armv7-unknown-linux-musleabihf"]);
    }

    #[test]
    fn test_edge_case_all_bool_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--no-default-features",
            "--workspace",
            "--bins",
            "--lib",
            "--examples",
            "--tests",
            "--benches",
            "--all-targets",
            "--locked",
            "--offline",
            "--keep-going",
        ])
        .unwrap();
        assert!(args.no_default_features);
        assert!(args.workspace);
        assert!(args.build_bins);
        assert!(args.build_lib);
        assert!(args.build_examples);
        assert!(args.build_tests);
        assert!(args.build_benches);
        assert!(args.build_all_targets);
        assert!(args.locked);
        assert!(args.offline);
        assert!(args.keep_going);
    }

    #[test]
    fn test_edge_case_mixed_equals_and_space() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t=x86_64-unknown-linux-musl",
            "--profile",
            "release",
            "-F=serde",
            "--crt-static",
            "true",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile, "release");
        assert_eq!(args.features, Some("serde".to_string()));
        assert_eq!(args.crt_static, Some(true));
    }

    #[test]
    fn test_edge_case_directory_option() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-C",
            "/path/to/project",
            "-t",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.cargo_cwd, Some(PathBuf::from("/path/to/project")));
    }

    #[test]
    fn test_edge_case_manifest_path() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--manifest-path",
            "/path/to/Cargo.toml",
        ])
        .unwrap();
        assert_eq!(
            args.manifest_path,
            Some(PathBuf::from("/path/to/Cargo.toml"))
        );
    }

    // =========================================================================
    // Cargo cross invocation style tests
    // =========================================================================

    #[test]
    fn test_cargo_cross_style_build() {
        let args: Vec<String> = vec![
            "cargo-cross".to_string(),
            "cross".to_string(),
            "build".to_string(),
            "-t".to_string(),
            "x86_64-unknown-linux-musl".to_string(),
        ];
        match parse_args_from(args).unwrap() {
            ParseResult::Build(args) => {
                assert_eq!(args.command, Command::Build);
                assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
            }
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn test_cargo_cross_style_with_toolchain() {
        let args: Vec<String> = vec![
            "cargo-cross".to_string(),
            "cross".to_string(),
            "+nightly".to_string(),
            "build".to_string(),
        ];
        match parse_args_from(args).unwrap() {
            ParseResult::Build(args) => {
                assert_eq!(args.toolchain, Some("nightly".to_string()));
                assert_eq!(args.command, Command::Build);
            }
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn test_cargo_cross_style_targets() {
        let args: Vec<String> = vec![
            "cargo-cross".to_string(),
            "cross".to_string(),
            "targets".to_string(),
        ];
        match parse_args_from(args).unwrap() {
            ParseResult::ShowTargets(_) => {}
            _ => panic!("expected ShowTargets"),
        }
    }

    // =========================================================================
    // New alias and option tests
    // =========================================================================

    #[test]
    fn test_github_proxy_mirror_alias() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--github-proxy-mirror",
            "https://mirror.example.com/",
        ])
        .unwrap();
        assert_eq!(
            args.github_proxy,
            Some("https://mirror.example.com/".to_string())
        );
    }

    #[test]
    fn test_github_proxy_original() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--github-proxy",
            "https://proxy.example.com/",
        ])
        .unwrap();
        assert_eq!(
            args.github_proxy,
            Some("https://proxy.example.com/".to_string())
        );
    }

    #[test]
    fn test_release_flag_short() {
        let args = parse(&["cargo-cross", "build", "-r"]).unwrap();
        assert!(args.release);
        assert_eq!(args.profile, "release");
    }

    #[test]
    fn test_release_flag_long() {
        let args = parse(&["cargo-cross", "build", "--release"]).unwrap();
        assert!(args.release);
        assert_eq!(args.profile, "release");
    }

    #[test]
    fn test_toolchain_option() {
        let args = parse(&["cargo-cross", "build", "--toolchain", "nightly"]).unwrap();
        assert_eq!(args.toolchain, Some("nightly".to_string()));
    }

    #[test]
    fn test_toolchain_option_with_version() {
        let args = parse(&["cargo-cross", "build", "--toolchain", "1.75.0"]).unwrap();
        assert_eq!(args.toolchain, Some("1.75.0".to_string()));
    }

    #[test]
    fn test_toolchain_plus_syntax_takes_precedence() {
        let args = parse(&["cargo-cross", "+nightly", "build", "--toolchain", "stable"]).unwrap();
        // +nightly syntax takes precedence over --toolchain
        assert_eq!(args.toolchain, Some("nightly".to_string()));
    }

    #[test]
    fn test_target_dir_alias() {
        let args = parse(&["cargo-cross", "build", "--target-dir", "/tmp/target"]).unwrap();
        assert_eq!(args.cargo_target_dir, Some(PathBuf::from("/tmp/target")));
    }

    #[test]
    fn test_cargo_target_dir_original() {
        let args = parse(&["cargo-cross", "build", "--cargo-target-dir", "/tmp/target"]).unwrap();
        assert_eq!(args.cargo_target_dir, Some(PathBuf::from("/tmp/target")));
    }

    #[test]
    fn test_args_alias() {
        let args = parse(&["cargo-cross", "build", "--args", "--verbose"]).unwrap();
        assert_eq!(args.cargo_args, Some("--verbose".to_string()));
    }

    #[test]
    fn test_cargo_args_original() {
        let args = parse(&["cargo-cross", "build", "--cargo-args", "--verbose"]).unwrap();
        assert_eq!(args.cargo_args, Some("--verbose".to_string()));
    }
}
