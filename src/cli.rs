//! Command-line argument parsing for cargo-cross using clap

use crate::config::{
    self, supported_freebsd_versions_str, supported_glibc_versions_str,
    supported_iphone_sdk_versions_str, supported_macos_sdk_versions_str,
    DEFAULT_CROSS_MAKE_VERSION, DEFAULT_FREEBSD_VERSION, DEFAULT_GLIBC_VERSION,
    DEFAULT_IPHONE_SDK_VERSION, DEFAULT_MACOS_SDK_VERSION, DEFAULT_NDK_VERSION,
    DEFAULT_QEMU_VERSION, SUPPORTED_FREEBSD_VERSIONS, SUPPORTED_GLIBC_VERSIONS,
    SUPPORTED_IPHONE_SDK_VERSIONS, SUPPORTED_MACOS_SDK_VERSIONS,
};
use crate::error::{CrossError, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::ArgAction;
use clap::{Args as ClapArgs, CommandFactory, FromArgMatches, Parser, Subcommand, ValueHint};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

/// Binary name from Cargo.toml (e.g., "cargo-cross")
const BIN_NAME: &str = env!("CARGO_PKG_NAME");

/// Define subcommand-related constants from a single literal
macro_rules! define_subcommand {
    ($name:literal) => {
        const SUBCOMMAND: &str = $name;
        const CARGO_DISPLAY_NAME: &str = concat!("cargo ", $name);
    };
}
define_subcommand!("cross");

/// Program name - either "cargo cross" or "cargo-cross" based on invocation style
static PROGRAM_NAME: LazyLock<&'static str> = LazyLock::new(|| {
    let args: Vec<String> = std::env::args().collect();
    let is_cargo_subcommand = std::env::var("CARGO").is_ok()
        && std::env::var("CARGO_HOME").is_ok()
        && args.get(1).map(String::as_str) == Some(SUBCOMMAND);

    if is_cargo_subcommand {
        CARGO_DISPLAY_NAME
    } else {
        BIN_NAME
    }
});

/// Get the program name for display
#[must_use]
pub fn program_name() -> &'static str {
    *PROGRAM_NAME
}

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
#[command(override_usage = "cargo-cross [+toolchain] [CARGO_OPTIONS] <COMMAND> [OPTIONS]")]
#[command(after_help = "\
Use 'cargo-cross <COMMAND> --help' for more information about a command.

TOOLCHAIN:
    If the first argument begins with +, it will be interpreted as a Rust toolchain
    name (such as +nightly, +stable, or +1.75.0). This follows the same convention
    as rustup and cargo.

CARGO OPTIONS:
    Cargo global options can precede supported Cargo commands: -v, -q, --color,
    -C, --locked, --offline, --frozen, --config, and -Z.

EXAMPLES:
    cargo-cross build -t x86_64-unknown-linux-musl
    cargo-cross --config build.jobs=4 build
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

When no target selection options are given, this tool will build all binary
and library targets of the selected packages.")]
    Build(BuildArgs),

    /// Analyze the current package and report errors, but don't build object files
    #[command(visible_alias = "c")]
    #[command(long_about = "\
Check the current package and all of its dependencies for errors.

This will essentially compile packages without performing the final step of
code generation, which is faster than running build.")]
    Check(BuildArgs),

    /// Run a binary or example of the current package
    #[command(visible_alias = "r")]
    #[command(long_about = "\
Run a binary or example of the local package.

For cross-compilation targets, QEMU user-mode emulation is used to run the binary.")]
    Run(BuildArgs),

    /// Run the tests
    #[command(visible_alias = "t")]
    #[command(long_about = "\
Execute all unit and integration tests and build examples of a local package.

For cross-compilation targets, QEMU user-mode emulation is used to run tests.")]
    Test(BuildArgs),

    /// Run the benchmarks
    #[command(long_about = "\
Execute all benchmarks of a local package.

For cross-compilation targets, QEMU user-mode emulation is used to run benchmarks.")]
    Bench(BuildArgs),

    /// Run Clippy lints on the current package
    #[command(long_about = "\
Run Clippy on the current package and all of its dependencies.

This forwards to `cargo clippy` with the configured cross-compilation environment.")]
    Clippy(BuildArgs),

    /// Prepare and print the configured cross-compilation environment
    #[command(long_about = "\
Prepare the cross-compilation environment and print environment variables.

This is intended for use with shell evaluation, for example:
    eval \"$(cargo cross setup -t aarch64-unknown-linux-musl)\"")]
    Setup(SetupCliArgs),

    /// Execute an arbitrary command inside the configured cross-compilation environment
    #[command(long_about = "\
Prepare the cross-compilation environment and execute an arbitrary command.

This is useful for running custom tooling that should inherit the configured
compiler, linker, PATH, and cargo target environment variables.")]
    Exec(ExecCliArgs),

    /// Display all supported cross-compilation targets
    #[command(long_about = "\
Display all supported cross-compilation targets.

You can also use glob patterns with --target to match multiple targets,
for example: --target '*-linux-musl' or --target 'aarch64-*'")]
    Targets(TargetsArgs),

    /// Print version information
    Version,
}

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

/// Output format for setup command
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum SetupOutputFormat {
    /// Detect the current shell from the environment, fallback to bash syntax
    #[default]
    Auto,
    /// Bash-compatible exports
    Bash,
    /// Zsh-compatible exports
    Zsh,
    /// Fish shell exports
    Fish,
    /// PowerShell environment assignments
    Powershell,
    /// Windows cmd.exe environment assignments
    Cmd,
    /// JSON object
    Json,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct SetupCliArgs {
    #[command(flatten)]
    pub build: BuildArgs,

    /// Initialize target runner variables when preparing the environment
    #[arg(
        long,
        env = "INIT_RUNNER",
        help = "Initialize target runner variables for the setup environment"
    )]
    pub init_runner: bool,

    /// Output format
    #[arg(
        short = 'f',
        long = "format",
        value_enum,
        default_value = "auto",
        help = "Output format (auto, bash, zsh, fish, powershell, cmd, json)"
    )]
    pub format: SetupOutputFormat,
}

#[derive(Debug, Clone)]
pub struct SetupArgs {
    pub args: Args,
    pub format: SetupOutputFormat,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct ExecCliArgs {
    #[command(flatten)]
    pub build: BuildArgs,

    /// Initialize target runner variables before executing the command
    #[arg(
        long,
        env = "INIT_RUNNER",
        help = "Initialize target runner variables for the exec environment"
    )]
    pub init_runner: bool,
}

#[derive(Debug, Clone)]
pub struct ExecArgs {
    pub args: Args,
    pub command: Vec<String>,
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
Run the 'targets' subcommand to see all supported targets.

Examples: -t x86_64-unknown-linux-musl, -t '*-linux-musl'"
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
Space or comma separated list of features to activate. Features of workspace members
may be enabled with package-name/feature-name syntax. May be specified multiple times."
    )]
    pub features: Vec<String>,

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
        conflicts_with = "profile",
        help_heading = "Profile",
        long_help = "\
Build artifacts in release mode, with optimizations. Equivalent to --profile=release."
    )]
    pub release: bool,

    /// Build artifacts with the specified profile
    #[arg(
        long,
        env = "PROFILE",
        value_name = "PROFILE-NAME",
        conflicts_with = "release",
        help_heading = "Profile",
        long_help = "\
Build artifacts with the specified profile. Built-in: dev, release, test, bench.
Custom profiles can be defined in Cargo.toml. Cargo selects the command-specific default."
    )]
    pub profile: Option<String>,

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
    pub package: Vec<String>,

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
    pub exclude: Vec<String>,

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
    pub bin_target: Vec<String>,

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
    pub example_target: Vec<String>,

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
    pub test_target: Vec<String>,

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
    pub bench_target: Vec<String>,

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
    #[arg(short = 'm', long, env = "MANIFEST_PATH", value_name = "PATH",
          value_hint = ValueHint::FilePath, help_heading = "Package Selection",
          long_help = "\
Path to Cargo.toml. By default, Cargo searches for the Cargo.toml file
in the current directory or any parent directory.")]
    pub manifest_path: Option<PathBuf>,

    // ===== Version Options =====
    /// Glibc version for Linux GNU targets
    #[arg(long, default_value = DEFAULT_GLIBC_VERSION, env = "GLIBC_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions")]
    pub glibc_version: String,

    /// iPhone SDK version for iOS targets
    #[arg(long, default_value = DEFAULT_IPHONE_SDK_VERSION, env = "IPHONE_SDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions")]
    pub iphone_sdk_version: String,

    /// Override iPhoneOS SDK path (skips version lookup)
    #[arg(long, env = "IPHONE_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override iPhoneOS SDK path for device targets. Skips version lookup.")]
    pub iphone_sdk_path: Option<PathBuf>,

    /// Override iPhoneSimulator SDK path
    #[arg(long, env = "IPHONE_SIMULATOR_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override iPhoneSimulator SDK path for simulator targets. Skips version lookup.")]
    pub iphone_simulator_sdk_path: Option<PathBuf>,

    /// macOS SDK version for Darwin targets
    #[arg(long, default_value = DEFAULT_MACOS_SDK_VERSION, env = "MACOS_SDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions")]
    pub macos_sdk_version: String,

    /// Override macOS SDK path (skips version lookup)
    #[arg(long, env = "MACOS_SDK_PATH", value_name = "PATH",
          value_hint = ValueHint::DirPath, help_heading = "Toolchain Versions",
          long_help = "\
Override macOS SDK path directly. Skips version lookup.")]
    pub macos_sdk_path: Option<PathBuf>,

    /// FreeBSD version for FreeBSD targets
    #[arg(long, default_value = DEFAULT_FREEBSD_VERSION, env = "FREEBSD_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions")]
    pub freebsd_version: String,

    /// Android NDK version
    #[arg(long, default_value = DEFAULT_NDK_VERSION, env = "NDK_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify Android NDK version for Android targets. Auto-downloaded from Google's official repository.")]
    pub ndk_version: String,

    /// QEMU version for user-mode emulation
    #[arg(long, default_value = DEFAULT_QEMU_VERSION, env = "QEMU_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify QEMU version for user-mode emulation. Used to run cross-compiled binaries during test/run/bench.")]
    pub qemu_version: String,

    /// Cross-compiler make version
    #[arg(long, default_value = DEFAULT_CROSS_MAKE_VERSION, env = "CROSS_MAKE_VERSION",
          value_name = "VERSION", hide_default_value = true, help_heading = "Toolchain Versions",
          long_help = "\
Specify cross-compiler make version. This determines which version of cross-compilation \
toolchains will be downloaded from the upstream repository.")]
    pub cross_make_version: String,

    // ===== Directories =====
    /// Directory for cross-compiler toolchains
    #[arg(long, env = "CROSS_COMPILER_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Directory where cross-compiler toolchains will be downloaded and stored. Defaults to temp dir.
