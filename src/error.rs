//! Error types for cargo-cross

use std::path::PathBuf;
use thiserror::Error;
use tokio::process::Command;

/// Main error type for cargo-cross
#[derive(Error, Debug)]
pub enum CrossError {
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Missing required value for option: {0}")]
    MissingValue(String),

    #[error("Unknown option: {0}")]
    UnknownOption(String),

    #[error("Target not found: {target}\nUse '--list-targets' to see available targets")]
    TargetNotFound { target: String },

    #[error("Unsupported target: {0}")]
    UnsupportedTarget(String),

    #[error("Unsupported glibc version '{version}'\nSupported versions: {supported}")]
    UnsupportedGlibcVersion { version: String, supported: String },

    #[error("Unsupported iPhone SDK version '{version}'\nSupported versions: {supported}")]
    UnsupportedIphoneSdkVersion { version: String, supported: String },

    #[error("Unsupported macOS SDK version '{version}'\nSupported versions: {supported}")]
    UnsupportedMacosSdkVersion { version: String, supported: String },

    #[error("Unsupported FreeBSD version '{version}'\nSupported versions: {supported}")]
    UnsupportedFreebsdVersion { version: String, supported: String },

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("IO error: {message}\nDetails: {source}")]
    IoError {
        message: String,
        source: std::io::Error,
    },

    #[error("Program not found: '{program}'\nPlease ensure it is installed and available in PATH")]
    ProgramNotFound { program: String },

    #[error("Failed to execute command: {command}\nError: {reason}")]
    CommandExecutionFailed { command: String, reason: String },

    #[error("Failed to extract archive: {0}")]
    ExtractionFailed(String),

    #[error("Unsupported archive format: {0}")]
    UnsupportedArchiveFormat(String),

    #[error("Cross-compiler not found at: {path}\nPlease check the toolchain installation")]
    CompilerNotFound { path: PathBuf },

    #[error("SDK not found at: {path}")]
    SdkNotFound { path: PathBuf },

    #[error("SDK path does not exist: {path}")]
    SdkPathNotExist { path: PathBuf },

    #[error("Command failed: {command}")]
    CommandFailed { command: String },

    #[error("Failed to install Rust target: {target}\nRun 'rustup target add {target}' manually to see details")]
    TargetInstallFailed { target: String },

    #[error("Target '{target}' requires build-std but is not in rustc target list\nUse BUILD_STD=core,alloc or similar to enable build-std")]
    BuildStdRequired { target: String },

    #[error("Cross-compilation to {target_os} is not supported from {host_os}")]
    CrossCompilationNotSupported { target_os: String, host_os: String },

    #[error("Unsupported architecture '{arch}' for {os}")]
    UnsupportedArchitecture { arch: String, os: String },

    #[error("Environment variable error: {0}")]
    EnvError(String),

    #[error("No matching targets found for pattern '{pattern}'\nUse '{prog} targets' to see available targets", prog = crate::cli::program_name())]
    NoMatchingTargets { pattern: String },

    #[error("Invalid target triple '{target}': contains invalid character '{char}'\nTarget triples may only contain lowercase letters (a-z), digits (0-9), hyphens (-), and underscores (_)")]
    InvalidTargetTriple { target: String, char: char },

    #[error("Cargo exited with code {code}")]
    CargoFailed { code: i32 },

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Regex error: {0}")]
    RegexError(#[from] regex_lite::Error),

    #[error("{0}")]
    Other(String),

    #[error("CLI argument error: {0}")]
    ClapError(String),
}

impl From<std::io::Error> for CrossError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::ProgramNotFound {
                program: "unknown".to_string(),
            },
            std::io::ErrorKind::PermissionDenied => Self::IoError {
                message: "Permission denied".to_string(),
                source: err,
            },
            _ => Self::IoError {
                message: err.kind().to_string(),
                source: err,
            },
        }
    }
}

/// Result type alias for cargo-cross
pub type Result<T> = std::result::Result<T, CrossError>;

/// Execute a command and return its status, with improved error messages
pub async fn run_command(cmd: &mut Command, program: &str) -> Result<std::process::ExitStatus> {
    cmd.status().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CrossError::ProgramNotFound {
            program: program.to_string(),
        },
        std::io::ErrorKind::PermissionDenied => CrossError::CommandExecutionFailed {
            command: program.to_string(),
            reason: "Permission denied".to_string(),
        },
        _ => CrossError::CommandExecutionFailed {
            command: program.to_string(),
            reason: e.to_string(),
        },
    })
}

/// Execute a command and return its output, with improved error messages
pub async fn run_command_output(cmd: &mut Command, program: &str) -> Result<std::process::Output> {
    cmd.output().await.map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => CrossError::ProgramNotFound {
            program: program.to_string(),
        },
        std::io::ErrorKind::PermissionDenied => CrossError::CommandExecutionFailed {
            command: program.to_string(),
            reason: "Permission denied".to_string(),
        },
        _ => CrossError::CommandExecutionFailed {
            command: program.to_string(),
            reason: e.to_string(),
        },
    })
}
