#!/bin/bash
set -e

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
readonly DEFAULT_CROSS_DEPS_VERSION="v0.6.6"
readonly DEFAULT_TTY_WIDTH="40"
readonly DEFAULT_NDK_VERSION="r27"
readonly DEFAULT_COMMAND="build"
readonly DEFAULT_TOOLCHAIN=""

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
	echo -e "${COLOR_LIGHT_GREEN}Usage:${COLOR_RESET}"
	echo -e "  [command] [options]"
	echo -e ""
	echo -e "${COLOR_LIGHT_RED}Commands:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_BLUE}build${COLOR_RESET}                               - Build the project (default)"
	echo -e "  ${COLOR_LIGHT_BLUE}test${COLOR_RESET}                                - Run tests"
	echo -e "  ${COLOR_LIGHT_BLUE}check${COLOR_RESET}                               - Check the project"
	echo -e ""
	echo -e "${COLOR_LIGHT_RED}Options:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_BLUE}--profile=<profile>${COLOR_RESET}               - Set the build profile (debug/release, default: ${DEFAULT_PROFILE})"
	echo -e "  ${COLOR_LIGHT_BLUE}--cross-compiler-dir=<dir>${COLOR_RESET}        - Specify the cross compiler directory"
	echo -e "  ${COLOR_LIGHT_BLUE}--features=<features>${COLOR_RESET}             - Comma-separated list of features to activate"
	echo -e "  ${COLOR_LIGHT_BLUE}--no-default-features${COLOR_RESET}             - Do not activate default features"
	echo -e "  ${COLOR_LIGHT_BLUE}--all-features${COLOR_RESET}                    - Activate all available features"
	echo -e "  ${COLOR_LIGHT_BLUE}-t=<targets>, --targets=<targets>${COLOR_RESET} - Rust target triple(s) (e.g., x86_64-unknown-linux-musl)"
	echo -e "  ${COLOR_LIGHT_BLUE}--show-all-targets${COLOR_RESET}                - Display all supported target triples"
	echo -e "  ${COLOR_LIGHT_BLUE}--github-proxy-mirror=<url>${COLOR_RESET}       - Use a GitHub proxy mirror"
	echo -e "  ${COLOR_LIGHT_BLUE}--ndk-version=<version>${COLOR_RESET}           - Specify the Android NDK version"
	echo -e "  ${COLOR_LIGHT_BLUE}--package=<name>${COLOR_RESET}                  - Package to build (workspace member)"
	echo -e "  ${COLOR_LIGHT_BLUE}--workspace${COLOR_RESET}                       - Build all workspace members"
	echo -e "  ${COLOR_LIGHT_BLUE}--bin=<name>${COLOR_RESET}                      - Binary target to build"
	echo -e "  ${COLOR_LIGHT_BLUE}--bins${COLOR_RESET}                            - Build all binary targets"
	echo -e "  ${COLOR_LIGHT_BLUE}--lib${COLOR_RESET}                             - Build only the library target"
	echo -e "  ${COLOR_LIGHT_BLUE}--all-targets${COLOR_RESET}                     - Build all targets (equivalent to --lib --bins --tests --benches --examples)"
	echo -e "  ${COLOR_LIGHT_BLUE}-r, --release${COLOR_RESET}                     - Build optimized artifacts with the release profile"
	echo -e "  ${COLOR_LIGHT_BLUE}-q, --quiet${COLOR_RESET}                       - Do not print cargo log messages"
	echo -e "  ${COLOR_LIGHT_BLUE}--message-format=<fmt>${COLOR_RESET}            - The output format for diagnostic messages"
	echo -e "  ${COLOR_LIGHT_BLUE}--ignore-rust-version${COLOR_RESET}             - Ignore rust-version specification in packages"
	echo -e "  ${COLOR_LIGHT_BLUE}--locked${COLOR_RESET}                          - Asserts that exact same dependencies are used as Cargo.lock"
	echo -e "  ${COLOR_LIGHT_BLUE}--offline${COLOR_RESET}                         - Prevents Cargo from accessing the network"
	echo -e "  ${COLOR_LIGHT_BLUE}--frozen${COLOR_RESET}                          - Equivalent to specifying both --locked and --offline"
	echo -e "  ${COLOR_LIGHT_BLUE}-j=<N>, --jobs=<N>${COLOR_RESET}                - Number of parallel jobs to run"
	echo -e "  ${COLOR_LIGHT_BLUE}--keep-going${COLOR_RESET}                      - Build as many crates as possible, don't abort on first failure"
	echo -e "  ${COLOR_LIGHT_BLUE}--future-incompat-report${COLOR_RESET}          - Displays a future-incompat report for warnings"
	echo -e "  ${COLOR_LIGHT_BLUE}--manifest-path=<path>${COLOR_RESET}            - Path to Cargo.toml"
	echo -e "  ${COLOR_LIGHT_BLUE}--use-default-linker${COLOR_RESET}              - Use system default linker (no cross-compiler download)"
	echo -e "  ${COLOR_LIGHT_BLUE}--cc=<path>${COLOR_RESET}                       - Force set the C compiler for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--cxx=<path>${COLOR_RESET}                      - Force set the C++ compiler for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--ar=<path>${COLOR_RESET}                       - Force set the ar for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--linker=<path>${COLOR_RESET}                   - Force set the linker for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--rustflags=<flags>${COLOR_RESET}               - Additional rustflags (can be specified multiple times)"
	echo -e "  ${COLOR_LIGHT_BLUE}--static-crt[=<true|false>]${COLOR_RESET}       - Add -C target-feature=+crt-static to rustflags (default: true)"
	echo -e "  ${COLOR_LIGHT_BLUE}--build-std[=<crates>]${COLOR_RESET}            - Use -Zbuild-std for building standard library from source"
	echo -e "  ${COLOR_LIGHT_BLUE}--args=<args>${COLOR_RESET}                     - Additional arguments to pass to cargo build"
	echo -e "  ${COLOR_LIGHT_BLUE}--toolchain=<toolchain>${COLOR_RESET}           - Rust toolchain to use (stable, nightly, etc.)"
	echo -e "  ${COLOR_LIGHT_BLUE}--cargo-trim-paths=<paths>${COLOR_RESET}        - Set CARGO_TRIM_PATHS environment variable"
	echo -e "  ${COLOR_LIGHT_BLUE}--no-embed-metadata${COLOR_RESET}               - Add -Zno-embed-metadata flag to cargo"
	echo -e "  ${COLOR_LIGHT_BLUE}-v, --verbose${COLOR_RESET}                     - Use verbose output"
	echo -e "  ${COLOR_LIGHT_BLUE}-h, --help${COLOR_RESET}                        - Display this help message"
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
	log_info "Downloading \"${url}\" to \"${file}\""

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
	log_success "Download and extraction successful (took $((end_time - start_time))s)"
}

