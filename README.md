# Rust Cross Build Action

A powerful GitHub Action for building, testing, and checking Rust projects with cross-compilation support. This action automatically downloads and configures the necessary cross-compilation toolchains, making it easy to execute various Rust commands for multiple platforms. No Need Docker!

## Features

- 🚀 **Cross-compilation support** for Linux (GNU/musl), Windows, macOS, FreeBSD, Android, and iOS
- 🖥️ **Multi-platform hosts** - runs on Linux (x86_64/aarch64/armv7/riscv64/s390x/powerpc64/powerpc64le/mips64/mips64el/loongarch64), macOS (x86_64/aarch64), and Windows (x86_64)
- 📦 **Automatic toolchain setup** - downloads and configures cross-compilers as needed
- 🎯 **Multiple target support** - build for 63+ target platforms in a single run
- 🏗️ **Workspace support** - work with entire workspaces or specific packages
- ⚡ **Flexible linking** - some musl targets default to static (varies by target), GNU targets default to dynamic, both configurable via `crt-static` parameter
- 🔧 **Flexible configuration** - extensive customization options
- 🛠️ **Multiple commands** - supports build, bench, test, and check operations

## Local Usage

### Installation as Cargo Subcommand

You can install this tool as a cargo subcommand for easy cross-compilation:

```bash
# Install from crate
cargo install cargo-cross

# Install from GitHub
cargo install cargo-cross --git https://github.com/zijiren233/cargo-cross

# Or install from local path
cargo install --path .
```

After installation, you can use `cargo cross` command:

```bash
# Show help
cargo cross --help

# Show all supported targets
cargo cross --show-all-targets

# Build for a specific target
cargo cross build --target x86_64-unknown-linux-musl

# Build for multiple targets
cargo cross build --targets x86_64-unknown-linux-musl,aarch64-unknown-linux-musl

# Build with release profile
cargo cross build --target x86_64-unknown-linux-musl --release

# Build with features
cargo cross build --target x86_64-unknown-linux-musl --features feature1,feature2

# Test for a target
cargo cross test --target x86_64-unknown-linux-musl

# Check the project
cargo cross check --target x86_64-unknown-linux-musl

# Build with specific glibc version for GNU targets
cargo cross build --target x86_64-unknown-linux-gnu --glibc-version 2.31

# Build iOS targets with specific iPhone SDK version
cargo cross build --target aarch64-apple-ios --iphone-sdk-version 18.2

# Build macOS targets with specific macOS SDK version (native macOS only)
cargo cross build --target aarch64-apple-darwin --macos-sdk-version 14.0

# Build FreeBSD targets with specific FreeBSD version
cargo cross build --target x86_64-unknown-freebsd --freebsd-version 14

# Build with specific cross-make version
cargo cross build --target x86_64-unknown-linux-musl --cross-make-version v0.7.7

# Build with custom SDK path (skips version lookup)
cargo cross build --target aarch64-apple-darwin --macos-sdk-path /path/to/MacOSX.sdk
cargo cross build --target aarch64-apple-ios --iphone-sdk-path /path/to/iPhoneOS.sdk
cargo cross build --target aarch64-apple-ios-sim --iphone-simulator-sdk-path /path/to/iPhoneSimulator.sdk
```

## GitHub Actions Usage

### Basic Usage

```yaml
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Cross compile
        uses: zijiren233/cargo-cross@v1
        with:
          command: build
          targets: |
            x86_64-unknown-linux-musl
            aarch64-unknown-linux-musl
```

### Build for Multiple Platforms

```yaml
name: Release

on:
  release:
    types: [created]

jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v3
      
      - name: Build Release
        uses: zijiren233/cargo-cross@v1
        with:
          command: build
          targets: |
            x86_64-unknown-linux-musl
            aarch64-unknown-linux-musl
            x86_64-pc-windows-gnu
            x86_64-apple-darwin
            aarch64-apple-darwin
          profile: release

      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: binaries-${{ matrix.os }}
          path: target/*/release/*
```

## Host Platforms (Runners)

This action can run on the following GitHub Actions runners or local platforms:

