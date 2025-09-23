# Rust Cross Build Action

A powerful GitHub Action for building, testing, and checking Rust projects with cross-compilation support. This action automatically downloads and configures the necessary cross-compilation toolchains, making it easy to execute various Rust commands for multiple platforms.

## Features

- 🚀 **Cross-compilation support** for Linux (GNU/musl), Windows, macOS, Android, and iOS
- 📦 **Automatic toolchain setup** - downloads and configures cross-compilers as needed
- 🎯 **Multiple target support** - execute commands for multiple targets in a single run
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
      
      - name: Cross compile
        uses: your-username/rust-cross-build@v1
        with:
          command: build
          targets: x86_64-unknown-linux-musl,aarch64-unknown-linux-musl
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
            x86_64-unknown-linux-musl,
            aarch64-unknown-linux-musl,
            x86_64-pc-windows-gnu,
            x86_64-apple-darwin,
            aarch64-apple-darwin
          profile: release
          
      - name: Upload artifacts
        uses: actions/upload-artifact@v3
        with:
          name: binaries-${{ matrix.os }}
          path: target/cross/*
```

## Supported Targets

### Linux (musl - fully static)

- `i686-unknown-linux-musl` - Linux 32-bit (static)
- `x86_64-unknown-linux-musl` - Linux 64-bit (static)
- `arm-unknown-linux-musleabi` - ARM Linux (static)
- `arm-unknown-linux-musleabihf` - ARM Linux hard-float (static)
- `armv7-unknown-linux-musleabi` - ARMv7 Linux (static)
- `armv7-unknown-linux-musleabihf` - ARMv7 Linux hard-float (static)
- `aarch64-unknown-linux-musl` - ARM64 Linux (static)
- `mips-unknown-linux-musl` - MIPS Linux (static)
- `mipsel-unknown-linux-musl` - MIPS little-endian Linux (static)
- `mips64-unknown-linux-muslabi64` - MIPS64 Linux (static)
- `mips64el-unknown-linux-muslabi64` - MIPS64 little-endian Linux (static)
- `powerpc64le-unknown-linux-musl` - PowerPC64 little-endian Linux (static)
- `riscv64gc-unknown-linux-musl` - RISC-V 64-bit Linux (static)
- `s390x-unknown-linux-musl` - S390x Linux (static)

### Windows

- `i686-pc-windows-gnu` - Windows 32-bit
- `x86_64-pc-windows-gnu` - Windows 64-bit

### macOS

- `x86_64-apple-darwin` - macOS Intel
- `aarch64-apple-darwin` - macOS Apple Silicon

### Android

- `i686-linux-android` - Android x86
- `x86_64-linux-android` - Android x86_64
- `armv7-linux-androideabi` - Android ARMv7
- `aarch64-linux-android` - Android ARM64

### iOS

- `x86_64-apple-ios` - iOS Simulator (Intel)
- `aarch64-apple-ios` - iOS ARM64

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `command` | Command to execute (`build`, `test`, `check`) | `build` |
| `targets` | Comma-separated list of Rust target triples | Host target |
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
| `add-rustflags` | Additional rustflags | |
| `args` | Additional arguments to pass to cargo command | |
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
