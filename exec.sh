#!/bin/bash
set -e

# Light Color definitions
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

# Default values
readonly DEFAULT_SOURCE_DIR="$(pwd)"
readonly DEFAULT_RESULT_DIR="${DEFAULT_SOURCE_DIR}/target/cross"
readonly DEFAULT_BUILD_CONFIG="${DEFAULT_SOURCE_DIR}/build.config.sh"
readonly DEFAULT_PROFILE="release"
readonly DEFAULT_CROSS_COMPILER_DIR="$(dirname $(mktemp -u))/rust-cross-compiler"
readonly DEFAULT_CROSS_DEPS_VERSION="v0.5.17"
readonly DEFAULT_TTY_WIDTH="40"
readonly DEFAULT_NDK_VERSION="r27"
readonly DEFAULT_COMMAND="build"
readonly DEFAULT_TOOLCHAIN=""

# Host environment
readonly HOST_OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
readonly HOST_ARCH="$(uname -m)"
readonly HOST_TRIPLE="$(rustc -vV | grep host | cut -d' ' -f2)"

# Supported Rust targets with their toolchain configurations
declare -A TOOLCHAIN_CONFIG=(
	# Linux musl targets
	["aarch64-unknown-linux-musl"]="linux:aarch64:musl"
	["arm-unknown-linux-musleabi"]="linux:armv6:musl:eabi"
	["arm-unknown-linux-musleabihf"]="linux:armv6:musl:eabihf"
	["armv5te-unknown-linux-musleabi"]="linux:armv5:musl:eabi"
	["armv7-unknown-linux-musleabi"]="linux:armv7:musl:eabi"
	["armv7-unknown-linux-musleabihf"]="linux:armv7:musl:eabihf"
	["i586-unknown-linux-musl"]="linux:i586:musl"
	["i686-unknown-linux-musl"]="linux:i686:musl"
	["loongarch64-unknown-linux-musl"]="linux:loongarch64:musl"
	["mips-unknown-linux-musl"]="linux:mips:musl"
	["mipsel-unknown-linux-musl"]="linux:mipsel:musl"
	["mips64-unknown-linux-muslabi64"]="linux:mips64:musl"
	["mips64-openwrt-linux-musl"]="linux:mips64:musl"
	["mips64el-unknown-linux-muslabi64"]="linux:mips64el:musl"
	["powerpc64-unknown-linux-musl"]="linux:powerpc64:musl"
	["powerpc64le-unknown-linux-musl"]="linux:powerpc64le:musl"
	["riscv32gc-unknown-linux-musl"]="linux:riscv32:musl"
	["riscv64gc-unknown-linux-musl"]="linux:riscv64:musl"
	["s390x-unknown-linux-musl"]="linux:s390x:musl"
	["x86_64-unknown-linux-musl"]="linux:x86_64:musl"

	# Linux GNU targets
	# ["i686-unknown-linux-gnu"]="linux:i686:gnu"
	# ["x86_64-unknown-linux-gnu"]="linux:x86_64:gnu"
	# ["aarch64-unknown-linux-gnu"]="linux:aarch64:gnu"
	# ["armv7-unknown-linux-gnueabihf"]="linux:armv7:gnu:eabihf"
	# ["powerpc64-unknown-linux-gnu"]="linux:powerpc64:gnu"
	# ["powerpc64le-unknown-linux-gnu"]="linux:powerpc64le:gnu"
	# ["riscv64gc-unknown-linux-gnu"]="linux:riscv64:gnu"
	# ["s390x-unknown-linux-gnu"]="linux:s390x:gnu"

	# Windows targets
	["i686-pc-windows-gnu"]="windows:i686:gnu"
	["x86_64-pc-windows-gnu"]="windows:x86_64:gnu"

	# macOS targets
	["x86_64-apple-darwin"]="darwin:x86_64"
	["x86_64h-apple-darwin"]="darwin:x86_64"
	["aarch64-apple-darwin"]="darwin:aarch64"
	["arm64e-apple-darwin"]="darwin:aarch64"

	# iOS targets
	["x86_64-apple-ios"]="ios:x86_64"
	["aarch64-apple-ios"]="ios:aarch64"

	# Android targets
	["aarch64-linux-android"]="android:aarch64"
	["arm-linux-androideabi"]="android:armv7"
	["armv7-linux-androideabi"]="android:armv7"
	["i686-linux-android"]="android:i686"
	["riscv64-linux-android"]="android:riscv64"
	["x86_64-linux-android"]="android:x86_64"
)

