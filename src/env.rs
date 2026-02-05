//! Environment variable management for cargo-cross

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::HostPlatform;

/// Cross-compilation environment
#[derive(Debug, Clone, Default)]
pub struct CrossEnv {
    /// Target C compiler
    pub cc: Option<String>,
    /// Target C++ compiler
    pub cxx: Option<String>,
    /// Target archiver
    pub ar: Option<String>,
    /// Target linker (for Cargo)
    pub linker: Option<String>,
    /// Target runner (for test/run commands)
    pub runner: Option<String>,
    /// Additional paths to prepend to PATH
    pub path: Vec<PathBuf>,
    /// RUSTFLAGS additions
    pub rustflags: Vec<String>,
    /// SDKROOT for Apple platforms
    pub sdkroot: Option<PathBuf>,
    /// `LD_LIBRARY_PATH` / `DYLD_LIBRARY_PATH` additions
    pub library_path: Vec<PathBuf>,
    /// CFLAGS additions
    pub cflags: Vec<String>,
    /// CXXFLAGS additions
    pub cxxflags: Vec<String>,
    /// LDFLAGS additions
    pub ldflags: Vec<String>,
    /// Use build-std (crates to build)
    pub build_std: Option<String>,
    /// Additional target-specific environment variables
    pub extra_env: HashMap<String, String>,
}

impl CrossEnv {
    #[must_use] 
    pub fn new() -> Self {
        Self::default()
    }

    /// Set C compiler
    pub fn set_cc(&mut self, cc: impl Into<String>) {
        self.cc = Some(cc.into());
    }

    /// Set C++ compiler
    pub fn set_cxx(&mut self, cxx: impl Into<String>) {
        self.cxx = Some(cxx.into());
    }

    /// Set archiver
    pub fn set_ar(&mut self, ar: impl Into<String>) {
        self.ar = Some(ar.into());
    }

    /// Set linker
    pub fn set_linker(&mut self, linker: impl Into<String>) {
        self.linker = Some(linker.into());
    }

    /// Set runner
    pub fn set_runner(&mut self, runner: impl Into<String>) {
        self.runner = Some(runner.into());
    }

    /// Add path to PATH
    pub fn add_path(&mut self, path: impl Into<PathBuf>) {
        self.path.push(path.into());
    }

    /// Add rustflag
    pub fn add_rustflag(&mut self, flag: impl Into<String>) {
        self.rustflags.push(flag.into());
    }

    /// Set SDKROOT
    pub fn set_sdkroot(&mut self, path: impl Into<PathBuf>) {
        self.sdkroot = Some(path.into());
    }

    /// Add library path
    pub fn add_library_path(&mut self, path: impl Into<PathBuf>) {
        self.library_path.push(path.into());
    }

    /// Add CFLAG
    pub fn add_cflag(&mut self, flag: impl Into<String>) {
        self.cflags.push(flag.into());
    }

    /// Add CXXFLAG
    pub fn add_cxxflag(&mut self, flag: impl Into<String>) {
        self.cxxflags.push(flag.into());
    }

    /// Add LDFLAG
    pub fn add_ldflag(&mut self, flag: impl Into<String>) {
        self.ldflags.push(flag.into());
    }

    /// Set build-std crates
    pub fn set_build_std(&mut self, crates: impl Into<String>) {
        self.build_std = Some(crates.into());
    }

    /// Set extra environment variable
    pub fn set_env(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extra_env.insert(key.into(), value.into());
    }

    /// Build environment variables for a target
    #[must_use] 
    pub fn build_env(&self, target: &str, host: &HostPlatform) -> HashMap<String, String> {
        let mut env = HashMap::new();

        // Target name variants for environment variables
        // CC crate uses lowercase (CC_<target>), Cargo uses uppercase (CARGO_TARGET_<TARGET>_*)
        let target_lower = target.replace('-', "_");
        let target_upper = target.to_uppercase().replace('-', "_");

        // Set CC/CXX/AR
        if let Some(ref cc) = self.cc {
            env.insert(format!("CC_{target_lower}"), cc.clone());
            env.insert("CC".to_string(), cc.clone());
        }
        if let Some(ref cxx) = self.cxx {
            env.insert(format!("CXX_{target_lower}"), cxx.clone());
            env.insert("CXX".to_string(), cxx.clone());
        }
        if let Some(ref ar) = self.ar {
            env.insert(format!("AR_{target_lower}"), ar.clone());
            env.insert("AR".to_string(), ar.clone());
        }

        // Set linker (Cargo uses uppercase)
        if let Some(ref linker) = self.linker {
            env.insert(
                format!("CARGO_TARGET_{target_upper}_LINKER"),
                linker.clone(),
            );
        }

        // Set runner (Cargo uses uppercase)
        if let Some(ref runner) = self.runner {
            env.insert(
                format!("CARGO_TARGET_{target_upper}_RUNNER"),
                runner.clone(),
            );
        }

        // Build PATH
        if !self.path.is_empty() {
            let sep = host.path_separator();
            let current_path = std::env::var("PATH").unwrap_or_default();
            let new_path = self
                .path
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(sep);
            env.insert("PATH".to_string(), format!("{new_path}{sep}{current_path}"));
        }

        // Set SDKROOT
        if let Some(ref sdkroot) = self.sdkroot {
            env.insert("SDKROOT".to_string(), sdkroot.display().to_string());
        }

        // Build library path
        if !self.library_path.is_empty() {
            let sep = host.path_separator();
            let lib_path = self
                .library_path
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(sep);

            let lib_var = if host.is_darwin() {
                "DYLD_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };

            let current = std::env::var(lib_var).unwrap_or_default();
            if current.is_empty() {
                env.insert(lib_var.to_string(), lib_path);
            } else {
                env.insert(lib_var.to_string(), format!("{lib_path}{sep}{current}"));
            }
        }

        // Set CFLAGS/CXXFLAGS/LDFLAGS
        if !self.cflags.is_empty() {
            let flags = self.cflags.join(" ");
            env.insert(format!("CFLAGS_{target_lower}"), flags.clone());
            env.insert("CFLAGS".to_string(), flags);
        }
        if !self.cxxflags.is_empty() {
            let flags = self.cxxflags.join(" ");
            env.insert(format!("CXXFLAGS_{target_lower}"), flags.clone());
            env.insert("CXXFLAGS".to_string(), flags);
        }
        if !self.ldflags.is_empty() {
            let flags = self.ldflags.join(" ");
            env.insert(format!("LDFLAGS_{target_lower}"), flags.clone());
            env.insert("LDFLAGS".to_string(), flags);
        }

        // Add extra environment variables
        for (key, value) in &self.extra_env {
            env.insert(key.clone(), value.clone());
        }

        env
    }

    /// Get RUSTFLAGS string
    #[must_use] 
    pub fn rustflags_string(&self) -> Option<String> {
        if self.rustflags.is_empty() {
            None
        } else {
            Some(self.rustflags.join(" "))
        }
    }
}

/// Set GCC library search paths for rustc
pub fn set_gcc_lib_paths(env: &mut CrossEnv, compiler_dir: &Path, target_prefix: &str) {
    // Add target library directory
    let target_lib = compiler_dir.join(target_prefix).join("lib");
    if target_lib.exists() {
        env.add_rustflag(format!("-L {}", target_lib.display()));
    }

    // Add GCC library directory (find the version directory)
    let gcc_lib_base = compiler_dir.join("lib").join("gcc").join(target_prefix);
    if let Ok(entries) = std::fs::read_dir(&gcc_lib_base) {
        for entry in entries.filter_map(std::result::Result::ok) {
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                env.add_rustflag(format!("-L {}", entry.path().display()));
                break;
            }
        }
    }
}