### Supported Host Platforms

| Platform | Architecture | GitHub Runner | Local Support |
|----------|--------------|---------------|---------------|
| **Linux** | x86_64 (amd64) | `ubuntu-latest`, `ubuntu-24.04`, `ubuntu-22.04`, `ubuntu-20.04` | ✅ Yes |
| **Linux** | aarch64 (arm64) | `ubuntu-24.04-arm` | ✅ Yes |
| **Linux** | armv7 | Self-hosted runners | ✅ Yes |
| **Linux** | riscv64 | Self-hosted runners | ✅ Yes |
| **Linux** | s390x | Self-hosted runners | ✅ Yes |
| **Linux** | powerpc64 | Self-hosted runners | ✅ Yes |
| **Linux** | powerpc64le | Self-hosted runners | ✅ Yes |
| **Linux** | mips64 | Self-hosted runners | ✅ Yes |
| **Linux** | mips64el | Self-hosted runners | ✅ Yes |
| **Linux** | loongarch64 | Self-hosted runners | ✅ Yes |
| **macOS** | x86_64 (Intel) | `macos-15-intel` | ✅ Yes |
| **macOS** | aarch64 (Apple Silicon) | `macos-15` | ✅ Yes |
| **Windows** | x86_64 | `windows-latest` | ✅ Yes |

### Recommended Usage

For maximum compatibility and cross-compilation support, use **Linux x86_64** runners:

```yaml
jobs:
  build:
    runs-on: ubuntu-latest  # Best for cross-compilation
    steps:
      - uses: actions/checkout@v3
      - uses: zijiren233/cargo-cross@v1
        with:
          targets: |
            x86_64-unknown-linux-musl
            aarch64-unknown-linux-musl
            x86_64-pc-windows-gnu
            aarch64-apple-darwin
            aarch64-apple-ios
```

For macOS/iOS native/cross builds, use macOS runners:

```yaml
jobs:
  build-macos:
    runs-on: macos-latest  # Apple Silicon
    steps:
      - uses: actions/checkout@v3
      - uses: zijiren233/cargo-cross@v1
        with:
          targets: |
            x86_64-unknown-linux-musl
            aarch64-unknown-linux-musl
            x86_64-pc-windows-gnu
            aarch64-apple-darwin
            aarch64-apple-ios
```

## Supported Targets

### Linux (musl)

Most musl targets produce **statically linked binaries by default**, but this varies by target (check `rustc --print=target-spec-json -Z unstable-options --target <target>` for the actual default). Use `crt-static: true/false` to explicitly control linking behavior.

- `i586-unknown-linux-musl` - Linux i586
- `i686-unknown-linux-musl` - Linux i686
- `x86_64-unknown-linux-musl` - Linux x86_64
- `arm-unknown-linux-musleabi` - ARM Linux
- `arm-unknown-linux-musleabihf` - ARM Linux hard-float
- `armv5te-unknown-linux-musleabi` - ARMv5TE Linux
- `armv7-unknown-linux-musleabi` - ARMv7 Linux
- `armv7-unknown-linux-musleabihf` - ARMv7 Linux hard-float
- `aarch64-unknown-linux-musl` - ARM64 Linux
- `aarch64_be-unknown-linux-musl` - ARM64 big-endian Linux
- `loongarch64-unknown-linux-musl` - LoongArch64 Linux
- `mips-unknown-linux-musl` - MIPS Linux
- `mipsel-unknown-linux-musl` - MIPS little-endian Linux
- `mips64-unknown-linux-muslabi64` - MIPS64 Linux
- `mips64-openwrt-linux-musl` - MIPS64 OpenWrt Linux
- `mips64el-unknown-linux-muslabi64` - MIPS64 little-endian Linux
- `powerpc64-unknown-linux-musl` - PowerPC64 Linux
- `powerpc64le-unknown-linux-musl` - PowerPC64 little-endian Linux
- `riscv32gc-unknown-linux-musl` - RISC-V 32-bit Linux
- `riscv64gc-unknown-linux-musl` - RISC-V 64-bit Linux
- `s390x-unknown-linux-musl` - S390x Linux

