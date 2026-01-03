//! Color output utilities for cargo-cross
//!
//! This module provides rich color support similar to the bash version,
//! allowing multiple colors within a single log line.

use colored::{ColoredString, Colorize};
use std::io::{self, Write};

pub fn cyan(s: &str) -> ColoredString {
    s.bright_cyan().bold()
}

/// Format text as bold bright yellow (for values, targets, times)
pub fn yellow(s: &str) -> ColoredString {
    s.bright_yellow().bold()
}

/// Format text as bold bright green (for URLs, paths, success highlights)
pub fn green(s: &str) -> ColoredString {
    s.bright_green().bold()
}

/// Format text as bold bright blue (for info text)
pub fn blue(s: &str) -> ColoredString {
    s.bright_blue().bold()
}

/// Format text as bold bright red (for errors)
pub fn red(s: &str) -> ColoredString {
    s.bright_red().bold()
}

/// Format text as bold bright magenta (for special highlights)
pub fn magenta(s: &str) -> ColoredString {
    s.bright_magenta().bold()
}

/// Format text as bold white (for separators, neutral highlights)
pub fn white(s: &str) -> ColoredString {
    s.bright_white().bold()
}

/// Format text as bold dim/dark gray
pub fn dim(s: &str) -> ColoredString {
    s.dimmed().bold()
}

/// Example: log_info(&format!("Downloading {} to {}", green(url), green(path)))
pub fn log_info(msg: &str) {
    println!("{}", msg.bright_blue().bold());
}

/// Log a success message (bold green, supports embedded colors)
/// Example: log_success(&format!("Completed in {}s", yellow(&secs.to_string())))
pub fn log_success(msg: &str) {
    println!("{}", msg.bright_green().bold());
}

/// Log a warning message (bold yellow, supports embedded colors)
pub fn log_warning(msg: &str) {
    println!("{}", msg.bright_yellow().bold());
}

/// Log an error message (bold red, supports embedded colors) to stderr
pub fn log_error(msg: &str) {
    eprintln!("{}", msg.bright_red().bold());
}

/// Print a separator line
pub fn print_separator() {
    let width = terminal_width();
    println!("{}", "-".repeat(width).dimmed());
}

/// Get terminal width, defaulting to 80 if unavailable
fn terminal_width() -> usize {
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
    format!(
        "  {}: {}",
        key.bright_cyan().bold(),
        value.bright_yellow().bold()
    )
}

/// Format environment variable for display
pub fn format_env(key: &str, value: &str) -> String {
    format!(
        "  {}={}",
        key.bright_cyan().bold(),
        value.bright_yellow().bold()
    )
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
