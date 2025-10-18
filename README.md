# Rust Cross Build Action

A powerful GitHub Action for building, testing, and checking Rust projects with cross-compilation support. This action automatically downloads and configures the necessary cross-compilation toolchains, making it easy to execute various Rust commands for multiple platforms.

## Features

- 🚀 **Cross-compilation support** for Linux (GNU/musl), Windows, macOS, Android, and iOS
- 🖥️ **Multi-platform hosts** - runs on Linux (x86_64/aarch64/armv7) and macOS (x86_64/aarch64)
- 📦 **Automatic toolchain setup** - downloads and configures cross-compilers as needed
- 🎯 **Multiple target support** - build for 63+ target platforms in a single run
- 🏗️ **Workspace support** - work with entire workspaces or specific packages
- ⚡ **Static linking** - produces statically linked binaries for easy distribution
- 🔧 **Flexible configuration** - extensive customization options
- 📁 **Organized output** - all artifacts collected in a single directory
- 🛠️ **Multiple commands** - supports build, test, and check operations

## Quick Start

### Basic Usage

```yaml
name: Build

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Cross compile (comma-separated)
        uses: your-username/rust-cross-build@v1
        with:
          command: build
          targets: x86_64-unknown-linux-musl,aarch64-unknown-linux-musl

      - name: Cross compile (newline-separated)
        uses: your-username/rust-cross-build@v1
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
        uses: your-username/rust-cross-build@v1
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
          path: target/cross/*
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
      - uses: your-username/rust-cross-build@v1
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
      - uses: your-username/rust-cross-build@v1
        with:
          targets: |
            x86_64-unknown-linux-musl
            aarch64-unknown-linux-musl
            x86_64-pc-windows-gnu
            aarch64-apple-darwin
            aarch64-apple-ios
```

## Supported Targets

### Linux (musl - fully static)

- `i586-unknown-linux-musl` - Linux i586 (static)
- `i686-unknown-linux-musl` - Linux i686 (static)
- `x86_64-unknown-linux-musl` - Linux x86_64 (static)
- `arm-unknown-linux-musleabi` - ARM Linux (static)
- `arm-unknown-linux-musleabihf` - ARM Linux hard-float (static)
- `armv5te-unknown-linux-musleabi` - ARMv5TE Linux (static)
- `armv7-unknown-linux-musleabi` - ARMv7 Linux (static)
- `armv7-unknown-linux-musleabihf` - ARMv7 Linux hard-float (static)
- `aarch64-unknown-linux-musl` - ARM64 Linux (static)
- `loongarch64-unknown-linux-musl` - LoongArch64 Linux (static)
- `mips-unknown-linux-musl` - MIPS Linux (static)
- `mipsel-unknown-linux-musl` - MIPS little-endian Linux (static)
- `mips64-unknown-linux-muslabi64` - MIPS64 Linux (static)
- `mips64-openwrt-linux-musl` - MIPS64 OpenWrt Linux (static)
- `mips64el-unknown-linux-muslabi64` - MIPS64 little-endian Linux (static)
- `powerpc64-unknown-linux-musl` - PowerPC64 Linux (static)
- `powerpc64le-unknown-linux-musl` - PowerPC64 little-endian Linux (static)
- `riscv64gc-unknown-linux-musl` - RISC-V 64-bit Linux (static)
- `s390x-unknown-linux-musl` - S390x Linux (static)

### Linux (GNU libc)

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

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `command` | Command to execute (`build`, `test`, `check`) | `build` |
| `targets` | Comma-separated or newline-separated list of Rust target triples | Host target |
| `profile` | Build profile (`debug` or `release`) | `release` |
| `features` | Comma-separated list of features to activate | |
| `no-default-features` | Do not activate default features | `false` |
| `all-features` | Activate all available features | `false` |
| `package` | Package to build (workspace member) | |
| `bin` | Binary target to build | |
| `workspace` | Build all workspace members | `false` |
| `manifest-path` | Path to Cargo.toml | |
| `source-dir` | Directory containing the Rust project | `${{ github.workspace }}` |
| `result-dir` | Directory to store build results | `target/cross` |
| `bin-name-no-suffix` | Don't append target suffix to binary name | `false` |
| `github-proxy-mirror` | GitHub proxy mirror URL | |
| `cross-compiler-dir` | Directory to store cross compilers | |
| `ndk-version` | Android NDK version | `r27` |
| `use-default-linker` | Use system default linker | `false` |
| `cc` | Force set the C compiler | |
| `cxx` | Force set the C++ compiler | |
| `rustflags` | Additional rustflags | |
| `static-crt` | Control CRT linking mode: `true` for static (+crt-static), `false` for dynamic (-crt-static), empty for default behavior | `` |
| `build-std` | Use -Zbuild-std for building standard library from source (`true` for default, or specify crates like `core,alloc`) | `false` |
| `args` | Additional arguments to pass to cargo command | |
| `toolchain` | Rust toolchain to use (stable, nightly, etc.) | `stable` |
| `cargo-trim-paths` | Set CARGO_TRIM_PATHS environment variable for reproducible builds | |
| `no-embed-metadata` | Add -Zno-embed-metadata flag to cargo | `false` |
| `clean-cache` | Clean build cache before building | `false` |
| `no-strip` | Do not strip binaries | `false` |
| `verbose` | Use verbose output | `false` |

## Outputs

| Output | Description |
|--------|-------------|
| `result-dir` | Directory containing built artifacts |
| `targets` | Targets that were processed |