Set this to reuse downloaded toolchains across builds.")]
    pub cross_compiler_dir: Option<PathBuf>,

    /// Directory for all generated artifacts
    #[arg(long, visible_alias = "target-dir", env = "CARGO_TARGET_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Directory for all generated artifacts and intermediate files. Defaults to 'target'.")]
    pub cargo_target_dir: Option<PathBuf>,

    /// Copy final artifacts to this directory (unstable)
    #[arg(long, env = "ARTIFACT_DIR", value_name = "DIR",
          value_hint = ValueHint::DirPath, help_heading = "Directories",
          long_help = "\
Copy final artifacts to this directory. Unstable, requires nightly toolchain.")]
    pub artifact_dir: Option<PathBuf>,

    // ===== Compiler Options =====
    /// Override C compiler path
    #[arg(long, env = "CC", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the C compiler path. By default, the appropriate cross-compiler is auto-configured.")]
    pub cc: Option<PathBuf>,

    /// Override C++ compiler path
    #[arg(long, env = "CXX", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the C++ compiler path. By default, the appropriate cross-compiler is auto-configured.")]
    pub cxx: Option<PathBuf>,

    /// Override archiver (ar) path
    #[arg(long, env = "AR", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the archiver (ar) path. By default, the appropriate archiver is auto-configured.")]
    pub ar: Option<PathBuf>,

    /// Override linker path
    #[arg(long, env = "LINKER", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath, help_heading = "Compiler Options",
          long_help = "\
Override the linker path. By default, the cross-compiler is used as linker.
This option takes precedence over auto-configured linker.")]
    pub linker: Option<PathBuf>,

    /// Additional flags for C compilation
    #[arg(
        long,
        env = "CFLAGS",
        value_name = "FLAGS",
        allow_hyphen_values = true,
        help_heading = "Compiler Options",
        long_help = "\
Additional flags to pass to the C compiler. Appended to default CFLAGS.
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
Additional flags to pass to the C++ compiler. Appended to default CXXFLAGS.
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
Additional flags to pass to the linker. Appended to default LDFLAGS.
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
Specify the C++ standard library to use (libc++, libstdc++, etc)."
    )]
    pub cxxstdlib: Option<String>,

    /// `CMake` generator to use (like cmake -G)
    #[arg(
        long,
        short = 'G',
        env = "CMAKE_GENERATOR",
        value_name = "GENERATOR",
        help_heading = "Compiler Options",
        long_help = "\
Specify the CMake generator to use. On Windows, this overrides the auto-detection.
Common generators: Ninja, 'MinGW Makefiles', 'Unix Makefiles', 'NMake Makefiles'.
If not specified, auto-detects: Ninja > MinGW Makefiles > Unix Makefiles."
    )]
    pub cmake_generator: Option<String>,

    /// Additional RUSTFLAGS (can be repeated)
    #[arg(long = "rustflag", visible_alias = "rustflags", value_name = "FLAG",
          env = "ADDITIONAL_RUSTFLAGS", allow_hyphen_values = true,
          action = clap::ArgAction::Append, help_heading = "Compiler Options",
          long_help = "\
Additional flags to pass to rustc via RUSTFLAGS. Can be specified multiple times.
Example: --rustflag '-C target-cpu=native' --rustflag '-C lto=thin'")]
    pub rustflags: Vec<String>,

    /// Rustc wrapper program
    #[arg(long, env = "RUSTC_WRAPPER", value_name = "PATH",
          value_hint = ValueHint::ExecutablePath,
          help_heading = "Compiler Options",
          long_help = "\
Specify the wrapper executable Cargo invokes for rustc processes.")]
    pub rustc_wrapper: Option<PathBuf>,

    /// Skip cross-compilation toolchain setup
    #[arg(
        long,
        env = "NO_TOOLCHAIN_SETUP",
        help_heading = "Compiler Options",
        long_help = "\
Skip downloading and configuring cross-compilation toolchain.
Use this when you have pre-configured system compilers or want to use
only CLI-provided compiler options (--cc, --cxx, --ar, --linker)."
    )]
    pub no_toolchain_setup: bool,

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
          value_name = "BOOL", num_args = 0..=1, default_missing_value = "true",
          help_heading = "Build Options",
          long_help = "\
Control whether the C runtime is statically linked. true=static (larger, portable),
false=dynamic (smaller, requires libc). Musl defaults to static, glibc to dynamic.")]
    pub crt_static: Option<bool>,

    /// Abort immediately on panic (smaller binary, implies --build-std)
    #[arg(
        long,
        env = "PANIC_IMMEDIATE_ABORT",
        help_heading = "Build Options",
        long_help = "\
Use panic=abort and remove panic formatting code for smaller binaries.
Requires nightly and implies --build-std. Stack traces will not be available."
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
          num_args = 0..=1, default_missing_value = "true",
          long_help = "\
Build the standard library from source (requires nightly). Without arguments, builds 'std'.
Use 'true' for full std or specify crates like 'core,alloc'. Required for unsupported targets or panic=abort.")]
    pub build_std: Option<String>,

    /// Features to enable when building std
    #[arg(
        long,
        env = "BUILD_STD_FEATURES",
        value_name = "FEATURES",
        requires = "build_std",
        help_heading = "Build Options",
        long_help = "\
Space-separated features for std. Common: panic_immediate_abort, optimize_for_size"
    )]
    pub build_std_features: Option<String>,

    /// Trim paths in compiler output for reproducible builds
    #[arg(
        long,
        visible_alias = "trim-paths",
        env = "CARGO_TRIM_PATHS",
        value_name = "VALUE",
        num_args = 0..=1,
        default_missing_value = "true",
        help_heading = "Build Options",
        long_help = "\
Control how paths are trimmed in compiler output for reproducible builds.
Valid: true, macro, diagnostics, object, all, none (default: false)"
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

    /// Set `RUSTC_BOOTSTRAP` for using nightly features on stable
    #[arg(
        long,
        env = "RUSTC_BOOTSTRAP",
        value_name = "VALUE",
        num_args = 0..=1,
        default_missing_value = "1",
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
Use verbose output. -v=commands/warnings, -vv=+deps/build scripts, -vvv=max verbosity")]
    pub verbose_level: u8,

    /// Do not print cargo log messages
    #[arg(
        short = 'q',
        long,
        env = "QUIET",
        conflicts_with = "verbose_level",
        help_heading = "Output Options",
        long_help = "\
Do not print cargo log messages. Shows only errors and warnings."
    )]
    pub quiet: bool,

    /// Diagnostic message format
    #[arg(
        long,
        env = "MESSAGE_FORMAT",
        value_name = "FMT",
        help_heading = "Output Options",
        long_help = "\
Output format for diagnostics. Valid: human, short, json, json-diagnostic-short,
json-diagnostic-rendered-ansi, json-render-diagnostics"
    )]
    pub message_format: Option<String>,

    /// Control when colored output is used
    #[arg(
        long,
        env = "COLOR",
        value_name = "WHEN",
        help_heading = "Output Options",
        long_help = "\
Control when colored output is used. Valid: auto (default), always, never"
    )]
    pub color: Option<String>,

    /// Output the build plan in JSON (requires nightly)
    #[arg(long, env = "BUILD_PLAN", hide = true, help_heading = "Output Options")]
    pub build_plan: bool,

    /// Output the build graph in JSON (requires nightly)
    #[arg(long, env = "UNIT_GRAPH", help_heading = "Output Options")]
    pub unit_graph: bool,

    /// Timing output formats (html, json)
    #[arg(long, env = "TIMINGS", value_name = "FMTS",
          num_args = 0..=1, default_missing_value = "true",
          help_heading = "Output Options",
          long_help = "\
Output timing information. --timings=HTML report, --timings=json for JSON. Saved to target/cargo-timings/.")]
    pub timings: Option<String>,

    // ===== Dependency Options =====
    /// Ignore `rust-version` specification in packages
    #[arg(
        long,
        env = "IGNORE_RUST_VERSION",
        help_heading = "Dependency Options",
        long_help = "\
Ignore rust-version specification in packages. Allows building with older Rust versions."
    )]
    pub ignore_rust_version: bool,

    /// Assert that Cargo.lock will remain unchanged
    #[arg(
        long,
        env = "LOCKED",
        help_heading = "Dependency Options",
        long_help = "\
Assert Cargo.lock will remain unchanged. Exits with error if missing or needs updating. Use in CI."
    )]
    pub locked: bool,

    /// Run without accessing the network
    #[arg(
        long,
        env = "OFFLINE",
        help_heading = "Dependency Options",
        long_help = "\
Prevent network access. Uses locally cached data. Run 'cargo fetch' first if needed."
    )]
    pub offline: bool,

    /// Require Cargo.lock and cache are up to date (implies --locked --offline)
    #[arg(
        long,
        env = "FROZEN",
        help_heading = "Dependency Options",
        long_help = "\
Equivalent to --locked --offline. Requires Cargo.lock and cache are up to date."
    )]
    pub frozen: bool,

    /// Path to Cargo.lock (unstable)
    #[arg(long, env = "LOCKFILE_PATH", value_name = "PATH",
          value_hint = ValueHint::FilePath, help_heading = "Dependency Options",
          long_help = "\
Override lockfile path from default (<workspace_root>/Cargo.lock). Requires nightly.")]
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
Number of parallel jobs. Defaults to logical CPUs. Negative=CPUs+N. 'default' to reset."
    )]
    pub jobs: Option<String>,

    /// Build as many crates as possible, rather than aborting on first error
    #[arg(
        long,
        env = "KEEP_GOING",
        help_heading = "Build Configuration",
        long_help = "\
Build as many crates in the dependency graph as possible. Rather than aborting on the first
crate that fails to build, continue with other crates in the dependency graph."
    )]
    pub keep_going: bool,

    /// Output a future incompatibility report after the build
    #[arg(
        long,
        env = "FUTURE_INCOMPAT_REPORT",
        help_heading = "Build Configuration",
        long_help = "\
Displays a future-incompat report for any future-incompatible warnings produced during
execution of this command. See 'cargo report' for more information."
    )]
    pub future_incompat_report: bool,

    // ===== Test and Benchmark Options =====
    /// Compile tests or benchmarks without running them
    #[arg(long, help_heading = "Test and Benchmark Options")]
    pub no_run: bool,

    /// Continue running all tests or benchmarks after a failure
    #[arg(long, help_heading = "Test and Benchmark Options")]
    pub no_fail_fast: bool,

    /// Test only this package's library documentation
    #[arg(long = "doc", help_heading = "Test and Benchmark Options")]
    pub doc_tests: bool,

    // ===== Additional Cargo Arguments =====
    /// Additional arguments to pass to cargo
    /// Note: `CARGO_ARGS` env var is handled manually in cargo.rs to support shell-style parsing
    #[arg(
        long,
        visible_alias = "args",
        value_name = "ARGS",
        hide = true,
        allow_hyphen_values = true,
        action = clap::ArgAction::Append,
        help_heading = "Additional Options"
    )]
    pub cargo_args: Vec<String>,

    /// Unstable (nightly-only) flags to Cargo
    #[arg(short = 'Z', value_name = "FLAG",
          action = clap::ArgAction::Append, help_heading = "Additional Options",
          long_help = "\
Unstable (nightly-only) flags to Cargo. Run 'cargo -Z help' for details on available flags.
Common flags: build-std, unstable-options")]
    pub cargo_z_flags: Vec<String>,

    /// Override a Cargo configuration value or load a configuration file
    #[arg(long = "config", value_name = "KEY=VALUE|PATH",
          action = clap::ArgAction::Append, help_heading = "Additional Options",
          long_help = "\
Override a Cargo configuration value or load a Cargo configuration file. Configuration values
use TOML syntax in KEY=VALUE form; file paths point to extra Cargo configuration files.
This flag may be specified multiple times.
Examples: --config 'build.jobs=4' --config path/to/cargo-config.toml")]
    pub cargo_config: Vec<String>,

    /// Change to directory before doing anything
    #[arg(short = 'C', long = "directory", env = "CARGO_CWD",
          value_name = "DIR", value_hint = ValueHint::DirPath,
          help_heading = "Additional Options",
          long_help = "\
Changes the current working directory before executing any specified operations.
This affects where cargo looks for the project manifest (Cargo.toml) and .cargo/config.toml.")]
    pub cargo_cwd: Option<PathBuf>,

    /// Rust toolchain to use (alternative to +toolchain syntax)
    #[arg(
        long = "toolchain",
        env = "TOOLCHAIN",
        value_name = "TOOLCHAIN",
        help_heading = "Additional Options",
        long_help = "\
Specify the Rust toolchain to use for compilation. This is an alternative to the +toolchain
syntax (e.g., +nightly). Examples: --toolchain nightly, --toolchain stable, --toolchain 1.75.0"
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
Clean the target directory before building. Equivalent to running 'cargo clean' before the build."
    )]
    pub clean_cache: bool,

    /// Disable automatic --target appending for `exec` cargo commands
    #[arg(
        long,
        env = "NO_APPEND_TARGET",
        help_heading = "Additional Options",
        long_help = "\
Disable automatic insertion of '--target <triple>' when using the 'exec' command
with a cargo invocation. By default, 'cargo cross exec --target ... -- cargo ...'
will add '--target <triple>' to the cargo command unless it is already present."
    )]
    pub no_append_target: bool,

    /// Arguments passed through to cargo (after --)
    /// Note: `CARGO_PASSTHROUGH_ARGS` env var is handled manually in cargo.rs to support shell-style parsing
    #[arg(
        last = true,
        allow_hyphen_values = true,
        value_name = "ARGS",
        help = "Arguments passed through to the underlying cargo command",
        long_help = "\
Arguments passed through to the underlying cargo command. Everything after -- is passed
directly to cargo/test runner. For test command, these are passed to the test binary.
Examples: test -- --nocapture --test-threads=1, run -- --arg1 --arg2"
    )]
    pub passthrough_args: Vec<String>,
}

impl BuildArgs {
    /// Create default `BuildArgs` with proper version defaults
    #[must_use]
    pub fn default_for_host() -> Self {
        Self {
            glibc_version: DEFAULT_GLIBC_VERSION.to_string(),
            iphone_sdk_version: DEFAULT_IPHONE_SDK_VERSION.to_string(),
            macos_sdk_version: DEFAULT_MACOS_SDK_VERSION.to_string(),
            freebsd_version: DEFAULT_FREEBSD_VERSION.to_string(),
            ndk_version: DEFAULT_NDK_VERSION.to_string(),
            qemu_version: DEFAULT_QEMU_VERSION.to_string(),
            cross_make_version: DEFAULT_CROSS_MAKE_VERSION.to_string(),
            ..Default::default()
        }
    }
}