### Linux (GNU libc - dynamic by default)

GNU libc targets produce **dynamically linked binaries by default**. Use `crt-static: true` to enable static linking.

**Glibc version**: By default, the latest stable version is used (no version suffix). You can specify a different version (2.17-2.43) using the `glibc-version` parameter for better compatibility with specific Linux distributions.

- `i586-unknown-linux-gnu` - Linux i586
- `i686-unknown-linux-gnu` - Linux i686
- `x86_64-unknown-linux-gnu` - Linux x86_64
- `x86_64-unknown-linux-gnux32` - Linux x86_64 x32 ABI
- `arm-unknown-linux-gnueabi` - ARM Linux
- `arm-unknown-linux-gnueabihf` - ARM Linux hard-float
- `armv5te-unknown-linux-gnueabi` - ARMv5TE Linux
- `armv7-unknown-linux-gnueabi` - ARMv7 Linux
- `armv7-unknown-linux-gnueabihf` - ARMv7 Linux hard-float
- `aarch64-unknown-linux-gnu` - ARM64 Linux
- `aarch64_be-unknown-linux-gnu` - ARM64 big-endian Linux
- `loongarch64-unknown-linux-gnu` - LoongArch64 Linux
- `mips-unknown-linux-gnu` - MIPS Linux
- `mipsel-unknown-linux-gnu` - MIPS little-endian Linux
- `mipsisa32r6-unknown-linux-gnu` - MIPS32 R6 Linux
- `mipsisa32r6el-unknown-linux-gnu` - MIPS32 R6 little-endian Linux
- `mips64-unknown-linux-gnuabi64` - MIPS64 Linux
- `mips64el-unknown-linux-gnuabi64` - MIPS64 little-endian Linux
- `mipsisa64r6-unknown-linux-gnuabi64` - MIPS64 R6 Linux
- `mipsisa64r6el-unknown-linux-gnuabi64` - MIPS64 R6 little-endian Linux
- `powerpc64-unknown-linux-gnu` - PowerPC64 Linux
- `powerpc64le-unknown-linux-gnu` - PowerPC64 little-endian Linux
- `riscv32gc-unknown-linux-gnu` - RISC-V 32-bit Linux
- `riscv64gc-unknown-linux-gnu` - RISC-V 64-bit Linux
- `s390x-unknown-linux-gnu` - S390x Linux

### Windows

- `i686-pc-windows-gnu` - Windows i686 (MinGW)
- `x86_64-pc-windows-gnu` - Windows x86_64 (MinGW)

### FreeBSD

- `x86_64-unknown-freebsd` - FreeBSD x86_64
- `aarch64-unknown-freebsd` - FreeBSD ARM64
- `powerpc64-unknown-freebsd` - FreeBSD PowerPC64
- `powerpc64le-unknown-freebsd` - FreeBSD PowerPC64 little-endian
- `riscv64gc-unknown-freebsd` - FreeBSD RISC-V 64-bit

### macOS

- `x86_64-apple-darwin` - macOS Intel (x86_64)
- `x86_64h-apple-darwin` - macOS Intel (x86_64h, optimized for Haswell+)
- `aarch64-apple-darwin` - macOS Apple Silicon (ARM64)
- `arm64e-apple-darwin` - macOS Apple Silicon (ARM64e)

### Android

- `i686-linux-android` - Android x86
- `x86_64-linux-android` - Android x86_64
- `armv7-linux-androideabi` - Android ARMv7
- `arm-linux-androideabi` - Android ARM
- `aarch64-linux-android` - Android ARM64
- `riscv64-linux-android` - Android RISC-V 64-bit

### iOS