# Download cross-compiler if needed
# Args: compiler_dir, compiler_name, download_url
download_cross_compiler() {
	local compiler_dir="$1"
	local compiler_name="$2"
	local download_url="$3"

	if [[ ! -d "${compiler_dir}" ]]; then
		download_and_extract "${download_url}" "${compiler_dir}" || return 2
	fi

	echo "${compiler_dir}"
}

# Set cross-compilation environment variables
# Args: cc, cxx, ar, linker, extra_path
set_cross_env() {
	TARGET_CC="$1"
	TARGET_CXX="$2"
	TARGET_AR="$3"
	TARGET_LINKER="$4"
	EXTRA_PATH="$5"
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
fix_darwin_linker_rpath() {
	local compiler_dir="$1"
	local arch_prefix="$2"
	patchelf --set-rpath "${compiler_dir}/lib" \
		${compiler_dir}/bin/${arch_prefix}-apple-darwin*-ld || return 2
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

	log_info "Adding rust-src component for target: $target${toolchain:+ and toolchain: $toolchain}"
	rustup component add rust-src --target="$target" $toolchain_flag || return $?
}

# Helper function to install target
install_target() {
	local rust_target="$1"
	local toolchain="$2"
	local toolchain_flag=""
	[[ -n "$toolchain" ]] && toolchain_flag="--toolchain=$toolchain"

	log_info "Installing Rust target: $rust_target${toolchain:+ for toolchain: $toolchain}"
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

# -----------------------------------------------------------------------------
# Cross-Compilation Environment Setup
# -----------------------------------------------------------------------------

# Get cross-compilation environment variables
# Returns environment variables as a string suitable for use with env command
get_cross_env() {
	local rust_target="$1"

	# Clear target-specific variables
	TARGET_CC="" TARGET_CXX="" TARGET_AR="" TARGET_LINKER="" TARGET_RUSTFLAGS="" TARGET_BUILD_STD=""
	EXTRA_PATH="" SDKROOT=""

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
				log_warning "Target $rust_target not available in rustup but exists in rustc, using build-std"
				TARGET_BUILD_STD=true
			else
				log_error "Target $rust_target not found in rustup or rustc target list"
				return 1
			fi
		fi
	fi

	# Skip toolchain setup if using default linker
	if [[ "$USE_DEFAULT_LINKER" == "true" ]]; then
		log_warning "Using system default linker for $rust_target"
		return 0
	fi

	# Convert target to uppercase for environment variable names
	local target_upper=$(echo "$rust_target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')

	# Check if target-specific environment variables are already set
	local cc_var="CC_${target_upper}"
	local cxx_var="CXX_${target_upper}"
	local ar_var="AR_${target_upper}"
	local linker_var="CARGO_TARGET_${target_upper}_LINKER"

	if [[ -n "${!cc_var}" ]]; then
		log_success "Using pre-configured ${cc_var}=${!cc_var}"
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
		return 0
	fi

	local toolchain_info="$(get_toolchain_config "$rust_target")"
	if [[ -z "$toolchain_info" ]]; then
		log_warning "No specific toolchain configuration for $rust_target, using default"
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
		get_ios_env "$arch" "$rust_target" || return $?
		;;
	*)
		log_warning "No cross-compilation setup needed for $rust_target"
		;;
	esac
}

# -----------------------------------------------------------------------------
# Platform-Specific Environment Functions
# -----------------------------------------------------------------------------

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
	local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}"

	# Download compiler if not present
	if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/musl-cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
		download_cross_compiler "${compiler_dir}" "${cross_compiler_name}" "${download_url}" || return 2
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

	log_success "Configured Linux ${libc} toolchain for $rust_target"
}

