use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{exit, Command, Stdio};

/// The embedded cross.sh script
const CROSS_SCRIPT: &[u8] = include_bytes!("../cross.sh");

/// Check if a command exists in the system PATH
fn command_exists(cmd: &str) -> bool {
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths).any(|dir| {
                let full_path = dir.join(cmd);
                is_executable(&full_path)
            })
        })
        .unwrap_or(false)
}

/// Check if a path is an executable file
fn is_executable(path: &Path) -> bool {
    fs::metadata(path)
        .map(|meta| meta.is_file() && (meta.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

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

    // Detect available shell: prefer bash, fall back to sh
    let shell = if command_exists("bash") {
        "bash"
    } else if command_exists("sh") {
        "sh"
    } else {
        eprintln!("No shell found (tried bash and sh)");
        exit(1);
    };

    // Build the shell command with arguments
    // We use 'shell -s' to read the script from stdin, followed by '--' and the arguments
    let mut command = Command::new(shell);
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
            eprintln!("Failed to spawn {shell}: {e}");
            exit(1);
        }
    };

    // Write the script to shell's stdin
    if let Some(mut stdin) = child.stdin.take() {
        if let Err(e) = stdin.write_all(CROSS_SCRIPT) {
            eprintln!("Failed to write script to {shell}: {e}");
            exit(1);
        }
        // stdin is automatically closed when it goes out of scope
    }

    // Wait for the process to complete
    match child.wait() {
        Ok(exit_status) => {
            exit(exit_status.code().unwrap_or(1));
        }
        Err(e) => {
            eprintln!("Failed to wait for {shell}: {e}");
            exit(1);
        }
    }
}