- `x86_64-apple-ios` - iOS Simulator (x86_64)
- `aarch64-apple-ios` - iOS ARM64
- `aarch64-apple-ios-sim` - iOS ARM64 Simulator

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `command` | Command to execute (`build`, `test`, `check`) | `build` |
| `targets` | Newline-separated list of Rust target triples (comma-separated also supported) | Host target |
| `profile` | Build profile (`debug` or `release`) | `release` |
| `features` | Comma-separated list of features to activate | |
| `no-default-features` | Do not activate default features | `false` |
| `all-features` | Activate all available features | `false` |
| `package` | Package to build (workspace member) | |
| `bin` | Binary target to build | |
| `workspace` | Build all workspace members | `false` |
| `manifest-path` | Path to Cargo.toml | |
| `source-dir` | Directory containing the Rust project | `${{ github.workspace }}` |
| `github-proxy-mirror` | GitHub proxy mirror URL | |
| `cross-compiler-dir` | Directory to store cross compilers | |
| `ndk-version` | Android NDK version (e.g., r27d, r29) | `r27d` (LTS) |
| `glibc-version` | Glibc version for GNU targets (e.g., 2.31, 2.42) | (default) |
| `iphone-sdk-version` | iPhone SDK version for iOS targets (non-macOS: bundled SDKs, macOS: installed Xcode SDK) | (default 26.2) |
| `iphone-sdk-path` | Override iPhoneOS SDK path for device targets (skips version lookup, native macOS only) | |
| `iphone-simulator-sdk-path` | Override iPhoneSimulator SDK path for simulator targets (skips version lookup, native macOS only) | |
| `macos-sdk-version` | macOS SDK version for Darwin targets (non-macOS: bundled SDKs, macOS: installed Xcode SDK) | (default 26.2) |
| `macos-sdk-path` | Override macOS SDK path directly (skips version lookup, native macOS only) | |
| `freebsd-version` | FreeBSD version for FreeBSD targets (13, 14, or 15) | `13` |
| `qemu-version` | QEMU version for user-mode emulation (e.g., v10.2.0) | `v10.2.0` |
| `cross-make-version` | Cross-compiler make version (e.g., v0.7.7) | `v0.7.7` |
| `use-default-linker` | Use system default linker | `false` |
| `cc` | Force set the C compiler | |
| `cxx` | Force set the C++ compiler | |
| `rustflags` | Additional rustflags | |
| `crt-static` | Control CRT linking mode: `true` for static (+crt-static), `false` for dynamic (-crt-static), empty for target default (varies by target) | |
| `build-std` | Use -Zbuild-std for building standard library from source (`true` for default, or specify crates like `core,alloc`) | `false` |
| `args` | Additional arguments to pass to cargo command | |
| `toolchain` | Rust toolchain to use (stable, nightly, etc.) | `stable` |
| `cargo-trim-paths` | Set CARGO_TRIM_PATHS environment variable for reproducible builds | |
| `no-embed-metadata` | Add -Zno-embed-metadata flag to cargo | `false` |
| `rustc-bootstrap` | Set RUSTC_BOOTSTRAP environment variable: `1` for all crates, `-1` for stable behavior, or `crate_name` for specific crate | |
| `clean-cache` | Clean build cache before building | `false` |
| `no-strip` | Do not strip binaries | `false` |
| `verbose` | Use verbose output | `false` |

## Outputs

| Output | Description |
|--------|-------------|
| `targets` | Targets that were processed |

## Advanced Examples

### Build with Features

```yaml
- name: Build with features
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    features: feature1,feature2
    no-default-features: true
```

### Build Specific Binary from Workspace

```yaml
- name: Build specific binary
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    package: my-package
    bin: my-binary
```

### Android Build

```yaml
- name: Build for Android
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      aarch64-linux-android
      armv7-linux-androideabi
    # ndk-version: r29  # Optional: specify NDK version (default: r27d LTS)
```

### Custom Compiler

```yaml
- name: Build with custom compiler
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    cc: /usr/bin/custom-gcc
    cxx: /usr/bin/custom-g++
```

### Static/Dynamic Linking Configuration

> **Note**: The default linking behavior varies by target. Most musl targets default to static linking, but not all. Use `crt-static` to explicitly control the behavior.