# Get Windows cross-compilation environment
get_windows_gnu_env() {
	local arch="$1"
	local rust_target="$2"

	# Validate architecture
	case "$arch" in
	"i686" | "x86_64") ;;
	*)
		log_error "Unsupported Windows architecture: $arch"
		return 1
		;;
	esac

	local cross_compiler_name="${arch}-w64-mingw32-cross"
	local gcc_name="${arch}-w64-mingw32-gcc"
	local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}"

	# Download compiler if not present
	if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/musl-cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
		download_cross_compiler "${compiler_dir}" "${cross_compiler_name}" "${download_url}" || return 2
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

	log_success "Configured Windows toolchain for $rust_target"
}

# Get FreeBSD cross-compilation environment
get_freebsd_env() {
	local arch="$1"
	local rust_target="$2"

	# Validate architecture
	case "$arch" in
	"x86_64" | "aarch64" | "powerpc" | "powerpc64" | "powerpc64le" | "riscv64") ;;
	*)
		log_error "Unsupported FreeBSD architecture: $arch"
		return 1
		;;
	esac

	local cross_compiler_name="${arch}-unknown-freebsd13-cross"
	local gcc_name="${arch}-unknown-freebsd13-gcc"
	local compiler_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}"

	# Download compiler if not present
	if [[ ! -x "${compiler_dir}/bin/${gcc_name}" ]]; then
		local host_platform=$(get_host_platform)
		local download_url="${GH_PROXY}https://github.com/zijiren233/musl-cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${host_platform}.tgz"
		download_cross_compiler "${compiler_dir}" "${cross_compiler_name}" "${download_url}" || return 2
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

	log_success "Configured FreeBSD toolchain for $rust_target"
}