/// Parse optional bool value (true/false)
fn parse_optional_bool(s: &str) -> std::result::Result<bool, String> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(format!("invalid bool value: {s}")),
    }
}

/// Parse build-std value (returns empty string for disabled, which is filtered later)
fn parse_build_std(s: &str) -> std::result::Result<String, String> {
    match s.to_lowercase().as_str() {
        "false" | "0" | "no" | "" => Ok(String::new()), // Will be converted to None later
        "true" | "1" | "yes" => Ok("true".to_string()),
        _ => Ok(s.to_string()),
    }
}

/// Cargo or internal command descriptor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command(String);

impl Default for Command {
    fn default() -> Self {
        Self::build()
    }
}

impl Command {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    #[must_use]
    pub fn build() -> Self {
        Self::new("build")
    }

    #[must_use]
    pub fn check() -> Self {
        Self::new("check")
    }

    #[must_use]
    pub fn run() -> Self {
        Self::new("run")
    }

    #[must_use]
    pub fn test() -> Self {
        Self::new("test")
    }

    #[must_use]
    pub fn bench() -> Self {
        Self::new("bench")
    }

    #[must_use]
    pub fn clippy() -> Self {
        Self::new("clippy")
    }

    #[must_use]
    pub fn setup() -> Self {
        Self::new("setup")
    }

    #[must_use]
    pub fn exec() -> Self {
        Self::new("exec")
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn needs_runner(&self) -> bool {
        matches!(self.as_str(), "run" | "test" | "bench")
    }
}

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
    /// Force runner setup even when the command does not normally execute target binaries
    pub init_runner: bool,
    /// Cross-make version for toolchain downloads
    pub cross_make_version: String,
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
    /// Create Args from `BuildArgs` and Command
    fn from_build_args(b: BuildArgs, command: Command, toolchain: Option<String>) -> Result<Self> {
        let raw_targets = if b.targets.is_empty() {
            configured_targets(&b)?
        } else {
            b.targets.clone()
        };
        let cross_compiler_dir = b
            .cross_compiler_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("rust-cross-compiler"));
        let targets = expand_target_list(&raw_targets)?;

        Ok(Self {
            toolchain,
            command,
            targets,
            no_cargo_target: false,
            init_runner: false,
            cross_make_version: b.cross_make_version.clone(),
            cross_compiler_dir,
            build: b,
        })
    }
}

/// Result of parsing CLI arguments
pub enum ParseResult {
    /// Normal build/check/run/test/bench command
    Build(Box<Args>),
    /// Print configured environment variables
    Setup(Box<SetupArgs>),
    /// Execute an arbitrary command after environment setup
    Exec(Box<ExecArgs>),
    /// Show targets command
    ShowTargets(OutputFormat),
    /// Show version
    ShowVersion,
}

/// Remove empty environment variables that clap would incorrectly treat as having values.
/// Clap's `env = "VAR"` attribute treats empty strings as valid values, which causes
/// parsing errors for `PathBuf` and other types that don't accept empty strings.
fn sanitize_clap_env() {
    let empty_vars: Vec<_> = std::env::vars()
        .filter(|(_, v)| v.is_empty())
        .map(|(k, _)| k)
        .collect();

    for var in empty_vars {
        std::env::remove_var(&var);
    }
}

/// Parse command-line arguments
pub fn parse_args() -> Result<ParseResult> {
    let args: Vec<String> = std::env::args().collect();
    parse_args_from(args)
}

#[derive(Parser, Debug)]
#[command(name = "cargo-cross")]
#[command(styles = cli_styles())]
struct ExternalCargoCli {
    #[command(flatten)]
    build: BuildArgs,
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
        && args.get(1).map(String::as_str) == Some(SUBCOMMAND);

    let skip_count = if is_cargo_subcommand { 2 } else { 1 };
    let mut args: Vec<String> = args.iter().skip(skip_count).cloned().collect();

    // Extract +toolchain from args (can appear at the beginning)
    // e.g., cargo cross +nightly build -t x86_64-unknown-linux-musl
    if let Some(tc) = args.first().and_then(|a| a.strip_prefix('+')) {
        toolchain = Some(tc.to_string());
        args.remove(0);
    }

    normalize_leading_cargo_options(&mut args);

    if let Some(command_name) = args.first().cloned() {
        let remaining_args: Vec<String> = args.iter().skip(1).cloned().collect();
        let help_or_version_requested = has_wrapper_help_or_version_request(&remaining_args);

        if let Some(canonical_name) = canonical_cargo_command_name(&command_name) {
            if !help_or_version_requested || is_supported_external_cargo_command(canonical_name) {
                return parse_cargo_command_args(
                    &command_name,
                    canonical_name,
                    remaining_args,
                    toolchain,
                );
            }
        }
        if should_parse_as_external_cargo_command(&command_name) {
            return parse_cargo_command_args(
                &command_name,
                &command_name,
                remaining_args,
                toolchain,
            );
        }
    }

    // Prepend program name for clap (internal name, not displayed)
    args.insert(0, BIN_NAME.to_string());

    // Build command with dynamic help text
    let cmd = build_command_with_dynamic_help();