```yaml
# Force static linking for musl target
- name: Build musl with static linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    crt-static: true

# Force dynamic linking for musl target
- name: Build musl with dynamic linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    crt-static: false

# GNU targets: dynamic by default, set to true for static linking
- name: Build GNU with static linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    crt-static: true

# Leave empty to use target's default behavior
- name: Build with default linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-unknown-linux-musl
      x86_64-unknown-linux-gnu
    # crt-static not specified - uses target defaults
```

### Custom Glibc Version

The cross-make toolchains support multiple glibc versions (2.28 to 2.42). Use the `glibc-version` parameter to specify a particular version for GNU targets.

```yaml
# Use glibc 2.31 (Ubuntu 20.04 compatible)
- name: Build with glibc 2.31
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    glibc-version: "2.31"

# Use latest glibc 2.42
- name: Build with glibc 2.42
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-unknown-linux-gnu
      aarch64-unknown-linux-gnu
    glibc-version: "2.42"

# Leave empty for default glibc version (2.28 for most targets)
- name: Build with default glibc
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    # glibc-version not specified - uses default
```

Supported glibc versions: 2.28 (default), 2.31, 2.32, 2.33, 2.34, 2.35, 2.36, 2.37, 2.38, 2.39, 2.40, 2.41, 2.42

### Custom iPhone SDK Version

You can specify a specific iPhone SDK version using the `iphone-sdk-version` parameter:

- **On non-macOS**: Uses bundled SDK versions for cross-compilation. Only supported versions can be used.
- **On macOS**: Uses installed Xcode SDK. If the specified version is not found, falls back to system default with a warning.

```yaml
# Use iPhone SDK 18.2
- name: Build with iPhone SDK 18.2
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: aarch64-apple-ios
    iphone-sdk-version: "18.2"

# Use iPhone SDK 17.5
- name: Build with iPhone SDK 17.5
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      aarch64-apple-ios
      aarch64-apple-ios-sim
    iphone-sdk-version: "17.5"

# Leave empty or use default (26.2)
- name: Build with default iPhone SDK
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: aarch64-apple-ios
    # iphone-sdk-version not specified - uses default 26.2
```

Supported iPhone SDK versions: 17.0, 17.2, 17.4, 17.5, 18.0, 18.1, 18.2, 18.4, 18.5, 26.0, 26.1, 26.2 (default)

### Custom macOS SDK Version

You can specify a specific macOS SDK version using the `macos-sdk-version` parameter:

- **On non-macOS**: Uses bundled SDK versions for cross-compilation via osxcross. Only supported versions can be used.
- **On macOS**: Uses installed Xcode SDK. If the specified version is not found, falls back to system default with a warning.

```yaml
# Use macOS SDK 15.2
- name: Build with macOS SDK 15.2
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: aarch64-apple-darwin
    macos-sdk-version: "15.2"

# Use macOS SDK 14.0
- name: Build with macOS SDK 14.0
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-apple-darwin
      aarch64-apple-darwin
    macos-sdk-version: "14.0"

# Leave empty or use default (26.2)
- name: Build with default macOS SDK
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: aarch64-apple-darwin
    # macos-sdk-version not specified - uses default 26.2
```

Supported macOS SDK versions (for non-macOS cross-compilation): 14.0, 14.2, 14.4, 14.5, 15.0, 15.1, 15.2, 15.4, 15.5, 26.0, 26.1, 26.2 (default)

On macOS, any SDK version installed via Xcode can be used.

### Custom FreeBSD Version

You can specify a specific FreeBSD version using the `freebsd-version` parameter. Available versions are 13, 14, and 15:

```yaml
# Use FreeBSD 15 (latest)
- name: Build with FreeBSD 15
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-freebsd
    freebsd-version: "15"

# Use FreeBSD 13 (default)
- name: Build with FreeBSD 13
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-unknown-freebsd
      aarch64-unknown-freebsd
    freebsd-version: "13"

# Leave empty for default (FreeBSD 13)
- name: Build with default FreeBSD
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-freebsd
    # freebsd-version not specified - uses default 13
```

Supported FreeBSD versions: 13 (default), 14, 15

### Custom Rustflags

```yaml
- name: Build with custom rustflags
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    rustflags: "-C opt-level=3 -C codegen-units=1"
```