/// Setup `BINDGEN_EXTRA_CLANG_ARGS` and related environment variables for cross-compilation sysroot
pub fn setup_sysroot_env(
    env: &mut CrossEnv,
    compiler_dir: &Path,
    bin_prefix: &str,
    rust_target: &str,
) {
    let sysroot = compiler_dir.join(bin_prefix);
    if !sysroot.exists() {
        return;
    }

    let target_underscores = rust_target.replace('-', "_");

    // Build clang args: --sysroot plus any additional GCC internal include dirs
    let mut clang_args = vec![format!("--sysroot={}", sysroot.display())];

    // Find GCC internal include directory (contains mm_malloc.h, stddef.h, etc.)
    let gcc_include_base = compiler_dir.join("lib").join("gcc").join(bin_prefix);
    if let Ok(entries) = std::fs::read_dir(&gcc_include_base) {
        for entry in entries.filter_map(std::result::Result::ok) {
            let include_dir = entry.path().join("include");
            if include_dir.exists() {
                clang_args.push(format!("-I{}", include_dir.display()));
                break;
            }
        }
    }

    // Add sysroot include paths
    let usr_include = sysroot.join("usr").join("include");
    let include = sysroot.join("include");

    if usr_include.exists() {
        clang_args.push(format!("-I{}", usr_include.display()));
    } else if include.exists() {
        clang_args.push(format!("-I{}", include.display()));
    }

    env.set_env(
        format!("BINDGEN_EXTRA_CLANG_ARGS_{target_underscores}"),
        clang_args.join(" "),
    );
}

/// Get standard build-std crates configuration
///
/// Crates explicitly listed for user visibility and completeness:
/// - std: standard library (depends on core, alloc, panic_*, `compiler_builtins`, etc.)
/// - core: `no_std` core library
/// - alloc: memory allocation (`no_std` + alloc)
/// - `proc_macro`: procedural macros
/// - test: test framework
/// - `panic_abort`: panic=abort strategy
/// - `panic_unwind`: panic=unwind strategy (cargo adds this automatically but we keep it explicit)
///
/// Note: When "std" is specified, cargo's `std_crates()` automatically adds:
/// core, alloc, `proc_macro`, `panic_unwind`, `compiler_builtins`
///
/// References:
/// - Rust standard library: <https://github.com/rust-lang/rust/tree/main/library>
/// - Cargo build-std: <https://github.com/rust-lang/cargo/blob/master/src/cargo/core/compiler/standard_lib.rs>
#[must_use] 
pub const fn get_build_std_config() -> &'static str {
    "std,core,alloc,proc_macro,test,panic_abort,panic_unwind"
}

/// Sanitize environment variables that could cause cargo errors
/// Call this once at program startup
pub fn sanitize_cargo_env() {
    // Remove empty CARGO_TARGET_DIR to prevent cargo error:
    // "the target directory is set to an empty string in the `CARGO_TARGET_DIR` environment variable"
    if std::env::var("CARGO_TARGET_DIR")
        .is_ok_and(|v| v.is_empty())
    {
        std::env::remove_var("CARGO_TARGET_DIR");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_env_build() {
        let mut env = CrossEnv::new();
        env.set_cc("aarch64-linux-gnu-gcc");
        env.set_cxx("aarch64-linux-gnu-g++");
        env.set_linker("aarch64-linux-gnu-gcc");

        let host = HostPlatform::detect();
        let vars = env.build_env("aarch64-unknown-linux-gnu", &host);

        assert_eq!(
            vars.get("CC_aarch64_unknown_linux_gnu"),
            Some(&"aarch64-linux-gnu-gcc".to_string())
        );
        assert!(vars.contains_key("CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER"));
    }
}
