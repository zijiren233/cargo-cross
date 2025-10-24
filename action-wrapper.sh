#!/bin/bash
set -e

# Action wrapper script for rust-build-action
# This script translates GitHub Actions inputs (via environment variables)
# into command-line arguments for cross.sh

# Build command arguments from environment variables
ARGS=()

# Command (build, test, check)
ARGS+=("${INPUT_COMMAND}")

# Targets (prioritize INPUT_TARGET over INPUT_TARGETS)
if [ -n "$INPUT_TARGET" ]; then
  ARGS+=("--targets=$INPUT_TARGET")
elif [ -n "$INPUT_TARGETS" ]; then
  ARGS+=("--targets=$INPUT_TARGETS")
fi

# Profile
ARGS+=("--profile=$INPUT_PROFILE")

# Features
[ -n "$INPUT_FEATURES" ] && ARGS+=("--features=$INPUT_FEATURES")
[ "$INPUT_NO_DEFAULT_FEATURES" = "true" ] && ARGS+=("--no-default-features")
[ "$INPUT_ALL_FEATURES" = "true" ] && ARGS+=("--all-features")

# Package/Binary options
[ -n "$INPUT_PACKAGE" ] && ARGS+=("--package=$INPUT_PACKAGE")
[ -n "$INPUT_BIN" ] && ARGS+=("--bin=$INPUT_BIN")
[ "$INPUT_BINS" = "true" ] && ARGS+=("--bins")
[ "$INPUT_LIB" = "true" ] && ARGS+=("--lib")
[ "$INPUT_ALL_TARGETS" = "true" ] && ARGS+=("--all-targets")
[ "$INPUT_WORKSPACE" = "true" ] && ARGS+=("--workspace")

# Build options
[ "$INPUT_RELEASE" = "true" ] && ARGS+=("--release")
[ "$INPUT_QUIET" = "true" ] && ARGS+=("--quiet")
[ -n "$INPUT_MESSAGE_FORMAT" ] && ARGS+=("--message-format=$INPUT_MESSAGE_FORMAT")

# Cargo options
[ "$INPUT_IGNORE_RUST_VERSION" = "true" ] && ARGS+=("--ignore-rust-version")
[ "$INPUT_LOCKED" = "true" ] && ARGS+=("--locked")
[ "$INPUT_OFFLINE" = "true" ] && ARGS+=("--offline")
[ "$INPUT_FROZEN" = "true" ] && ARGS+=("--frozen")
[ -n "$INPUT_JOBS" ] && ARGS+=("--jobs=$INPUT_JOBS")
[ "$INPUT_KEEP_GOING" = "true" ] && ARGS+=("--keep-going")
[ "$INPUT_FUTURE_INCOMPAT_REPORT" = "true" ] && ARGS+=("--future-incompat-report")

# Path options
[ -n "$INPUT_MANIFEST_PATH" ] && ARGS+=("--manifest-path=$INPUT_MANIFEST_PATH")

# Binary name options
[ "$INPUT_BIN_NAME_NO_SUFFIX" = "true" ] && ARGS+=("--bin-name-no-suffix")

# Cross-compilation options
[ -n "$INPUT_GITHUB_PROXY_MIRROR" ] && ARGS+=("--github-proxy-mirror=$INPUT_GITHUB_PROXY_MIRROR")
[ -n "$INPUT_CROSS_COMPILER_DIR" ] && ARGS+=("--cross-compiler-dir=$INPUT_CROSS_COMPILER_DIR")
ARGS+=("--ndk-version=$INPUT_NDK_VERSION")
[ "$INPUT_USE_DEFAULT_LINKER" = "true" ] && ARGS+=("--use-default-linker")

# Compiler options
[ -n "$INPUT_CC" ] && ARGS+=("--cc=$INPUT_CC")
[ -n "$INPUT_CXX" ] && ARGS+=("--cxx=$INPUT_CXX")

# Rust flags
[ -n "$INPUT_RUSTFLAGS" ] && ARGS+=("--rustflags=$INPUT_RUSTFLAGS")
[ -n "$INPUT_STATIC_CRT" ] && ARGS+=("--static-crt=$INPUT_STATIC_CRT")

# Build-std
if [ "$INPUT_BUILD_STD" != "false" ] && [ -n "$INPUT_BUILD_STD" ]; then
  if [ "$INPUT_BUILD_STD" = "true" ]; then
    ARGS+=("--build-std")
  else
    ARGS+=("--build-std=$INPUT_BUILD_STD")
  fi
fi

# Cache and strip options
[ "$INPUT_CLEAN_CACHE" = "true" ] && ARGS+=("--clean-cache")
[ "$INPUT_NO_STRIP" = "true" ] && ARGS+=("--no-strip")

# Verbose
[ "$INPUT_VERBOSE" = "true" ] && ARGS+=("--verbose")

# Additional args
[ -n "$INPUT_ARGS" ] && ARGS+=("--args=$INPUT_ARGS")

# Toolchain
[ -n "$INPUT_TOOLCHAIN" ] && ARGS+=("--toolchain=$INPUT_TOOLCHAIN")

# Trim paths
if [ -n "$INPUT_CARGO_TRIM_PATHS" ]; then
  ARGS+=("--cargo-trim-paths=$INPUT_CARGO_TRIM_PATHS")
elif [ -n "$INPUT_TRIM_PATHS" ]; then
  ARGS+=("--trim-paths=$INPUT_TRIM_PATHS")
fi

# No embed metadata
[ "$INPUT_NO_EMBED_METADATA" = "true" ] && ARGS+=("--no-embed-metadata")

# Execute command
"$ACTION_PATH/cross.sh" "${ARGS[@]}"

# Determine targets for output (prioritize INPUT_TARGET over INPUT_TARGETS)
OUTPUT_TARGETS="${INPUT_TARGET}"
if [ -z "$OUTPUT_TARGETS" ]; then
  OUTPUT_TARGETS="${INPUT_TARGETS}"
fi
if [ -z "$OUTPUT_TARGETS" ]; then
  OUTPUT_TARGETS="$(rustc -vV | grep host | cut -d' ' -f2)"
fi
echo "targets=$OUTPUT_TARGETS" >> $GITHUB_OUTPUT
