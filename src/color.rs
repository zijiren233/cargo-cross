//! Color output utilities for cargo-cross

use colored::Colorize;
use std::io::{self, Write};

/// Log an informational message (blue, bold)
pub fn log_info(msg: &str) {
    println!("{}", msg.bright_blue().bold());
}

/// Log a success message (green, bold)
pub fn log_success(msg: &str) {
    println!("{}", msg.bright_green().bold());
}

/// Log a warning message (yellow, bold)
pub fn log_warning(msg: &str) {
    println!("{}", msg.bright_yellow().bold());
}

/// Log an error message (red, bold) to stderr
pub fn log_error(msg: &str) {
    eprintln!("{}", msg.bright_red().bold());
}

/// Print a separator line
pub fn print_separator() {
    let width = terminal_width();
    println!("{}", "-".repeat(width).bright_white());
}

/// Get terminal width, defaulting to 80 if unavailable
fn terminal_width() -> usize {
    // Try to get terminal width from environment variable
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(width) = cols.parse::<usize>() {
            if width > 0 {
                return width;
            }
        }
    }
    80
}

/// Format a key-value pair for configuration display
pub fn format_config(key: &str, value: &str) -> String {
    format!("  {}: {}", key.bright_cyan().bold(), value.bright_yellow())
}

/// Format environment variable for display
pub fn format_env(key: &str, value: &str) -> String {
    format!("  {}={}", key.bright_cyan().bold(), value.bright_yellow())
}

/// Format a command for display
pub fn format_command(cmd: &str) -> String {
    format!("  {}", cmd.bright_cyan().bold())
}

/// Print execution configuration header
pub fn print_config_header() {
    log_info("Execution configuration:");
}

/// Print environment variables header
pub fn print_env_header() {
    log_info("Environment variables:");
}

/// Print run command header
pub fn print_run_header() {
    log_info("Run command:");
}

/// Flush stdout
pub fn flush() {
    let _ = io::stdout().flush();
}