### Custom Cross-Make Version

You can specify a different cross-make toolchain version using the `cross-make-version` parameter:

```yaml
# Use a specific cross-make version
- name: Build with cross-make v0.7.7
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    cross-make-version: "v0.7.7"

# Use latest version
- name: Build with latest cross-make
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-unknown-linux-musl
      aarch64-unknown-linux-musl
    cross-make-version: "v0.7.7"

# Leave empty for default (v0.7.7)
- name: Build with default cross-make
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    # cross-make-version not specified - uses default v0.7.7
```

### Build Standard Library from Source

```yaml
# Build with default std crates
- name: Build with build-std (default)
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    build-std: true
    toolchain: nightly

# Build with specific crates
- name: Build with build-std (custom crates)
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    build-std: core,alloc
    toolchain: nightly
```

### Matrix Build

```yaml
jobs:
  build:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-musl
          - aarch64-unknown-linux-musl
          - x86_64-pc-windows-gnu
          - x86_64-apple-darwin
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Build
        uses: zijiren233/cargo-cross@v1
        with:
          command: build
          targets: ${{ matrix.target }}
          
      - name: Upload
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.target }}
          path: target/${{ matrix.target }}/release/*
```

### Test Across Multiple Targets

```yaml
- name: Test cross-platform
  uses: zijiren233/cargo-cross@v1
  with:
    command: test
    targets: |
      x86_64-unknown-linux-musl
      aarch64-unknown-linux-musl
```

### Check Code Quality

```yaml
- name: Check code
  uses: zijiren233/cargo-cross@v1
  with:
    command: check
    targets: x86_64-unknown-linux-musl
    all-features: true
```

### Use Nightly Toolchain

```yaml
- name: Build with nightly
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    toolchain: nightly
```

### Reproducible Builds with CARGO_TRIM_PATHS

```yaml
- name: Build with reproducible paths
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    cargo-trim-paths: all
```

### Build without Metadata Embedding

```yaml
- name: Build without embedding metadata
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    no-embed-metadata: true
    toolchain: nightly
```

### Build with RUSTC_BOOTSTRAP

The RUSTC_BOOTSTRAP environment variable tells rustc to act as if it is a nightly compiler, allowing use of `#![feature(...)]` attributes and `-Z` flags even on the stable release channel.

```yaml
# Enable nightly features for all crates
- name: Build with nightly features on stable
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    rustc-bootstrap: "1"

# Enable nightly features for specific crate
- name: Build with nightly features for specific crate
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    rustc-bootstrap: "my_crate_name"

# Force stable behavior even on nightly
- name: Build with stable behavior on nightly
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    toolchain: nightly
    rustc-bootstrap: "-1"
```

## Toolchain Versions