    // Try to parse with clap using modified command
    let cli = match cmd.try_get_matches_from(&args) {
        Ok(matches) => {
            Cli::from_arg_matches(&matches).map_err(|e| CrossError::ClapError(e.to_string()))?
        }
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

/// Cargo accepts its global options before the command. Move those options after
/// supported Cargo commands so they can share the existing `BuildArgs` parser.
fn normalize_leading_cargo_options(args: &mut Vec<String>) {
    let mut index = 0;

    while index < args.len() {
        let token = args[index].as_str();

        if let Some(option) = classify_cargo_global_option(token) {
            index += match option {
                CargoGlobalOption::Standalone => 1,
                CargoGlobalOption::TakesNextValue => 2,
            };
            if index <= args.len() {
                continue;
            }
            return;
        }

        if index > 0
            && (canonical_cargo_command_name(token).is_some()
                || is_supported_external_cargo_command(token))
        {
            let command = args.remove(index);
            args.insert(0, command);
        }
        return;
    }
}

#[derive(Clone, Copy)]
enum CargoGlobalOption {
    Standalone,
    TakesNextValue,
}

fn classify_cargo_global_option(arg: &str) -> Option<CargoGlobalOption> {
    if matches!(arg, "--config" | "--color" | "--directory") {
        return Some(CargoGlobalOption::TakesNextValue);
    }
    if arg.starts_with("--config=")
        || arg.starts_with("--color=")
        || arg.starts_with("--directory=")
        || matches!(
            arg,
            "--verbose" | "--quiet" | "-q" | "--locked" | "--offline" | "--frozen"
        )
    {
        return Some(CargoGlobalOption::Standalone);
    }

    let cluster = arg
        .strip_prefix('-')
        .filter(|cluster| !cluster.is_empty())?;
    if cluster.starts_with('-') {
        return None;
    }

    let mut shorts = cluster.chars().peekable();
    while let Some(short) = shorts.next() {
        match short {
            'v' | 'q' => {}
            'C' | 'Z' => {
                return Some(if shorts.peek().is_some() {
                    CargoGlobalOption::Standalone
                } else {
                    CargoGlobalOption::TakesNextValue
                });
            }
            _ => return None,
        }
    }

    Some(CargoGlobalOption::Standalone)
}

fn has_wrapper_help_or_version_request(args: &[String]) -> bool {
    args.iter()
        .take_while(|arg| arg.as_str() != "--")
        .any(|arg| matches!(arg.as_str(), "--help" | "-h" | "--version" | "-V"))
}

/// Build the clap Command with dynamic help text based on invocation style
fn build_command_with_dynamic_help() -> clap::Command {
    let prog = program_name();

    // Build dynamic help strings
    let usage = format!("{prog} [+toolchain] [CARGO_OPTIONS] <COMMAND> [OPTIONS]");
    let after_help = format!(
        "Use '{prog} <COMMAND> --help' for more information about a command.\n\n\
TOOLCHAIN:\n    \
    If the first argument begins with +, it will be interpreted as a Rust toolchain\n    \
    name (such as +nightly, +stable, or +1.75.0). This follows the same convention\n    \
    as rustup and cargo.\n\n\
CARGO OPTIONS:\n    \
    Cargo global options can precede supported Cargo commands: -v, -q, --color,\n    \
    -C, --locked, --offline, --frozen, --config, and -Z.\n\n\
EXAMPLES:\n    \
    {prog} build -t x86_64-unknown-linux-musl\n    \
    {prog} --config build.jobs=4 build\n    \
    {prog} +nightly build -t aarch64-unknown-linux-gnu --profile release\n    \
    {prog} build -t '*-linux-musl' --crt-static true\n    \
    {prog} test -t x86_64-unknown-linux-musl -- --nocapture"
    );

    // Build dynamic version help texts
    let glibc_help = format!(
        "Specify glibc version for GNU libc targets. The version determines the minimum Linux kernel\n\
         version required. Lower versions provide better compatibility with older systems.\n\
         Supported: {}",
        supported_glibc_versions_str()
    );
    let freebsd_help = format!(
        "Specify FreeBSD version for FreeBSD targets. Supported: {}",
        supported_freebsd_versions_str()
    );
    let iphone_sdk_help = format!(
        "Specify iPhone SDK version for iOS targets. On Linux: uses pre-built SDK from releases.\n\
         On macOS: uses installed Xcode SDK. Supported on Linux: {}",
        supported_iphone_sdk_versions_str()
    );
    let macos_sdk_help = format!(
        "Specify macOS SDK version for Darwin targets. On Linux: uses osxcross with pre-built SDK.\n\
         On macOS: uses installed Xcode SDK. Supported on Linux: {}",
        supported_macos_sdk_versions_str()
    );

    // Get base command and modify it
    let mut cmd = Cli::command().override_usage(usage).after_help(after_help);

    // Update subcommand usage strings and version help texts
    for subcmd_name in &["build", "check", "run", "test", "bench", "clippy"] {
        let glibc_help = glibc_help.clone();
        let freebsd_help = freebsd_help.clone();
        let iphone_sdk_help = iphone_sdk_help.clone();
        let macos_sdk_help = macos_sdk_help.clone();
        cmd = cmd.mut_subcommand(*subcmd_name, |subcmd| {
            let subcmd = subcmd
                .override_usage(format!(
                    "{prog} [+toolchain] {subcmd_name} [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
                ))
                .mut_arg("glibc_version", |arg| arg.long_help(glibc_help))
                .mut_arg("freebsd_version", |arg| arg.long_help(freebsd_help))
                .mut_arg("iphone_sdk_version", |arg| arg.long_help(iphone_sdk_help))
                .mut_arg("macos_sdk_version", |arg| arg.long_help(macos_sdk_help));

            match *subcmd_name {
                "test" => subcmd,
                "bench" => subcmd.mut_arg("doc_tests", |arg| arg.hide(true)),
                _ => subcmd
                    .mut_arg("no_run", |arg| arg.hide(true))
                    .mut_arg("no_fail_fast", |arg| arg.hide(true))
                    .mut_arg("doc_tests", |arg| arg.hide(true)),
            }
        });
    }

    cmd
}

fn should_parse_as_external_cargo_command(command_name: &str) -> bool {
    if command_name.starts_with('-') {
        return false;
    }

    canonical_cargo_command_name(command_name).is_none()
        && is_supported_external_cargo_command(command_name)
}

fn canonical_cargo_command_name(command_name: &str) -> Option<&'static str> {
    match command_name {
        "build" | "b" => Some("build"),
        "check" | "c" => Some("check"),
        "run" | "r" => Some("run"),
        "test" | "t" => Some("test"),
        "bench" => Some("bench"),
        "clippy" => Some("clippy"),
        "doc" | "d" => Some("doc"),
        _ => None,
    }
}

fn is_supported_external_cargo_command(command_name: &str) -> bool {
    matches!(command_name, "doc" | "fix" | "rustc" | "rustdoc")
}

fn parse_cargo_command_args(
    display_name: &str,
    canonical_name: &str,
    args: Vec<String>,
    toolchain: Option<String>,
) -> Result<ParseResult> {
    let processed_args = preprocess_cargo_args(args);
    let mut clap_args = Vec::with_capacity(processed_args.len() + 1);
    clap_args.push(BIN_NAME.to_string());
    clap_args.extend(processed_args);

    let cmd = build_external_cargo_command_with_dynamic_help(display_name, canonical_name);
    let cli = match cmd.try_get_matches_from(&clap_args) {
        Ok(matches) => ExternalCargoCli::from_arg_matches(&matches)
            .map_err(|e| CrossError::ClapError(e.to_string()))?,
        Err(e) => {
            if matches!(
                e.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                e.exit();
            }
            return Err(CrossError::ClapError(e.render().to_string()));
        }
    };

    let args = finalize_args(cli.build, Command::new(canonical_name), toolchain)?;
    Ok(ParseResult::Build(Box::new(args)))
}

#[derive(Clone, Copy, Debug)]
struct KnownArgSpec {
    takes_value: bool,
    min_values: usize,
    max_values: usize,
    allow_hyphen_values: bool,
}

fn preprocess_cargo_args(args: Vec<String>) -> Vec<String> {
    let parser = ExternalCargoCli::command();
    let (long_args, short_args) = known_arg_maps(&parser);
    let mut processed = Vec::with_capacity(args.len());
    let mut pending_value: Option<KnownArgSpec> = None;
    let mut passthrough = false;
    let mut index = 0;

    while index < args.len() {
        let token = &args[index];

        if passthrough {
            processed.push(token.clone());
            index += 1;
            continue;
        }

        if token == "--" {
            pending_value = None;
            passthrough = true;
            processed.push(token.clone());
            index += 1;
            continue;
        }

        if matches!(token.as_str(), "--help" | "-h" | "--version" | "-V") {
            pending_value = None;
            processed.push(token.clone());
            index += 1;
            continue;
        }

        if let Some(spec) = pending_value {
            if should_consume_known_value(token, spec) {
                processed.push(token.clone());
                pending_value = None;
                index += 1;
                continue;
            }
            pending_value = None;
        }

        if let Some(spec) = classify_known_long_arg(token, &long_args) {
            processed.push(token.clone());
            pending_value = needs_following_value(token, spec);
            index += 1;
            continue;
        }

        if let Some(short_arg) = classify_known_short_arg(token, &short_args) {
            processed.push(token.clone());
            pending_value = if short_arg.spec.takes_value && !short_arg.has_inline_value {
                Some(short_arg.spec)
            } else {
                None
            };
            index += 1;
            continue;
        }

        push_cargo_arg(&mut processed, token);
        if should_pair_unknown_value(token, args.get(index + 1).map(String::as_str)) {
            if let Some(value) = args.get(index + 1) {
                push_cargo_arg(&mut processed, value);
                index += 1;
            }
        }
        index += 1;
    }

    processed
}

fn known_arg_maps(
    command: &clap::Command,
) -> (HashMap<String, KnownArgSpec>, HashMap<char, KnownArgSpec>) {
    let mut long_args = HashMap::new();
    let mut short_args = HashMap::new();

    for arg in command.get_arguments() {
        if arg.is_positional() {
            continue;
        }

        let (takes_value, min_values, max_values) = infer_arg_value_shape(arg);
        let spec = KnownArgSpec {
            takes_value,
            min_values,
            max_values,
            allow_hyphen_values: arg.is_allow_hyphen_values_set(),
        };

        if let Some(long) = arg.get_long() {
            long_args.insert(long.to_string(), spec);
        }
        if let Some(aliases) = arg.get_long_and_visible_aliases() {
            for alias in aliases {
                long_args.insert(alias.to_string(), spec);
            }
        }
        if let Some(short) = arg.get_short() {
            short_args.insert(short, spec);
        }
        if let Some(aliases) = arg.get_short_and_visible_aliases() {
            for alias in aliases {
                short_args.insert(alias, spec);
            }
        }
    }

    (long_args, short_args)
}

fn infer_arg_value_shape(arg: &clap::Arg) -> (bool, usize, usize) {
    if let Some(range) = arg.get_num_args() {
        return (range.takes_values(), range.min_values(), range.max_values());
    }

    match arg.get_action() {
        ArgAction::Set | ArgAction::Append => (true, 1, 1),
        ArgAction::SetTrue
        | ArgAction::SetFalse
        | ArgAction::Count
        | ArgAction::Help
        | ArgAction::HelpShort
        | ArgAction::HelpLong
        | ArgAction::Version => (false, 0, 0),
        _ => (false, 0, 0),
    }
}

fn classify_known_long_arg(
    token: &str,
    long_args: &HashMap<String, KnownArgSpec>,
) -> Option<KnownArgSpec> {
    if !token.starts_with("--") || token == "--" {
        return None;
    }

    let name = token
        .trim_start_matches("--")
        .split_once('=')
        .map_or_else(|| token.trim_start_matches("--"), |(name, _)| name);
    long_args.get(name).copied()
}

struct KnownShortArg {
    spec: KnownArgSpec,
    has_inline_value: bool,
}

fn classify_known_short_arg(
    token: &str,
    short_args: &HashMap<char, KnownArgSpec>,
) -> Option<KnownShortArg> {
    if !token.starts_with('-') || token.starts_with("--") || token == "-" {
        return None;
    }

    let rest = token.strip_prefix('-')?;
    let mut chars = rest.chars().peekable();
    let mut first_spec = None;

    while let Some(short) = chars.next() {
        let spec = short_args.get(&short).copied()?;
        first_spec.get_or_insert(spec);
        if spec.takes_value {
            return Some(KnownShortArg {
                spec,
                has_inline_value: chars.peek().is_some(),
            });
        }
    }

    Some(KnownShortArg {
        spec: first_spec?,
        has_inline_value: false,
    })
}

fn needs_following_value(token: &str, spec: KnownArgSpec) -> Option<KnownArgSpec> {
    if !spec.takes_value || spec.max_values == 0 {
        return None;
    }

    let has_inline_value = if token.starts_with("--") {
        token.contains('=')
    } else {
        token.len() > 2
    };

    if has_inline_value {
        None
    } else {
        Some(spec)
    }
}

fn should_consume_known_value(token: &str, spec: KnownArgSpec) -> bool {
    if token == "--" {
        return false;
    }

    if spec.min_values == 0 && token.starts_with('-') && !spec.allow_hyphen_values {
        return false;
    }

    !token.starts_with('-') || spec.allow_hyphen_values || spec.min_values > 0
}

fn should_pair_unknown_value(current: &str, next: Option<&str>) -> bool {
    if current == "-" || current == "--" {
        return false;
    }

    if !(current.starts_with("--") || (current.starts_with('-') && current.len() == 2)) {
        return false;
    }

    let Some(next) = next else {
        return false;
    };

    !next.starts_with('-')
}

fn push_cargo_arg(processed: &mut Vec<String>, value: &str) {
    processed.push("--cargo-args".to_string());
    processed.push(value.to_string());
}

fn build_external_cargo_command_with_dynamic_help(
    display_name: &str,
    canonical_name: &str,
) -> clap::Command {
    let prog = program_name();
    let after_help = format!(
        "This command forwards to 'cargo {canonical_name}' after configuring the\n\
cross-compilation environment.\n\n\
EXAMPLES:\n    \
{prog} {display_name} -t x86_64-unknown-linux-musl\n    \
{prog} {display_name} -t aarch64-unknown-linux-gnu -- --help"
    );

    let command = ExternalCargoCli::command()
        .override_usage(format!(
            "{prog} [+toolchain] {display_name} [OPTIONS] [-- <PASSTHROUGH_ARGS>...]"
        ))
        .after_help(after_help);

    match canonical_name {
        "test" | "t" => command,
        "bench" => command.mut_arg("doc_tests", |arg| arg.hide(true)),
        _ => command
            .mut_arg("no_run", |arg| arg.hide(true))
            .mut_arg("no_fail_fast", |arg| arg.hide(true))
            .mut_arg("doc_tests", |arg| arg.hide(true)),
    }
}

fn process_cli(cli: Cli, toolchain: Option<String>) -> Result<ParseResult> {
    match cli.command {
        CliCommand::Build(args) => {
            let args = finalize_args(args, Command::build(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Check(args) => {
            let args = finalize_args(args, Command::check(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Run(args) => {
            let args = finalize_args(args, Command::run(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Test(args) => {
            let args = finalize_args(args, Command::test(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Bench(args) => {
            let args = finalize_args(args, Command::bench(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Clippy(args) => {
            let args = finalize_args(args, Command::clippy(), toolchain)?;
            Ok(ParseResult::Build(Box::new(args)))
        }
        CliCommand::Setup(setup) => {
            let args = finalize_args(setup.build, Command::setup(), toolchain)?;
            let mut args = args;
            args.init_runner = setup.init_runner;
            Ok(ParseResult::Setup(Box::new(SetupArgs {
                args,
                format: setup.format,
            })))
        }
        CliCommand::Exec(exec) => {
            let mut build = exec.build;
            populate_env_arg_fallbacks(&mut build);
            let command = std::mem::take(&mut build.passthrough_args);
            if command.is_empty() {
                return Err(CrossError::InvalidArgument(
                    "exec requires a command after `--`".to_string(),
                ));
            }
            let args = finalize_args(build, Command::exec(), toolchain)?;
            let mut args = args;
            args.init_runner = exec.init_runner;
            Ok(ParseResult::Exec(Box::new(ExecArgs { args, command })))
        }
        CliCommand::Targets(args) => Ok(ParseResult::ShowTargets(args.format)),
        CliCommand::Version => Ok(ParseResult::ShowVersion),
    }
}

/// Check if a string is a glob pattern (contains *, ?, or [)
fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Validate that a target triple only contains valid characters (a-z, 0-9, -, _)
fn validate_target_triple(target: &str) -> Result<()> {
    if config::is_custom_target_path(target) {
        return Ok(());
    }

    for c in target.chars() {
        if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' && c != '_' {
            return Err(CrossError::InvalidTargetTriple {
                target: target.to_string(),
                char: c,
            });
        }
    }
    Ok(())
}

/// Expand target list, handling glob patterns
fn expand_target_list(targets: &[String]) -> Result<Vec<String>> {
    let mut result = Vec::new();
    for target in targets {
        let target = target.trim();
        if config::is_custom_target_path(target) {
            validate_target_triple(target)?;
            if !result.iter().any(|existing| existing == target) {
                result.push(target.to_string());
            }
            continue;
        }

        // Split by comma or newline to support multiple delimiters
        for part in target.split([',', '\n']) {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let expanded = config::expand_targets(part);
            if expanded.is_empty() {
                // If it was a glob pattern that matched nothing, error
                if is_glob_pattern(part) {
                    return Err(CrossError::NoMatchingTargets {
                        pattern: part.to_string(),
                    });
                }
                // Not a glob pattern, validate and use as-is
                validate_target_triple(part)?;
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
    Ok(result)
}

fn configured_targets(args: &BuildArgs) -> Result<Vec<String>> {
    let cargo_build_target = nonempty_env("CARGO_BUILD_TARGET");
    configured_targets_with_env(args, cargo_build_target.as_deref())
}

#[derive(Debug, PartialEq, Eq)]
enum ConfiguredTargets {
    Scalar(String),
    Array(Vec<String>),
    Environment(String),
}

impl ConfiguredTargets {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::Scalar(target) | Self::Environment(target) => vec![target],
            Self::Array(targets) => targets,
        }
    }
}

fn configured_targets_with_env(
    args: &BuildArgs,
    cargo_build_target: Option<&str>,
) -> Result<Vec<String>> {
    let cwd = effective_cargo_cwd(args)?;
    let mut targets = None;

    for path in discovered_config_paths(&cwd) {
        if let Some(config_targets) = target_from_config_file(&path)? {
            targets = Some(merge_config_targets(targets, config_targets)?);
        }
    }

    if let Some(target) = cargo_build_target {
        targets = Some(merge_environment_target(
            targets,
            normalize_config_target(target, &cwd),
        ));
    }

    for config_arg in &args.cargo_config {
        if let Some(config_targets) = target_from_config_arg(config_arg, &cwd)? {
            targets = Some(merge_config_targets(targets, config_targets)?);
        }
    }

    Ok(targets.map(ConfiguredTargets::into_vec).unwrap_or_default())
}

fn merge_config_targets(
    current: Option<ConfiguredTargets>,
    incoming: ConfiguredTargets,
) -> Result<ConfiguredTargets> {
    match (current, incoming) {
        (None, incoming) => Ok(incoming),
        (Some(ConfiguredTargets::Scalar(_)), ConfiguredTargets::Scalar(target))
        | (Some(ConfiguredTargets::Environment(_)), ConfiguredTargets::Scalar(target)) => {
            Ok(ConfiguredTargets::Scalar(target))
        }
        (Some(ConfiguredTargets::Array(mut current)), ConfiguredTargets::Array(incoming)) => {
            current.extend(incoming);
            Ok(ConfiguredTargets::Array(current))
        }
        (Some(ConfiguredTargets::Environment(target)), ConfiguredTargets::Array(incoming)) => {
            let mut targets = Vec::with_capacity(incoming.len() + 1);
            targets.push(target);
            targets.extend(incoming);
            Ok(ConfiguredTargets::Array(targets))
        }
        (Some(ConfiguredTargets::Scalar(_)), ConfiguredTargets::Array(_)) => {
            Err(CrossError::InvalidArgument(
                "failed to merge Cargo build.target: expected string, found array".to_string(),
            ))
        }
        (Some(ConfiguredTargets::Array(_)), ConfiguredTargets::Scalar(_)) => {
            Err(CrossError::InvalidArgument(
                "failed to merge Cargo build.target: expected array, found string".to_string(),
            ))
        }
        (_, ConfiguredTargets::Environment(_)) => unreachable!("environment target is merged once"),
    }
}

fn merge_environment_target(
    current: Option<ConfiguredTargets>,
    target: String,
) -> ConfiguredTargets {
    match current {
        None => ConfiguredTargets::Environment(target),
        Some(ConfiguredTargets::Scalar(_)) | Some(ConfiguredTargets::Environment(_)) => {
            ConfiguredTargets::Scalar(target)
        }
        Some(ConfiguredTargets::Array(mut targets)) => {
            targets.push(target);
            ConfiguredTargets::Array(targets)
        }
    }
}

fn effective_cargo_cwd(args: &BuildArgs) -> Result<PathBuf> {
    let current = std::env::current_dir().map_err(|source| CrossError::IoError {
        message: "failed to determine current directory".to_string(),
        source,
    })?;
    Ok(match &args.cargo_cwd {
        Some(path) if path.is_absolute() => path.clone(),
        Some(path) => current.join(path),
        None => current,
    })
}

fn discovered_config_paths(cwd: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(cargo_home) = cargo_home() {
        if let Some(path) = config_file_in(&cargo_home) {
            paths.push(path);
        }
    }

    let mut ancestors: Vec<_> = cwd.ancestors().collect();
    ancestors.reverse();
    for ancestor in ancestors {
        if let Some(path) = config_file_in(&ancestor.join(".cargo")) {
            if !paths.contains(&path) {
                paths.push(path);
            }
        }
    }

    paths
}

fn cargo_home() -> Option<PathBuf> {
    cargo_home_from_env(
        nonempty_env("CARGO_HOME"),
        nonempty_env("HOME"),
        nonempty_env("USERPROFILE"),
        cfg!(windows),
    )
}

fn cargo_home_from_env(
    cargo_home: Option<String>,
    home: Option<String>,
    user_profile: Option<String>,
    windows: bool,
) -> Option<PathBuf> {
    cargo_home.map(PathBuf::from).or_else(|| {
        let home = if windows { user_profile.or(home) } else { home };
        home.map(|home| PathBuf::from(home).join(".cargo"))
    })
}

fn config_file_in(directory: &Path) -> Option<PathBuf> {
    let legacy = directory.join("config");
    if legacy.is_file() {
        return Some(legacy);
    }

    let toml = directory.join("config.toml");
    toml.is_file().then_some(toml)
}

fn target_from_config_arg(config_arg: &str, cwd: &Path) -> Result<Option<ConfiguredTargets>> {
    let path = Path::new(config_arg);
    let resolved_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    if resolved_path.is_file() || !config_arg.contains('=') {
        return target_from_config_file(&resolved_path);
    }

    let value: toml::Value = toml::from_str(config_arg).map_err(|error| {
        CrossError::InvalidArgument(format!(
            "invalid Cargo --config value '{config_arg}': {error}"
        ))
    })?;
    target_from_config_value(&value, cwd, cwd, &mut Vec::new())
}

fn target_from_config_file(path: &Path) -> Result<Option<ConfiguredTargets>> {
    target_from_config_file_recursive(path, &mut Vec::new())
}

fn target_from_config_file_recursive(
    path: &Path,
    include_stack: &mut Vec<PathBuf>,
) -> Result<Option<ConfiguredTargets>> {
    let resolved_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if include_stack.contains(&resolved_path) {
        return Err(CrossError::InvalidArgument(format!(
            "cyclic Cargo config include involving {}",
            resolved_path.display()
        )));
    }
    include_stack.push(resolved_path.clone());

    let result = read_config_targets(&resolved_path, include_stack);
    include_stack.pop();
    result
}

fn read_config_targets(
    path: &Path,
    include_stack: &mut Vec<PathBuf>,
) -> Result<Option<ConfiguredTargets>> {
    let content = std::fs::read_to_string(path).map_err(|source| CrossError::IoError {
        message: format!("failed to read Cargo config {}", path.display()),
        source,
    })?;
    let value: toml::Value = toml::from_str(&content).map_err(|error| {
        CrossError::InvalidArgument(format!("invalid Cargo config {}: {error}", path.display()))
    })?;
    target_from_config_value(
        &value,
        path.parent().unwrap_or_else(|| Path::new(".")),
        &config_base_dir(path),
        include_stack,
    )
}

fn config_base_dir(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    parent.parent().unwrap_or(parent).to_path_buf()
}

#[derive(Debug)]
struct ConfigInclude {
    path: PathBuf,
    optional: bool,
}

fn target_from_config_value(
    value: &toml::Value,
    include_base: &Path,
    target_base: &Path,
    include_stack: &mut Vec<PathBuf>,
) -> Result<Option<ConfiguredTargets>> {
    let mut targets = None;

    for include in config_includes(value)? {
        let include_path = if include.path.is_absolute() {
            include.path
        } else {
            include_base.join(include.path)
        };
        if include.optional && !include_path.exists() {
            continue;
        }
        if let Some(included_targets) =
            target_from_config_file_recursive(&include_path, include_stack)?
        {
            targets = Some(merge_config_targets(targets, included_targets)?);
        }
    }

    if let Some(own_targets) = target_from_toml(value, target_base)? {
        targets = Some(merge_config_targets(targets, own_targets)?);
    }

    Ok(targets)
}

fn config_includes(value: &toml::Value) -> Result<Vec<ConfigInclude>> {
    let Some(include) = value.get("include") else {
        return Ok(Vec::new());
    };
    let Some(entries) = include.as_array() else {
        return Err(CrossError::InvalidArgument(
            "Cargo config include must be an array of strings or tables".to_string(),
        ));
    };

    if entries.iter().all(toml::Value::is_str) {
        return Ok(entries
            .iter()
            .filter_map(toml::Value::as_str)
            .map(|path| ConfigInclude {
                path: PathBuf::from(path),
                optional: false,
            })
            .collect());
    }
    if !entries.iter().all(toml::Value::is_table) {
        return Err(CrossError::InvalidArgument(
            "Cargo config include must contain only strings or only tables".to_string(),
        ));
    }

    entries
        .iter()
        .map(|entry| {
            let table = entry.as_table().expect("validated as a table");
            let path = table
                .get("path")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    CrossError::InvalidArgument(
                        "Cargo config include tables require a string path".to_string(),
                    )
                })?;
            let optional = match table.get("optional") {
                Some(value) => value.as_bool().ok_or_else(|| {
                    CrossError::InvalidArgument(
                        "Cargo config include optional value must be a boolean".to_string(),
                    )
                })?,
                None => false,
            };
            Ok(ConfigInclude {
                path: PathBuf::from(path),
                optional,
            })
        })
        .collect()
}

fn target_from_toml(value: &toml::Value, base: &Path) -> Result<Option<ConfiguredTargets>> {
    let Some(target) = value.get("build").and_then(|build| build.get("target")) else {
        return Ok(None);
    };

    let targets = match target {
        toml::Value::String(target) => {
            ConfiguredTargets::Scalar(normalize_config_target(target, base))
        }
        toml::Value::Array(targets) => ConfiguredTargets::Array(
            targets
                .iter()
                .map(|target| {
                    target
                        .as_str()
                        .map(|target| normalize_config_target(target, base))
                        .ok_or_else(|| {
                            CrossError::InvalidArgument(
                                "Cargo build.target arrays must contain only strings".to_string(),
                            )
                        })
                })
                .collect::<Result<Vec<_>>>()?,
        ),
        _ => {
            return Err(CrossError::InvalidArgument(
                "Cargo build.target must be a string or an array of strings".to_string(),
            ));
        }
    };

    Ok(Some(targets))
}

fn normalize_config_target(target: &str, base: &Path) -> String {
    let path = Path::new(target);
    if config::is_custom_target_path(target) && path.is_relative() {
        base.join(path).display().to_string()
    } else {
        target.to_string()
    }
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn finalize_args(
    mut build_args: BuildArgs,
    command: Command,
    toolchain: Option<String>,
) -> Result<Args> {
    validate_command_specific_args(&build_args, &command)?;

    // Handle --release flag: set profile to "release"
    if build_args.release {
        build_args.profile = Some("release".to_string());
    }

    // Handle build_std: empty string means disabled (from env var "false")
    if build_args
        .build_std
        .as_ref()
        .is_some_and(std::string::String::is_empty)
    {
        build_args.build_std = None;
    }

    populate_env_arg_fallbacks(&mut build_args);

    // Merge toolchain: +toolchain syntax takes precedence over --toolchain option
    let final_toolchain = toolchain.or_else(|| build_args.toolchain_option.clone());

    let mut args = Args::from_build_args(build_args, command, final_toolchain)?;

    // Validate versions
    validate_versions(&args)?;

    // Handle empty targets - default to host
    if args.targets.is_empty() {
        let host = config::HostPlatform::detect();
        args.targets.push(host.triple);
        args.no_toolchain_setup = true;
        args.no_cargo_target = true;
    }
    // Note: "host-tuple" is handled dynamically in execute_target

    Ok(args)
}

fn validate_command_specific_args(args: &BuildArgs, command: &Command) -> Result<()> {
    if (args.no_run || args.no_fail_fast) && !matches!(command.as_str(), "test" | "bench") {
        return Err(CrossError::InvalidArgument(format!(
            "--no-run and --no-fail-fast are supported by test and bench, not {}",
            command.as_str()
        )));
    }
    if args.doc_tests && command.as_str() != "test" {
        return Err(CrossError::InvalidArgument(format!(
            "--doc is supported by test, not {}",
            command.as_str()
        )));
    }
    Ok(())
}

fn populate_env_arg_fallbacks(build_args: &mut BuildArgs) {
    if build_args.cargo_args.is_empty() {
        if let Some(env_args) = parse_env_args("CARGO_ARGS") {
            build_args.cargo_args = env_args;
        }
    }
    if build_args.passthrough_args.is_empty() {
        if let Some(env_args) = parse_passthrough_env_args("CARGO_PASSTHROUGH_ARGS") {
            build_args.passthrough_args = env_args;
        }
    }
}

fn parse_passthrough_env_args(env_name: &str) -> Option<Vec<String>> {
    let mut args = parse_env_args(env_name)?;
    if args.first().is_some_and(|arg| arg == "--") {
        args.remove(0);
    }
    if args.is_empty() {
        None
    } else {
        Some(args)
    }
}

/// Parse environment variable as shell-style arguments using shlex
/// Returns None if env var is not set or empty
fn parse_env_args(env_name: &str) -> Option<Vec<String>> {
    let value = std::env::var(env_name).ok()?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    match shlex::split(value) {
        Some(parsed) if !parsed.is_empty() => Some(parsed),
        Some(_) => None,
        None => {
            eprintln!("Warning: Failed to parse {env_name} (mismatched quotes?): {value}");
            None
        }
    }
}

/// Validate version options
fn validate_versions(args: &Args) -> Result<()> {
    // Only validate glibc version if it's specified (non-empty)
    // Empty string means use default version, which is valid for both gnu and musl targets
    if !args.glibc_version.is_empty()
        && !SUPPORTED_GLIBC_VERSIONS.contains(&args.glibc_version.as_str())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args> {
        let args: Vec<String> = args.iter().map(std::string::ToString::to_string).collect();
        match parse_args_from(args)? {
            ParseResult::Build(args) => Ok(*args),
            ParseResult::ShowTargets(_) => panic!("unexpected ShowTargets"),
            ParseResult::Setup(_) => panic!("unexpected Setup"),
            ParseResult::Exec(_) => panic!("unexpected Exec"),
            ParseResult::ShowVersion => panic!("unexpected ShowVersion"),
        }
    }

    fn parse_setup(args: &[&str]) -> Result<SetupArgs> {
        let args: Vec<String> = args.iter().map(std::string::ToString::to_string).collect();
        match parse_args_from(args)? {
            ParseResult::Setup(args) => Ok(*args),
            _ => panic!("unexpected parse result"),
        }
    }

    fn parse_exec(args: &[&str]) -> Result<ExecArgs> {
        let args: Vec<String> = args.iter().map(std::string::ToString::to_string).collect();
        match parse_args_from(args)? {
            ParseResult::Exec(args) => Ok(*args),
            _ => panic!("unexpected parse result"),
        }
    }

    // Note: test_parse_empty_requires_subcommand removed because MissingSubcommand
    // now calls exit() which cannot be tested

    #[test]
    fn test_parse_build_command() {
        let args = parse(&["cargo-cross", "build"]).unwrap();
        assert_eq!(args.command, Command::build());
        assert_eq!(args.profile, None);
    }

    #[test]
    fn test_parse_check_command() {
        let args = parse(&["cargo-cross", "check"]).unwrap();
        assert_eq!(args.command, Command::check());
    }

    #[test]
    fn test_parse_clippy_command() {
        let args = parse(&["cargo-cross", "clippy"]).unwrap();
        assert_eq!(args.command, Command::clippy());
    }

    #[test]
    fn test_parse_clippy_unknown_cargo_flags() {
        let args = parse(&[
            "cargo-cross",
            "clippy",
            "--workspace",
            "--all-targets",
            "--fix",
            "--allow-dirty",
            "--target",
            "x86_64-pc-windows-msvc",
        ])
        .unwrap();
        assert!(args.workspace);
        assert!(args.build_all_targets);
        assert_eq!(
            args.cargo_args,
            vec!["--fix".to_string(), "--allow-dirty".to_string()]
        );
        assert_eq!(args.targets, vec!["x86_64-pc-windows-msvc"]);
    }

    #[test]
    fn test_parse_external_cargo_command() {
        let args = parse(&["cargo-cross", "doc"]).unwrap();
        assert_eq!(args.command, Command::new("doc"));
    }

    #[test]
    fn test_parse_doc_short_alias() {
        let args = parse(&["cargo-cross", "d"]).unwrap();
        assert_eq!(args.command, Command::new("doc"));
    }

    #[test]
    fn test_reject_non_build_like_external_cargo_command() {
        let result = parse(&["cargo-cross", "metadata"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_external_command_unknown_cargo_flags() {
        let args = parse(&[
            "cargo-cross",
            "doc",
            "--workspace",
            "--open",
            "--target",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.command, Command::new("doc"));
        assert!(args.workspace);
        assert_eq!(args.cargo_args, vec!["--open".to_string()]);
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_parse_test_specific_flags_and_filter() {
        let args = parse(&[
            "cargo-cross",
            "test",
            "--no-run",
            "name_filter",
            "--",
            "--nocapture",
        ])
        .unwrap();

        assert!(args.no_run);
        assert_eq!(args.cargo_args, vec!["name_filter"]);
        assert_eq!(args.passthrough_args, vec!["--nocapture"]);
    }

    #[test]
    fn test_parse_bench_specific_flags() {
        let args = parse(&[
            "cargo-cross",
            "bench",
            "--no-run",
            "--no-fail-fast",
            "--unit-graph",
        ])
        .unwrap();

        assert!(args.no_run);
        assert!(args.no_fail_fast);
        assert!(args.unit_graph);
    }

    #[test]
    fn test_reject_test_specific_flags_for_build() {
        let error = parse(&["cargo-cross", "build", "--no-run"]).unwrap_err();
        assert!(error.to_string().contains("test and bench"));
    }

    #[test]
    fn test_parse_setup_command() {
        let args =
            parse_setup(&["cargo-cross", "setup", "-t", "x86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.args.command, Command::setup());
        assert_eq!(args.format, SetupOutputFormat::Auto);
    }

    #[test]
    fn test_parse_setup_command_explicit_format() {
        let args = parse_setup(&[
            "cargo-cross",
            "setup",
            "-t",
            "x86_64-unknown-linux-musl",
            "--format",
            "fish",
        ])
        .unwrap();
        assert_eq!(args.format, SetupOutputFormat::Fish);
    }

    #[test]
    fn test_parse_setup_command_init_runner() {
        let args = parse_setup(&[
            "cargo-cross",
            "setup",
            "--init-runner",
            "-t",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();
        assert!(args.args.init_runner);
    }

    #[test]
    fn test_parse_exec_command() {
        let args = parse_exec(&[
            "cargo-cross",
            "exec",
            "-t",
            "x86_64-unknown-linux-musl",
            "--",
            "env",
            "FOO=bar",
        ])
        .unwrap();
        assert_eq!(args.args.command, Command::exec());
        assert_eq!(args.command, vec!["env", "FOO=bar"]);
    }

    #[test]
    fn test_parse_exec_command_init_runner() {
        let args = parse_exec(&[
            "cargo-cross",
            "exec",
            "--init-runner",
            "-t",
            "x86_64-unknown-linux-musl",
            "--",
            "env",
        ])
        .unwrap();
        assert!(args.args.init_runner);
        assert_eq!(args.command, vec!["env"]);
    }

    #[test]
    fn test_parse_exec_command_disable_auto_target() {
        let args = parse_exec(&[
            "cargo-cross",
            "exec",
            "--no-append-target",
            "-t",
            "x86_64-unknown-linux-musl",
            "--",
            "cargo",
            "clippy",
        ])
        .unwrap();
        assert!(args.args.no_append_target);
        assert_eq!(args.command, vec!["cargo", "clippy"]);
    }

    #[test]
    fn test_parse_exec_command_from_env_passthrough() {
        std::env::set_var("CARGO_PASSTHROUGH_ARGS", "-- env FOO=bar");
        let args = parse_exec(&["cargo-cross", "exec", "-t", "x86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.args.command, Command::exec());
        assert_eq!(args.command, vec!["env", "FOO=bar"]);
        std::env::remove_var("CARGO_PASSTHROUGH_ARGS");
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
    fn test_parse_custom_json_target_path() {
        let args = parse(&["cargo-cross", "build", "--target", "targets/MyBoard.json"]).unwrap();
        assert_eq!(args.targets, vec!["targets/MyBoard.json"]);
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
    fn test_parse_crt_static_no_value() {
        // --crt-static without value should default to true
        let args = parse(&["cargo-cross", "build", "--crt-static"]).unwrap();
        assert_eq!(args.crt_static, Some(true));
    }

    #[test]
    fn test_parse_crt_static_not_provided() {
        // When --crt-static is not provided at all, value should be None
        let args = parse(&["cargo-cross", "build"]).unwrap();
        assert_eq!(args.crt_static, None);
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
    fn test_parse_build_std_false() {
        let args = parse(&["cargo-cross", "build", "--build-std", "false"]).unwrap();
        assert_eq!(args.build_std, None);
    }

    #[test]
    fn test_parse_build_std_no_value() {
        // --build-std without value should default to "true"
        let args = parse(&["cargo-cross", "build", "--build-std"]).unwrap();
        assert_eq!(args.build_std, Some("true".to_string()));
    }

    #[test]
    fn test_parse_features() {
        let args = parse(&["cargo-cross", "build", "--features", "foo,bar"]).unwrap();
        assert_eq!(args.features, vec!["foo,bar"]);
    }

    #[test]
    fn test_parse_repeatable_cargo_selection_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-p",
            "first",
            "--package",
            "second",
            "-F",
            "serde",
            "--features",
            "json",
            "--bin",
            "one",
            "--bin",
            "two",
            "--example",
            "example-one",
            "--example",
            "example-two",
        ])
        .unwrap();

        assert_eq!(args.package, vec!["first", "second"]);
        assert_eq!(args.features, vec!["serde", "json"]);
        assert_eq!(args.bin_target, vec!["one", "two"]);
        assert_eq!(args.example_target, vec!["example-one", "example-two"]);
    }

    #[test]
    fn test_parse_no_default_features() {
        let args = parse(&["cargo-cross", "build", "--no-default-features"]).unwrap();
        assert!(args.no_default_features);
    }

    #[test]
    fn test_parse_profile() {
        let args = parse(&["cargo-cross", "build", "--profile", "dev"]).unwrap();
        assert_eq!(args.profile.as_deref(), Some("dev"));
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
    fn test_parse_passthrough_args_from_env_with_legacy_separator() {
        std::env::set_var("CARGO_PASSTHROUGH_ARGS", "-- --foo --bar");
        let args = parse(&["cargo-cross", "build"]).unwrap();
        assert_eq!(args.passthrough_args, vec!["--foo", "--bar"]);
        std::env::remove_var("CARGO_PASSTHROUGH_ARGS");
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
    fn test_parse_config_before_command() {
        let args = parse(&[
            "cargo-cross",
            "--config",
            "build.jobs=4",
            "--config=profile.release.lto=true",
            "build",
        ])
        .unwrap();
        assert_eq!(
            args.cargo_config,
            vec!["build.jobs=4", "profile.release.lto=true"]
        );
    }

    #[test]
    fn test_parse_cargo_global_options_before_command() {
        let args = parse(&[
            "cargo-cross",
            "-Zunstable-options",
            "--color=always",
            "--locked",
            "-vv",
            "build",
        ])
        .unwrap();

        assert_eq!(args.cargo_z_flags, vec!["unstable-options"]);
        assert_eq!(args.color.as_deref(), Some("always"));
        assert!(args.locked);
        assert_eq!(args.verbose_level, 2);
    }

    #[test]
    fn test_parse_compact_mixed_cargo_global_options() {
        let args = parse(&["cargo-cross", "-vZunstable-options", "-vC.", "build"]).unwrap();

        assert_eq!(args.verbose_level, 2);
        assert_eq!(args.cargo_z_flags, vec!["unstable-options"]);
        assert_eq!(args.cargo_cwd, Some(PathBuf::from(".")));
    }

    #[test]
    fn test_parse_compact_global_option_with_separate_value() {
        let args = parse(&["cargo-cross", "-vZ", "unstable-options", "build"]).unwrap();

        assert_eq!(args.verbose_level, 1);
        assert_eq!(args.cargo_z_flags, vec!["unstable-options"]);
    }

    #[test]
    fn test_parse_config_before_external_cargo_command() {
        let args = parse(&["cargo-cross", "--config", "build.jobs=4", "doc"]).unwrap();

        assert_eq!(args.command, Command::new("doc"));
        assert_eq!(args.cargo_config, vec!["build.jobs=4"]);
    }

    #[test]
    fn test_config_target_is_used_for_toolchain_setup() {
        let args = parse(&[
            "cargo-cross",
            "--config",
            "build.target='aarch64-unknown-linux-gnu'",
            "build",
        ])
        .unwrap();

        assert_eq!(args.targets, vec!["aarch64-unknown-linux-gnu"]);
        assert!(!args.no_cargo_target);
    }

    #[test]
    fn test_config_target_arrays_are_merged() {
        let args = parse(&[
            "cargo-cross",
            "--config",
            "build.target=['x86_64-unknown-linux-musl']",
            "--config",
            "build.target=['aarch64-unknown-linux-musl']",
            "build",
        ])
        .unwrap();

        assert_eq!(
            args.targets,
            vec!["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
        );
    }

    #[test]
    fn test_config_target_scalar_and_array_types_cannot_be_merged() {
        let result = parse(&[
            "cargo-cross",
            "--config",
            "build.target='aarch64-unknown-linux-gnu'",
            "--config",
            "build.target=['x86_64-unknown-linux-musl']",
            "build",
        ]);

        assert!(result.is_err());
    }

    #[test]
    fn test_explicit_target_overrides_config_target() {
        let args = parse(&[
            "cargo-cross",
            "--config",
            "build.target='aarch64-unknown-linux-gnu'",
            "build",
            "--target",
            "x86_64-unknown-linux-musl",
        ])
        .unwrap();

        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_cli_config_target_overrides_environment_target() {
        let args = BuildArgs {
            cargo_config: vec!["build.target='aarch64-unknown-linux-gnu'".to_string()],
            ..BuildArgs::default_for_host()
        };

        let targets =
            configured_targets_with_env(&args, Some("x86_64-unknown-linux-musl")).unwrap();
        assert_eq!(targets, vec!["aarch64-unknown-linux-gnu"]);
    }

    #[test]
    fn test_environment_target_merges_with_cli_config_array() {
        let args = BuildArgs {
            cargo_config: vec!["build.target=['aarch64-unknown-linux-musl']".to_string()],
            ..BuildArgs::default_for_host()
        };

        let targets =
            configured_targets_with_env(&args, Some("x86_64-unknown-linux-musl")).unwrap();
        assert_eq!(
            targets,
            vec!["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"]
        );
    }

    #[test]
    fn test_config_file_target_is_resolved() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-cross-config-target-test-{}",
            std::process::id()
        ));
        let config_dir = temp_dir.join("cfg");
        std::fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("cross.toml");
        std::fs::write(&config_path, "[build]\ntarget = 'custom/MyBoard.json'\n").unwrap();

        let args = BuildArgs {
            cargo_config: vec![config_path.display().to_string()],
            cargo_cwd: Some(temp_dir.clone()),
            ..BuildArgs::default_for_host()
        };
        let targets = configured_targets_with_env(&args, None).unwrap();

        assert_eq!(
            targets,
            vec![std::fs::canonicalize(&temp_dir)
                .unwrap()
                .join("custom/MyBoard.json")
                .display()
                .to_string()]
        );
        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_config_includes_are_merged_recursively() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-cross-config-include-test-{}",
            std::process::id()
        ));
        let config_dir = temp_dir.join(".cargo");
        std::fs::create_dir_all(config_dir.join("nested")).unwrap();
        std::fs::write(
            config_dir.join("config.toml"),
            "include = [\n\
             { path = 'base.toml' },\n\
             { path = 'missing.toml', optional = true },\n\
             { path = 'nested/child.toml' },\n\
             ]\n\
             [build]\n\
             target = ['aarch64-apple-ios']\n",
        )
        .unwrap();
        std::fs::write(
            config_dir.join("base.toml"),
            "[build]\ntarget = ['x86_64-apple-darwin']\n",
        )
        .unwrap();
        std::fs::write(
            config_dir.join("nested/child.toml"),
            "include = [{ path = '../grandchild.toml' }]\n\
             [build]\n\
             target = ['aarch64-apple-darwin']\n",
        )
        .unwrap();
        std::fs::write(
            config_dir.join("grandchild.toml"),
            "[build]\ntarget = ['x86_64-apple-ios']\n",
        )
        .unwrap();

        let targets = target_from_config_file(&config_dir.join("config.toml"))
            .unwrap()
            .unwrap()
            .into_vec();
        assert_eq!(
            targets,
            vec![
                "x86_64-apple-darwin",
                "x86_64-apple-ios",
                "aarch64-apple-darwin",
                "aarch64-apple-ios",
            ]
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_config_include_cycles_are_rejected() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-cross-config-include-cycle-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let first = temp_dir.join("first.toml");
        let second = temp_dir.join("second.toml");
        std::fs::write(&first, "include = ['second.toml']\n").unwrap();
        std::fs::write(&second, "include = ['first.toml']\n").unwrap();

        assert!(target_from_config_file(&first).is_err());

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_nearest_discovered_config_and_environment_precedence() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-cross-config-hierarchy-test-{}",
            std::process::id()
        ));
        let child = temp_dir.join("workspace");
        std::fs::create_dir_all(temp_dir.join(".cargo")).unwrap();
        std::fs::create_dir_all(child.join(".cargo")).unwrap();
        std::fs::write(
            temp_dir.join(".cargo/config.toml"),
            "[build]\ntarget = 'aarch64-unknown-linux-gnu'\n",
        )
        .unwrap();
        std::fs::write(
            child.join(".cargo/config.toml"),
            "[build]\ntarget = 'x86_64-unknown-linux-musl'\n",
        )
        .unwrap();

        let args = BuildArgs {
            cargo_cwd: Some(child),
            ..BuildArgs::default_for_host()
        };
        assert_eq!(
            configured_targets_with_env(&args, None).unwrap(),
            vec!["x86_64-unknown-linux-musl"]
        );
        assert_eq!(
            configured_targets_with_env(&args, Some("aarch64-unknown-linux-musl")).unwrap(),
            vec!["aarch64-unknown-linux-musl"]
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_config_target_arrays_merge_across_all_sources() {
        let temp_dir = std::env::temp_dir().join(format!(
            "cargo-cross-config-array-hierarchy-test-{}",
            std::process::id()
        ));
        let child = temp_dir.join("workspace");
        std::fs::create_dir_all(temp_dir.join(".cargo")).unwrap();
        std::fs::create_dir_all(child.join(".cargo")).unwrap();
        std::fs::write(
            temp_dir.join(".cargo/config.toml"),
            "[build]\ntarget = ['x86_64-unknown-linux-musl']\n",
        )
        .unwrap();
        std::fs::write(
            child.join(".cargo/config.toml"),
            "[build]\ntarget = ['aarch64-unknown-linux-musl']\n",
        )
        .unwrap();

        let args = BuildArgs {
            cargo_cwd: Some(child),
            cargo_config: vec!["build.target=['armv7-unknown-linux-musleabihf']".to_string()],
            ..BuildArgs::default_for_host()
        };
        assert_eq!(
            configured_targets_with_env(&args, Some("aarch64-unknown-linux-gnu")).unwrap(),
            vec![
                "x86_64-unknown-linux-musl",
                "aarch64-unknown-linux-musl",
                "aarch64-unknown-linux-gnu",
                "armv7-unknown-linux-musleabihf",
            ]
        );

        std::fs::remove_dir_all(temp_dir).unwrap();
    }

    #[test]
    fn test_cargo_home_uses_windows_user_profile() {
        assert_eq!(
            cargo_home_from_env(
                None,
                Some(r"C:\home".to_string()),
                Some(r"C:\Users\cargo".to_string()),
                true,
            ),
            Some(PathBuf::from(r"C:\Users\cargo").join(".cargo"))
        );
        assert_eq!(
            cargo_home_from_env(
                Some(r"D:\cargo-home".to_string()),
                None,
                Some(r"C:\Users\cargo".to_string()),
                true,
            ),
            Some(PathBuf::from(r"D:\cargo-home"))
        );
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
        assert_eq!(args.command, Command::build());
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

    // Equals syntax vs space syntax tests

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
        assert_eq!(args.profile.as_deref(), Some("dev"));
    }

    #[test]
    fn test_equals_syntax_features() {
        let args = parse(&["cargo-cross", "build", "--features=foo,bar"]).unwrap();
        assert_eq!(args.features, vec!["foo,bar"]);
    }

    #[test]
    fn test_equals_syntax_short_features() {
        let args = parse(&["cargo-cross", "build", "-F=foo,bar"]).unwrap();
        assert_eq!(args.features, vec!["foo,bar"]);
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

    #[test]
    fn test_equals_syntax_build_std() {
        let args = parse(&["cargo-cross", "build", "--build-std=core,alloc"]).unwrap();
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
    }

    #[test]
    fn test_equals_syntax_manifest_path() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--manifest-path=/path/to/Cargo.toml",
        ])
        .unwrap();
        assert_eq!(
            args.manifest_path,
            Some(PathBuf::from("/path/to/Cargo.toml"))
        );
    }

    #[test]
    fn test_manifest_path_short_option() {
        let args = parse(&["cargo-cross", "build", "-m", "/path/to/Cargo.toml"]).unwrap();
        assert_eq!(
            args.manifest_path,
            Some(PathBuf::from("/path/to/Cargo.toml"))
        );
    }

    #[test]
    fn test_equals_syntax_cross_compiler_dir() {
        let args = parse(&["cargo-cross", "build", "--cross-compiler-dir=/opt/cross"]).unwrap();
        assert_eq!(args.cross_compiler_dir, PathBuf::from("/opt/cross"));
    }

    #[test]
    fn test_equals_syntax_glibc_version() {
        let args = parse(&["cargo-cross", "build", "--glibc-version=2.31"]).unwrap();
        assert_eq!(args.glibc_version, "2.31");
    }

    #[test]
    fn test_equals_syntax_cc_cxx_ar() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--cc=/usr/bin/gcc",
            "--cxx=/usr/bin/g++",
            "--ar=/usr/bin/ar",
        ])
        .unwrap();
        assert_eq!(args.cc, Some(PathBuf::from("/usr/bin/gcc")));
        assert_eq!(args.cxx, Some(PathBuf::from("/usr/bin/g++")));
        assert_eq!(args.ar, Some(PathBuf::from("/usr/bin/ar")));
    }

    #[test]
    fn test_equals_syntax_linker() {
        let args = parse(&["cargo-cross", "build", "--linker=/usr/bin/ld.lld"]).unwrap();
        assert_eq!(args.linker, Some(PathBuf::from("/usr/bin/ld.lld")));
    }

    #[test]
    fn test_equals_syntax_cflags_with_spaces() {
        let args = parse(&["cargo-cross", "build", "--cflags=-O2 -Wall -Wextra"]).unwrap();
        assert_eq!(args.cflags, Some("-O2 -Wall -Wextra".to_string()));
    }

    #[test]
    fn test_equals_syntax_ldflags() {
        let args = parse(&["cargo-cross", "build", "--ldflags=-L/usr/local/lib -static"]).unwrap();
        assert_eq!(args.ldflags, Some("-L/usr/local/lib -static".to_string()));
    }

    #[test]
    fn test_equals_syntax_rustflag() {
        let args = parse(&["cargo-cross", "build", "--rustflag=-C opt-level=3"]).unwrap();
        assert_eq!(args.rustflags, vec!["-C opt-level=3"]);
    }

    #[test]
    fn test_equals_syntax_github_proxy() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--github-proxy=https://mirror.example.com/",
        ])
        .unwrap();
        assert_eq!(
            args.github_proxy,
            Some("https://mirror.example.com/".to_string())
        );
    }

    #[test]
    fn test_equals_syntax_rustc_wrapper() {
        let args = parse(&["cargo-cross", "build", "--rustc-wrapper=/usr/bin/cachepot"]).unwrap();
        assert_eq!(args.rustc_wrapper, Some(PathBuf::from("/usr/bin/cachepot")));
    }

    #[test]
    fn test_equals_syntax_config_with_equals_in_value() {
        let args = parse(&["cargo-cross", "build", "--config=build.jobs=4"]).unwrap();
        assert_eq!(args.cargo_config, vec!["build.jobs=4"]);
    }

    #[test]
    fn test_equals_syntax_multiple_options() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t=x86_64-unknown-linux-musl",
            "--profile=release",
            "-F=serde,json",
            "-j=8",
            "--crt-static=true",
            "--build-std=core,alloc",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.features, vec!["serde,json"]);
        assert_eq!(args.jobs, Some("8".to_string()));
        assert_eq!(args.crt_static, Some(true));
        assert_eq!(args.build_std, Some("core,alloc".to_string()));
    }

    #[test]
    fn test_equals_syntax_toolchain() {
        let args = parse(&["cargo-cross", "build", "--toolchain=nightly-2024-01-01"]).unwrap();
        assert_eq!(args.toolchain, Some("nightly-2024-01-01".to_string()));
    }

    #[test]
    fn test_equals_syntax_target_dir() {
        let args = parse(&["cargo-cross", "build", "--target-dir=/tmp/target"]).unwrap();
        assert_eq!(args.cargo_target_dir, Some(PathBuf::from("/tmp/target")));
    }

    #[test]
    fn test_equals_syntax_z_flag() {
        let args = parse(&["cargo-cross", "build", "-Z=build-std"]).unwrap();
        assert_eq!(args.cargo_z_flags, vec!["build-std"]);
    }

    #[test]
    fn test_equals_syntax_directory() {
        let args = parse(&["cargo-cross", "build", "-C=/path/to/project"]).unwrap();
        assert_eq!(args.cargo_cwd, Some(PathBuf::from("/path/to/project")));
    }

    #[test]
    fn test_equals_syntax_message_format() {
        let args = parse(&["cargo-cross", "build", "--message-format=json"]).unwrap();
        assert_eq!(args.message_format, Some("json".to_string()));
    }

    #[test]
    fn test_equals_syntax_color() {
        let args = parse(&["cargo-cross", "build", "--color=always"]).unwrap();
        assert_eq!(args.color, Some("always".to_string()));
    }

    // Mixed flags and options tests

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
        assert_eq!(args.features, vec!["serde"]);
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
        assert_eq!(args.profile.as_deref(), Some("release"));
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
        assert_eq!(args.features, vec!["serde,json"]);
        assert_eq!(args.crt_static, Some(true));
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.verbose_level, 2);
    }

    // Complex option ordering tests

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
        assert_eq!(args.profile.as_deref(), Some("dev"));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.features, vec!["foo"]);
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
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.features, vec!["foo"]);
        assert!(args.no_default_features);
        assert_eq!(args.jobs, Some("4".to_string()));
        assert!(args.locked);
    }

    // Verbose flag variations

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

    // Timings option (optional value) tests

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

    // Multiple values / repeated options tests

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

    // Hyphen values tests (for compiler flags)

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

    // Passthrough arguments tests

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
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.passthrough_args, vec!["--foo", "--bar"]);
    }

    // Alias tests

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

    #[test]
    fn test_trim_paths_no_value() {
        // --trim-paths without value should default to "true"
        let args = parse(&["cargo-cross", "build", "--trim-paths"]).unwrap();
        assert_eq!(args.cargo_trim_paths, Some("true".to_string()));
    }

    #[test]
    fn test_cargo_trim_paths_no_value() {
        // --cargo-trim-paths without value should default to "true"
        let args = parse(&["cargo-cross", "build", "--cargo-trim-paths"]).unwrap();
        assert_eq!(args.cargo_trim_paths, Some("true".to_string()));
    }

    #[test]
    fn test_rustc_bootstrap_no_value() {
        // --rustc-bootstrap without value should default to "1"
        let args = parse(&["cargo-cross", "build", "--rustc-bootstrap"]).unwrap();
        assert_eq!(args.rustc_bootstrap, Some("1".to_string()));
    }

    #[test]
    fn test_rustc_bootstrap_with_value() {
        let args = parse(&["cargo-cross", "build", "--rustc-bootstrap", "mycrate"]).unwrap();
        assert_eq!(args.rustc_bootstrap, Some("mycrate".to_string()));
    }

    // Command alias tests

    #[test]
    fn test_command_alias_b() {
        let args = parse(&["cargo-cross", "b"]).unwrap();
        assert_eq!(args.command, Command::build());
    }

    #[test]
    fn test_command_alias_c() {
        let args = parse(&["cargo-cross", "c"]).unwrap();
        assert_eq!(args.command, Command::check());
    }

    #[test]
    fn test_command_alias_r() {
        let args = parse(&["cargo-cross", "r"]).unwrap();
        assert_eq!(args.command, Command::run());
    }

    #[test]
    fn test_command_alias_t() {
        let args = parse(&["cargo-cross", "t"]).unwrap();
        assert_eq!(args.command, Command::test());
    }

    // Requires relationship tests

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
        assert_eq!(args.exclude, vec!["foo"]);
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

    // Conflicts relationship tests

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
    fn test_no_toolchain_setup() {
        let args = parse(&["cargo-cross", "build", "--no-toolchain-setup"]).unwrap();
        assert!(args.no_toolchain_setup);
    }

    #[test]
    fn test_linker_with_no_toolchain_setup() {
        // --linker and --no-toolchain-setup can be used together
        let args = parse(&[
            "cargo-cross",
            "build",
            "--linker",
            "/usr/bin/ld",
            "--no-toolchain-setup",
        ])
        .unwrap();
        assert!(args.no_toolchain_setup);
        assert_eq!(args.linker, Some(PathBuf::from("/usr/bin/ld")));
    }

    // Complex real-world scenario tests

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
        assert_eq!(args.command, Command::build());
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.crt_static, Some(true));
        assert!(args.no_default_features);
        assert_eq!(args.features, vec!["serde,json"]);
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
        assert_eq!(args.command, Command::test());
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
        assert_eq!(args.exclude, vec!["test-crate"]);
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    // Edge case tests

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
        assert_eq!(args.profile.as_deref(), Some("release"));
        assert_eq!(args.features, vec!["serde"]);
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

    // Cargo cross invocation style tests

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
                assert_eq!(args.command, Command::build());
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
                assert_eq!(args.command, Command::build());
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

    // New alias and option tests

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
        assert_eq!(args.profile.as_deref(), Some("release"));
    }

    #[test]
    fn test_release_flag_long() {
        let args = parse(&["cargo-cross", "build", "--release"]).unwrap();
        assert!(args.release);
        assert_eq!(args.profile.as_deref(), Some("release"));
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
        assert_eq!(args.cargo_args, vec!["--verbose"]);
    }

    #[test]
    fn test_cargo_args_original() {
        let args = parse(&["cargo-cross", "build", "--cargo-args", "--verbose"]).unwrap();
        assert_eq!(args.cargo_args, vec!["--verbose"]);
    }

    #[test]
    fn test_cargo_args_multiple() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "--cargo-args",
            "--verbose",
            "--cargo-args",
            "--locked",
        ])
        .unwrap();
        assert_eq!(args.cargo_args, vec!["--verbose", "--locked"]);
    }

    // Target validation tests

    #[test]
    fn test_invalid_target_triple_uppercase() {
        let result = parse(&["cargo-cross", "build", "-t", "X86_64-unknown-linux-musl"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, CrossError::InvalidTargetTriple { target, char } if target == "X86_64-unknown-linux-musl" && char == 'X')
        );
    }

    #[test]
    fn test_glob_pattern_matches_target() {
        // x86_64*unknown-linux-musl matches x86_64-unknown-linux-musl (glob * matches -)
        let args = parse(&["cargo-cross", "build", "-t", "x86_64*unknown-linux-musl"]).unwrap();
        assert!(args
            .targets
            .contains(&"x86_64-unknown-linux-musl".to_string()));
    }

    #[test]
    fn test_invalid_target_triple_slash() {
        // Slash is not a glob character, so it should fail validation as invalid character
        let result = parse(&["cargo-cross", "build", "-t", "x86_64/unknown-linux-musl"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, CrossError::InvalidTargetTriple { target, char } if target == "x86_64/unknown-linux-musl" && char == '/')
        );
    }

    #[test]
    fn test_invalid_target_triple_dot() {
        let result = parse(&["cargo-cross", "build", "-t", "x86_64.unknown-linux-musl"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, CrossError::InvalidTargetTriple { target, char } if target == "x86_64.unknown-linux-musl" && char == '.')
        );
    }

    #[test]
    fn test_no_matching_targets_glob() {
        let result = parse(&["cargo-cross", "build", "-t", "*mingw*"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            CrossError::NoMatchingTargets { pattern } if pattern == "*mingw*"
        ));
    }

    #[test]
    fn test_no_matching_targets_glob_complex() {
        let result = parse(&["cargo-cross", "build", "-t", "*nonexistent-platform*"]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(
            err,
            CrossError::NoMatchingTargets { pattern } if pattern == "*nonexistent-platform*"
        ));
    }

    #[test]
    fn test_valid_target_triple_with_numbers() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "armv7-unknown-linux-gnueabihf",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["armv7-unknown-linux-gnueabihf"]);
    }

    #[test]
    fn test_valid_target_triple_underscore() {
        let args = parse(&["cargo-cross", "build", "-t", "x86_64_unknown_linux_musl"]).unwrap();
        assert_eq!(args.targets, vec!["x86_64_unknown_linux_musl"]);
    }

    #[test]
    fn test_valid_glob_pattern_matches() {
        let args = parse(&["cargo-cross", "build", "-t", "*-linux-musl"]).unwrap();
        assert!(!args.targets.is_empty());
        for target in &args.targets {
            assert!(target.ends_with("-linux-musl"));
        }
    }

    #[test]
    fn test_is_glob_pattern() {
        assert!(is_glob_pattern("*-linux-musl"));
        assert!(is_glob_pattern("x86_64-*-linux"));
        assert!(is_glob_pattern("x86_64-?-linux"));
        assert!(is_glob_pattern("[ab]-linux"));
        assert!(!is_glob_pattern("x86_64-unknown-linux-musl"));
        assert!(!is_glob_pattern("aarch64-linux-android"));
    }

    #[test]
    fn test_validate_target_triple() {
        assert!(validate_target_triple("x86_64-unknown-linux-musl").is_ok());
        assert!(validate_target_triple("aarch64-unknown-linux-gnu").is_ok());
        assert!(validate_target_triple("armv7-unknown-linux-gnueabihf").is_ok());
        assert!(validate_target_triple("i686-pc-windows-msvc").is_ok());
        assert!(validate_target_triple("x86_64-pc-windows-msvc").is_ok());
        assert!(validate_target_triple("X86_64-unknown-linux-musl").is_err()); // uppercase X
        assert!(validate_target_triple("x86_64*linux").is_err()); // special char *
        assert!(validate_target_triple("x86_64.linux").is_err()); // special char .
        assert!(validate_target_triple("x86_64 linux").is_err()); // space
    }

    // Short argument concatenation tests (no separator between flag and value)

    #[test]
    fn test_short_concat_features() {
        let args = parse(&["cargo-cross", "build", "-Ffoo,bar"]).unwrap();
        assert_eq!(args.features, vec!["foo,bar"]);
    }

    #[test]
    fn test_short_concat_jobs() {
        let args = parse(&["cargo-cross", "build", "-j4"]).unwrap();
        assert_eq!(args.jobs, Some("4".to_string()));
    }

    #[test]
    fn test_short_concat_package() {
        let args = parse(&["cargo-cross", "build", "-pmypackage"]).unwrap();
        assert_eq!(args.package, vec!["mypackage"]);
    }

    #[test]
    fn test_short_concat_target() {
        let args = parse(&["cargo-cross", "build", "-tx86_64-unknown-linux-musl"]).unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
    }

    #[test]
    fn test_short_concat_z_flag() {
        let args = parse(&["cargo-cross", "build", "-Zbuild-std"]).unwrap();
        assert_eq!(args.cargo_z_flags, vec!["build-std"]);
    }

    #[test]
    fn test_short_concat_directory() {
        let args = parse(&["cargo-cross", "build", "-C/path/to/project"]).unwrap();
        assert_eq!(args.cargo_cwd, Some(PathBuf::from("/path/to/project")));
    }

    #[test]
    fn test_short_concat_multiple() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-tx86_64-unknown-linux-musl",
            "-Ffoo,bar",
            "-j8",
            "-pmypkg",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.features, vec!["foo,bar"]);
        assert_eq!(args.jobs, Some("8".to_string()));
        assert_eq!(args.package, vec!["mypkg"]);
    }

