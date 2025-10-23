use std::env;
use std::io::Write;
use std::process::{exit, Command, Stdio};

/// The embedded exec.sh script
const EXEC_SCRIPT: &str = include_str!("../exec.sh");

fn main() {
    let args: Vec<String> = env::args().collect();

    // Remove the first argument (program name)
    // For cargo subcommands, args will be: ["cargo-cross", "cross", ...]
    // We need to skip both "cargo-cross" and potentially "cross"
    let filtered_args: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|arg| *arg != "cross")
        .map(std::string::ToString::to_string)
        .collect();

    // Build the bash command with arguments
    // We use 'bash -s' to read the script from stdin, followed by '--' and the arguments
    let mut command = Command::new("bash");
    command
        .arg("-s")
        .arg("--")
        .args(&filtered_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    // Spawn the process
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            eprintln!("Failed to spawn bash: {e}");
            exit(1);
        },
    };

    // Write the script to bash's stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(EXEC_SCRIPT.as_bytes()) {
            eprintln!("Failed to write script to bash: {e}");
            exit(1);
        }
        // stdin is automatically closed when it goes out of scope
    }

    // Wait for the process to complete
    match child.wait() {
        Ok(exit_status) => {
            exit(exit_status.code().unwrap_or(1));
        },
        Err(e) => {
            eprintln!("Failed to wait for bash: {e}");
            exit(1);
        },
    }
}