# Get Darwin (macOS) environment
# Need install patchelf
get_darwin_env() {
	local arch="$1"
	local rust_target="$2"

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		log_success "Using native macOS toolchain for $rust_target"
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
			log_warning "Cross-compilation to macOS not supported on ${HOST_OS}/${HOST_ARCH}"
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

		fix_darwin_linker_rpath "${osxcross_dir}" "${arch}"

		set_cross_env \
			"${arch}-apple-darwin24.5-clang" \
			"${arch}-apple-darwin24.5-clang++" \
			"${arch}-apple-darwin24.5-ar" \
			"${arch}-apple-darwin24.5-clang" \
			"${osxcross_dir}/bin:${osxcross_dir}/clang/bin"

		export MACOSX_DEPLOYMENT_TARGET="10.12"

		log_success "Configured osxcross toolchain for $rust_target"
		;;
	*)
		log_warning "Cross-compilation to macOS not supported on ${HOST_OS}"
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
	"armv7") clang_prefix="armv7a-linux-androideabi${API}" ;;
	"aarch64") clang_prefix="aarch64-linux-android${API}" ;;
	"i686") clang_prefix="i686-linux-android${API}" ;;
	"x86_64") clang_prefix="x86_64-linux-android${API}" ;;
	*)
		log_error "Unsupported Android architecture: $arch"
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

	log_success "Configured Android toolchain for $rust_target"
}

# Get iOS environment
get_ios_env() {
	local arch="$1"
	local rust_target="$2"

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		log_success "Using native macOS toolchain for $rust_target"
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
			log_warning "Unknown iOS architecture: ${arch}"
			return 2
			;;
		esac

		local cross_compiler_name="ios-${arch_prefix}-cross"
		if [[ "${arch}" == "x86_64" ]]; then
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
			if [[ "${arch}" == "x86_64" ]]; then
				ios_sdk_type="iPhoneSimulator"
				ios_arch="x86_64"
			fi

			local download_url="${GH_PROXY}https://github.com/zijiren233/cctools-port/releases/download/v0.1.6/ioscross-${ios_sdk_type}18-5-${ios_arch}-${host_platform}-gnu-ubuntu-${ubuntu_version}.tar.gz"
			download_and_extract "$download_url" "${compiler_dir}" || return 2
		fi

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

		log_success "Configured iOS toolchain for $rust_target"
		;;
	*)
		log_warning "Cross-compilation to macOS not supported on ${HOST_OS}"
		return 1
		;;
	esac
}

# -----------------------------------------------------------------------------
# Build Support Functions
# -----------------------------------------------------------------------------