    #[test]
    fn test_short_concat_mixed_with_space() {
        let args = parse(&[
            "cargo-cross",
            "build",
            "-j4",
            "-t",
            "x86_64-unknown-linux-musl",
            "-Fbar",
        ])
        .unwrap();
        assert_eq!(args.jobs, Some("4".to_string()));
        assert_eq!(args.targets, vec!["x86_64-unknown-linux-musl"]);
        assert_eq!(args.features, vec!["bar"]);
    }

    // Tests for parse_env_args (shell-style argument parsing from env vars)

    #[test]
    fn test_parse_env_args_not_set() {
        std::env::remove_var("TEST_PARSE_ENV_ARGS_NOT_SET");
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_NOT_SET");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_env_args_empty() {
        std::env::set_var("TEST_PARSE_ENV_ARGS_EMPTY", "");
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_EMPTY");
        assert!(result.is_none());
        std::env::remove_var("TEST_PARSE_ENV_ARGS_EMPTY");
    }

    #[test]
    fn test_parse_env_args_simple() {
        std::env::set_var("TEST_PARSE_ENV_ARGS_SIMPLE", "--verbose --locked");
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_SIMPLE");
        assert_eq!(
            result,
            Some(vec!["--verbose".to_string(), "--locked".to_string()])
        );
        std::env::remove_var("TEST_PARSE_ENV_ARGS_SIMPLE");
    }

