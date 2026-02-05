//! Runner setup for cross-compiled binaries

use crate::cli::Args;
use crate::color;
use crate::config::{Arch, HostPlatform};
use crate::download::download_and_extract;
use crate::env::CrossEnv;
use crate::error::Result;
use std::path::Path;
use tokio::fs;

/// Setup QEMU runner for cross-compiled Linux binaries
pub async fn setup_qemu_runner(
    env: &mut CrossEnv,
    arch: Arch,
    bin_prefix: &str,
    compiler_dir: &Path,
    args: &Args,
    host: &HostPlatform,
) -> Result<()> {
    let Some(qemu_binary) = arch.qemu_binary_name() else {
        return Ok(());
    };

    let qemu_dir = args
        .cross_compiler_dir
        .join(format!("qemu-user-static-{}", args.qemu_version));

    let qemu_path = qemu_dir.join(qemu_binary);

    // Download QEMU if not present
    if !qemu_path.exists() {
        let host_platform = host.download_platform();
        let download_url = format!(
            "https://github.com/zijiren233/qemu-user-static/releases/download/{}/qemu-user-static-{}-musl.tgz",
            args.qemu_version,
            host_platform
        );
        download_and_extract(&download_url, &qemu_dir, None, args.github_proxy.as_deref()).await?;
    }

    if qemu_path.exists() {
        // Add QEMU directory to PATH
        env.add_path(&qemu_dir);

        // Set runner using command name (relies on PATH) with sysroot
        let sysroot = compiler_dir.join(bin_prefix);
        if sysroot.join("lib").exists() {
            env.set_runner(format!("{} -L {}", qemu_binary, sysroot.display()));
        } else {
            env.set_runner(qemu_binary);
        }

        color::log_success(&format!(
            "Configured QEMU runner: {} for {}",
            color::yellow(qemu_binary),
            color::yellow(arch.as_str())
        ));
    }

    Ok(())
}

/// Setup Docker QEMU runner for cross-compiled Linux binaries (for macOS host)
pub async fn setup_docker_qemu_runner(
    env: &mut CrossEnv,
    arch: Arch,
    bin_prefix: &str,
    compiler_dir: &Path,
    libc: &str,
    args: &Args,
    host: &HostPlatform,
) -> Result<()> {
    // Check if Docker is available
    if which::which("docker").is_err() {
        color::log_warning("Docker not found, skipping Docker QEMU runner setup");
        return Ok(());
    }

    let Some(qemu_binary) = arch.qemu_binary_name() else {
        return Ok(());
    };

    // Download QEMU for Linux (to run inside Docker container)
    let qemu_dir = args.cross_compiler_dir.join(format!(
        "qemu-user-static-{}-linux-{}",
        args.qemu_version, host.arch
    ));

    let qemu_path = qemu_dir.join(qemu_binary);

    if !qemu_path.exists() {
        let download_url = format!(
            "https://github.com/zijiren233/qemu-user-static/releases/download/{}/qemu-user-static-linux-{}-musl.tgz",
            args.qemu_version,
            host.arch
        );
        download_and_extract(&download_url, &qemu_dir, None, args.github_proxy.as_deref()).await?;
    }

    if !qemu_path.exists() {
        return Ok(());
    }

    // Select Docker image based on libc type
    let docker_image = if libc == "musl" {
        "alpine:latest"
    } else {
        "ubuntu:latest"
    };

    // Create runner script
    let runner_script =
        args.cross_compiler_dir
            .join(format!("docker-qemu-runner-{}-{}.sh", arch.as_str(), libc));

    let sysroot = compiler_dir.join(bin_prefix);

    let script_content = format!(
        r#"#!/bin/bash
set -e

# Docker QEMU Runner Script
QEMU_PATH="{qemu_path}"
QEMU_BINARY="{qemu_binary}"
SYSROOT="{sysroot}"
DOCKER_IMAGE="{docker_image}"

if [[ $# -lt 1 ]]; then
    echo "Usage: $0 <binary> [args...]" >&2
    exit 1
fi

BINARY="$1"
shift

if [[ ! -f "$BINARY" ]]; then
    echo "Error: Binary not found: $BINARY" >&2
    exit 1
fi

BINARY_NAME=$(basename "$BINARY")

# Create container
CONTAINER_ID=$(docker create --rm -i "$DOCKER_IMAGE" /bin/sh -c "sleep infinity")

cleanup() {{
    docker rm -f "$CONTAINER_ID" >/dev/null 2>&1 || true
}}
trap cleanup EXIT

# Start the container
docker start "$CONTAINER_ID" >/dev/null

# Copy QEMU binary to container
docker cp "$QEMU_PATH" "$CONTAINER_ID:/usr/bin/$QEMU_BINARY" >/dev/null
docker exec "$CONTAINER_ID" chmod +x "/usr/bin/$QEMU_BINARY"

# Copy sysroot lib to container
if [[ -d "$SYSROOT/lib" ]]; then
    docker cp "$SYSROOT" "$CONTAINER_ID:/sysroot" >/dev/null
fi

# Copy the binary to execute
docker cp "$BINARY" "$CONTAINER_ID:/tmp/$BINARY_NAME" >/dev/null
docker exec "$CONTAINER_ID" chmod +x "/tmp/$BINARY_NAME"

# Run the binary with QEMU
docker exec "$CONTAINER_ID" /usr/bin/$QEMU_BINARY -L /sysroot /tmp/$BINARY_NAME "$@"
"#,
        qemu_path = qemu_path.display(),
        qemu_binary = qemu_binary,
        sysroot = sysroot.display(),
        docker_image = docker_image,
    );

    fs::write(&runner_script, &script_content).await?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&runner_script).await?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&runner_script, perms).await?;
    }

    env.set_runner(runner_script.display().to_string());

    color::log_success(&format!(
        "Configured Docker QEMU runner: {} for {} (image: {})",
        color::yellow(qemu_binary),
        color::yellow(arch.as_str()),
        color::cyan(docker_image)
    ));

    Ok(())
}

/// Setup Wine runner for Windows targets
pub fn setup_wine_runner(env: &mut CrossEnv, rust_target: &str) {
    if which::which("wine").is_ok() {
        env.set_runner("wine");
        color::log_success(&format!(
            "Configured Wine runner for {}",
            color::yellow(rust_target)
        ));
    }
}

/// Setup Rosetta runner for `x86_64` Darwin binaries on ARM Darwin hosts
pub fn setup_rosetta_runner(
    env: &mut CrossEnv,
    arch: Arch,
    rust_target: &str,
    host: &HostPlatform,
) {
    // Only setup Rosetta on Darwin hosts
    if !host.is_darwin() {
        return;
    }

    // Only for x86_64 Darwin targets
    if arch != Arch::X86_64 {
        return;
    }

    if !rust_target.contains("-apple-darwin") {
        return;
    }

    // Check if host is ARM
    if host.arch != "aarch64" {
        return;
    }

    env.set_runner("arch -x86_64");
    color::log_success(&format!(
        "Configured Rosetta runner for {}",
        color::yellow(rust_target)
    ));
}
