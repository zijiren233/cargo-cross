use std::process::Command;

fn run_help(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-cross"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run cargo-cross")
}

#[test]
fn build_help_uses_wrapper_help() {
    let output = run_help(&["build", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: cargo-cross [+toolchain] build [OPTIONS]"));
    assert!(stdout.contains("--no-append-target"));
    assert!(!stdout.contains("Execution configuration:"));
}

#[test]
fn doc_help_uses_wrapper_help() {
    let output = run_help(&["doc", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: cargo-cross [+toolchain] doc [OPTIONS]"));
    assert!(stdout.contains("This command forwards to 'cargo doc'"));
    assert!(stdout.contains("--no-append-target"));
    assert!(!stdout.contains("Execution configuration:"));
}
