# Rust Cross Build Action

A powerful GitHub Action for building, testing, and checking Rust projects with cross-compilation support. This action automatically downloads and configures the necessary cross-compilation toolchains, making it easy to execute various Rust commands for multiple platforms. No Need Docker!

## Features

- 🚀 **Cross-compilation support** for Linux (GNU/musl), Windows, macOS, FreeBSD, Android, and iOS
- 🖥️ **Multi-platform hosts** - runs on Linux (x86_64/aarch64/armv7) and macOS (x86_64/aarch64)
- 📦 **Automatic toolchain setup** - downloads and configures cross-compilers as needed
- 🎯 **Multiple target support** - build for 63+ target platforms in a single run
- 🏗️ **Workspace support** - work with entire workspaces or specific packages
- ⚡ **Flexible linking** - musl targets default to static, GNU targets default to dynamic, both configurable via `crt-static` parameter
- 🔧 **Flexible configuration** - extensive customization options
- 📁 **Organized output** - all artifacts collected in a single directory
- 🛠️ **Multiple commands** - supports build, test, and check operations

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
| **Linux** | armv7 | Self-hosted ARMv7 runners | ✅ Yes |
| **macOS** | x86_64 (Intel) | `macos-15-intel` | ✅ Yes |
| **macOS** | aarch64 (Apple Silicon) | `macos-15` | ✅ Yes |

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

### Linux (musl - static by default)

musl targets produce **statically linked binaries by default**. Use `static-crt: false` to enable dynamic linking.

- `i586-unknown-linux-musl` - Linux i586
- `i686-unknown-linux-musl` - Linux i686
- `x86_64-unknown-linux-musl` - Linux x86_64
- `arm-unknown-linux-musleabi` - ARM Linux
- `arm-unknown-linux-musleabihf` - ARM Linux hard-float
- `armv5te-unknown-linux-musleabi` - ARMv5TE Linux
- `armv7-unknown-linux-musleabi` - ARMv7 Linux
- `armv7-unknown-linux-musleabihf` - ARMv7 Linux hard-float
- `aarch64-unknown-linux-musl` - ARM64 Linux
- `loongarch64-unknown-linux-musl` - LoongArch64 Linux
- `mips-unknown-linux-musl` - MIPS Linux
- `mipsel-unknown-linux-musl` - MIPS little-endian Linux
- `mips64-unknown-linux-muslabi64` - MIPS64 Linux
- `mips64-openwrt-linux-musl` - MIPS64 OpenWrt Linux
- `mips64el-unknown-linux-muslabi64` - MIPS64 little-endian Linux
- `powerpc64-unknown-linux-musl` - PowerPC64 Linux
- `powerpc64le-unknown-linux-musl` - PowerPC64 little-endian Linux
- `riscv64gc-unknown-linux-musl` - RISC-V 64-bit Linux
- `s390x-unknown-linux-musl` - S390x Linux

### Linux (GNU libc - dynamic by default)

GNU libc targets produce **dynamically linked binaries by default**. Use `static-crt: true` to enable static linking.

- `i586-unknown-linux-gnu` - Linux i586
- `i686-unknown-linux-gnu` - Linux i686
- `x86_64-unknown-linux-gnu` - Linux x86_64
- `arm-unknown-linux-gnueabi` - ARM Linux
- `arm-unknown-linux-gnueabihf` - ARM Linux hard-float
- `armv5te-unknown-linux-gnueabi` - ARMv5TE Linux
- `armv7-unknown-linux-gnueabi` - ARMv7 Linux
- `armv7-unknown-linux-gnueabihf` - ARMv7 Linux hard-float
- `aarch64-unknown-linux-gnu` - ARM64 Linux
- `loongarch64-unknown-linux-gnu` - LoongArch64 Linux
- `mips-unknown-linux-gnu` - MIPS Linux
- `mipsel-unknown-linux-gnu` - MIPS little-endian Linux
- `mips64-unknown-linux-gnuabi64` - MIPS64 Linux
- `mips64el-unknown-linux-gnuabi64` - MIPS64 little-endian Linux
- `powerpc64-unknown-linux-gnu` - PowerPC64 Linux
- `powerpc64le-unknown-linux-gnu` - PowerPC64 little-endian Linux
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
| `bin-name-no-suffix` | Don't append target suffix to binary name | `false` |
| `github-proxy-mirror` | GitHub proxy mirror URL | |
| `cross-compiler-dir` | Directory to store cross compilers | |
| `ndk-version` | Android NDK version | `r27` |
| `use-default-linker` | Use system default linker | `false` |
| `cc` | Force set the C compiler | |
| `cxx` | Force set the C++ compiler | |
| `rustflags` | Additional rustflags | |
| `static-crt` | Control CRT linking mode: `true` for static (+crt-static), `false` for dynamic (-crt-static), empty for default (musl=static, gnu=dynamic) | `` |
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
    ndk-version: r27
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

```yaml
# musl targets: static by default, set to false for dynamic linking
- name: Build musl with dynamic linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    static-crt: false

# GNU targets: dynamic by default, set to true for static linking
- name: Build GNU with static linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    static-crt: true

# Leave empty to use default behavior (musl=static, gnu=dynamic)
- name: Build with default linking
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: |
      x86_64-unknown-linux-musl
      x86_64-unknown-linux-gnu
    # static-crt not specified - uses defaults
```

### Custom Rustflags

```yaml
- name: Build with custom rustflags
  uses: zijiren233/cargo-cross@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    rustflags: "-C opt-level=3 -C codegen-units=1"
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

- **musl targets** produce statically linked binaries by default, which are larger but completely self-contained
- **GNU targets** produce dynamically linked binaries by default, which are smaller but require system libraries
- You can configure the linking behavior using the `static-crt` parameter

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
