#!/bin/bash

set -e
set -m

cleanup() {
	kill -TERM -$$ 2>/dev/null
	exit 0
}

trap cleanup SIGTERM SIGINT

# =============================================================================
# Rust Cross-Compilation Build Script
# =============================================================================
# This script provides cross-compilation support for Rust projects across
# multiple platforms including Linux, Windows, macOS, FreeBSD, iOS, and Android.
# =============================================================================

# -----------------------------------------------------------------------------
# Color Definitions
# -----------------------------------------------------------------------------
readonly COLOR_LIGHT_RED='\033[1;31m'
readonly COLOR_LIGHT_GREEN='\033[1;32m'
readonly COLOR_LIGHT_YELLOW='\033[1;33m'
readonly COLOR_LIGHT_BLUE='\033[1;34m'
readonly COLOR_LIGHT_MAGENTA='\033[1;35m'
readonly COLOR_LIGHT_CYAN='\033[1;36m'
readonly COLOR_LIGHT_GRAY='\033[0;37m'
readonly COLOR_DARK_GRAY='\033[1;30m'
readonly COLOR_WHITE='\033[1;37m'
readonly COLOR_RESET='\033[0m'

# -----------------------------------------------------------------------------
# Default Configuration
# -----------------------------------------------------------------------------
readonly DEFAULT_SOURCE_DIR="$(pwd)"
readonly DEFAULT_PROFILE="release"
readonly DEFAULT_CROSS_COMPILER_DIR="$(dirname $(mktemp -u))/rust-cross-compiler"
readonly DEFAULT_CROSS_DEPS_VERSION="v0.6.9"
readonly DEFAULT_TTY_WIDTH="40"
readonly DEFAULT_NDK_VERSION="r27"
readonly DEFAULT_COMMAND="build"
readonly DEFAULT_TOOLCHAIN=""
readonly SUPPORTED_COMMANDS="b|build|check|c|run|r|test|t|bench"
readonly DEFAULT_QEMU_VERSION="v10.1.3"

# -----------------------------------------------------------------------------
# Host Environment Detection
# -----------------------------------------------------------------------------
readonly HOST_OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
readonly HOST_ARCH="$(uname -m)"
readonly HOST_TRIPLE="$(rustc -vV | grep host | cut -d' ' -f2)"

# -----------------------------------------------------------------------------
# Target Configuration Database
# -----------------------------------------------------------------------------
# Supported Rust targets with their toolchain configurations
# Format: "target=os:arch:libc:abi"
readonly TOOLCHAIN_CONFIG="
aarch64-unknown-linux-musl=linux:aarch64:musl
arm-unknown-linux-musleabi=linux:armv6:musl:eabi
arm-unknown-linux-musleabihf=linux:armv6:musl:eabihf
armv5te-unknown-linux-musleabi=linux:armv5:musl:eabi
armv7-unknown-linux-musleabi=linux:armv7:musl:eabi
armv7-unknown-linux-musleabihf=linux:armv7:musl:eabihf
i586-unknown-linux-musl=linux:i586:musl
i686-unknown-linux-musl=linux:i686:musl
loongarch64-unknown-linux-musl=linux:loongarch64:musl
mips-unknown-linux-musl=linux:mips:musl
mipsel-unknown-linux-musl=linux:mipsel:musl
mips64-unknown-linux-muslabi64=linux:mips64:musl
mips64-openwrt-linux-musl=linux:mips64:musl
mips64el-unknown-linux-muslabi64=linux:mips64el:musl
powerpc64-unknown-linux-musl=linux:powerpc64:musl
powerpc64le-unknown-linux-musl=linux:powerpc64le:musl
riscv64gc-unknown-linux-musl=linux:riscv64:musl
s390x-unknown-linux-musl=linux:s390x:musl
x86_64-unknown-linux-musl=linux:x86_64:musl
aarch64-unknown-linux-gnu=linux:aarch64:gnu
arm-unknown-linux-gnueabi=linux:armv6:gnu:eabi
arm-unknown-linux-gnueabihf=linux:armv6:gnu:eabihf
armv5te-unknown-linux-gnueabi=linux:armv5:gnu:eabi
armv7-unknown-linux-gnueabi=linux:armv7:gnu:eabi
armv7-unknown-linux-gnueabihf=linux:armv7:gnu:eabihf
i586-unknown-linux-gnu=linux:i586:gnu
i686-unknown-linux-gnu=linux:i686:gnu
loongarch64-unknown-linux-gnu=linux:loongarch64:gnu
mips-unknown-linux-gnu=linux:mips:gnu
mipsel-unknown-linux-gnu=linux:mipsel:gnu
mips64-unknown-linux-gnuabi64=linux:mips64:gnu
mips64el-unknown-linux-gnuabi64=linux:mips64el:gnu
powerpc64-unknown-linux-gnu=linux:powerpc64:gnu
powerpc64le-unknown-linux-gnu=linux:powerpc64le:gnu
riscv64gc-unknown-linux-gnu=linux:riscv64:gnu
s390x-unknown-linux-gnu=linux:s390x:gnu
x86_64-unknown-linux-gnu=linux:x86_64:gnu
i686-pc-windows-gnu=windows:i686:gnu
x86_64-pc-windows-gnu=windows:x86_64:gnu
x86_64-unknown-freebsd=freebsd:x86_64
aarch64-unknown-freebsd=freebsd:aarch64
powerpc64-unknown-freebsd=freebsd:powerpc64
powerpc64le-unknown-freebsd=freebsd:powerpc64le
riscv64gc-unknown-freebsd=freebsd:riscv64
x86_64-apple-darwin=darwin:x86_64
x86_64h-apple-darwin=darwin:x86_64h
aarch64-apple-darwin=darwin:aarch64
arm64e-apple-darwin=darwin:arm64e
x86_64-apple-ios=ios:x86_64
aarch64-apple-ios=ios:aarch64
aarch64-apple-ios-sim=ios-sim:aarch64
aarch64-linux-android=android:aarch64
arm-linux-androideabi=android:armv7
armv7-linux-androideabi=android:armv7
i686-linux-android=android:i686
riscv64-linux-android=android:riscv64
x86_64-linux-android=android:x86_64
"

# -----------------------------------------------------------------------------
# Utility Functions
# -----------------------------------------------------------------------------

# Get toolchain configuration for a target
get_toolchain_config() {
	local target="$1"
	echo "$TOOLCHAIN_CONFIG" | grep "^${target}=" | cut -d'=' -f2
}

# Sets a variable to a default value if it's not already set
set_default() {
	local var_name="$1"
	local default_value="$2"
	[[ -z "${!var_name}" ]] && eval "${var_name}=\"${default_value}\"" || true
}

# Get separator line
print_separator() {
	local width=$(tput cols 2>/dev/null || echo $DEFAULT_TTY_WIDTH)
	printf '%*s\n' "$width" '' | tr ' ' -
}

# Log functions for consistent output
log_info() {
	echo -e "${COLOR_LIGHT_BLUE}$*${COLOR_RESET}"
}

log_success() {
	echo -e "${COLOR_LIGHT_GREEN}$*${COLOR_RESET}"
}

log_warning() {
	echo -e "${COLOR_LIGHT_YELLOW}$*${COLOR_RESET}"
}

log_error() {
	echo -e "${COLOR_LIGHT_RED}$*${COLOR_RESET}" >&2
}

# Get next argument value or exit with error
get_arg_value() {
	local option_name="$1"
	local next_value="$2"

	if [[ -z "$next_value" || "$next_value" == -* ]]; then
		log_error "Error: $option_name requires a value"
		exit 1
	fi
	echo "$next_value"
}