# Prints help information
function printHelp() {
	echo -e "${COLOR_LIGHT_GREEN}Usage:${COLOR_RESET}"
	echo -e "  $(basename "$0") [command] [options]"
	echo -e ""
	echo -e "${COLOR_LIGHT_RED}Commands:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_BLUE}build${COLOR_RESET}                               - Build the project (default)"
	echo -e "  ${COLOR_LIGHT_BLUE}test${COLOR_RESET}                                - Run tests"
	echo -e "  ${COLOR_LIGHT_BLUE}check${COLOR_RESET}                               - Check the project"
	echo -e ""
	echo -e "${COLOR_LIGHT_RED}Options:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_BLUE}--bin-name=<name>${COLOR_RESET}                 - Specify the binary name (auto-detect if not set)"
	echo -e "  ${COLOR_LIGHT_BLUE}--bin-name-no-suffix${COLOR_RESET}              - Do not append the target suffix to the binary name"
	echo -e "  ${COLOR_LIGHT_BLUE}--profile=<profile>${COLOR_RESET}               - Set the build profile (debug/release, default: ${DEFAULT_PROFILE})"
	echo -e "  ${COLOR_LIGHT_BLUE}--cross-compiler-dir=<dir>${COLOR_RESET}        - Specify the cross compiler directory"
	echo -e "  ${COLOR_LIGHT_BLUE}--features=<features>${COLOR_RESET}             - Comma-separated list of features to activate"
	echo -e "  ${COLOR_LIGHT_BLUE}--no-default-features${COLOR_RESET}             - Do not activate default features"
	echo -e "  ${COLOR_LIGHT_BLUE}--all-features${COLOR_RESET}                    - Activate all available features"
	echo -e "  ${COLOR_LIGHT_BLUE}-t=<targets>, --targets=<targets>${COLOR_RESET} - Rust target triple(s) (e.g., x86_64-unknown-linux-musl)"
	echo -e "  ${COLOR_LIGHT_BLUE}--result-dir=<dir>${COLOR_RESET}                - Specify the build result directory"
	echo -e "  ${COLOR_LIGHT_BLUE}--show-all-targets${COLOR_RESET}                - Display all supported target triples"
	echo -e "  ${COLOR_LIGHT_BLUE}--github-proxy-mirror=<url>${COLOR_RESET}       - Use a GitHub proxy mirror"
	echo -e "  ${COLOR_LIGHT_BLUE}--ndk-version=<version>${COLOR_RESET}           - Specify the Android NDK version"
	echo -e "  ${COLOR_LIGHT_BLUE}--package=<name>${COLOR_RESET}                  - Package to build (workspace member)"
	echo -e "  ${COLOR_LIGHT_BLUE}--workspace${COLOR_RESET}                       - Build all workspace members"
	echo -e "  ${COLOR_LIGHT_BLUE}--bin=<name>${COLOR_RESET}                      - Binary target to build"
	echo -e "  ${COLOR_LIGHT_BLUE}--manifest-path=<path>${COLOR_RESET}            - Path to Cargo.toml"
	echo -e "  ${COLOR_LIGHT_BLUE}--use-default-linker${COLOR_RESET}              - Use system default linker (no cross-compiler download)"
	echo -e "  ${COLOR_LIGHT_BLUE}--cc=<path>${COLOR_RESET}                       - Force set the C compiler for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--cxx=<path>${COLOR_RESET}                      - Force set the C++ compiler for target"
	echo -e "  ${COLOR_LIGHT_BLUE}--rustflags=<flags>${COLOR_RESET}               - Additional rustflags"
	echo -e "  ${COLOR_LIGHT_BLUE}--static-crt${COLOR_RESET}                      - Add -C target-feature=+crt-static to rustflags"
	echo -e "  ${COLOR_LIGHT_BLUE}--build-std${COLOR_RESET}                       - Use -Zbuild-std for building standard library from source"
	echo -e "  ${COLOR_LIGHT_BLUE}--args=<args>${COLOR_RESET}                     - Additional arguments to pass to cargo build"
	echo -e "  ${COLOR_LIGHT_BLUE}--toolchain=<toolchain>${COLOR_RESET}           - Rust toolchain to use (stable, nightly, etc.)"
	echo -e "  ${COLOR_LIGHT_BLUE}-v, --verbose${COLOR_RESET}                     - Use verbose output"
	echo -e "  ${COLOR_LIGHT_BLUE}-h, --help${COLOR_RESET}                        - Display this help message"
}

# Sets a variable to a default value if it's not already set
function setDefault() {
	local var_name="$1"
	local default_value="$2"
	[[ -z "${!var_name}" ]] && eval "${var_name}=\"${default_value}\"" || true
}