## Advanced Examples

### Build with Features

```yaml
- name: Build with features
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    features: feature1,feature2
    no-default-features: true
```

### Build Specific Binary from Workspace

```yaml
- name: Build specific binary
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    package: my-package
    bin: my-binary
```

### Android Build

```yaml
- name: Build for Android
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: aarch64-linux-android,armv7-linux-androideabi
    ndk-version: r27
```

### Custom Compiler

```yaml
- name: Build with custom compiler
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-gnu
    cc: /usr/bin/custom-gcc
    cxx: /usr/bin/custom-g++
```

### Static CRT Linking

```yaml
# Enable static CRT linking
- name: Build with static CRT
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    static-crt: true

# Disable static CRT linking (force dynamic)
- name: Build with dynamic CRT
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-pc-windows-gnu
    static-crt: false
```

### Custom Rustflags

```yaml
- name: Build with custom rustflags
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    rustflags: "-C opt-level=3 -C codegen-units=1"
```

### Build Standard Library from Source

```yaml
# Build with default std crates
- name: Build with build-std (default)
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    build-std: true
    toolchain: nightly

# Build with specific crates
- name: Build with build-std (custom crates)
  uses: your-username/rust-cross-build@v1
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
        uses: your-username/rust-cross-build@v1
        with:
          command: build
          targets: ${{ matrix.target }}
          
      - name: Upload
        uses: actions/upload-artifact@v3
        with:
          name: ${{ matrix.target }}
          path: target/cross/*
```

### Test Across Multiple Targets

```yaml
- name: Test cross-platform
  uses: your-username/rust-cross-build@v1
  with:
    command: test
    targets: x86_64-unknown-linux-musl,aarch64-unknown-linux-musl
```

### Check Code Quality

```yaml
- name: Check code
  uses: your-username/rust-cross-build@v1
  with:
    command: check
    targets: x86_64-unknown-linux-musl
    all-features: true
```

### Use Nightly Toolchain

```yaml
- name: Build with nightly
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    toolchain: nightly
```

### Reproducible Builds with CARGO_TRIM_PATHS

```yaml
- name: Build with reproducible paths
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    cargo-trim-paths: all
```

### Build without Metadata Embedding

```yaml
- name: Build without embedding metadata
  uses: your-username/rust-cross-build@v1
  with:
    command: build
    targets: x86_64-unknown-linux-musl
    no-embed-metadata: true
    toolchain: nightly
```

## Local Usage

You can also use the execution script locally:

```bash
# Build for a specific target (default command)
./exec.sh --targets=x86_64-unknown-linux-musl

# Explicitly specify build command
./exec.sh build --targets=x86_64-unknown-linux-musl

# Test for multiple targets
./exec.sh test --targets=x86_64-unknown-linux-musl,aarch64-unknown-linux-musl

# Check the project
./exec.sh check --targets=x86_64-unknown-linux-musl

# Show all supported targets
./exec.sh --show-all-targets

# Build with features
./exec.sh build --targets=x86_64-unknown-linux-musl --features=feature1,feature2

# Build entire workspace
./exec.sh build --targets=x86_64-unknown-linux-musl --workspace

# Build with nightly toolchain
./exec.sh build --targets=x86_64-unknown-linux-musl --toolchain=nightly

# Test with stable toolchain (explicitly)
./exec.sh test --targets=x86_64-unknown-linux-musl --toolchain=stable

# Build with static CRT linking
./exec.sh build --targets=x86_64-unknown-linux-musl --static-crt=true

# Build with dynamic CRT linking
./exec.sh build --targets=x86_64-pc-windows-gnu --static-crt=false

# Build with custom rustflags
./exec.sh build --targets=x86_64-unknown-linux-musl --rustflags="-C opt-level=3 -C codegen-units=1"

# Build with build-std (build standard library from source)
./exec.sh build --targets=x86_64-unknown-linux-musl --build-std

# Build with build-std using specific crates
./exec.sh build --targets=x86_64-unknown-linux-musl --build-std=core,alloc

# Build with reproducible paths
./exec.sh build --targets=x86_64-unknown-linux-musl --cargo-trim-paths=all

# Build without embedding metadata (requires nightly toolchain)
./exec.sh build --targets=x86_64-unknown-linux-musl --no-embed-metadata --toolchain=nightly
```

## How It Works

1. **Command Detection**: The action detects the requested command (build, test, or check)
2. **Target Detection**: The action detects the requested target platforms
3. **Toolchain Setup**: Automatically downloads and configures the necessary cross-compilation toolchains
4. **Environment Configuration**: Sets up the correct environment variables for cross-compilation
5. **Command Execution**: Runs the specified cargo command with the appropriate flags and configuration
6. **Artifact Collection**: For build commands, collects all built binaries and libraries in the result directory

## Troubleshooting

### Build fails with "linker not found"

Make sure you're running on a supported host OS. Linux hosts support the most targets. For macOS and Windows targets, you may need to run on the respective OS runners.

### Binary is too large

Use `profile: release` and ensure stripping is enabled (default). The musl targets produce fully static binaries which are larger but completely self-contained.

### Android build fails

Ensure you have enough disk space for the Android NDK download. You can also try a different NDK version with the `ndk-version` input.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Credits

This action uses cross-compilation toolchains from:

- [musl-cross-make](https://github.com/richfelker/musl-cross-make) for Linux targets
- [osxcross](https://github.com/tpoechtrager/osxcross) for macOS targets
- Android NDK for Android targets
- MinGW for Windows targets
