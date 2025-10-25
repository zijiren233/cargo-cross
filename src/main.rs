use std::env;
use std::io::Write;
use std::process::{exit, Command, Stdio};

/// The embedded cross.sh script
const CROSS_SCRIPT: &[u8] = include_bytes!("../cross.sh");

fn main() {
    let args: Vec<String> = env::args().collect();

    // When invoked as `cargo cross`, cargo sets the CARGO env var and passes
    // args as ["cargo-cross", "cross", ...]. We need to skip both.
    // When invoked directly as `cargo-cross`, only skip the program name.
    let skip_count = if env::var("CARGO").is_ok()
        && env::var("CARGO_HOME").is_ok()
        && args.get(1).map(std::string::String::as_str) == Some("cross")
    {
        2
    } else {
        1
    };
    let filtered_args: Vec<String> = args.iter().skip(skip_count).cloned().collect();

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
        if let Err(e) = stdin.write_all(CROSS_SCRIPT) {
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