    #[test]
    fn test_parse_env_args_with_single_quotes() {
        std::env::set_var(
            "TEST_PARSE_ENV_ARGS_SINGLE_QUOTES",
            "--config 'build.jobs=4' --verbose",
        );
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_SINGLE_QUOTES");
        assert_eq!(
            result,
            Some(vec![
                "--config".to_string(),
                "build.jobs=4".to_string(),
                "--verbose".to_string()
            ])
        );
        std::env::remove_var("TEST_PARSE_ENV_ARGS_SINGLE_QUOTES");
    }

    #[test]
    fn test_parse_env_args_with_double_quotes() {
        std::env::set_var(
            "TEST_PARSE_ENV_ARGS_DOUBLE_QUOTES",
            "--message-format \"json with spaces\"",
        );
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_DOUBLE_QUOTES");
        assert_eq!(
            result,
            Some(vec![
                "--message-format".to_string(),
                "json with spaces".to_string()
            ])
        );
        std::env::remove_var("TEST_PARSE_ENV_ARGS_DOUBLE_QUOTES");
    }

    #[test]
    fn test_parse_env_args_complex() {
        std::env::set_var(
            "TEST_PARSE_ENV_ARGS_COMPLEX",
            "--config 'key=\"value with spaces\"' --verbose",
        );
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_COMPLEX");
        assert_eq!(
            result,
            Some(vec![
                "--config".to_string(),
                "key=\"value with spaces\"".to_string(),
                "--verbose".to_string()
            ])
        );
        std::env::remove_var("TEST_PARSE_ENV_ARGS_COMPLEX");
    }

    #[test]
    fn test_parse_env_args_whitespace_only() {
        std::env::set_var("TEST_PARSE_ENV_ARGS_WHITESPACE", "   ");
        let result = parse_env_args("TEST_PARSE_ENV_ARGS_WHITESPACE");
        assert!(result.is_none());
        std::env::remove_var("TEST_PARSE_ENV_ARGS_WHITESPACE");
    }

    #[test]
    fn test_musl_target_with_default_glibc() {
        // musl targets should work with default (empty) glibc version
        let args = parse(&[
            "cargo-cross",
            "build",
            "-t",
            "aarch64_be-unknown-linux-musl",
        ])
        .unwrap();
        assert_eq!(args.targets, vec!["aarch64_be-unknown-linux-musl"]);
        assert_eq!(args.glibc_version, ""); // default is empty string
    }
}