# Downloads and extracts a file
function downloadAndUnzip() {
	local url="$1"
	local file="$2"
	local type="${3:-$(echo "${url}" | sed 's/.*\.//g')}"

	mkdir -p "${file}" || return $?
	file="$(cd "${file}" && pwd)" || return $?
	if [ "$(ls -A "${file}")" ]; then
		rm -rf "${file}"/* || return $?
	fi
	echo -e "${COLOR_LIGHT_BLUE}Downloading ${COLOR_LIGHT_CYAN}\"${url}\"${COLOR_LIGHT_BLUE} to ${COLOR_LIGHT_CYAN}\"${file}\"${COLOR_RESET}"

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
	echo -e "${COLOR_LIGHT_GREEN}Download and extraction successful (took $((end_time - start_time))s)${COLOR_RESET}"
}

# Get separator line
function printSeparator() {
	local width=$(tput cols 2>/dev/null || echo $DEFAULT_TTY_WIDTH)
	printf '%*s\n' "$width" '' | tr ' ' -
}

# Helper function to add rust-src component
function addRustSrc() {
	local toolchain="$1"
	if [[ -n "$toolchain" ]]; then
		echo -e "${COLOR_LIGHT_BLUE}Adding rust-src component for toolchain: $toolchain${COLOR_RESET}"
		rustup component add rust-src --toolchain="$toolchain" || return $?
	else
		echo -e "${COLOR_LIGHT_BLUE}Adding rust-src component${COLOR_RESET}"
		rustup component add rust-src || return $?
	fi
}

# Helper function to install target
function installTarget() {
	local rust_target="$1"
	local toolchain="$2"
	if [[ -n "$toolchain" ]]; then
		echo -e "${COLOR_LIGHT_BLUE}Installing Rust target: $rust_target for toolchain: $toolchain${COLOR_RESET}"
		rustup target add "$rust_target" --toolchain="$toolchain" || return $?
	else
		echo -e "${COLOR_LIGHT_BLUE}Installing Rust target: $rust_target${COLOR_RESET}"
		rustup target add "$rust_target" || return $?
	fi
}

# Helper function to check if target is installed
function isTargetInstalled() {
	local rust_target="$1"
	local toolchain="$2"
	if [[ -n "$toolchain" ]]; then
		rustup target list --installed --toolchain="$toolchain" | grep -q "^$rust_target$"
	else
		rustup target list --installed | grep -q "^$rust_target$"
	fi
}

# Helper function to check if target is available in rustup
function isTargetAvailable() {
	local rust_target="$1"
	local toolchain="$2"
	if [[ -n "$toolchain" ]]; then
		rustup target list --toolchain="$toolchain" | grep -q "^$rust_target$"
	else
		rustup target list | grep -q "^$rust_target$"
	fi
}

# Get cross-compilation environment variables
# Returns environment variables as a string suitable for use with env command
function getCrossEnv() {
	local rust_target="$1"
	local toolchain_info="${TOOLCHAIN_CONFIG[$rust_target]}"

	# Clear target-specific variables
	TARGET_CC="" TARGET_CXX="" TARGET_AR="" TARGET_LINKER="" TARGET_RUSTFLAGS="" TARGET_BUILD_STD=""
	EXTRA_PATH="" SDKROOT=""

	if [[ -z "$toolchain_info" ]]; then
		echo -e "${COLOR_LIGHT_YELLOW}No specific toolchain configuration for $rust_target, using default${COLOR_RESET}"
		return 0
	fi

	# Install Rust target if not already installed, or use build-std if target not available in rustup
	# curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

	# Add rust-src component when build-std is explicitly requested
	if [[ "$BUILD_STD" == "true" ]]; then
		addRustSrc "$TOOLCHAIN" || return $?
	fi

	# Install target if not already installed, or use build-std if target not available in rustup
	if ! isTargetInstalled "$rust_target" "$TOOLCHAIN"; then
		# Check if target is available for installation in rustup
		if isTargetAvailable "$rust_target" "$TOOLCHAIN"; then
			installTarget "$rust_target" "$TOOLCHAIN" || return $?
		else
			# Check if target exists in rustc --print=target-list
			if rustc --print=target-list | grep -q "^$rust_target$"; then
				echo -e "${COLOR_LIGHT_YELLOW}Target $rust_target not available in rustup but exists in rustc, using build-std${COLOR_RESET}"
				TARGET_BUILD_STD=true
				# Add rust-src component for build-std
				addRustSrc "$TOOLCHAIN" || return $?
			else
				echo -e "${COLOR_LIGHT_RED}Target $rust_target not found in rustup or rustc target list${COLOR_RESET}"
				return 1
			fi
		fi
	fi

	# Skip toolchain setup if using default linker
	if [[ "$USE_DEFAULT_LINKER" == "true" ]]; then
		echo -e "${COLOR_LIGHT_YELLOW}Using system default linker for $rust_target${COLOR_RESET}"
		return 0
	fi

	if [[ -n "$CC" ]] && [[ -n "$CXX" ]]; then
		TARGET_CC="$CC"
		TARGET_CXX="$CXX"
		TARGET_AR="${CC%-gcc}-ar"
		TARGET_LINKER="$CC"
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
		if [[ "$libc" == "musl" ]]; then
			getLinuxMuslEnv "$arch" "$abi" "$rust_target" || return $?
		else
			getLinuxGnuEnv "$arch" "$abi" "$rust_target" || return $?
		fi
		;;
	"windows")
		getWindowsEnv "$arch" "$rust_target" || return $?
		;;
	"darwin")
		getDarwinEnv "$arch" "$rust_target" || return $?
		;;
	"android")
		getAndroidEnv "$arch" "$rust_target" || return $?
		;;
	"ios")
		getIosEnv "$arch" "$rust_target" || return $?
		;;
	*)
		echo -e "${COLOR_LIGHT_YELLOW}No cross-compilation setup needed for $rust_target${COLOR_RESET}"
		;;
	esac
}

# Get Linux musl cross-compilation environment
function getLinuxMuslEnv() {
	local arch="$1"
	local abi="$2"
	local rust_target="$3"

	# Map architecture to cross-compiler prefix
	local arch_prefix="$arch"
	case "$arch" in
	"armv6" | "armv7")
		arch_prefix="$arch"
		;;
	"powerpc64le")
		arch_prefix="powerpc64le"
		;;
	"riscv64")
		arch_prefix="riscv64"
		;;
	"loongarch64")
		arch_prefix="loongarch64"
		;;
	esac

	local cross_compiler_name="${arch_prefix}-linux-musl${abi}-cross"
	local gcc_name="${arch_prefix}-linux-musl${abi}-gcc"
	local ar_name="${arch_prefix}-linux-musl${abi}-ar"

	# Check if cross-compiler exists or download it
	if ! command -v "$gcc_name" >/dev/null 2>&1; then
		if [[ ! -x "${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/${gcc_name}" ]]; then
			local unamespacer="${HOST_OS}-${HOST_ARCH}"
			[[ "${HOST_ARCH}" == "arm" ]] && unamespacer="${HOST_OS}-arm32v7"
			[[ "${HOST_ARCH}" == "x86_64" ]] && unamespacer="${HOST_OS}-amd64"

			downloadAndUnzip "${GH_PROXY}https://github.com/zijiren233/musl-cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${unamespacer}.tgz" \
				"${CROSS_COMPILER_DIR}/${cross_compiler_name}" || return 2
		fi
		# Store the additional path needed for this target
		EXTRA_PATH="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin"
	fi

	TARGET_CC="${gcc_name}"
	TARGET_CXX="${arch_prefix}-linux-musl${abi}-g++"
	TARGET_AR="${ar_name}"
	TARGET_LINKER="${gcc_name}"

	echo -e "${COLOR_LIGHT_GREEN}Configured Linux musl toolchain for $rust_target${COLOR_RESET}"
}

# Get Linux GNU cross-compilation environment
function getLinuxGnuEnv() {
	echo -e "${COLOR_LIGHT_YELLOW}Using default GNU toolchain for $rust_target${COLOR_RESET}"
}

# Get Windows cross-compilation environment
function getWindowsEnv() {
	local arch="$1"
	local rust_target="$2"

	local arch_prefix=""
	case "$arch" in
	"i686")
		arch_prefix="i686"
		;;
	"x86_64")
		arch_prefix="x86_64"
		;;
	*)
		echo -e "${COLOR_LIGHT_RED}Unsupported Windows architecture: $arch${COLOR_RESET}"
		return 1
		;;
	esac

	local cross_compiler_name="${arch_prefix}-w64-mingw32-cross"
	local gcc_name="${arch_prefix}-w64-mingw32-gcc"
	local ar_name="${arch_prefix}-w64-mingw32-ar"
	local linker_name="${gcc_name}"

	# Check if cross-compiler exists or download it
	if ! command -v "$gcc_name" >/dev/null 2>&1; then
		if [[ ! -x "${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/${gcc_name}" ]]; then
			local unamespacer="${HOST_OS}-${HOST_ARCH}"
			[[ "${HOST_ARCH}" == "arm" ]] && unamespacer="${HOST_OS}-arm32v7"
			[[ "${HOST_ARCH}" == "x86_64" ]] && unamespacer="${HOST_OS}-amd64"

			downloadAndUnzip "${GH_PROXY}https://github.com/zijiren233/musl-cross-make/releases/download/${CROSS_DEPS_VERSION}/${cross_compiler_name}-${unamespacer}.tgz" \
				"${CROSS_COMPILER_DIR}/${cross_compiler_name}" || return 2
		fi
		# Store the additional path needed for this target
		EXTRA_PATH="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin"
	fi

	TARGET_CC="${gcc_name}"
	TARGET_CXX="${arch_prefix}-w64-mingw32-g++"
	TARGET_AR="${ar_name}"
	TARGET_LINKER="${linker_name}"

	echo -e "${COLOR_LIGHT_GREEN}Configured Windows toolchain for $rust_target${COLOR_RESET}"
}

# Get Darwin (macOS) environment
# Need install patchelf
function getDarwinEnv() {
	local arch="$1"
	local rust_target="$2"

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		echo -e "${COLOR_LIGHT_GREEN}Using native macOS toolchain for $rust_target${COLOR_RESET}"
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
			echo -e "${COLOR_LIGHT_YELLOW}Cross-compilation to macOS not supported on ${HOST_OS}/${HOST_ARCH}${COLOR_RESET}"
			return 1
			;;
		esac

		local osxcross_dir="${CROSS_COMPILER_DIR}/osxcross-${host_arch_name}"

		if command -v o64-clang >/dev/null 2>&1; then
			if [[ "${arch}" == "x86_64" ]]; then
				TARGET_CC="x86_64-apple-darwin23.5-clang"
				TARGET_CXX="x86_64-apple-darwin23.5-clang++"
				TARGET_AR="x86_64-apple-darwin23.5-ar"
				TARGET_LINKER="x86_64-apple-darwin23.5-clang"
			else
				TARGET_CC="aarch64-apple-darwin23.5-clang"
				TARGET_CXX="aarch64-apple-darwin23.5-clang++"
				TARGET_AR="aarch64-apple-darwin23.5-ar"
				TARGET_LINKER="aarch64-apple-darwin23.5-clang"
			fi
		elif [[ -x "${osxcross_dir}/bin/o64-clang" ]]; then
			patchelf --set-rpath "${osxcross_dir}/lib" \
				${osxcross_dir}/bin/x86_64-apple-darwin*-ld || return 2

			EXTRA_PATH="${osxcross_dir}/bin:${osxcross_dir}/clang/bin"
		else
			# Determine download URL based on host architecture
			local download_url=""
			local ubuntu_version=$(lsb_release -rs 2>/dev/null || echo "20.04")
			[[ "$ubuntu_version" != *"."* ]] && ubuntu_version="20.04"
			if [[ "${host_arch_name}" == "amd64" ]]; then
				download_url="${GH_PROXY}https://github.com/zijiren233/osxcross/releases/download/v0.2.2/osxcross-14-5-linux-x86_64-gnu-ubuntu-${ubuntu_version}.tar.gz"
			else
				download_url="${GH_PROXY}https://github.com/zijiren233/osxcross/releases/download/v0.2.2/osxcross-14-5-linux-aarch64-gnu-ubuntu-${ubuntu_version}.tar.gz"
			fi

			downloadAndUnzip "${download_url}" "${osxcross_dir}" || return 2

			patchelf --set-rpath "${osxcross_dir}/lib" \
				${osxcross_dir}/bin/x86_64-apple-darwin*-ld || return 2

			EXTRA_PATH="${osxcross_dir}/bin:${osxcross_dir}/clang/bin"
		fi

		# Set compiler paths based on target architecture
		if [[ "${arch}" == "x86_64" ]]; then
			TARGET_CC="${osxcross_dir}/bin/x86_64-apple-darwin23.5-clang"
			TARGET_CXX="${osxcross_dir}/bin/x86_64-apple-darwin23.5-clang++"
			TARGET_AR="${osxcross_dir}/bin/x86_64-apple-darwin23.5-ar"
			TARGET_LINKER="${osxcross_dir}/bin/x86_64-apple-darwin23.5-clang"
		else
			TARGET_CC="${osxcross_dir}/bin/aarch64-apple-darwin23.5-clang"
			TARGET_CXX="${osxcross_dir}/bin/aarch64-apple-darwin23.5-clang++"
			TARGET_AR="${osxcross_dir}/bin/aarch64-apple-darwin23.5-ar"
			TARGET_LINKER="${osxcross_dir}/bin/aarch64-apple-darwin23.5-clang"
		fi

		echo -e "${COLOR_LIGHT_GREEN}Configured osxcross toolchain for $rust_target${COLOR_RESET}"
		;;
	*)
		echo -e "${COLOR_LIGHT_YELLOW}Cross-compilation to macOS not supported on ${HOST_OS}${COLOR_RESET}"
		return 1
		;;
	esac
}

# Get Android environment
function getAndroidEnv() {
	local arch="$1"
	local rust_target="$2"

	local ndk_dir="${CROSS_COMPILER_DIR}/android-ndk-${HOST_OS}-${NDK_VERSION}"
	local clang_base_dir="${ndk_dir}/toolchains/llvm/prebuilt/${HOST_OS}-x86_64/bin"

	if [[ ! -d "${ndk_dir}" ]]; then
		local ndk_url="https://dl.google.com/android/repository/android-ndk-${NDK_VERSION}-${HOST_OS}.zip"
		downloadAndUnzip "${ndk_url}" "${ndk_dir}" "zip" || return 2
		mv "$ndk_dir/android-ndk-${NDK_VERSION}/"* "$ndk_dir"
		rmdir "$ndk_dir/android-ndk-${NDK_VERSION}" || return 2
	fi

	local API="${API:-24}"
	local clang_prefix=""

	case "$arch" in
	"armv7")
		clang_prefix="armv7a-linux-androideabi${API}"
		;;
	"aarch64")
		clang_prefix="aarch64-linux-android${API}"
		;;
	"i686")
		clang_prefix="i686-linux-android${API}"
		;;
	"x86_64")
		clang_prefix="x86_64-linux-android${API}"
		;;
	esac

	TARGET_CC="${clang_base_dir}/${clang_prefix}-clang"
	TARGET_CXX="${clang_base_dir}/${clang_prefix}-clang++"
	TARGET_AR="${clang_base_dir}/llvm-ar"
	TARGET_LINKER="${clang_base_dir}/${clang_prefix}-clang"

	echo -e "${COLOR_LIGHT_GREEN}Configured Android toolchain for $rust_target${COLOR_RESET}"
}

# Get iOS environment
function getIosEnv() {
	local arch="$1"
	local rust_target="$2"

	case "${HOST_OS}" in
	"darwin")
		# Native compilation on macOS
		echo -e "${COLOR_LIGHT_GREEN}Using native macOS toolchain for $rust_target${COLOR_RESET}"
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
			echo -e "${COLOR_LIGHT_YELLOW}Unknown iOS architecture: ${arch}${COLOR_RESET}"
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

		if command -v "$clang_name" >/dev/null 2>&1; then
			# Cross-compiler already available in PATH
			# Get CC directory and set SDKROOT to ../SDK/ first folder
			local cc_dir="$(dirname "$(command -v "$clang_name")")"
			local sdk_dir="${cc_dir}/../SDK"
			if [[ -d "$sdk_dir" ]]; then
				local first_sdk="$(find "$sdk_dir" -maxdepth 1 -type d ! -path "$sdk_dir" | head -n 1)"
				if [[ -n "$first_sdk" ]]; then
					SDKROOT="$first_sdk"
				fi
			fi

			TARGET_CC="${clang_name}"
			TARGET_CXX="${clangxx_name}"
			TARGET_AR="${ar_name}"
			TARGET_LINKER="${linker_name}"
		elif [[ -x "${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/${clang_name}" ]]; then
			# Cross-compiler already downloaded
			# Fix rpath if on Linux
			patchelf --set-rpath "${CROSS_COMPILER_DIR}/${cross_compiler_name}/lib" \
				${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/${arch_prefix}-apple-darwin*-ld || return 2

			EXTRA_PATH="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin"
			# Set SDKROOT to first folder in SDK directory
			local sdk_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}/SDK"
			if [[ -d "$sdk_dir" ]]; then
				local first_sdk="$(find "$sdk_dir" -maxdepth 1 -type d ! -path "$sdk_dir" | head -n 1)"
				if [[ -n "$first_sdk" ]]; then
					SDKROOT="$first_sdk"
				fi
			fi
		else
			# Download cross-compiler
			local unamespacer="${HOST_OS}-${HOST_ARCH}"

			local ubuntu_version=""
			ubuntu_version=$(lsb_release -rs 2>/dev/null || echo "20.04")
			[[ "$ubuntu_version" != *"."* ]] && ubuntu_version="20.04"

			local download_url=""
			if [[ "${arch}" == "x86_64" ]]; then
				download_url="${GH_PROXY}https://github.com/zijiren233/cctools-port/releases/download/v0.1.6/ioscross-iPhoneSimulator18-5-x86_64-${unamespacer}-gnu-ubuntu-${ubuntu_version}.tar.gz"
			else
				download_url="${GH_PROXY}https://github.com/zijiren233/cctools-port/releases/download/v0.1.6/ioscross-iPhoneOS18-5-arm64-${unamespacer}-gnu-ubuntu-${ubuntu_version}.tar.gz"
			fi

			downloadAndUnzip "$download_url" "${CROSS_COMPILER_DIR}/${cross_compiler_name}" || return 2

			# Fix rpath if on Linux
			patchelf --set-rpath "${CROSS_COMPILER_DIR}/${cross_compiler_name}/lib" \
				${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/${arch_prefix}-apple-darwin*-ld || return 2

			EXTRA_PATH="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin:${CROSS_COMPILER_DIR}/${cross_compiler_name}/clang/bin"
			# Set SDKROOT to first folder in SDK directory
			local sdk_dir="${CROSS_COMPILER_DIR}/${cross_compiler_name}/SDK"
			if [[ -d "$sdk_dir" ]]; then
				local first_sdk="$(find "$sdk_dir" -maxdepth 1 -type d ! -path "$sdk_dir" | head -n 1)"
				if [[ -n "$first_sdk" ]]; then
					SDKROOT="$first_sdk"
				fi
			fi
		fi

		# Set compiler paths based on target architecture
		if [[ "${arch}" == "x86_64" ]]; then
			TARGET_CC="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/x86_64-apple-darwin11-clang"
			TARGET_CXX="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/x86_64-apple-darwin11-clang++"
			TARGET_AR="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/x86_64-apple-darwin11-ar"
			TARGET_LINKER="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/x86_64-apple-darwin11-ld"
		else
			TARGET_CC="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/arm64-apple-darwin11-clang"
			TARGET_CXX="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/arm64-apple-darwin11-clang++"
			TARGET_AR="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/arm64-apple-darwin11-ar"
			TARGET_LINKER="${CROSS_COMPILER_DIR}/${cross_compiler_name}/bin/arm64-apple-darwin11-ld"
		fi

		echo -e "${COLOR_LIGHT_GREEN}Configured iOS toolchain for $rust_target${COLOR_RESET}"
		;;
	*)
		echo -e "${COLOR_LIGHT_YELLOW}Cross-compilation to macOS not supported on ${HOST_OS}${COLOR_RESET}"
		return 1
		;;
	esac
}

# Clean cache
function cleanCache() {
	if [[ "$CLEAN_CACHE" == "true" ]]; then
		echo -e "${COLOR_LIGHT_BLUE}Cleaning cache...${COLOR_RESET}"
		cargo clean --target "$1" 2>/dev/null || true
	fi
}

# Find built binaries in target directory
function findBuiltBinaries() {
	local target_dir="$1"
	local rust_target="$2"
	local profile="$3"

	# Determine file extension based on target
	local ext=""
	if [[ "$rust_target" == *"windows"* ]]; then
		ext=".exe"
	fi

	# Find all executable files in the target directory
	local binaries=()

	# Look for executables in the target directory
	if [[ -d "${target_dir}/${rust_target}/${profile}" ]]; then
		while IFS= read -r -d '' file; do
			# Check if it's a regular file and executable (or .exe for Windows)
			if [[ -f "$file" ]]; then
				if [[ -n "$ext" && "$file" == *"$ext" ]]; then
					binaries+=("$file")
				elif [[ -z "$ext" && -x "$file" && ! "$file" == *.* ]]; then
					# On Unix, executable without extension
					binaries+=("$file")
				fi
			fi
		done < <(find "${target_dir}/${rust_target}/${profile}" -maxdepth 1 -type f -print0 2>/dev/null)
	fi

	printf '%s\n' "${binaries[@]}"
}

# Execute command for a specific target
# https://doc.rust-lang.org/cargo/reference/config.html
function executeTarget() {
	local rust_target="$1"
	local command="$2"

	echo -e "${COLOR_LIGHT_GRAY}$(printSeparator)${COLOR_RESET}"
	echo -e "${COLOR_LIGHT_MAGENTA}Executing ${command} for ${rust_target}...${COLOR_RESET}"

	# Clean cache if requested
	cleanCache "$rust_target" || return $?

	# Initialize build-std flags
	TARGET_BUILD_STD=""

	# Get cross-compilation environment and capture variables
	getCrossEnv "$rust_target" || return $?

	# Prepare environment variables
	local build_env=()

	# Set up PATH with target-specific tools if needed
	local target_path="$PATH"
	if [[ -n "$EXTRA_PATH" ]]; then
		target_path="${EXTRA_PATH}:${PATH}"
		build_env+=("PATH=${target_path}")
	fi

	# Set up environment based on target
	local target_upper=$(echo "$rust_target" | tr '[:lower:]' '[:upper:]' | tr '-' '_')

	if [[ -n "$TARGET_CC" ]]; then
		build_env+=("CC_${target_upper}=${TARGET_CC}")
		build_env+=("CC=${TARGET_CC}")
	fi

	if [[ -n "$TARGET_CXX" ]]; then
		build_env+=("CXX_${target_upper}=${TARGET_CXX}")
		build_env+=("CXX=${TARGET_CXX}")
	fi

	if [[ -n "$TARGET_AR" ]]; then
		build_env+=("AR_${target_upper}=${TARGET_AR}")
		build_env+=("AR=${TARGET_AR}")
	fi

	if [[ -n "$TARGET_LINKER" ]]; then
		build_env+=("CARGO_TARGET_${target_upper}_LINKER=${TARGET_LINKER}")
	fi

	if [[ -n "$SDKROOT" ]]; then
		build_env+=("SDKROOT=${SDKROOT}")
	fi

	# Prepare rustflags
	local rustflags=""
	if [[ -n "$TARGET_RUSTFLAGS" ]]; then
		rustflags="$TARGET_RUSTFLAGS"
	fi
	if [[ "$STATIC_CRT" == "true" ]]; then
		rustflags="${rustflags:+$rustflags }-C target-feature=+crt-static"
	fi
	if [[ -n "$ADDITIONAL_RUSTFLAGS" ]]; then
		rustflags="${rustflags:+$rustflags }$ADDITIONAL_RUSTFLAGS"
	fi
	if [[ -n "$rustflags" ]]; then
		build_env+=("RUSTFLAGS=${rustflags}")
	fi

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
	[[ "$WORKSPACE" == "true" ]] && cargo_cmd="$cargo_cmd --workspace"
	[[ -n "$MANIFEST_PATH" ]] && cargo_cmd="$cargo_cmd --manifest-path $MANIFEST_PATH"
	# Add build-std flag if needed (either from args or target requirements)
	if [[ "$BUILD_STD" == "true" ]] || [[ "$TARGET_BUILD_STD" == "true" ]]; then
		cargo_cmd="$cargo_cmd -Zbuild-std"
	fi
	[[ "$VERBOSE" == "true" ]] && cargo_cmd="$cargo_cmd --verbose"
	[[ -n "$ADDITIONAL_ARGS" ]] && cargo_cmd="$cargo_cmd $ADDITIONAL_ARGS"

	echo -e "${COLOR_LIGHT_BLUE}Environment variables:${COLOR_RESET}"
	for env_var in "${build_env[@]}"; do
		echo -e "  ${COLOR_LIGHT_CYAN}${env_var}${COLOR_RESET}"
	done

	echo -e "${COLOR_LIGHT_BLUE}Run command:${COLOR_RESET}"
	echo -e "  ${COLOR_LIGHT_CYAN}${cargo_cmd}${COLOR_RESET}"

	local start_time=$(date +%s)

	# Execute command with environment variables
	if [[ ${#build_env[@]} -gt 0 ]]; then
		env "${build_env[@]}" $cargo_cmd || return $?
	else
		$cargo_cmd || return $?
	fi

	local end_time=$(date +%s)

	# Only handle binary output for build command
	if [[ "$command" != "build" ]]; then
		echo -e "${COLOR_LIGHT_GREEN}${command^} successful: ${rust_target} (took $((end_time - start_time))s)${COLOR_RESET}"
		return
	fi

	# Find and copy built binaries
	echo -e "${COLOR_LIGHT_BLUE}Looking for built binaries...${COLOR_RESET}"

	local found_binaries=()
	readarray -t found_binaries < <(findBuiltBinaries "${SOURCE_DIR}/target" "$rust_target" "$PROFILE")

	if [[ ${#found_binaries[@]} -eq 0 ]]; then
		echo -e "${COLOR_LIGHT_YELLOW}No binaries found, checking for libraries...${COLOR_RESET}"

		# Check for library outputs
		for lib_ext in ".a" ".so" ".dylib" ".dll" ".rlib"; do
			for lib_file in "${SOURCE_DIR}/target/${rust_target}/${PROFILE}/"*"${lib_ext}"; do
				if [[ -f "$lib_file" ]]; then
					local lib_name=$(basename "$lib_file")
					local dest_lib="${RESULT_DIR}/${lib_name%.${lib_ext}}"

					# Add target suffix unless disabled
					if [[ -z "$BIN_NAME_NO_SUFFIX" ]]; then
						local suffix=$(echo "$rust_target" | sed 's/-unknown//g')
						dest_lib="${dest_lib}-${suffix}"
					fi
					dest_lib="${dest_lib}${lib_ext}"

					mkdir -p "${RESULT_DIR}"
					cp "$lib_file" "$dest_lib"
					echo -e "${COLOR_LIGHT_GREEN}Library copied: ${dest_lib}${COLOR_RESET}"
				fi
			done
		done
		return
	fi

	# Copy found binaries
	for binary in "${found_binaries[@]}"; do
		local binary_name=$(basename "$binary")
		# Remove .exe extension for naming
		local base_name="${binary_name%.exe}"

		# Determine destination name
		local dest_binary="${RESULT_DIR}/${base_name}"

		# Add target suffix unless disabled
		if [[ -z "$BIN_NAME_NO_SUFFIX" ]]; then
			local suffix=$(echo "$rust_target" | sed 's/-unknown//g')
			dest_binary="${dest_binary}-${suffix}"
		fi

		# Add back extension if needed
		[[ "$binary_name" == *.exe ]] && dest_binary="${dest_binary}.exe"

		mkdir -p "${RESULT_DIR}"
		cp "$binary" "$dest_binary"

		# Strip binary in release mode
		if [[ "$PROFILE" == "release" ]] && [[ "$NO_STRIP" != "true" ]]; then
			# Try to find appropriate strip tool
			local strip_cmd=""
			if [[ -n "$TARGET_LINKER" ]]; then
				# Try to find strip based on linker
				local strip_tool="${TARGET_LINKER%-gcc}-strip"
				if command -v "$strip_tool" >/dev/null 2>&1; then
					strip_cmd="$strip_tool"
				fi
			fi

			# Fallback to default strip for native builds
			if [[ -z "$strip_cmd" ]] && [[ "$rust_target" == "$HOST_TRIPLE" ]]; then
				strip_cmd="strip"
			fi

			if [[ -n "$strip_cmd" ]] && command -v "$strip_cmd" >/dev/null 2>&1; then
				echo -e "${COLOR_LIGHT_BLUE}Stripping binary with: ${strip_cmd}${COLOR_RESET}"
				"$strip_cmd" "$dest_binary" 2>/dev/null || true
			fi
		fi

		echo -e "${COLOR_LIGHT_GREEN}Binary copied: ${dest_binary} (size: $(du -sh "${dest_binary}" | cut -f1))${COLOR_RESET}"
	done

	echo -e "${COLOR_LIGHT_GREEN}Build successful: ${rust_target} (took $((end_time - start_time))s)${COLOR_RESET}"
}

# Expand target patterns (e.g., "linux/*" or "all")
function expandTargets() {
	local targets="$1"
	local expanded=""

	IFS=',' read -ra TARGET_ARRAY <<<"$targets"
	for target in "${TARGET_ARRAY[@]}"; do
		target=$(echo "$target" | xargs) # Trim whitespace

		if [[ "$target" == "all" ]]; then
			# Return all supported targets
			for key in "${!TOOLCHAIN_CONFIG[@]}"; do
				expanded="${expanded}${key},"
			done
		elif [[ "$target" == *"*"* ]]; then
			# Pattern matching (e.g., "*-linux-musl")
			for key in "${!TOOLCHAIN_CONFIG[@]}"; do
				if [[ "$key" == $target ]]; then
					expanded="${expanded}${key},"
				fi
			done
		else
			# Direct target
			expanded="${expanded}${target},"
		fi
	done

	# Remove trailing comma and duplicates
	expanded="${expanded%,}"
	echo "$expanded" | tr ',' '\n' | sort -u | paste -sd ',' -
}

# Initialize variables
setDefault "SOURCE_DIR" "${DEFAULT_SOURCE_DIR}"
SOURCE_DIR="$(cd "${SOURCE_DIR}" && pwd)"
setDefault "BUILD_CONFIG" "${SOURCE_DIR}/build.config.sh"
setDefault "RESULT_DIR" "${DEFAULT_RESULT_DIR}"
setDefault "PROFILE" "${DEFAULT_PROFILE}"
setDefault "CROSS_COMPILER_DIR" "${DEFAULT_CROSS_COMPILER_DIR}"
setDefault "CROSS_DEPS_VERSION" "${DEFAULT_CROSS_DEPS_VERSION}"
setDefault "NDK_VERSION" "${DEFAULT_NDK_VERSION}"
setDefault "COMMAND" "${DEFAULT_COMMAND}"
setDefault "TOOLCHAIN" "${DEFAULT_TOOLCHAIN}"

# Load build configuration if exists
if [[ -f "${BUILD_CONFIG}" ]]; then
	echo -e "${COLOR_LIGHT_BLUE}Loading build configuration from ${BUILD_CONFIG}${COLOR_RESET}"
	source "${BUILD_CONFIG}"
fi

# Parse command-line arguments
# First argument might be a command
if [[ $# -gt 0 ]] && [[ "$1" =~ ^(build|test|check)$ ]]; then
	COMMAND="$1"
	shift
fi

while [[ $# -gt 0 ]]; do
	case "${1}" in
	-h | --help)
		printHelp
		exit 0
		;;
	--profile=*)
		PROFILE="${1#*=}"
		;;
	--profile)
		shift
		PROFILE="$1"
		;;
	--bin-name=*)
		BIN_NAME="${1#*=}"
		;;
	--bin-name)
		shift
		BIN_NAME="$1"
		;;
	--bin-name-no-suffix)
		BIN_NAME_NO_SUFFIX="true"
		;;
	--features=*)
		FEATURES="${1#*=}"
		;;
	--features)
		shift
		FEATURES="$1"
		;;
	--no-default-features)
		NO_DEFAULT_FEATURES="true"
		;;
	--all-features)
		ALL_FEATURES="true"
		;;
	-t=* | --targets=*)
		TARGETS="${1#*=}"
		;;
	-t | --targets)
		shift
		TARGETS="$1"
		;;
	--result-dir=*)
		RESULT_DIR="${1#*=}"
		;;
	--result-dir)
		shift
		RESULT_DIR="$1"
		;;
	--show-all-targets)
		echo -e "${COLOR_LIGHT_GREEN}Supported Rust targets:${COLOR_RESET}"
		for key in $(printf '%s\n' "${!TOOLCHAIN_CONFIG[@]}" | sort); do
			echo "  $key"
		done
		exit 0
		;;
	--github-proxy-mirror=*)
		GH_PROXY="${1#*=}"
		;;
	--github-proxy-mirror)
		shift
		GH_PROXY="$1"
		;;
	--cross-compiler-dir=*)
		CROSS_COMPILER_DIR="${1#*=}"
		;;
	--cross-compiler-dir)
		shift
		CROSS_COMPILER_DIR="$1"
		;;
	--ndk-version=*)
		NDK_VERSION="${1#*=}"
		;;
	--ndk-version)
		shift
		NDK_VERSION="$1"
		;;
	--package=*)
		PACKAGE="${1#*=}"
		;;
	--package)
		shift
		PACKAGE="$1"
		;;
	--bin=*)
		BIN_TARGET="${1#*=}"
		;;
	--bin)
		shift
		BIN_TARGET="$1"
		;;
	--workspace)
		WORKSPACE="true"
		;;
	--manifest-path=*)
		MANIFEST_PATH="${1#*=}"
		;;
	--manifest-path)
		shift
		MANIFEST_PATH="$1"
		;;
	--use-default-linker)
		USE_DEFAULT_LINKER="true"
		;;
	--cc=*)
		CC="${1#*=}"
		;;
	--cc)
		shift
		CC="$1"
		;;
	--cxx=*)
		CXX="${1#*=}"
		;;
	--cxx)
		shift
		CXX="$1"
		;;
	--rustflags=*)
		ADDITIONAL_RUSTFLAGS="${1#*=}"
		;;
	--rustflags)
		shift
		ADDITIONAL_RUSTFLAGS="$1"
		;;
	--static-crt)
		STATIC_CRT="true"
		;;
	--build-std)
		BUILD_STD="true"
		;;
	--args=*)
		ADDITIONAL_ARGS="${1#*=}"
		;;
	--args)
		shift
		ADDITIONAL_ARGS="$1"
		;;
	--toolchain=*)
		TOOLCHAIN="${1#*=}"
		;;
	--toolchain)
		shift
		TOOLCHAIN="$1"
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
		echo -e "${COLOR_LIGHT_RED}Invalid option: $1${COLOR_RESET}"
		exit 1
		;;
	esac
	shift
done

# Default to host target if not specified
if [[ -z "$TARGETS" ]]; then
	TARGETS="$HOST_TRIPLE"
	echo -e "${COLOR_LIGHT_BLUE}No target specified, using host: ${TARGETS}${COLOR_RESET}"
fi

# Expand target patterns
TARGETS=$(expandTargets "$TARGETS")

# Print execution information
echo -e "${COLOR_LIGHT_BLUE}Execution configuration:${COLOR_RESET}"
echo -e "  Command: ${COMMAND}"
echo -e "  Source directory: ${SOURCE_DIR}"
echo -e "  Result directory: ${RESULT_DIR}"
[[ -n "$PACKAGE" ]] && echo -e "  Package: ${PACKAGE}"
[[ -n "$BIN_TARGET" ]] && echo -e "  Binary target: ${BIN_TARGET}"
[[ "$WORKSPACE" == "true" ]] && echo -e "  Building workspace: true"
echo -e "  Profile: ${PROFILE}"
[[ -n "$TOOLCHAIN" ]] && echo -e "  Toolchain: ${TOOLCHAIN}"
echo -e "  Targets: ${TARGETS}"
[[ -n "$FEATURES" ]] && echo -e "  Features: ${FEATURES}"
[[ "$NO_DEFAULT_FEATURES" == "true" ]] && echo -e "  No default features: true"
[[ "$ALL_FEATURES" == "true" ]] && echo -e "  All features: true"
[[ -n "$ADDITIONAL_RUSTFLAGS" ]] && echo -e "  Additional rustflags: ${ADDITIONAL_RUSTFLAGS}"
[[ -n "$ADDITIONAL_ARGS" ]] && echo -e "  Additional args: ${ADDITIONAL_ARGS}"

# Build for each target
IFS=',' read -ra TARGET_ARRAY <<<"$TARGETS"
TOTAL_TARGETS=${#TARGET_ARRAY[@]}
CURRENT_TARGET=0
BUILD_START_TIME=$(date +%s)

for target in "${TARGET_ARRAY[@]}"; do
	CURRENT_TARGET=$((CURRENT_TARGET + 1))
	echo -e "${COLOR_LIGHT_GREEN}[${CURRENT_TARGET}/${TOTAL_TARGETS}] Processing target: ${target}${COLOR_RESET}"
	executeTarget "$target" "$COMMAND" || {
		echo -e "${COLOR_LIGHT_RED}${COMMAND^} failed for target: ${target}${COLOR_RESET}"
		exit 1
	}
done

BUILD_END_TIME=$(date +%s)
TOTAL_TIME=$((BUILD_END_TIME - BUILD_START_TIME))

echo -e "${COLOR_LIGHT_GRAY}$(printSeparator)${COLOR_RESET}"
echo -e "${COLOR_LIGHT_GREEN}All ${COMMAND} operations completed successfully!${COLOR_RESET}"
echo -e "${COLOR_LIGHT_GREEN}Total time: ${TOTAL_TIME}s${COLOR_RESET}"

# Only show result directory for build command
if [[ "$COMMAND" == "build" ]]; then
	echo -e "${COLOR_LIGHT_GREEN}Results in: ${RESULT_DIR}${COLOR_RESET}"

	# List all built files
	if [[ -d "$RESULT_DIR" ]]; then
		echo -e "${COLOR_LIGHT_BLUE}Built files:${COLOR_RESET}"
		ls -lh "$RESULT_DIR" | tail -n +2 | while read line; do
			echo -e "  ${COLOR_LIGHT_CYAN}${line}${COLOR_RESET}"
		done
	fi
fi