This action uses the following toolchain versions from [cross-make](https://github.com/zijiren233/cross-make) v0.7.7 by default. You can specify a different version using the `cross-make-version` parameter:

### Core Components

| Component | Version |
|-----------|---------|
| GCC | 15.2.0 |
| Binutils | 2.45.1 |
| GMP | 6.3.0 |
| MPC | 1.3.1 |
| MPFR | 4.2.2 |
| ISL | 0.27 |
| Zstd | 1.5.7 |

### Platform-Specific

| Platform | C Library / SDK | Version |
|----------|-----------------|---------|
| Linux musl | musl | 1.2.5 |
| Linux GNU | glibc | 2.28 (default), 2.31-2.42 available |
| Linux | Linux Headers | 6.12.59 |
| Windows | MinGW-w64 | v13.0.0 |
| FreeBSD 13 | FreeBSD | 13.5 |
| FreeBSD 14 | FreeBSD | 14.3 |
| FreeBSD 15 | FreeBSD | 15.0 |
| macOS | macOS SDK | 26.2 (default), 14.0-26.2 available |
| iOS | iPhone SDK | 26.2 (default), 17.0-26.2 available |
| Android | NDK | r27d LTS (default), r29 stable available |

### Supported Glibc Versions

For GNU libc targets, use `glibc-version` parameter to select:

| Version | Compatible With |
|---------|-----------------|
| 2.28 | Debian 10, Ubuntu 18.04, RHEL 8 |
| 2.31 | Ubuntu 20.04, Debian 11 |
| 2.34 | RHEL 9, Ubuntu 22.04 |
| 2.35 | Ubuntu 22.04 |
| 2.36 | Debian 12 |
| 2.38 | Ubuntu 24.04 |
| 2.39-2.42 | Latest distributions |

### Supported iPhone SDK Versions

For iOS targets, use `iphone-sdk-version` parameter to select the SDK version. On non-macOS hosts, only the following bundled versions are available:

| Version | Notes |
|---------|-------|
| 17.0 | iOS 17.0 SDK |
| 17.2 | iOS 17.2 SDK |
| 17.4 | iOS 17.4 SDK |
| 17.5 | iOS 17.5 SDK |
| 18.0 | iOS 18.0 SDK |
| 18.1 | iOS 18.1 SDK |
| 18.2 | iOS 18.2 SDK |
| 18.4 | iOS 18.4 SDK |
| 18.5 | iOS 18.5 SDK |
| 26.0 | iOS 26.0 SDK |
| 26.1 | iOS 26.1 SDK |
| 26.2 | iOS 26.2 SDK (default) |

On macOS, any SDK version installed via Xcode can be used. If the specified version is not found, the system default SDK will be used with a warning.

### Supported macOS SDK Versions

For macOS (Darwin) targets, use `macos-sdk-version` parameter to select the SDK version. On non-macOS hosts, only the following bundled versions are available:

| Version | Notes |
|---------|-------|
| 14.0 | macOS 14.0 (Sonoma) SDK |
| 14.2 | macOS 14.2 SDK |
| 14.4 | macOS 14.4 SDK |
| 14.5 | macOS 14.5 SDK |
| 15.0 | macOS 15.0 (Sequoia) SDK |
| 15.1 | macOS 15.1 SDK |
| 15.2 | macOS 15.2 SDK |
| 15.4 | macOS 15.4 SDK |
| 15.5 | macOS 15.5 SDK |
| 26.0 | macOS 26.0 SDK |
| 26.1 | macOS 26.1 SDK |
| 26.2 | macOS 26.2 SDK (default) |

On macOS, any SDK version installed via Xcode can be used.

### Supported FreeBSD Versions

For FreeBSD targets, use `freebsd-version` parameter to select the FreeBSD version:

| Version | Notes |
|---------|-------|
| 13 | FreeBSD 13.5 (default) |
| 14 | FreeBSD 14.3 |
| 15 | FreeBSD 15.0 |

## How It Works

1. **Command Detection**: The action detects the requested command (build, test, or check)
2. **Target Detection**: The action detects the requested target platforms
3. **Toolchain Setup**: Automatically downloads and configures the necessary cross-compilation toolchains
4. **Environment Configuration**: Sets up the correct environment variables for cross-compilation
5. **Command Execution**: Runs the specified cargo command with the appropriate flags and configuration
6. **Artifact Output**: Built binaries and libraries are placed in `target/{target}/{profile}/` directories following Cargo's standard structure

## Troubleshooting

### Build fails with "linker not found"

Make sure you're running on a supported host OS. Linux hosts support the most targets. For macOS and Windows targets, you may need to run on the respective OS runners.

### Binary is too large

Use `profile: release` and ensure stripping is enabled (default). Note that:

- **musl targets** usually produce statically linked binaries by default (varies by target), which are larger but completely self-contained
- **GNU targets** produce dynamically linked binaries by default, which are smaller but require system libraries
- You can explicitly configure the linking behavior using the `crt-static` parameter

### Android build fails

Ensure you have enough disk space for the Android NDK download. You can also try a different NDK version with the `ndk-version` input.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Credits

This action uses cross-compilation toolchains from:

- [cross-make](https://github.com/zijiren233/cross-make) for Linux/Windows/Freebsd targets
- [osxcross](https://github.com/zijiren233/osxcross) for macOS targets
- [cctools-port](https://github.com/zijiren233/cctools-port) for ios targets
- Android NDK for Android targets