# Parse single-value option argument
parse_option_value() {
	local option_name="$1"
	shift
	if [[ $# -gt 0 ]]; then
		echo "$1"
	else
		log_error "Error: $option_name requires a value"
		exit 1
	fi
}

# -----------------------------------------------------------------------------
# Help and Information
# -----------------------------------------------------------------------------

# Prints help information
print_help() {
	echo -e "${COLOR_LIGHT_GREEN}Usage:${COLOR_RESET} ${COLOR_LIGHT_CYAN}[+toolchain] [command] [options]${COLOR_RESET}"
	echo -e ""
	echo -e "${COLOR_LIGHT_GREEN}Commands:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_CYAN}b${COLOR_RESET}, ${COLOR_LIGHT_CYAN}build${COLOR_RESET}    Compile the package (default)"
	echo -e "  ${COLOR_LIGHT_CYAN}c${COLOR_RESET}, ${COLOR_LIGHT_CYAN}check${COLOR_RESET}    Analyze the package and report errors"
	echo -e "  ${COLOR_LIGHT_CYAN}r${COLOR_RESET}, ${COLOR_LIGHT_CYAN}run${COLOR_RESET}      Run a binary or example of the package"
	echo -e "  ${COLOR_LIGHT_CYAN}t${COLOR_RESET}, ${COLOR_LIGHT_CYAN}test${COLOR_RESET}     Run the tests"
	echo -e "  ${COLOR_LIGHT_CYAN}bench${COLOR_RESET}       Run the benchmarks"
	echo -e ""
	echo -e "${COLOR_LIGHT_GREEN}Options:${COLOR_RESET}"
	echo -e "      ${COLOR_LIGHT_CYAN}--command${COLOR_RESET} ${COLOR_LIGHT_CYAN}<COMMAND>${COLOR_RESET}               Set the cargo command to run (build|check|run|test|bench)"
	echo -e "      ${COLOR_LIGHT_CYAN}--profile${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PROFILE>${COLOR_RESET}               Set the build profile (debug/release, default: ${DEFAULT_PROFILE})"
	echo -e "      ${COLOR_LIGHT_CYAN}--cross-compiler-dir${COLOR_RESET} ${COLOR_LIGHT_CYAN}<DIR>${COLOR_RESET}        Specify the cross compiler directory"
	echo -e "  ${COLOR_LIGHT_CYAN}-F${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--features${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FEATURES>${COLOR_RESET}             Space or comma separated list of features to activate"
	echo -e "      ${COLOR_LIGHT_CYAN}--no-default-features${COLOR_RESET}             Do not activate default features"
	echo -e "      ${COLOR_LIGHT_CYAN}--all-features${COLOR_RESET}                    Activate all available features"
	echo -e "  ${COLOR_LIGHT_CYAN}-t${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--target${COLOR_RESET} ${COLOR_LIGHT_CYAN}<TRIPLE>${COLOR_RESET}                 Rust target triple(s) (e.g., x86_64-unknown-linux-musl)"
	echo -e "      ${COLOR_LIGHT_CYAN}--show-all-targets${COLOR_RESET}                Display all supported target triples"
	echo -e "      ${COLOR_LIGHT_CYAN}--github-proxy-mirror${COLOR_RESET} ${COLOR_LIGHT_CYAN}<URL>${COLOR_RESET}       Use a GitHub proxy mirror"
	echo -e "      ${COLOR_LIGHT_CYAN}--ndk-version${COLOR_RESET} ${COLOR_LIGHT_CYAN}<VERSION>${COLOR_RESET}           Specify the Android NDK version"
	echo -e "  ${COLOR_LIGHT_CYAN}-p${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--package${COLOR_RESET} ${COLOR_LIGHT_CYAN}<SPEC>${COLOR_RESET}                  Package to build (workspace member)"
	echo -e "      ${COLOR_LIGHT_CYAN}--workspace${COLOR_RESET}                       Build all workspace members"
	echo -e "      ${COLOR_LIGHT_CYAN}--exclude${COLOR_RESET} ${COLOR_LIGHT_CYAN}<SPEC>${COLOR_RESET}                  Exclude packages from the build (must be used with --workspace)"
	echo -e "      ${COLOR_LIGHT_CYAN}--bin${COLOR_RESET} ${COLOR_LIGHT_CYAN}<NAME>${COLOR_RESET}                      Binary target to build"
	echo -e "      ${COLOR_LIGHT_CYAN}--bins${COLOR_RESET}                            Build all binary targets"
	echo -e "      ${COLOR_LIGHT_CYAN}--lib${COLOR_RESET}                             Build only the library target"
	echo -e "      ${COLOR_LIGHT_CYAN}--example${COLOR_RESET} ${COLOR_LIGHT_CYAN}<NAME>${COLOR_RESET}                  Example target to build"
	echo -e "      ${COLOR_LIGHT_CYAN}--examples${COLOR_RESET}                        Build all example targets"
	echo -e "      ${COLOR_LIGHT_CYAN}--test${COLOR_RESET} ${COLOR_LIGHT_CYAN}<NAME>${COLOR_RESET}                     Integration test to build"
	echo -e "      ${COLOR_LIGHT_CYAN}--tests${COLOR_RESET}                           Build all test targets"
	echo -e "      ${COLOR_LIGHT_CYAN}--bench${COLOR_RESET} ${COLOR_LIGHT_CYAN}<NAME>${COLOR_RESET}                    Benchmark target to build"
	echo -e "      ${COLOR_LIGHT_CYAN}--benches${COLOR_RESET}                         Build all benchmark targets"
	echo -e "      ${COLOR_LIGHT_CYAN}--all-targets${COLOR_RESET}                     Build all targets (equivalent to --lib --bins --tests --benches --examples)"
	echo -e "  ${COLOR_LIGHT_CYAN}-r${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--release${COLOR_RESET}                         Build optimized artifacts with the release profile"
	echo -e "  ${COLOR_LIGHT_CYAN}-q${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--quiet${COLOR_RESET}                           Do not print cargo log messages"
	echo -e "      ${COLOR_LIGHT_CYAN}--message-format${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FMT>${COLOR_RESET}            The output format for diagnostic messages"
	echo -e "      ${COLOR_LIGHT_CYAN}--ignore-rust-version${COLOR_RESET}             Ignore rust-version specification in packages"
	echo -e "      ${COLOR_LIGHT_CYAN}--locked${COLOR_RESET}                          Asserts that exact same dependencies are used as Cargo.lock"
	echo -e "      ${COLOR_LIGHT_CYAN}--offline${COLOR_RESET}                         Prevents Cargo from accessing the network"
	echo -e "      ${COLOR_LIGHT_CYAN}--frozen${COLOR_RESET}                          Equivalent to specifying both --locked and --offline"
	echo -e "  ${COLOR_LIGHT_CYAN}-j${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--jobs${COLOR_RESET} ${COLOR_LIGHT_CYAN}<N>${COLOR_RESET}                        Number of parallel jobs to run"
	echo -e "      ${COLOR_LIGHT_CYAN}--keep-going${COLOR_RESET}                      Build as many crates as possible, don't abort on first failure"
	echo -e "      ${COLOR_LIGHT_CYAN}--future-incompat-report${COLOR_RESET}          Displays a future-incompat report for warnings"
	echo -e "      ${COLOR_LIGHT_CYAN}--manifest-path${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}            Path to Cargo.toml"
	echo -e "      ${COLOR_LIGHT_CYAN}--use-default-linker${COLOR_RESET}              Use system default linker (no cross-compiler download)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cc${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}                       Force set the C compiler for target"
	echo -e "      ${COLOR_LIGHT_CYAN}--cxx${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}                      Force set the C++ compiler for target"
	echo -e "      ${COLOR_LIGHT_CYAN}--ar${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}                       Force set the ar for target"
	echo -e "      ${COLOR_LIGHT_CYAN}--linker${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}                   Force set the linker for target"
	echo -e "      ${COLOR_LIGHT_CYAN}--rustflags${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FLAGS>${COLOR_RESET}               Additional rustflags (can be specified multiple times)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cflags${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FLAGS>${COLOR_RESET}                  C compiler flags (cc crate)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cxxflags${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FLAGS>${COLOR_RESET}                C++ compiler flags (cc crate)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cxxstdlib${COLOR_RESET} ${COLOR_LIGHT_CYAN}<NAME>${COLOR_RESET}                C++ standard library (cc crate)"
	echo -e "      ${COLOR_LIGHT_CYAN}--rustc-wrapper${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}            Compiler wrapper for caching (sccache, ccache, etc.)"
	echo -e "      ${COLOR_LIGHT_CYAN}--enable-sccache${COLOR_RESET}                  Enable sccache for compilation caching"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-dir${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}              Sccache local cache directory"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-cache-size${COLOR_RESET} ${COLOR_LIGHT_CYAN}<SIZE>${COLOR_RESET}       Maximum sccache cache size (e.g., 2G, 10G)"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-idle-timeout${COLOR_RESET} ${COLOR_LIGHT_CYAN}<SEC>${COLOR_RESET}      Sccache daemon idle timeout in seconds"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-log${COLOR_RESET} ${COLOR_LIGHT_CYAN}<LEVEL>${COLOR_RESET}             Sccache log level (error, warn, info, debug, trace)"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-no-daemon${COLOR_RESET}               Disable sccache background daemon"
	echo -e "      ${COLOR_LIGHT_CYAN}--sccache-direct${COLOR_RESET}                  Enable sccache preprocessor caching"
	echo -e "      ${COLOR_LIGHT_CYAN}--cc-no-defaults${COLOR_RESET}                  Disable default cc crate compiler flags"
	echo -e "      ${COLOR_LIGHT_CYAN}--cc-shell-escaped-flags${COLOR_RESET}          Parse *FLAGS using shell argument parsing"
	echo -e "      ${COLOR_LIGHT_CYAN}--cc-enable-debug${COLOR_RESET}                 Enable cc crate debug output"
	echo -e "      ${COLOR_LIGHT_CYAN}--crt-static${COLOR_RESET}[=${COLOR_LIGHT_CYAN}<true|false>${COLOR_RESET}]       Add -C target-feature=+crt-static to rustflags"
	echo -e "      ${COLOR_LIGHT_CYAN}--panic-immediate-abort${COLOR_RESET}           Enable panic=immediate-abort (requires nightly-2025-09-24+)"
	echo -e "      ${COLOR_LIGHT_CYAN}--fmt-debug${COLOR_RESET} ${COLOR_LIGHT_CYAN}<MODE>${COLOR_RESET}                Set -Zfmt-debug (full, shallow, none)"
	echo -e "      ${COLOR_LIGHT_CYAN}--location-detail${COLOR_RESET} ${COLOR_LIGHT_CYAN}<DETAIL>${COLOR_RESET}        Set -Zlocation-detail (none, or: file,line,column)"
	echo -e "      ${COLOR_LIGHT_CYAN}--build-std${COLOR_RESET}[=${COLOR_LIGHT_CYAN}<CRATES>${COLOR_RESET}]            Use -Zbuild-std for building standard library from source"
	echo -e "      ${COLOR_LIGHT_CYAN}--build-std-features${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FEATURES>${COLOR_RESET}   Features to enable for -Zbuild-std (e.g., panic-unwind)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cargo-args${COLOR_RESET} ${COLOR_LIGHT_CYAN}<ARGS>${COLOR_RESET}               Additional arguments to pass to cargo command"
	echo -e "      ${COLOR_LIGHT_CYAN}--toolchain${COLOR_RESET} ${COLOR_LIGHT_CYAN}<TOOLCHAIN>${COLOR_RESET}           Rust toolchain to use (stable, nightly, etc.)"
	echo -e "      ${COLOR_LIGHT_CYAN}--cargo-trim-paths${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATHS>${COLOR_RESET}        Set CARGO_TRIM_PATHS environment variable"
	echo -e "      ${COLOR_LIGHT_CYAN}--no-embed-metadata${COLOR_RESET}               Add -Zno-embed-metadata flag to cargo"
	echo -e "      ${COLOR_LIGHT_CYAN}--rustc-bootstrap${COLOR_RESET}[=${COLOR_LIGHT_CYAN}<VALUE>${COLOR_RESET}]       Set RUSTC_BOOTSTRAP (default: 1, or specify -1/crate_name)"
	echo -e "      ${COLOR_LIGHT_CYAN}--target-dir${COLOR_RESET} ${COLOR_LIGHT_CYAN}<DIR>${COLOR_RESET}                Directory for all generated artifacts"
	echo -e "      ${COLOR_LIGHT_CYAN}--artifact-dir${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}             Copy final artifacts to this directory (unstable, requires nightly)"
	echo -e "      ${COLOR_LIGHT_CYAN}--color${COLOR_RESET} ${COLOR_LIGHT_CYAN}<WHEN>${COLOR_RESET}                    Control when colored output is used (auto, always, never)"
	echo -e "      ${COLOR_LIGHT_CYAN}--build-plan${COLOR_RESET}                      Outputs a series of JSON messages (unstable, requires nightly)"
	echo -e "      ${COLOR_LIGHT_CYAN}--timings${COLOR_RESET}[=${COLOR_LIGHT_CYAN}<FMTS>${COLOR_RESET}]                Output information about compilation timing"
	echo -e "      ${COLOR_LIGHT_CYAN}--lockfile-path${COLOR_RESET} ${COLOR_LIGHT_CYAN}<PATH>${COLOR_RESET}            Path to Cargo.lock (unstable, requires nightly)"
	echo -e "      ${COLOR_LIGHT_CYAN}--config${COLOR_RESET} ${COLOR_LIGHT_CYAN}<KEY=VALUE>${COLOR_RESET}              Override a Cargo configuration value"
	echo -e "  ${COLOR_LIGHT_CYAN}-C${COLOR_RESET} ${COLOR_LIGHT_CYAN}<DIR>${COLOR_RESET}                              Change current working directory before executing"
	echo -e "  ${COLOR_LIGHT_CYAN}-Z${COLOR_RESET} ${COLOR_LIGHT_CYAN}<FLAG>${COLOR_RESET}                             Unstable (nightly-only) flags to Cargo"
	echo -e "  ${COLOR_LIGHT_CYAN}-v${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--verbose${COLOR_RESET}                         Use verbose output"
	echo -e "  ${COLOR_LIGHT_CYAN}-h${COLOR_RESET}, ${COLOR_LIGHT_CYAN}--help${COLOR_RESET}                            Display this help message"
}

# -----------------------------------------------------------------------------
# Download and Archive Handling
# -----------------------------------------------------------------------------

# Get host platform string for downloads
get_host_platform() {
	local host_arch="${HOST_ARCH}"
	[[ "${HOST_ARCH}" == "arm" ]] && host_arch="armv7"
	[[ "${HOST_ARCH}" == "arm64" ]] && host_arch="aarch64"
	[[ "${HOST_ARCH}" == "amd64" ]] && host_arch="x86_64"
	echo "${HOST_OS}-${host_arch}"
}

# Downloads and extracts a file
download_and_extract() {
	local url="$1"
	local file="$2"
	local type="${3:-$(echo "${url}" | sed 's/.*\.//g')}"

	mkdir -p "${file}" || return $?
	file="$(cd "${file}" && pwd)" || return $?
	if [ "$(ls -A "${file}")" ]; then
		rm -rf "${file}"/* || return $?
	fi
	log_info "Downloading \"${COLOR_LIGHT_GREEN}${url}${COLOR_LIGHT_BLUE}\" to \"${COLOR_LIGHT_GREEN}${file}${COLOR_LIGHT_BLUE}\""

	local start_time=$(date +%s)

	case "${type}" in
	"tgz" | "gz")
		curl -sL "${url}" | tar -xf - -C "${file}" --strip-components 1 -z || return $?
		;;
	"bz2")
		curl -sL "${url}" | tar -xf - -C "${file}" --strip-components 1 -j || return $?
		;;
	"xz")
		curl -sL "${url}" | tar -xf - -C "${file}" --strip-components 1 -J || return $?
		;;
	"zip")
		curl -sL "${url}" -o "${file}/tmp.zip" || return $?
		unzip -q -o "${file}/tmp.zip" -d "${file}" || return $?
		rm -f "${file}/tmp.zip" || return $?
		;;
	*)
		echo -e "${COLOR_LIGHT_RED}Unsupported compression type: ${type}${COLOR_RESET}"
		return 2
		;;
	esac

	local end_time=$(date +%s)
	log_success "Download and extraction successful (took ${COLOR_LIGHT_YELLOW}$((end_time - start_time))s${COLOR_LIGHT_GREEN})"
}

# Download cross-compiler if needed
# Args: compiler_dir, download_url
download_cross_compiler() {
	local compiler_dir="$1"
	local download_url="$2"

	if [[ ! -d "${compiler_dir}" ]]; then
		download_and_extract "${download_url}" "${compiler_dir}" || return 2
	fi
}

# Set cross-compilation environment variables
# Args: cc, cxx, ar, linker, extra_path
set_cross_env() {
	TARGET_CC="$1"
	TARGET_CXX="$2"
	TARGET_AR="$3"
	TARGET_LINKER="$4"
	TARGET_PATH="$5"
}

# Set gcc library search paths for rustc
# Args: compiler_dir, target_prefix
set_gcc_lib_paths() {
	local compiler_dir="$1"
	local target_prefix="$2"

	# Add target library directory
	local target_lib="${compiler_dir}/${target_prefix}/lib"
	TARGET_RUSTFLAGS="-L ${target_lib}"

	# Add gcc library directory (find the version directory)
	local gcc_lib_dir=$(find "${compiler_dir}/lib/gcc/${target_prefix}" -maxdepth 1 -type d ! -path "${compiler_dir}/lib/gcc/${target_prefix}" 2>/dev/null | head -n 1)
	[[ -n "$gcc_lib_dir" ]] && TARGET_RUSTFLAGS="${TARGET_RUSTFLAGS} -L ${gcc_lib_dir}"
}

# Set iOS/Darwin SDK root from compiler directory
# https://doc.rust-lang.org/unstable-book/compiler-environment-variables/SDKROOT.html
# Args: cross_compiler_name
set_ios_sdk_root() {
	local cross_compiler_name="$1"
	local sdk_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}/SDK"
	if [[ -d "$sdk_dir" ]]; then
		local first_sdk="$(find "$sdk_dir" -maxdepth 1 -type d ! -path "$sdk_dir" | head -n 1)"
		if [[ -n "$first_sdk" ]]; then
			SDKROOT="$first_sdk"
		fi
	fi
}

# Add environment variable to build_env array if value is non-empty
# Args: env_var_name, value
add_env_if_set() {
	local var_name="$1"
	local var_value="$2"
	[[ -n "$var_value" ]] && build_env+=("${var_name}=${var_value}")
}

# Fix rpath for Darwin/iOS linkers
# Args: compiler_dir, arch_prefix
# Returns: 0 on success, sets TARGET_LIBRARY_PATH if using environment variable fallback
fix_darwin_linker_rpath() {
	local compiler_dir="$1"
	local arch_prefix="$2"
	local linker_path="${compiler_dir}/bin/${arch_prefix}-apple-darwin"*"-ld"

	# Try patchelf first
	if command -v patchelf &>/dev/null; then
		if patchelf --set-rpath "${compiler_dir}/lib" ${linker_path} 2>/dev/null; then
			return 0
		fi
	fi

	# Try chrpath as fallback
	if command -v chrpath &>/dev/null; then
		if chrpath -r "${compiler_dir}/lib" ${linker_path} 2>/dev/null; then
			return 0
		fi
	fi

	# Fallback to environment variable approach
	TARGET_LIBRARY_PATH="${compiler_dir}/lib"
	return 0
}

# -----------------------------------------------------------------------------
# Rust Toolchain Management
# -----------------------------------------------------------------------------

# Helper function to add rust-src component
add_rust_src() {
	local target="$1"
	local toolchain="$2"
	local toolchain_flag=""
	[[ -n "$toolchain" ]] && toolchain_flag="--toolchain=$toolchain"

	log_info "Adding rust-src component for target: ${COLOR_LIGHT_YELLOW}$target${COLOR_LIGHT_BLUE}${toolchain:+ and toolchain: ${COLOR_LIGHT_YELLOW}$toolchain${COLOR_LIGHT_BLUE}}"
	rustup component add rust-src --target="$target" $toolchain_flag || return $?
}

# Helper function to install target
install_target() {
	local rust_target="$1"
	local toolchain="$2"
	local toolchain_flag=""
	[[ -n "$toolchain" ]] && toolchain_flag="--toolchain=$toolchain"

	log_info "Installing Rust target: ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_BLUE}${toolchain:+ for toolchain: ${COLOR_LIGHT_YELLOW}$toolchain${COLOR_LIGHT_BLUE}}"
	rustup target add "$rust_target" $toolchain_flag || return $?
}

# Helper function to check if target is installed
is_target_installed() {
	local rust_target="$1"
	local toolchain="$2"
	local toolchain_flag=""
	[[ -n "$toolchain" ]] && toolchain_flag="--toolchain=$toolchain"

	rustup target list --installed $toolchain_flag | grep -q "^$rust_target$"
}

# Helper function to check if target is available in rustup
is_target_available() {
	local rust_target="$1"
	local toolchain="$2"
	local toolchain_flag=""
	[[ -n "$toolchain" ]] && toolchain_flag="--toolchain=$toolchain"

	rustup target list $toolchain_flag | grep -q "^$rust_target$"
}

# Helper function to get appropriate build-std configuration for a target
# https://github.com/rust-lang/rust/tree/master/library
get_build_std_config() {
	# local rust_target="$1"

	echo "core,std,alloc,proc_macro,test,compiler_builtins,panic_abort,panic_unwind"
}

# -----------------------------------------------------------------------------
# Cross-Compilation Environment Setup
# -----------------------------------------------------------------------------

clean_cross_env() {
	TARGET_CC="" TARGET_CXX="" TARGET_AR="" TARGET_LINKER="" TARGET_RUSTFLAGS="" TARGET_BUILD_STD=""
	TARGET_LIBRARY_PATH="" TARGET_PATH="" SDKROOT="" TARGET_RUNNER=""
}

# Get cross-compilation environment variables
# Returns environment variables as a string suitable for use with env command
get_cross_env() {
	local rust_target="$1"

	# Clear target-specific variables
	clean_cross_env

	# Install Rust target if not already installed, or use build-std if target not available in rustup
	# curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

	# Install target if not already installed, or use build-std if target not available in rustup
	if ! is_target_installed "$rust_target" "$TOOLCHAIN"; then
		# Check if target is available for installation in rustup
		if is_target_available "$rust_target" "$TOOLCHAIN"; then
			install_target "$rust_target" "$TOOLCHAIN" || return $?
		else
			# Check if target exists in rustc --print=target-list
			if rustc --print=target-list | grep -q "^$rust_target$"; then
				log_warning "Target ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_YELLOW} not available in rustup but exists in rustc, using build-std"
				TARGET_BUILD_STD=$(get_build_std_config "$rust_target")
			else
				log_error "Target ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_RED} not found in rustup or rustc target list"
				return 1
			fi
		fi
	fi

	# Skip toolchain setup if using default linker
	if [[ "$USE_DEFAULT_LINKER" == "true" ]]; then
		log_warning "Using system default linker for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_YELLOW}"
		return 0
	fi

	# Convert target to uppercase for environment variable names
	local target_upper=$(echo "$rust_target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')

	# Check if target-specific environment variables are already set
	local cc_var="CC_${target_upper}"
	local cxx_var="CXX_${target_upper}"
	local ar_var="AR_${target_upper}"
	local linker_var="CARGO_TARGET_${target_upper}_LINKER"
	local runner_var="CARGO_TARGET_${target_upper}_RUNNER"

	if [[ -n "${!cc_var}" ]]; then
		log_success "Using pre-configured ${COLOR_LIGHT_YELLOW}${cc_var}${COLOR_LIGHT_GREEN}=${COLOR_LIGHT_CYAN}${!cc_var}${COLOR_LIGHT_GREEN}"
		TARGET_CC="${!cc_var}"
		if [[ -n "${!cxx_var}" ]]; then
			TARGET_CXX="${!cxx_var}"
		fi
		if [[ -n "${!ar_var}" ]]; then
			TARGET_AR="${!ar_var}"
		fi
		if [[ -n "${!linker_var}" ]]; then
			TARGET_LINKER="${!linker_var}"
		fi
		if [[ -n "${!runner_var}" ]]; then
			TARGET_RUNNER="${!runner_var}"
		fi
		return 0
	fi

	if [[ -n "$CC" ]] && [[ -n "$CXX" ]]; then
		TARGET_CC="$CC"
		TARGET_CXX="$CXX"
		if [[ -n "$AR" ]]; then
			TARGET_AR="${AR}"
		else
			TARGET_AR="${CC%-gcc}-ar"
		fi
		if [[ -n "$LINKER" ]]; then
			TARGET_LINKER="$LINKER"
		else
			TARGET_LINKER="$CC"
		fi
		if [[ -n "$RUNNER" ]]; then
			TARGET_RUNNER="$RUNNER"
		fi
		return 0
	fi

	local toolchain_info="$(get_toolchain_config "$rust_target")"
	if [[ -z "$toolchain_info" ]]; then
		log_warning "No specific toolchain configuration for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_YELLOW}, using default"
		return 0
	fi

	# Parse toolchain configuration
	IFS=':' read -ra CONFIG <<<"$toolchain_info"
	local os="${CONFIG[0]}"
	local arch="${CONFIG[1]}"
	local libc="${CONFIG[2]}"
	local abi="${CONFIG[3]}"

	case "$os" in
	"linux")
		get_linux_env "$arch" "$libc" "$abi" "$rust_target" || return $?
		;;
	"windows")
		get_windows_gnu_env "$arch" "$rust_target" || return $?
		;;
	"freebsd")
		get_freebsd_env "$arch" "$rust_target" || return $?
		;;
	"darwin")
		get_darwin_env "$arch" "$rust_target" || return $?
		;;
	"android")
		get_android_env "$arch" "$rust_target" || return $?
		;;
	"ios")
		get_ios_env "$arch" "$rust_target" "device" || return $?
		;;
	"ios-sim")
		get_ios_env "$arch" "$rust_target" "simulator" || return $?
		;;
	*)
		log_warning "No cross-compilation setup needed for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_YELLOW}"
		;;
	esac
}

# -----------------------------------------------------------------------------
# Platform-Specific Environment Functions
# -----------------------------------------------------------------------------

# Map toolchain architecture to QEMU binary name
# Args: arch (from TOOLCHAIN_CONFIG)
# Returns: QEMU binary name (e.g., qemu-aarch64)
# Supported QEMU binaries:
#   qemu-aarch64, qemu-aarch64_be, qemu-alpha, qemu-arm, qemu-armeb,
#   qemu-hexagon, qemu-hppa, qemu-i386, qemu-loongarch64, qemu-m68k,
#   qemu-microblaze, qemu-microblazeel, qemu-mips, qemu-mips64, qemu-mips64el,
#   qemu-mipsel, qemu-mipsn32, qemu-mipsn32el, qemu-or1k, qemu-ppc, qemu-ppc64,
#   qemu-ppc64le, qemu-riscv32, qemu-riscv64, qemu-s390x, qemu-sh4, qemu-sh4eb,
#   qemu-sparc, qemu-sparc32plus, qemu-sparc64, qemu-x86_64, qemu-xtensa, qemu-xtensaeb
get_qemu_binary_name() {
	local arch="$1"
	case "$arch" in
	"aarch64") echo "qemu-aarch64" ;;
	"aarch64_be") echo "qemu-aarch64_be" ;;
	"alpha") echo "qemu-alpha" ;;
	"armv5" | "armv6" | "armv7" | "arm") echo "qemu-arm" ;;
	"armeb") echo "qemu-armeb" ;;
	"hexagon") echo "qemu-hexagon" ;;
	"hppa") echo "qemu-hppa" ;;
	"i586" | "i686" | "i386") echo "qemu-i386" ;;
	"loongarch64") echo "qemu-loongarch64" ;;
	"m68k") echo "qemu-m68k" ;;
	"microblaze") echo "qemu-microblaze" ;;
	"microblazeel") echo "qemu-microblazeel" ;;
	"mips") echo "qemu-mips" ;;
	"mips64") echo "qemu-mips64" ;;
	"mips64el") echo "qemu-mips64el" ;;
	"mipsel") echo "qemu-mipsel" ;;
	"mipsn32") echo "qemu-mipsn32" ;;
	"mipsn32el") echo "qemu-mipsn32el" ;;
	"or1k") echo "qemu-or1k" ;;
	"powerpc" | "ppc") echo "qemu-ppc" ;;
	"powerpc64" | "ppc64") echo "qemu-ppc64" ;;
	"powerpc64le" | "ppc64le") echo "qemu-ppc64le" ;;
	"riscv32") echo "qemu-riscv32" ;;
	"riscv64") echo "qemu-riscv64" ;;
	"s390x") echo "qemu-s390x" ;;
	"sh4") echo "qemu-sh4" ;;
	"sh4eb") echo "qemu-sh4eb" ;;
	"sparc") echo "qemu-sparc" ;;
	"sparc32plus") echo "qemu-sparc32plus" ;;
	"sparc64") echo "qemu-sparc64" ;;
	"x86_64") echo "qemu-x86_64" ;;
	"xtensa") echo "qemu-xtensa" ;;
	"xtensaeb") echo "qemu-xtensaeb" ;;
	*) echo "" ;;
	esac
}

# Check if host can natively run the target architecture
# Args: target_arch (from TOOLCHAIN_CONFIG)
# Returns: 0 if native execution is possible, 1 otherwise
can_run_natively() {
	local target_arch="$1"
	local host_arch="${HOST_ARCH}"

	# Normalize host architecture
	[[ "$host_arch" == "arm64" ]] && host_arch="aarch64"
	[[ "$host_arch" == "amd64" ]] && host_arch="x86_64"

	# Check if target can run natively on host
	case "$host_arch" in
	"x86_64")
		# x86_64 can run x86_64, i686, i586
		[[ "$target_arch" == "x86_64" || "$target_arch" == "i686" || "$target_arch" == "i586" ]] && return 0
		;;
	"aarch64")
		# aarch64 can run aarch64 and arm variants (with kernel support)
		[[ "$target_arch" == "aarch64" || "$target_arch" =~ ^arm ]] && return 0
		;;
	"i686" | "i586")
		[[ "$target_arch" == "i686" || "$target_arch" == "i586" ]] && return 0
		;;
	*)
		[[ "$host_arch" == "$target_arch" ]] && return 0
		;;
	esac

	return 1
}

# Check if the current command needs a runner (executes compiled binaries)
# Args: command
# Returns: 0 if runner is needed, 1 otherwise
command_needs_runner() {
	local cmd="$1"
	case "$cmd" in
	"run" | "r" | "test" | "t" | "bench")
		return 0
		;;
	*)
		return 1
		;;
	esac
}

# Setup native runner for cross-compiled binaries using sysroot's dynamic linker
# Args: target_prefix (e.g., armv6-linux-musleabihf), compiler_dir
# Sets: TARGET_RUNNER
# This only can run dynamic link program
setup_native_runner() {
	local target_prefix="$1"
	local compiler_dir="$2"

	local sysroot="${compiler_dir}/${target_prefix}"
	local lib_dir="${sysroot}/lib"

	[[ ! -d "$lib_dir" ]] && return 0

	# Find the dynamic linker in sysroot
	local ld_so=""
	for pattern in 'ld-linux*.so*' 'ld-musl*.so*'; do
		local found=$(find "$lib_dir" -maxdepth 1 -name "$pattern" \( -type f -o -type l \) 2>/dev/null | head -n 1)
		if [[ -n "$found" && -x "$found" ]]; then
			ld_so="$found"
			break
		fi
	done

	if [[ -n "$ld_so" ]]; then
		TARGET_RUNNER="${ld_so} --library-path ${lib_dir}"
	fi
}

# Setup QEMU runner for cross-compiled Linux binaries
# Args: arch, target_prefix (e.g., armv6-linux-musleabihf), compiler_dir
# Sets: TARGET_RUNNER, TARGET_PATH (appends qemu directory)
setup_qemu_runner() {
	local arch="$1"
	local target_prefix="$2"
	local compiler_dir="$3"

	# Check if native execution is possible
	# if can_run_natively "$arch"; then
	# 	setup_native_runner "$target_prefix" "$compiler_dir"
	# 	return 0
	# fi

	local qemu_binary=$(get_qemu_binary_name "$arch")
	[[ -z "$qemu_binary" ]] && return 0

	local qemu_dir="${CROSS_COMPILER_DIR}/qemu-user-static-${QEMU_VERSION}"
	local qemu_path="${qemu_dir}/${qemu_binary}"

	# Download QEMU if not present
	if [[ ! -x "$qemu_path" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/qemu-user-static/releases/download/${QEMU_VERSION}/qemu-user-static-${host_platform}-musl.tgz"
		download_and_extract "${download_url}" "${qemu_dir}" || return 2
	fi

	if [[ -x "$qemu_path" ]]; then
		# Add QEMU directory to TARGET_PATH
		TARGET_PATH="${qemu_dir}${TARGET_PATH:+:$TARGET_PATH}"

		# Set runner using command name (relies on PATH) with sysroot
		local sysroot="${compiler_dir}/${target_prefix}"
		if [[ -d "${sysroot}/lib" ]]; then
			TARGET_RUNNER="${qemu_binary} -L ${sysroot}"
		else
			TARGET_RUNNER="${qemu_binary}"
		fi
		log_success "Configured QEMU runner: ${COLOR_LIGHT_YELLOW}${qemu_binary}${COLOR_LIGHT_GREEN} for ${COLOR_LIGHT_YELLOW}${arch}${COLOR_LIGHT_GREEN}"
	fi
}

# Setup Docker QEMU runner for cross-compiled Linux binaries
# Args: arch, target_prefix (e.g., armv6-linux-musleabihf), compiler_dir, libc (musl/gnu)
# Sets: TARGET_RUNNER
# Note: Uses docker cp instead of volume mounts for compatibility with various Docker implementations
setup_docker_qemu_runner() {
	local arch="$1"
	local target_prefix="$2"
	local compiler_dir="$3"
	local libc="$4"

	# Check if Docker is available
	if ! command -v docker &>/dev/null; then
		log_warning "Docker not found, skipping Docker QEMU runner setup"
		return 0
	fi

	local qemu_binary=$(get_qemu_binary_name "$arch")
	[[ -z "$qemu_binary" ]] && return 0

	# Determine host architecture for downloading Linux QEMU binary
	local host_arch="${HOST_ARCH}"
	[[ "$host_arch" == "arm64" ]] && host_arch="aarch64"
	[[ "$host_arch" == "amd64" ]] && host_arch="x86_64"

	# Download QEMU for Linux (to run inside Docker container)
	local qemu_dir="${CROSS_COMPILER_DIR}/qemu-user-static-${QEMU_VERSION}-linux-${host_arch}"
	local qemu_path="${qemu_dir}/${qemu_binary}"

	if [[ ! -x "$qemu_path" ]]; then
		local download_url="${GH_PROXY}https://github.com/zijiren233/qemu-user-static/releases/download/${QEMU_VERSION}/qemu-user-static-linux-${host_arch}-musl.tgz"
		download_and_extract "${download_url}" "${qemu_dir}" || return 2
	fi

	[[ ! -x "$qemu_path" ]] && return 1

	# Select Docker image based on libc type
	local docker_image
	if [[ "$libc" == "musl" ]]; then
		docker_image="alpine:latest"
	else
		docker_image="ubuntu:latest"
	fi

	# Create runner script
	local runner_script="${CROSS_COMPILER_DIR}/docker-qemu-runner-${arch}-${libc}.sh"
	local sysroot="${compiler_dir}/${target_prefix}"

	cat >"$runner_script" <<RUNNER_SCRIPT_EOF
#!/bin/bash
set -e

# Docker QEMU Runner Script
# This script runs a binary inside a Docker container using QEMU user-mode emulation

QEMU_PATH="${qemu_path}"
QEMU_BINARY="${qemu_binary}"
SYSROOT="${sysroot}"
DOCKER_IMAGE="${docker_image}"

if [[ \$# -lt 1 ]]; then
	echo "Usage: \$0 <binary> [args...]" >&2
	exit 1
fi

BINARY="\$1"
shift

if [[ ! -f "\$BINARY" ]]; then
	echo "Error: Binary not found: \$BINARY" >&2
	exit 1
fi

BINARY_NAME=\$(basename "\$BINARY")

# Create container (detached, interactive mode to keep it running)
CONTAINER_ID=\$(docker create --rm -i "\$DOCKER_IMAGE" /bin/sh -c "sleep infinity")

cleanup() {
	docker rm -f "\$CONTAINER_ID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Start the container
docker start "\$CONTAINER_ID" >/dev/null

# Copy QEMU binary to container
docker cp "\$QEMU_PATH" "\$CONTAINER_ID:/usr/bin/\$QEMU_BINARY" >/dev/null
docker exec "\$CONTAINER_ID" chmod +x "/usr/bin/\$QEMU_BINARY"

# Copy sysroot lib to container /lib
if [[ -d "\$SYSROOT/lib" ]]; then
	docker cp "\$SYSROOT" "\$CONTAINER_ID:/sysroot" >/dev/null
fi

# Copy the binary to execute
docker cp "\$BINARY" "\$CONTAINER_ID:/tmp/\$BINARY_NAME" >/dev/null
docker exec "\$CONTAINER_ID" chmod +x "/tmp/\$BINARY_NAME"

# Run the binary with QEMU
docker exec "\$CONTAINER_ID" /usr/bin/\$QEMU_BINARY -L /sysroot /tmp/\$BINARY_NAME "\$@"
RUNNER_SCRIPT_EOF

	chmod +x "$runner_script"

	TARGET_RUNNER="$runner_script"
	log_success "Configured Docker QEMU runner: ${COLOR_LIGHT_YELLOW}${qemu_binary}${COLOR_LIGHT_GREEN} for ${COLOR_LIGHT_YELLOW}${arch}${COLOR_LIGHT_GREEN} (image: ${COLOR_LIGHT_CYAN}${docker_image}${COLOR_LIGHT_GREEN})"
}

# Setup Rosetta runner for x86_64 Darwin binaries on ARM Darwin hosts
# Args: arch, rust_target
# Sets: TARGET_RUNNER
setup_rosetta_runner() {
	local arch="$1"
	local rust_target="$2"

	# Only setup Rosetta on Darwin hosts
	[[ "$HOST_OS" != "darwin" ]] && return 0

	# Only for x86_64 Darwin targets
	[[ "$arch" != "x86_64" ]] && return 0
	[[ "$rust_target" != *"-apple-darwin"* ]] && return 0

	# Check if host is ARM
	local host_arch="${HOST_ARCH}"
	[[ "$host_arch" == "arm64" ]] && host_arch="aarch64"
	[[ "$host_arch" != "aarch64" ]] && return 0

	TARGET_RUNNER="arch -x86_64"
	log_success "Configured Rosetta runner for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
}

# Get Linux cross-compilation environment
get_linux_env() {
	local arch="$1"
	local libc="$2"
	local abi="$3"
	local rust_target="$4"

	[[ -z "$libc" ]] && return 1

	local arch_prefix="$arch"
	local cross_compiler_name="${arch_prefix}-linux-${libc}${abi}-cross"
	local gcc_name="${arch_prefix}-linux-${libc}${abi}-gcc"
	local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}-${CROSS_DEPS_VERSION}"

	# Download compiler if not present
	if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
		download_cross_compiler "${compiler_dir}" "${download_url}" || return 2
	fi

	# Set environment variables
	set_cross_env \
		"${gcc_name}" \
		"${arch_prefix}-linux-${libc}${abi}-g++" \
		"${arch_prefix}-linux-${libc}${abi}-ar" \
		"${gcc_name}" \
		"${compiler_dir}/bin"

	# Add library search paths from gcc to rustc
	set_gcc_lib_paths "${compiler_dir}" "${arch_prefix}-linux-${libc}${abi}"

	# Setup runner only if the command needs to execute binaries
	if command_needs_runner "$COMMAND"; then
		case "$HOST_OS" in
		"darwin")
			setup_docker_qemu_runner "$arch_prefix" "${arch_prefix}-linux-${libc}${abi}" "${compiler_dir}" "$libc"
			;;
		"linux")
			setup_qemu_runner "$arch_prefix" "${arch_prefix}-linux-${libc}${abi}" "${compiler_dir}"
			;;
		esac
	fi

	log_success "Configured Linux ${COLOR_LIGHT_YELLOW}${libc}${COLOR_LIGHT_GREEN} toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
}

# Get Windows cross-compilation environment
get_windows_gnu_env() {
	local arch="$1"
	local rust_target="$2"

	# Validate architecture
	case "$arch" in
	"i686" | "x86_64") ;;
	*)
		log_error "Unsupported Windows architecture: ${COLOR_LIGHT_YELLOW}$arch${COLOR_LIGHT_RED}"
		return 1
		;;
	esac

	case "${HOST_OS}" in
	*"mingw"* | *"msys"* | *"cygwin"*)
		# Native compilation on Windows (MinGW/MSYS2/Cygwin environment)
		log_success "Using native Windows toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		return 0
		;;
	*)
		# Cross-compilation from Linux/macOS to Windows
		local cross_compiler_name="${arch}-w64-mingw32-cross"
		local gcc_name="${arch}-w64-mingw32-gcc"
		local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}-${CROSS_DEPS_VERSION}"

		# Download compiler if not present
		if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
			local host_platform=$(get_host_platform)
			local download_url="${GH_PROXY}https://github.com/zijiren233/cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
			download_cross_compiler "${compiler_dir}" "${download_url}" || return 2
		fi

		# Set environment variables
		set_cross_env \
			"${gcc_name}" \
			"${arch}-w64-mingw32-g++" \
			"${arch}-w64-mingw32-ar" \
			"${gcc_name}" \
			"${compiler_dir}/bin"

		# Add library search paths from gcc to rustc
		set_gcc_lib_paths "${compiler_dir}" "${arch}-w64-mingw32"

		# Setup wine runner for cross-compiled Windows binaries
		if command -v wine &>/dev/null; then
			TARGET_RUNNER="wine"
			log_success "Configured Wine runner for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		fi

		log_success "Configured Windows toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		;;
	esac
}

# Get FreeBSD cross-compilation environment
get_freebsd_env() {
	local arch="$1"
	local rust_target="$2"

	# Validate architecture
	case "$arch" in
	"x86_64" | "aarch64" | "powerpc" | "powerpc64" | "powerpc64le" | "riscv64") ;;
	*)
		log_error "Unsupported FreeBSD architecture: ${COLOR_LIGHT_YELLOW}$arch${COLOR_LIGHT_RED}"
		return 1
		;;
	esac

	local cross_compiler_name="${arch}-unknown-freebsd13-cross"
	local gcc_name="${arch}-unknown-freebsd13-gcc"
	local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}-${CROSS_DEPS_VERSION}"

	# Download compiler if not present
	if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
		download_cross_compiler "${compiler_dir}" "${download_url}" || return 2
	fi

	# Set environment variables
	set_cross_env \
		"${gcc_name}" \
		"${arch}-unknown-freebsd13-g++" \
		"${arch}-unknown-freebsd13-ar" \
		"${gcc_name}" \
		"${compiler_dir}/bin"

	# Add library search paths from gcc to rustc
	set_gcc_lib_paths "${compiler_dir}" "${arch}-unknown-freebsd13"

	log_success "Configured FreeBSD toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
}

# Get Darwin (macOS) environment
get_darwin_env() {
	local arch="$1"
	local rust_target="$2"

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		setup_rosetta_runner "$arch" "$rust_target"
		log_success "Using native macOS toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		;;
	"linux")
		# Cross-compilation from Linux to macOS using osxcross
		export OSXCROSS_MP_INC=1
		export MACOSX_DEPLOYMENT_TARGET=10.7

		# Map host architecture to osxcross directory name
		local host_arch_name=""
		case "${HOST_ARCH}" in
		"x86_64" | "amd64")
			host_arch_name="amd64"
			;;
		"aarch64" | "arm64")
			host_arch_name="aarch64"
			;;
		*)
			log_warning "Cross-compilation to macOS not supported on ${COLOR_LIGHT_YELLOW}${HOST_OS}/${HOST_ARCH}${COLOR_LIGHT_YELLOW}"
			return 1
			;;
		esac

		local osxcross_dir="${CROSS_COMPILER_DIR}/osxcross-${host_arch_name}"

		if [[ ! -x "${osxcross_dir}/bin/o64-clang" ]]; then
			# Determine download URL based on host architecture
			local ubuntu_version=$(lsb_release -rs 2>/dev/null || echo "20.04")
			[[ "$ubuntu_version" != *"."* ]] && ubuntu_version="20.04"

			local url_arch="${host_arch_name}"
			[[ "${host_arch_name}" == "amd64" ]] && url_arch="x86_64"

			local download_url="${GH_PROXY}https://github.com/zijiren233/osxcross/releases/download/v0.2.3/osxcross-15-5-linux-${url_arch}-gnu-ubuntu-${ubuntu_version}.tar.gz"
			download_and_extract "${download_url}" "${osxcross_dir}" || return 2
		fi

		# Fix linker rpath (sets TARGET_LIBRARY_PATH if using env var fallback)
		fix_darwin_linker_rpath "${osxcross_dir}" "${arch}"

		set_cross_env \
			"${arch}-apple-darwin24.5-clang" \
			"${arch}-apple-darwin24.5-clang++" \
			"${arch}-apple-darwin24.5-ar" \
			"${arch}-apple-darwin24.5-clang" \
			"${osxcross_dir}/bin:${osxcross_dir}/clang/bin"

		export MACOSX_DEPLOYMENT_TARGET="10.12"

		log_success "Configured osxcross toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		;;
	*)
		log_warning "Cross-compilation to macOS not supported on ${COLOR_LIGHT_YELLOW}${HOST_OS}${COLOR_LIGHT_YELLOW}"
		return 1
		;;
	esac
}

# Get Android environment
get_android_env() {
	local arch="$1"
	local rust_target="$2"

	local ndk_dir="${CROSS_COMPILER_DIR}/android-ndk-${HOST_OS}-${NDK_VERSION}"
	local clang_base_dir="${ndk_dir}/toolchains/llvm/prebuilt/${HOST_OS}-x86_64/bin"

	if [[ ! -d "${ndk_dir}" ]] || [[ ! -d "${clang_base_dir}" ]]; then
		local ndk_url="https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-${HOST_OS}.zip"
		download_and_extract "${ndk_url}" "${ndk_dir}" "zip" || return 2
		mv "$ndk_dir/android-ndk-${NDK_VERSION}/"* "$ndk_dir"
		rmdir "$ndk_dir/android-ndk-${NDK_VERSION}" || return 2
	fi

	# Map architecture to Android target prefix
	local API="${API:-24}"
	local clang_prefix
	case "$arch" in
	"armv7") clang_prefix="armv7a-linux-androideabi24" ;;
	"aarch64") clang_prefix="aarch64-linux-android24" ;;
	"i686") clang_prefix="i686-linux-android24" ;;
	"x86_64") clang_prefix="x86_64-linux-android24" ;;
	"riscv64") clang_prefix="riscv64-linux-android35" ;;
	*)
		log_error "Unsupported Android architecture: ${COLOR_LIGHT_YELLOW}$arch${COLOR_LIGHT_RED}"
		return 1
		;;
	esac

	# Set environment variables
	set_cross_env \
		"${clang_prefix}-clang" \
		"${clang_prefix}-clang++" \
		"llvm-ar" \
		"${clang_prefix}-clang" \
		"${clang_base_dir}"

	log_success "Configured Android toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
}

# Get iOS environment
get_ios_env() {
	local arch="$1"
	local rust_target="$2"
	local target_type="${3:-device}" # device or simulator

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		log_success "Using native macOS toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		;;
	"linux")
		# Map architecture to cross-compiler prefix
		case "$arch" in
		"aarch64")
			local arch_prefix="arm64"
			;;
		"x86_64")
			local arch_prefix="x86_64"
			;;
		*)
			log_warning "Unknown iOS architecture: ${COLOR_LIGHT_YELLOW}${arch}${COLOR_LIGHT_YELLOW}"
			return 2
			;;
		esac

		local cross_compiler_name="ios-${arch_prefix}-cross"
		if [[ "${arch}" == "x86_64" ]] || [[ "${target_type}" == "simulator" ]]; then
			cross_compiler_name="${cross_compiler_name}-simulator"
		fi

		# Set architecture-specific compiler names
		local clang_name="${arch_prefix}-apple-darwin11-clang"
		local clangxx_name="${arch_prefix}-apple-darwin11-clang++"
		local ar_name="${arch_prefix}-apple-darwin11-ar"
		local linker_name="${arch_prefix}-apple-darwin11-ld"

		local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}"

		if [[ ! -x "${compiler_dir}/bin/${clang_name}" ]]; then
			# Download cross-compiler
			local host_platform=$(get_host_platform)

			local ubuntu_version=""
			ubuntu_version=$(lsb_release -rs 2>/dev/null || echo "20.04")
			[[ "$ubuntu_version" != *"."* ]] && ubuntu_version="20.04"

			local ios_sdk_type="iPhoneOS"
			local ios_arch="${arch_prefix}"
			if [[ "${arch}" == "x86_64" ]] || [[ "${target_type}" == "simulator" ]]; then
				ios_sdk_type="iPhoneSimulator"
				ios_arch="${arch_prefix}"
			fi

			local download_url="${GH_PROXY}https://github.com/zijiren233/cctools-port/releases/download/v0.1.6/ioscross-${ios_sdk_type}18-5-${ios_arch}-${host_platform}-gnu-ubuntu-${ubuntu_version}.tar.gz"
			download_and_extract "$download_url" "${compiler_dir}" || return 2
		fi

		# Fix linker rpath (sets TARGET_LIBRARY_PATH if using env var fallback)
		fix_darwin_linker_rpath "${compiler_dir}" "${arch_prefix}"

		# Set SDKROOT to first folder in SDK directory
		set_ios_sdk_root "${cross_compiler_name}"

		# Set compiler paths based on target architecture
		set_cross_env \
			"${arch_prefix}-apple-darwin11-clang" \
			"${arch_prefix}-apple-darwin11-clang++" \
			"${arch_prefix}-apple-darwin11-ar" \
			"${arch_prefix}-apple-darwin11-ld" \
			"${compiler_dir}/bin:${compiler_dir}/clang/bin"

		log_success "Configured iOS toolchain for ${COLOR_LIGHT_YELLOW}$rust_target${COLOR_LIGHT_GREEN}"
		;;
	*)
		log_warning "Cross-compilation to iOS not supported on ${COLOR_LIGHT_YELLOW}${HOST_OS}${COLOR_LIGHT_YELLOW}"
		return 1
		;;
	esac
}

# -----------------------------------------------------------------------------
# Build Support Functions
# -----------------------------------------------------------------------------

# Print environment variables if any exist
# Args: build_env array (passed by reference)
print_env_vars() {
	if [[ ${#build_env[@]} -gt 0 ]]; then
		log_info "Environment variables:"
		for env_var in "${build_env[@]}"; do
			local key="${env_var%%=*}"
			local value="${env_var#*=}"
			echo -e "  ${COLOR_LIGHT_CYAN}${key}${COLOR_RESET}=${COLOR_LIGHT_YELLOW}${value}${COLOR_RESET}"
		done
	fi
}

# Add flag to cargo command if condition is true
# Args: condition, flag
add_flag() {
	[[ "$1" == "true" ]] && cargo_cmd="$cargo_cmd $2"
}

# Add option with value to cargo command if value is non-empty (space separated)
# Args: value, option_name
add_option() {
	[[ -n "$1" ]] && cargo_cmd="$cargo_cmd $2 $1"
}

# Add option with value using = separator (e.g., -Zbuild-std-features=value)
# Args: value, option_name
add_option_eq() {
	[[ -n "$1" ]] && cargo_cmd="$cargo_cmd $2=$1"
}

# Add argument(s) to cargo command if condition is true
# Args: condition, args...
add_arg_if() {
	local condition="$1"
	shift
	[[ "$condition" == "true" ]] && cargo_cmd="$cargo_cmd $*"
}

# Add option with optional value using space separator (flag if true, flag value otherwise)
# Args: value, option_name
add_option_or_flag() {
	local value="$1"
	local option="$2"
	if [[ "$value" == "true" ]]; then
		cargo_cmd="$cargo_cmd $option"
	elif [[ -n "$value" && "$value" != "false" ]]; then
		cargo_cmd="$cargo_cmd $option $value"
	fi
}

# Add option with optional value using = separator (flag if true, flag=value otherwise)
# Args: value, option_name
add_option_eq_or_flag() {
	local value="$1"
	local option="$2"
	if [[ "$value" == "true" ]]; then
		cargo_cmd="$cargo_cmd $option"
	elif [[ -n "$value" && "$value" != "false" ]]; then
		cargo_cmd="$cargo_cmd $option=$value"
	fi
}

# Add arguments unconditionally to cargo command
# Args: args...
add_args() {
	cargo_cmd="$cargo_cmd $*"
}

# Clean cache
clean_cache() {
	if [[ "$CLEAN_CACHE" == "true" ]]; then
		log_info "Cleaning cache..."
		cargo clean 2>/dev/null || true
	fi
}

# Execute command for a specific target
# https://doc.rust-lang.org/cargo/reference/config.html
execute_target() {
	local rust_target="$1"
	local command="$2"

	echo -e "${COLOR_LIGHT_GRAY}$(print_separator)${COLOR_RESET}"
	echo -e "${COLOR_LIGHT_MAGENTA}Executing ${command} for ${rust_target}...${COLOR_RESET}" # Keep as-is for visibility

	# Clean cache if requested
	clean_cache "$rust_target" || return $?

	# Get cross-compilation environment and capture variables
	get_cross_env "$rust_target" || return $?

	# Prepare environment variables
	local build_env=()

	# Set up PATH with target-specific tools if needed
	if [[ -n "$TARGET_PATH" ]]; then
		build_env+=("PATH=${TARGET_PATH}:${PATH}")
	fi

	# Set up LD_LIBRARY_PATH if needed (e.g., for Darwin linker when patchelf/chrpath not available)
	if [[ -n "$TARGET_LIBRARY_PATH" ]]; then
		if [[ "$HOST_OS" == "darwin" ]]; then
			build_env+=("DYLD_LIBRARY_PATH=${TARGET_LIBRARY_PATH}${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}")
		else
			build_env+=("LD_LIBRARY_PATH=${TARGET_LIBRARY_PATH}${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}")
		fi
	fi

	# Set up environment based on target
	local target_upper=$(echo "$rust_target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')

	# Enable sccache if requested
	if [[ "$ENABLE_SCCACHE" == "true" ]]; then
		RUSTC_WRAPPER="sccache"
	fi

	# Sccache environment variables
	add_env_if_set "SCCACHE_DIR" "$SCCACHE_DIR"
	add_env_if_set "SCCACHE_CACHE_SIZE" "$SCCACHE_CACHE_SIZE"
	add_env_if_set "SCCACHE_IDLE_TIMEOUT" "$SCCACHE_IDLE_TIMEOUT"
	add_env_if_set "SCCACHE_LOG" "$SCCACHE_LOG"
	add_env_if_set "SCCACHE_NO_DAEMON" "$SCCACHE_NO_DAEMON"
	add_env_if_set "SCCACHE_DIRECT" "$SCCACHE_DIRECT"
	add_env_if_set "SCCACHE_ERROR_LOG" "$SCCACHE_ERROR_LOG"
	add_env_if_set "SCCACHE_RECACHE" "$SCCACHE_RECACHE"
	add_env_if_set "SCCACHE_IGNORE_SERVER_IO_ERROR" "$SCCACHE_IGNORE_SERVER_IO_ERROR"

	# Sccache S3 backend
	add_env_if_set "SCCACHE_BUCKET" "$SCCACHE_BUCKET"
	add_env_if_set "SCCACHE_ENDPOINT" "$SCCACHE_ENDPOINT"
	add_env_if_set "SCCACHE_REGION" "$SCCACHE_REGION"
	add_env_if_set "SCCACHE_S3_USE_SSL" "$SCCACHE_S3_USE_SSL"
	add_env_if_set "SCCACHE_S3_KEY_PREFIX" "$SCCACHE_S3_KEY_PREFIX"

	# Sccache Redis backend
	add_env_if_set "SCCACHE_REDIS_ENDPOINT" "$SCCACHE_REDIS_ENDPOINT"
	add_env_if_set "SCCACHE_REDIS_USERNAME" "$SCCACHE_REDIS_USERNAME"
	add_env_if_set "SCCACHE_REDIS_PASSWORD" "$SCCACHE_REDIS_PASSWORD"
	add_env_if_set "SCCACHE_REDIS_DB" "$SCCACHE_REDIS_DB"
	add_env_if_set "SCCACHE_REDIS_EXPIRATION" "$SCCACHE_REDIS_EXPIRATION"
	add_env_if_set "SCCACHE_REDIS_KEY_PREFIX" "$SCCACHE_REDIS_KEY_PREFIX"

	# Sccache GCS backend
	add_env_if_set "SCCACHE_GCS_BUCKET" "$SCCACHE_GCS_BUCKET"
	add_env_if_set "SCCACHE_GCS_KEY_PREFIX" "$SCCACHE_GCS_KEY_PREFIX"
	add_env_if_set "SCCACHE_GCS_RW_MODE" "$SCCACHE_GCS_RW_MODE"
	add_env_if_set "SCCACHE_GCS_KEY_PATH" "$SCCACHE_GCS_KEY_PATH"

	# Sccache Azure backend
	add_env_if_set "SCCACHE_AZURE_CONNECTION_STRING" "$SCCACHE_AZURE_CONNECTION_STRING"
	add_env_if_set "SCCACHE_AZURE_BLOB_CONTAINER" "$SCCACHE_AZURE_BLOB_CONTAINER"
	add_env_if_set "SCCACHE_AZURE_KEY_PREFIX" "$SCCACHE_AZURE_KEY_PREFIX"

	# Sccache GitHub Actions backend
	add_env_if_set "SCCACHE_GHA_CACHE_TO" "$SCCACHE_GHA_CACHE_TO"
	add_env_if_set "SCCACHE_GHA_CACHE_FROM" "$SCCACHE_GHA_CACHE_FROM"

	# CC crate environment variables
	add_env_if_set "CC_ENABLE_DEBUG_OUTPUT" "${CC_ENABLE_DEBUG_OUTPUT:-$([[ $VERBOSE_LEVEL -gt 0 ]] && echo 1)}"
	add_env_if_set "CRATE_CC_NO_DEFAULTS" "$CRATE_CC_NO_DEFAULTS"
	add_env_if_set "CC_SHELL_ESCAPED_FLAGS" "$CC_SHELL_ESCAPED_FLAGS"
	add_env_if_set "CC_FORCE_DISABLE" "$CC_FORCE_DISABLE"
	add_env_if_set "RUSTC_WRAPPER" "$RUSTC_WRAPPER"
	add_env_if_set "CC_KNOWN_WRAPPER_CUSTOM" "$CC_KNOWN_WRAPPER_CUSTOM"

	# Compiler and flags - target-specific
	add_env_if_set "CC_${target_upper}" "$TARGET_CC"
	add_env_if_set "CC" "$TARGET_CC"
	add_env_if_set "CXX_${target_upper}" "$TARGET_CXX"
	add_env_if_set "CXX" "$TARGET_CXX"
	add_env_if_set "AR_${target_upper}" "$TARGET_AR"
	add_env_if_set "AR" "$TARGET_AR"
	add_env_if_set "CARGO_TARGET_${target_upper}_LINKER" "$TARGET_LINKER"
	add_env_if_set "CARGO_TARGET_${target_upper}_RUNNER" "$TARGET_RUNNER"

	# Compiler flags
	add_env_if_set "CFLAGS_${target_upper}" "$CFLAGS"
	add_env_if_set "CFLAGS" "$CFLAGS"
	add_env_if_set "CXXFLAGS_${target_upper}" "$CXXFLAGS"
	add_env_if_set "CXXFLAGS" "$CXXFLAGS"
	add_env_if_set "CXXSTDLIB_${target_upper}" "$CXXSTDLIB"
	add_env_if_set "CXXSTDLIB" "$CXXSTDLIB"

	# SDK and library paths
	add_env_if_set "SDKROOT" "$SDKROOT"

	if [ "$rust_target" = "$HOST_TRIPLE" ]; then
		# https://github.com/rust-lang/cargo/issues/8147
		# https://github.com/rust-lang/cargo/pull/9322
		# https://github.com/rust-lang/cargo/pull/9603
		build_env+=("CARGO_UNSTABLE_HOST_CONFIG=true")
		build_env+=("CARGO_UNSTABLE_TARGET_APPLIES_TO_HOST=true")
		build_env+=("CARGO_TARGET_APPLIES_TO_HOST=false")
	fi

	local rustflags="$RUSTFLAGS"

	if [[ -n "$TARGET_RUSTFLAGS" ]]; then
		rustflags="${rustflags:+$rustflags }$TARGET_RUSTFLAGS"
	fi

	if [[ "$CRT_STATIC" == "true" ]]; then
		rustflags="${rustflags:+$rustflags }-C target-feature=+crt-static"
	elif [[ "$CRT_STATIC" == "false" ]]; then
		rustflags="${rustflags:+$rustflags }-C target-feature=-crt-static"
	fi

	# Add panic=immediate-abort flag if specified (requires nightly-2025-09-24+)
	if [[ "$PANIC_IMMEDIATE_ABORT" == "true" ]]; then
		rustflags="${rustflags:+$rustflags }-Zunstable-options -Cpanic=immediate-abort"
		# Auto-enable build-std if not already set
		[[ -z "$BUILD_STD" || "$BUILD_STD" == "false" ]] && BUILD_STD="true"
	fi

	# Add fmt-debug flag if specified
	if [[ -n "$FMT_DEBUG" ]]; then
		rustflags="${rustflags:+$rustflags }-Zfmt-debug=$FMT_DEBUG"
	fi

	# Add location-detail flag if specified
	if [[ -n "$LOCATION_DETAIL" ]]; then
		rustflags="${rustflags:+$rustflags }-Zlocation-detail=$LOCATION_DETAIL"
	fi

	# Add rustflags from command-line arguments (--rustflags)
	if [[ ${#ADDITIONAL_RUSTFLAGS_ARRAY[@]} -gt 0 ]]; then
		for flag in "${ADDITIONAL_RUSTFLAGS_ARRAY[@]}"; do
			rustflags="${rustflags:+$rustflags }$flag"
		done
	fi

	# Add rustflags from environment variable
	if [[ -n "$ADDITIONAL_RUSTFLAGS" ]]; then
		rustflags="${rustflags:+$rustflags }$ADDITIONAL_RUSTFLAGS"
	fi

	add_env_if_set "RUSTFLAGS" "$rustflags"
	add_env_if_set "CARGO_TRIM_PATHS" "$CARGO_TRIM_PATHS"
	# https://doc.rust-lang.org/unstable-book/compiler-environment-variables/RUSTC_BOOTSTRAP.html
	add_env_if_set "RUSTC_BOOTSTRAP" "$RUSTC_BOOTSTRAP"

	# Prepare command
	local cargo_cmd="cargo"
	[[ -n "$TOOLCHAIN" ]] && add_args "+$TOOLCHAIN"

	add_args "$command"

	# Add -C flag if specified (must come before command)
	[[ -n "$CARGO_CWD" ]] && add_args "-C $CARGO_CWD"

	# Add -Z flags if specified (must come before command)
	if [[ ${#CARGO_Z_FLAGS_ARRAY[@]} -gt 0 ]]; then
		for flag in "${CARGO_Z_FLAGS_ARRAY[@]}"; do
			add_args "-Z $flag"
		done
	fi

	# Add --config flags if specified (must come before command)
	if [[ ${#CARGO_CONFIG_ARRAY[@]} -gt 0 ]]; then
		for config in "${CARGO_CONFIG_ARRAY[@]}"; do
			add_args "--config $config"
		done
	fi

	if [[ "$NO_CARGO_TARGET" != "true" ]]; then
		add_args "--target $rust_target"
	fi

	# Build profile and features
	[[ "$command" == "build" && "$PROFILE" == "release" ]] && add_args "--release"
	add_option "$FEATURES" "--features"
	add_flag "$NO_DEFAULT_FEATURES" "--no-default-features"
	add_flag "$ALL_FEATURES" "--all-features"

	# Package and target selection
	add_option "$PACKAGE" "--package"
	add_flag "$BUILD_WORKSPACE" "--workspace"
	add_option "$EXCLUDE" "--exclude"
	add_option "$BIN_TARGET" "--bin"
	add_flag "$BUILD_BINS" "--bins"
	add_flag "$BUILD_LIB" "--lib"
	add_option "$EXAMPLE_TARGET" "--example"
	add_flag "$BUILD_EXAMPLES" "--examples"
	add_option "$TEST_TARGET" "--test"
	add_flag "$BUILD_TESTS" "--tests"
	add_option "$BENCH_TARGET" "--bench"
	add_flag "$BUILD_BENCHES" "--benches"
	add_flag "$BUILD_ALL_TARGETS" "--all-targets"
	add_option "$MANIFEST_PATH" "--manifest-path"

	# Build-std flag: BUILD_STD (user specified) takes precedence over TARGET_BUILD_STD (auto-detected)
	local build_std_value=""
	[[ -n "$TARGET_BUILD_STD" && "$TARGET_BUILD_STD" != "false" ]] && build_std_value="$TARGET_BUILD_STD"
	[[ -n "$BUILD_STD" && "$BUILD_STD" != "false" ]] && build_std_value="$BUILD_STD"

	if [[ -n "$build_std_value" ]]; then
		if [[ "$build_std_value" == "true" ]]; then
			add_option_eq_or_flag "$(get_build_std_config "$rust_target")" "-Zbuild-std"
		else
			add_option_eq_or_flag "$build_std_value" "-Zbuild-std"
		fi
		add_rust_src "$rust_target" "$TOOLCHAIN" || return $?
	fi

	# Build-std-features flag (requires = separator)
	add_option_eq "$BUILD_STD_FEATURES" "-Zbuild-std-features"

	# Output and verbosity
	for ((i = 0; i < VERBOSE_LEVEL; i++)); do
		add_args "--verbose"
	done
	add_flag "$QUIET" "--quiet"
	add_option "$MESSAGE_FORMAT" "--message-format"
	add_option "$COLOR" "--color"
	add_flag "$BUILD_PLAN" "--build-plan"
	add_option_eq_or_flag "$TIMINGS" "--timings"

	# Dependency and version management
	add_flag "$IGNORE_RUST_VERSION" "--ignore-rust-version"
	add_flag "$LOCKED" "--locked"
	add_flag "$OFFLINE" "--offline"
	add_flag "$FROZEN" "--frozen"
	add_option "$LOCKFILE_PATH" "--lockfile-path"

	# Build configuration
	add_option "$JOBS" "--jobs"
	add_flag "$KEEP_GOING" "--keep-going"
	add_flag "$FUTURE_INCOMPAT_REPORT" "--future-incompat-report"
	add_flag "$NO_EMBED_METADATA" "-Zno-embed-metadata"
	add_option "$CARGO_TARGET_DIR" "--target-dir"
	add_option "$ARTIFACT_DIR" "--artifact-dir"

	# Additional arguments
	[[ -n "$CARGO_ARGS" ]] && add_args "$CARGO_ARGS"

	# Passthrough arguments (must be last, after --)
	[[ -n "$CARGO_PASSTHROUGH_ARGS" ]] && add_args "$CARGO_PASSTHROUGH_ARGS"

	print_env_vars

	log_info "Run command:"
	echo -e "  ${COLOR_LIGHT_CYAN}${cargo_cmd}${COLOR_RESET}"

	local start_time=$(date +%s)

	# Execute command with environment variables
	if [[ ${#build_env[@]} -gt 0 ]]; then
		env "${build_env[@]}" $cargo_cmd || return $?
	else
		$cargo_cmd || return $?
	fi

	local end_time=$(date +%s)

	# Report success
	local command_capitalized="$(echo "${command:0:1}" | tr '[:lower:]' '[:upper:]')${command:1}"
	log_success "${command_capitalized} successful: ${COLOR_LIGHT_YELLOW}${rust_target}${COLOR_LIGHT_GREEN} (took ${COLOR_LIGHT_YELLOW}$((end_time - start_time))s${COLOR_LIGHT_GREEN})"
}

# -----------------------------------------------------------------------------
# Target Pattern Expansion
# -----------------------------------------------------------------------------

# Expand target patterns (e.g., "linux/*" or "all")
expand_targets() {
	local targets="$1"
	local expanded=""

	# Normalize: replace newlines with commas, then split by comma
	targets=$(echo "$targets" | tr '\n' ',' | sed 's/,\+/,/g')
	IFS=',' read -ra TARGET_ARRAY <<<"$targets"
	for target in "${TARGET_ARRAY[@]}"; do
		target=$(echo "$target" | xargs) # Trim whitespace
		[[ -z "$target" ]] && continue   # Skip empty entries

		if [[ "$target" == "all" ]]; then
			# Return all supported targets
			while IFS= read -r line; do
				[[ -z "$line" ]] && continue
				local key="${line%%=*}"
				expanded="${expanded}${key},"
			done <<<"$TOOLCHAIN_CONFIG"
		elif [[ "$target" == *"*"* ]]; then
			# Pattern matching (e.g., "*-linux-musl")
			while IFS= read -r line; do
				[[ -z "$line" ]] && continue
				local key="${line%%=*}"
				if [[ "$key" == $target ]]; then
					expanded="${expanded}${key},"
				fi
			done <<<"$TOOLCHAIN_CONFIG"
		else
			# Direct target
			expanded="${expanded}${target},"
		fi
	done

	# Remove trailing comma and duplicates
	expanded="${expanded%,}"
	echo "$expanded" | tr ',' '\n' | sort -u | paste -sd ',' -
}

set_github_output() {
	if [[ -z "$GITHUB_OUTPUT" ]]; then
		return
	fi

	# Convert comma-separated targets to JSON array using jq
	local json_array
	if command -v jq &>/dev/null; then
		# Use jq for proper JSON encoding (handles special characters and escaping)
		json_array=$(echo "$TARGETS" | tr ',' '\n' | jq -R -s -c 'split("\n") | map(select(length > 0))')
	else
		# Fallback: manual JSON array construction
		json_array="["
		IFS=',' read -ra TARGET_ARRAY <<<"$TARGETS"
		local first=true
		for target in "${TARGET_ARRAY[@]}"; do
			if [[ "$first" == "true" ]]; then
				first=false
			else
				json_array+=","
			fi
			json_array+="\"$target\""
		done
		json_array+="]"
	fi
	echo "targets=$json_array" >>$GITHUB_OUTPUT
}

# -----------------------------------------------------------------------------
# Argument Parsing and Main Script
# -----------------------------------------------------------------------------

# Initialize variables
set_default "VERBOSE_LEVEL" "0"
set_default "SOURCE_DIR" "${DEFAULT_SOURCE_DIR}"
SOURCE_DIR="$(cd "${SOURCE_DIR}" && pwd)"
set_default "PROFILE" "${DEFAULT_PROFILE}"
set_default "CROSS_COMPILER_DIR" "${DEFAULT_CROSS_COMPILER_DIR}"
set_default "CROSS_DEPS_VERSION" "${DEFAULT_CROSS_DEPS_VERSION}"
set_default "NDK_VERSION" "${DEFAULT_NDK_VERSION}"
set_default "QEMU_VERSION" "${DEFAULT_QEMU_VERSION}"
set_default "COMMAND" "${DEFAULT_COMMAND}"
set_default "TOOLCHAIN" "${DEFAULT_TOOLCHAIN}"

# Helper function to check if the next argument is an option or command
is_next_arg_option() {
	if [[ $# -le 1 ]]; then
		return 1
	fi

	local next_arg="$2"

	# Check if it's a command
	if [[ "$next_arg" =~ ^(${SUPPORTED_COMMANDS})$ ]]; then
		return 0
	fi

	# Check if it's a long option (starts with --)
	if [[ "$next_arg" =~ ^-- ]]; then
		return 0
	fi

	# Check if it's a short option (-X or -X=*)
	# This matches single letter options like -h, -v, -Z, -C, -t, -j, -p, -F, etc.
	if [[ "$next_arg" =~ ^-[a-zA-Z](=.*)?$ ]]; then
		return 0
	fi

	return 1
}

# Parse command-line arguments
while [[ $# -gt 0 ]]; do
	# Check if current argument is +toolchain (e.g., +nightly, +stable, +1.70.0)
	# Only parse if TOOLCHAIN is not already set to avoid affecting other arguments
	if [[ -z "$TOOLCHAIN" && "$1" =~ ^\+(.+)$ ]]; then
		TOOLCHAIN="${BASH_REMATCH[1]}"
		shift
		continue
	fi

	# Check if current argument is a command (including short aliases)
	if [[ "$1" =~ ^(${SUPPORTED_COMMANDS})$ ]]; then
		COMMAND="$1"
		shift
		continue
	fi

	# Support arbitrary number of v's: -v, -vv, -vvv, -vvvvv, etc.
	if [[ "$1" =~ ^-v+$ ]]; then
		v_str=${1#-}
		VERBOSE_LEVEL=$((VERBOSE_LEVEL + ${#v_str}))
		shift
		continue
	fi

	case "${1}" in
	-h | --help)
		print_help
		exit 0
		;;
	--profile=*)
		PROFILE="${1#*=}"
		;;
	--profile)
		shift
		PROFILE="$(parse_option_value "--profile" "$@")"
		;;
	--command=*)
		COMMAND="${1#*=}"
		;;
	--command)
		shift
		COMMAND="$(parse_option_value "--command" "$@")"
		;;
	-F=* | --features=*)
		FEATURES="${1#*=}"
		;;
	-F?*)
		# Support -Ffoo format (no space or equals)
		FEATURES="${1#-F}"
		;;
	-F | --features)
		shift
		FEATURES="$(parse_option_value "--features" "$@")"
		;;
	--no-default-features)
		NO_DEFAULT_FEATURES="true"
		;;
	--all-features)
		ALL_FEATURES="true"
		;;
	-t=* | --targets=* | --target=*)
		if [[ -n "$TARGETS" ]]; then
			TARGETS="${TARGETS},${1#*=}"
		else
			TARGETS="${1#*=}"
		fi
		;;
	-t?*)
		# Support -ttarget format (no space or equals)
		__target_value="${1#-t}"
		if [[ -n "$TARGETS" ]]; then
			TARGETS="${TARGETS},${__target_value}"
		else
			TARGETS="${__target_value}"
		fi
		;;
	-t | --targets | --target)
		shift
		__target_value="$(parse_option_value "--targets" "$@")"
		if [[ -n "$TARGETS" ]]; then
			TARGETS="${TARGETS},${__target_value}"
		else
			TARGETS="${__target_value}"
		fi
		;;
	--show-all-targets)
		echo -e "${COLOR_LIGHT_GREEN}Supported Rust targets:${COLOR_RESET}"
		echo "$TOOLCHAIN_CONFIG" | grep -v '^$' | cut -d'=' -f1 | sort | while read -r target; do
			[[ -n "$target" ]] && echo "  ${COLOR_LIGHT_CYAN}$target${COLOR_RESET}"
		done
		exit 0
		;;
	--github-proxy-mirror=*)
		GH_PROXY="${1#*=}"
		;;
	--github-proxy-mirror)
		shift
		GH_PROXY="$(parse_option_value "--github-proxy-mirror" "$@")"
		;;
	--cross-compiler-dir=*)
		CROSS_COMPILER_DIR="${1#*=}"
		;;
	--cross-compiler-dir)
		shift
		CROSS_COMPILER_DIR="$(parse_option_value "--cross-compiler-dir" "$@")"
		;;
	--ndk-version=*)
		NDK_VERSION="${1#*=}"
		;;
	--ndk-version)
		shift
		NDK_VERSION="$(parse_option_value "--ndk-version" "$@")"
		;;
	-p=* | --package=*)
		PACKAGE="${1#*=}"
		;;
	-p?*)
		# Support -ppkg format (no space or equals)
		PACKAGE="${1#-p}"
		;;
	-p | --package)
		shift
		PACKAGE="$(parse_option_value "--package" "$@")"
		;;
	--exclude=*)
		EXCLUDE="${1#*=}"
		;;
	--exclude)
		shift
		EXCLUDE="$(parse_option_value "--exclude" "$@")"
		;;
	--bin=*)
		BIN_TARGET="${1#*=}"
		;;
	--bin)
		shift
		BIN_TARGET="$(parse_option_value "--bin" "$@")"
		;;
	--bins)
		BUILD_BINS="true"
		;;
	--lib)
		BUILD_LIB="true"
		;;
	--example=*)
		EXAMPLE_TARGET="${1#*=}"
		;;
	--example)
		shift
		EXAMPLE_TARGET="$(parse_option_value "--example" "$@")"
		;;
	--examples)
		BUILD_EXAMPLES="true"
		;;
	--test=*)
		TEST_TARGET="${1#*=}"
		;;
	--test)
		shift
		TEST_TARGET="$(parse_option_value "--test" "$@")"
		;;
	--tests)
		BUILD_TESTS="true"
		;;
	--bench=*)
		BENCH_TARGET="${1#*=}"
		;;
	--bench)
		shift
		BENCH_TARGET="$(parse_option_value "--bench" "$@")"
		;;
	--benches)
		BUILD_BENCHES="true"
		;;
	--all-targets)
		BUILD_ALL_TARGETS="true"
		;;
	-r | --release)
		PROFILE="release"
		;;
	-q | --quiet)
		QUIET="true"
		;;
	--message-format=*)
		MESSAGE_FORMAT="${1#*=}"
		;;
	--message-format)
		shift
		MESSAGE_FORMAT="$(parse_option_value "--message-format" "$@")"
		;;
	--ignore-rust-version)
		IGNORE_RUST_VERSION="true"
		;;
	--locked)
		LOCKED="true"
		;;
	--offline)
		OFFLINE="true"
		;;
	--frozen)
		FROZEN="true"
		;;
	-j=* | --jobs=*)
		JOBS="${1#*=}"
		;;
	-j?*)
		# Support -j4 format (no space or equals)
		JOBS="${1#-j}"
		;;
	-j | --jobs)
		shift
		JOBS="$(parse_option_value "--jobs" "$@")"
		;;
	--keep-going)
		KEEP_GOING="true"
		;;
	--future-incompat-report)
		FUTURE_INCOMPAT_REPORT="true"
		;;
	--workspace)
		BUILD_WORKSPACE="true"
		;;
	--manifest-path=*)
		MANIFEST_PATH="${1#*=}"
		;;
	--manifest-path)
		shift
		MANIFEST_PATH="$(parse_option_value "--manifest-path" "$@")"
		;;
	--use-default-linker)
		USE_DEFAULT_LINKER="true"
		;;
	--cc=*)
		CC="${1#*=}"
		;;
	--cc)
		shift
		CC="$(parse_option_value "--cc" "$@")"
		;;
	--cxx=*)
		CXX="${1#*=}"
		;;
	--cxx)
		shift
		CXX="$(parse_option_value "--cxx" "$@")"
		;;
	--ar=*)
		AR="${1#*=}"
		;;
	--ar)
		shift
		AR="$(parse_option_value "--ar" "$@")"
		;;
	--linker=*)
		LINKER="${1#*=}"
		;;
	--linker)
		shift
		LINKER="$(parse_option_value "--linker" "$@")"
		;;
	--cflags=*)
		CFLAGS="${1#*=}"
		;;
	--cflags)
		shift
		CFLAGS="$(parse_option_value "--cflags" "$@")"
		;;
	--cxxflags=*)
		CXXFLAGS="${1#*=}"
		;;
	--cxxflags)
		shift
		CXXFLAGS="$(parse_option_value "--cxxflags" "$@")"
		;;
	--cxxstdlib=*)
		CXXSTDLIB="${1#*=}"
		;;
	--cxxstdlib)
		shift
		CXXSTDLIB="$(parse_option_value "--cxxstdlib" "$@")"
		;;
	--rustc-wrapper=*)
		RUSTC_WRAPPER="${1#*=}"
		;;
	--rustc-wrapper)
		shift
		RUSTC_WRAPPER="$(parse_option_value "--rustc-wrapper" "$@")"
		;;
	--enable-sccache)
		ENABLE_SCCACHE="true"
		;;
	--sccache-dir=*)
		SCCACHE_DIR="${1#*=}"
		;;
	--sccache-dir)
		shift
		SCCACHE_DIR="$(parse_option_value "--sccache-dir" "$@")"
		;;
	--sccache-cache-size=*)
		SCCACHE_CACHE_SIZE="${1#*=}"
		;;
	--sccache-cache-size)
		shift
		SCCACHE_CACHE_SIZE="$(parse_option_value "--sccache-cache-size" "$@")"
		;;
	--sccache-idle-timeout=*)
		SCCACHE_IDLE_TIMEOUT="${1#*=}"
		;;
	--sccache-idle-timeout)
		shift
		SCCACHE_IDLE_TIMEOUT="$(parse_option_value "--sccache-idle-timeout" "$@")"
		;;
	--sccache-log=*)
		SCCACHE_LOG="${1#*=}"
		;;
	--sccache-log)
		shift
		SCCACHE_LOG="$(parse_option_value "--sccache-log" "$@")"
		;;
	--sccache-no-daemon)
		SCCACHE_NO_DAEMON="1"
		;;
	--sccache-direct)
		SCCACHE_DIRECT="true"
		;;
	--cc-no-defaults)
		CRATE_CC_NO_DEFAULTS="1"
		;;
	--cc-shell-escaped-flags)
		CC_SHELL_ESCAPED_FLAGS="1"
		;;
	--cc-enable-debug)
		CC_ENABLE_DEBUG_OUTPUT="1"
		;;
	--rustflags=*)
		ADDITIONAL_RUSTFLAGS_ARRAY+=("${1#*=}")
		;;
	--rustflags)
		shift
		ADDITIONAL_RUSTFLAGS_ARRAY+=("$(parse_option_value "--rustflags" "$@")")
		;;
	--crt-static=* | --static-crt=*)
		CRT_STATIC="${1#*=}"
		[[ -z "$CRT_STATIC" ]] && CRT_STATIC="true"
		;;
	--crt-static | --static-crt)
		if is_next_arg_option "$@"; then
			CRT_STATIC="true"
		else
			if [[ $# -gt 1 ]]; then
				shift
				CRT_STATIC="$1"
			else
				CRT_STATIC="true"
			fi
		fi
		;;
	--panic-immediate-abort)
		PANIC_IMMEDIATE_ABORT="true"
		;;
	--fmt-debug=*)
		FMT_DEBUG="${1#*=}"
		;;
	--fmt-debug)
		shift
		FMT_DEBUG="$(parse_option_value "--fmt-debug" "$@")"
		;;
	--location-detail=*)
		LOCATION_DETAIL="${1#*=}"
		;;
	--location-detail)
		shift
		LOCATION_DETAIL="$(parse_option_value "--location-detail" "$@")"
		;;
	--build-std=*)
		BUILD_STD="${1#*=}"
		[[ -z "$BUILD_STD" ]] && BUILD_STD="true"
		;;
	--build-std)
		if is_next_arg_option "$@"; then
			BUILD_STD="true"
		else
			if [[ $# -gt 1 ]]; then
				shift
				BUILD_STD="$1"
			else
				BUILD_STD="true"
			fi
		fi
		;;
	--build-std-features=*)
		BUILD_STD_FEATURES="${1#*=}"
		;;
	--build-std-features)
		shift
		BUILD_STD_FEATURES="$(parse_option_value "--build-std-features" "$@")"
		;;
	--args=* | --cargo-args=*)
		CARGO_ARGS="${1#*=}"
		;;
	--args | --cargo-args)
		shift
		CARGO_ARGS="$(parse_option_value "--args" "$@")"
		;;
	--toolchain=*)
		TOOLCHAIN="${1#*=}"
		;;
	--toolchain)
		shift
		TOOLCHAIN="$(parse_option_value "--toolchain" "$@")"
		;;
	--cargo-trim-paths=* | --trim-paths=*)
		CARGO_TRIM_PATHS="${1#*=}"
		;;
	--cargo-trim-paths | --trim-paths)
		if is_next_arg_option "$@"; then
			CARGO_TRIM_PATHS="true"
		else
			if [[ $# -gt 1 ]]; then
				shift
				CARGO_TRIM_PATHS="$1"
			else
				CARGO_TRIM_PATHS="true"
			fi
		fi
		;;
	--no-embed-metadata)
		NO_EMBED_METADATA="true"
		;;
	--rustc-bootstrap=*)
		RUSTC_BOOTSTRAP="${1#*=}"
		[[ -z "$RUSTC_BOOTSTRAP" ]] && RUSTC_BOOTSTRAP="1"
		;;
	--rustc-bootstrap)
		if is_next_arg_option "$@"; then
			RUSTC_BOOTSTRAP="1"
		else
			if [[ $# -gt 1 ]]; then
				shift
				RUSTC_BOOTSTRAP="$1"
			else
				RUSTC_BOOTSTRAP="1"
			fi
		fi
		;;
	--target-dir=*)
		CARGO_TARGET_DIR="${1#*=}"
		;;
	--target-dir)
		shift
		CARGO_TARGET_DIR="$(parse_option_value "--target-dir" "$@")"
		;;
	--artifact-dir=*)
		ARTIFACT_DIR="${1#*=}"
		;;
	--artifact-dir)
		shift
		ARTIFACT_DIR="$(parse_option_value "--artifact-dir" "$@")"
		;;
	--color=*)
		COLOR="${1#*=}"
		;;
	--color)
		shift
		COLOR="$(parse_option_value "--color" "$@")"
		;;
	--build-plan)
		BUILD_PLAN="true"
		;;
	--timings=*)
		TIMINGS="${1#*=}"
		;;
	--timings)
		if is_next_arg_option "$@"; then
			TIMINGS="true"
		else
			if [[ $# -gt 1 ]]; then
				shift
				TIMINGS="$1"
			else
				TIMINGS="true"
			fi
		fi
		;;
	--lockfile-path=*)
		LOCKFILE_PATH="${1#*=}"
		;;
	--lockfile-path)
		shift
		LOCKFILE_PATH="$(parse_option_value "--lockfile-path" "$@")"
		;;
	--config=*)
		CARGO_CONFIG_ARRAY+=("${1#*=}")
		;;
	--config)
		shift
		CARGO_CONFIG_ARRAY+=("$(parse_option_value "--config" "$@")")
		;;
	-C=*)
		CARGO_CWD="${1#*=}"
		;;
	-C?*)
		# Support -C/path format (no space or equals)
		CARGO_CWD="${1#-C}"
		;;
	-C)
		shift
		CARGO_CWD="$(parse_option_value "-C" "$@")"
		;;
	-Z=*)
		CARGO_Z_FLAGS_ARRAY+=("${1#*=}")
		;;
	-Z?*)
		# Support -Zflag format (no space or equals)
		CARGO_Z_FLAGS_ARRAY+=("${1#-Z}")
		;;
	-Z)
		shift
		CARGO_Z_FLAGS_ARRAY+=("$(parse_option_value "-Z" "$@")")
		;;
	--clean-cache)
		CLEAN_CACHE="true"
		;;
	--no-strip)
		NO_STRIP="true"
		;;
	--verbose)
		VERBOSE_LEVEL=$((VERBOSE_LEVEL + 1))
		;;
	--)
		# Stop parsing options, pass remaining args to cargo
		shift
		CARGO_PASSTHROUGH_ARGS="-- $*"
		break
		;;
	*)
		log_error "Invalid option: $1"
		exit 1
		;;
	esac
	shift
done

# Default to host target if not specified
if [[ -z "$TARGETS" ]]; then
	TARGETS="$HOST_TRIPLE"
	USE_DEFAULT_LINKER="true"
	NO_CARGO_TARGET="true"
	log_info "No target specified, using host: ${COLOR_LIGHT_YELLOW}${TARGETS}${COLOR_LIGHT_BLUE}"
else
	# Expand target patterns
	TARGETS=$(expand_targets "$TARGETS")
	# Check if expansion resulted in empty string
	if [[ -z "$TARGETS" ]]; then
		log_error "Error: Target expansion resulted in no valid targets"
		exit 1
	fi
	NO_CARGO_TARGET=""
fi

# Print execution information
log_info "Execution configuration:"
echo -e "  ${COLOR_LIGHT_CYAN}Command:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${COMMAND}${COLOR_RESET}"
echo -e "  ${COLOR_LIGHT_CYAN}Source directory:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${SOURCE_DIR}${COLOR_RESET}"
[[ -n "$PACKAGE" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Package:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${PACKAGE}${COLOR_RESET}"
[[ -n "$BIN_TARGET" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Binary target:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${BIN_TARGET}${COLOR_RESET}"
[[ "$BUILD_BINS" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Build all binaries:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ "$BUILD_LIB" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Build library:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ "$BUILD_ALL_TARGETS" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Build all targets:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ "$BUILD_WORKSPACE" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Building workspace:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
echo -e "  ${COLOR_LIGHT_CYAN}Profile:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${PROFILE}${COLOR_RESET}"
[[ -n "$TOOLCHAIN" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Toolchain:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${TOOLCHAIN}${COLOR_RESET}"
echo -e "  ${COLOR_LIGHT_CYAN}Targets:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${TARGETS}${COLOR_RESET}"
[[ -n "$FEATURES" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Features:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${FEATURES}${COLOR_RESET}"
[[ "$NO_DEFAULT_FEATURES" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}No default features:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ "$ALL_FEATURES" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}All features:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ -n "$RUSTFLAGS" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Default rustflags env:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${RUSTFLAGS}${COLOR_RESET}"
[[ ${#ADDITIONAL_RUSTFLAGS_ARRAY[@]} -gt 0 ]] && echo -e "  ${COLOR_LIGHT_CYAN}Additional rustflags:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${ADDITIONAL_RUSTFLAGS_ARRAY[*]}${COLOR_RESET}"
[[ -n "$BUILD_STD" && "$BUILD_STD" != "false" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Build std:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}$([ "$BUILD_STD" == "true" ] && echo "true" || echo "$BUILD_STD")${COLOR_RESET}"
[[ -n "$BUILD_STD_FEATURES" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Build std features:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${BUILD_STD_FEATURES}${COLOR_RESET}"
[[ -n "$CARGO_ARGS" ]] && echo -e "  ${COLOR_LIGHT_CYAN}Cargo args:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${CARGO_ARGS}${COLOR_RESET}"
[[ "$NO_EMBED_METADATA" == "true" ]] && echo -e "  ${COLOR_LIGHT_CYAN}No embed metadata:${COLOR_RESET} ${COLOR_LIGHT_GREEN}true${COLOR_RESET}"
[[ -n "$RUSTC_BOOTSTRAP" ]] && echo -e "  ${COLOR_LIGHT_CYAN}RUSTC_BOOTSTRAP:${COLOR_RESET} ${COLOR_LIGHT_YELLOW}${RUSTC_BOOTSTRAP}${COLOR_RESET}"

# Build for each target
set_github_output
IFS=',' read -ra TARGET_ARRAY <<<"$TARGETS"
TOTAL_TARGETS=${#TARGET_ARRAY[@]}
CURRENT_TARGET=0
BUILD_START_TIME=$(date +%s)

for target in "${TARGET_ARRAY[@]}"; do
	CURRENT_TARGET=$((CURRENT_TARGET + 1))
	log_success "[${COLOR_LIGHT_YELLOW}${CURRENT_TARGET}${COLOR_LIGHT_GREEN}/${COLOR_LIGHT_YELLOW}${TOTAL_TARGETS}${COLOR_LIGHT_GREEN}] Processing target: ${COLOR_LIGHT_CYAN}${target}${COLOR_LIGHT_GREEN}"
	execute_target "$target" "$COMMAND" || {
		command_capitalized="$(echo "${COMMAND:0:1}" | tr '[:lower:]' '[:upper:]')${COMMAND:1}"
		log_error "${command_capitalized} failed for target: ${COLOR_LIGHT_YELLOW}${target}${COLOR_LIGHT_RED}"
		exit 1
	}
done

BUILD_END_TIME=$(date +%s)
TOTAL_TIME=$((BUILD_END_TIME - BUILD_START_TIME))

echo -e "${COLOR_LIGHT_GRAY}$(print_separator)${COLOR_RESET}"
log_success "All ${COLOR_LIGHT_CYAN}${COMMAND}${COLOR_LIGHT_GREEN} operations completed successfully!"
log_success "Total time: ${COLOR_LIGHT_YELLOW}${TOTAL_TIME}s${COLOR_LIGHT_GREEN}"