# Clean cache
clean_cache() {
	if [[ "$CLEAN_CACHE" == "true" ]]; then
		log_info "Cleaning cache..."
		cargo clean --target "$1" 2>/dev/null || true
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
	if [[ -n "$EXTRA_PATH" ]]; then
		build_env+=("PATH=${EXTRA_PATH}:${PATH}")
	fi

	# Set up environment based on target
	local target_upper=$(echo "$rust_target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')

	add_env_if_set "CC_${target_upper}" "$TARGET_CC"
	add_env_if_set "CC" "$TARGET_CC"
	add_env_if_set "CXX_${target_upper}" "$TARGET_CXX"
	add_env_if_set "CXX" "$TARGET_CXX"
	add_env_if_set "AR_${target_upper}" "$TARGET_AR"
	add_env_if_set "AR" "$TARGET_AR"
	add_env_if_set "CARGO_TARGET_${target_upper}_LINKER" "$TARGET_LINKER"
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

	if [[ "$STATIC_CRT" == "true" ]]; then
		rustflags="${rustflags:+$rustflags }-C target-feature=+crt-static"
	elif [[ "$STATIC_CRT" == "false" ]]; then
		rustflags="${rustflags:+$rustflags }-C target-feature=-crt-static"
	fi

	if [[ ${#ADDITIONAL_RUSTFLAGS_ARRAY[@]} -gt 0 ]]; then
		for flag in "${ADDITIONAL_RUSTFLAGS_ARRAY[@]}"; do
			rustflags="${rustflags:+$rustflags }$flag"
		done
	fi

	add_env_if_set "RUSTFLAGS" "$rustflags"
	add_env_if_set "CARGO_TRIM_PATHS" "$CARGO_TRIM_PATHS"

	# Prepare command
	local cargo_cmd="cargo"
	if [[ -n "$TOOLCHAIN" ]]; then
		cargo_cmd="cargo +$TOOLCHAIN"
	fi
	cargo_cmd="$cargo_cmd $command --target $rust_target"

	# Only add profile for build command
	[[ "$command" == "build" && "$PROFILE" == "release" ]] && cargo_cmd="$cargo_cmd --release"
	[[ -n "$FEATURES" ]] && cargo_cmd="$cargo_cmd --features $FEATURES"
	[[ "$NO_DEFAULT_FEATURES" == "true" ]] && cargo_cmd="$cargo_cmd --no-default-features"
	[[ "$ALL_FEATURES" == "true" ]] && cargo_cmd="$cargo_cmd --all-features"
	[[ -n "$PACKAGE" ]] && cargo_cmd="$cargo_cmd --package $PACKAGE"
	[[ -n "$BIN_TARGET" ]] && cargo_cmd="$cargo_cmd --bin $BIN_TARGET"
	[[ "$BINS" == "true" ]] && cargo_cmd="$cargo_cmd --bins"
	[[ "$LIB" == "true" ]] && cargo_cmd="$cargo_cmd --lib"
	[[ "$ALL_TARGETS" == "true" ]] && cargo_cmd="$cargo_cmd --all-targets"
	[[ "$WORKSPACE" == "true" ]] && cargo_cmd="$cargo_cmd --workspace"
	[[ -n "$MANIFEST_PATH" ]] && cargo_cmd="$cargo_cmd --manifest-path $MANIFEST_PATH"
	# Add build-std flag if needed (either from args or target requirements)
	if [[ -n "$BUILD_STD" && "$BUILD_STD" != "false" ]] || [[ -n "$TARGET_BUILD_STD" && "$TARGET_BUILD_STD" != "false" ]]; then
		if [[ "$TARGET_BUILD_STD" == "true" ]]; then
			# Default for automatic build-std
			cargo_cmd="$cargo_cmd -Zbuild-std"
		elif [[ -n "$TARGET_BUILD_STD" ]]; then
			# Custom build-std parameters
			cargo_cmd="$cargo_cmd -Zbuild-std=$TARGET_BUILD_STD"
		elif [[ "$BUILD_STD" == "true" ]]; then
			# --build-std without parameters
			cargo_cmd="$cargo_cmd -Zbuild-std"
		elif [[ -n "$BUILD_STD" ]]; then
			# Custom build-std parameters
			cargo_cmd="$cargo_cmd -Zbuild-std=$BUILD_STD"
		fi
		add_rust_src "$rust_target" "$TOOLCHAIN" || return $?
	fi
	[[ "$VERBOSE" == "true" ]] && cargo_cmd="$cargo_cmd --verbose"
	[[ "$QUIET" == "true" ]] && cargo_cmd="$cargo_cmd --quiet"
	[[ -n "$MESSAGE_FORMAT" ]] && cargo_cmd="$cargo_cmd --message-format $MESSAGE_FORMAT"
	[[ "$IGNORE_RUST_VERSION" == "true" ]] && cargo_cmd="$cargo_cmd --ignore-rust-version"
	[[ "$LOCKED" == "true" ]] && cargo_cmd="$cargo_cmd --locked"
	[[ "$OFFLINE" == "true" ]] && cargo_cmd="$cargo_cmd --offline"
	[[ "$FROZEN" == "true" ]] && cargo_cmd="$cargo_cmd --frozen"
	[[ -n "$JOBS" ]] && cargo_cmd="$cargo_cmd --jobs $JOBS"
	[[ "$KEEP_GOING" == "true" ]] && cargo_cmd="$cargo_cmd --keep-going"
	[[ "$FUTURE_INCOMPAT_REPORT" == "true" ]] && cargo_cmd="$cargo_cmd --future-incompat-report"
	[[ "$NO_EMBED_METADATA" == "true" ]] && cargo_cmd="$cargo_cmd -Zno-embed-metadata"
	[[ -n "$ADDITIONAL_ARGS" ]] && cargo_cmd="$cargo_cmd $ADDITIONAL_ARGS"

	log_info "Environment variables:"
	for env_var in "${build_env[@]}"; do
		echo -e "  ${COLOR_LIGHT_CYAN}${env_var}${COLOR_RESET}"
	done

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
	log_success "${command_capitalized} successful: ${rust_target} (took $((end_time - start_time))s)"
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

# -----------------------------------------------------------------------------
# Argument Parsing and Main Script
# -----------------------------------------------------------------------------

# Initialize variables
set_default "SOURCE_DIR" "${DEFAULT_SOURCE_DIR}"
SOURCE_DIR="$(cd "${SOURCE_DIR}" && pwd)"
set_default "PROFILE" "${DEFAULT_PROFILE}"
set_default "CROSS_COMPILER_DIR" "${DEFAULT_CROSS_COMPILER_DIR}"
set_default "CROSS_DEPS_VERSION" "${DEFAULT_CROSS_DEPS_VERSION}"
set_default "NDK_VERSION" "${DEFAULT_NDK_VERSION}"
set_default "COMMAND" "${DEFAULT_COMMAND}"
set_default "TOOLCHAIN" "${DEFAULT_TOOLCHAIN}"

# Helper function to check if the next argument is an option
is_next_arg_option() {
	if [[ $# -le 1 ]]; then
		return 1
	fi

	local next_arg="$2"

	# Check if it's a long option (starts with --)
	if [[ "$next_arg" =~ ^-- ]]; then
		return 0
	fi

	# Check if it's a known short option (exact match or with = for those that support it)
	case "$next_arg" in
	-h | -r | -v | -q)
		# These short options don't support = form
		return 0
		;;
	-t | -j)
		# These short options exist without =
		return 0
		;;
	-t=* | -j=*)
		# These short options support = form
		return 0
		;;
	*)
		return 1
		;;
	esac
}

# Parse command-line arguments
# First argument might be a command
if [[ $# -gt 0 ]] && [[ "$1" =~ ^(build|test|check)$ ]]; then
	COMMAND="$1"
	shift
fi

while [[ $# -gt 0 ]]; do
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
	--features=*)
		FEATURES="${1#*=}"
		;;
	--features)
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
			[[ -n "$target" ]] && echo "  $target"
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
	--package=*)
		PACKAGE="${1#*=}"
		;;
	--package)
		shift
		PACKAGE="$(parse_option_value "--package" "$@")"
		;;
	--bin=*)
		BIN_TARGET="${1#*=}"
		;;
	--bin)
		shift
		BIN_TARGET="$(parse_option_value "--bin" "$@")"
		;;
	--bins)
		BINS="true"
		;;
	--lib)
		LIB="true"
		;;
	--all-targets)
		ALL_TARGETS="true"
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
		WORKSPACE="true"
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
	--rustflags=*)
		ADDITIONAL_RUSTFLAGS_ARRAY+=("${1#*=}")
		;;
	--rustflags)
		shift
		ADDITIONAL_RUSTFLAGS_ARRAY+=("$(parse_option_value "--rustflags" "$@")")
		;;
	--static-crt=*)
		STATIC_CRT="${1#*=}"
		[[ -z "$STATIC_CRT" ]] && STATIC_CRT="true"
		;;
	--static-crt)
		if is_next_arg_option "$@"; then
			STATIC_CRT="true"
		else
			if [[ $# -gt 1 ]]; then
				shift
				STATIC_CRT="$1"
			else
				STATIC_CRT="true"
			fi
		fi
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
	--args=*)
		ADDITIONAL_ARGS="${1#*=}"
		;;
	--args)
		shift
		ADDITIONAL_ARGS="$(parse_option_value "--args" "$@")"
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
	--clean-cache)
		CLEAN_CACHE="true"
		;;
	--no-strip)
		NO_STRIP="true"
		;;
	-v | --verbose)
		VERBOSE="true"
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
	log_info "No target specified, using host: ${TARGETS}"
