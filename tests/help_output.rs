use std::process::Command;

fn run_help(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-cross"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to run cargo-cross")
}

#[test]
fn root_help_shows_cargo_global_options() {
    let output = run_help(&["--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[CARGO_OPTIONS] <COMMAND>"));
    assert!(stdout.contains("cargo-cross --config build.jobs=4 build"));
}

#[test]
fn build_help_uses_wrapper_help() {
    let output = run_help(&["build", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: cargo-cross [+toolchain] build [OPTIONS]"));
    assert!(stdout.contains("--no-append-target"));
    assert!(stdout.contains("-m, --manifest-path"));
    assert!(!stdout.contains("--no-run"));
    assert!(!stdout.contains("Execution configuration:"));
}

#[test]
fn test_and_bench_help_show_command_specific_options() {
    let test_output = run_help(&["test", "--help"]);
    assert!(test_output.status.success());
    let test_stdout = String::from_utf8_lossy(&test_output.stdout);
    assert!(test_stdout.contains("--no-run"));
    assert!(test_stdout.contains("--no-fail-fast"));
    assert!(test_stdout.contains("--doc"));

    let bench_output = run_help(&["bench", "--help"]);
    assert!(bench_output.status.success());
    let bench_stdout = String::from_utf8_lossy(&bench_output.stdout);
    assert!(bench_stdout.contains("--no-run"));
    assert!(bench_stdout.contains("--no-fail-fast"));
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

#[test]
fn doc_short_alias_uses_doc_help() {
    let output = run_help(&["d", "--help"]);
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: cargo-cross [+toolchain] d [OPTIONS]"));
    assert!(stdout.contains("This command forwards to 'cargo doc'"));
}