else
	# Expand target patterns
	TARGETS=$(expand_targets "$TARGETS")
	# Check if expansion resulted in empty string
	if [[ -z "$TARGETS" ]]; then
		log_error "Error: Target expansion resulted in no valid targets"
		exit 1
	fi
fi

# Print execution information
log_info "Execution configuration:"
echo -e "  Command: ${COMMAND}"
echo -e "  Source directory: ${SOURCE_DIR}"
[[ -n "$PACKAGE" ]] && echo -e "  Package: ${PACKAGE}"
[[ -n "$BIN_TARGET" ]] && echo -e "  Binary target: ${BIN_TARGET}"
[[ "$BINS" == "true" ]] && echo -e "  Build all binaries: true"
[[ "$LIB" == "true" ]] && echo -e "  Build library: true"
[[ "$ALL_TARGETS" == "true" ]] && echo -e "  Build all targets: true"
[[ "$WORKSPACE" == "true" ]] && echo -e "  Building workspace: true"
echo -e "  Profile: ${PROFILE}"
[[ -n "$TOOLCHAIN" ]] && echo -e "  Toolchain: ${TOOLCHAIN}"
echo -e "  Targets: ${TARGETS}"
[[ -n "$FEATURES" ]] && echo -e "  Features: ${FEATURES}"
[[ "$NO_DEFAULT_FEATURES" == "true" ]] && echo -e "  No default features: true"
[[ "$ALL_FEATURES" == "true" ]] && echo -e "  All features: true"
[[ -n "$RUSTFLAGS" ]] && echo -e "  Default rustflags env: ${RUSTFLAGS}"
[[ ${#ADDITIONAL_RUSTFLAGS_ARRAY[@]} -gt 0 ]] && echo -e "  Additional rustflags: ${ADDITIONAL_RUSTFLAGS_ARRAY[*]}"
[[ -n "$BUILD_STD" && "$BUILD_STD" != "false" ]] && echo -e "  Build std: $([ "$BUILD_STD" == "true" ] && echo "true" || echo "$BUILD_STD")"
[[ -n "$ADDITIONAL_ARGS" ]] && echo -e "  Additional args: ${ADDITIONAL_ARGS}"
[[ "$NO_EMBED_METADATA" == "true" ]] && echo -e "  No embed metadata: true"

# Build for each target
IFS=',' read -ra TARGET_ARRAY <<<"$TARGETS"
TOTAL_TARGETS=${#TARGET_ARRAY[@]}
CURRENT_TARGET=0
BUILD_START_TIME=$(date +%s)

for target in "${TARGET_ARRAY[@]}"; do
	CURRENT_TARGET=$((CURRENT_TARGET + 1))
	log_success "[${CURRENT_TARGET}/${TOTAL_TARGETS}] Processing target: ${target}"
	execute_target "$target" "$COMMAND" || {
		local command_capitalized="$(echo "${COMMAND:0:1}" | tr '[:lower:]' '[:upper:]')${COMMAND:1}"
		log_error "${command_capitalized} failed for target: ${target}"
		exit 1
	}
done

BUILD_END_TIME=$(date +%s)
TOTAL_TIME=$((BUILD_END_TIME - BUILD_START_TIME))

echo -e "${COLOR_LIGHT_GRAY}$(print_separator)${COLOR_RESET}"
log_success "All ${COMMAND} operations completed successfully!"
log_success "Total time: ${TOTAL_TIME}s"
