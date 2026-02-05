//! Download and archive extraction utilities for cargo-cross

use crate::color;
use crate::error::{CrossError, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

/// Shared tick interval for progress bars (100ms)
const TICK_INTERVAL: Duration = Duration::from_millis(100);

/// Maximum number of retry attempts for downloads
const MAX_RETRIES: u32 = 3;

/// Initial retry delay (doubles with each retry)
const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Cached progress styles to avoid repeated template parsing
static DOWNLOAD_SPINNER_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_spinner()
        .template("{spinner:.green} Downloading [{elapsed_precise}] {bytes}")
        .unwrap()
});

static DOWNLOAD_BAR_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{spinner:.green} Downloading [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("=> ")
});

static EXTRACT_SPINNER_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_spinner()
        .template("{spinner:.magenta} Extracting  [{elapsed_precise}] {pos} files ({my_per_sec}/s)")
        .unwrap()
        .with_key(
            "my_per_sec",
            |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
                write!(w, "{:.0}", state.per_sec()).unwrap();
            },
        )
});

static EXTRACT_BAR_STYLE: LazyLock<ProgressStyle> = LazyLock::new(|| {
    ProgressStyle::default_bar()
        .template("{spinner:.magenta} Extracting  [{elapsed_precise}] [{bar:40.magenta/white}] {pos}/{len} files ({my_per_sec}/s, {eta})")
        .unwrap()
        .progress_chars("=> ")
        .with_key("my_per_sec", |state: &indicatif::ProgressState, w: &mut dyn std::fmt::Write| {
            write!(w, "{:.0}", state.per_sec()).unwrap();
        })
});

/// Archive format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    Zip,
}

impl ArchiveFormat {
    /// Detect format from URL or filename
    #[must_use] 
    pub fn from_url(url: &str) -> Option<Self> {
        let lower = url.to_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::TarGz)
        } else if lower.ends_with(".zip") {
            Some(Self::Zip)
        } else {
            None
        }
    }
}

/// HTTP client wrapper for consistent configuration
fn create_http_client() -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("cargo-cross")
        .http1_only()
        .timeout(Duration::from_mins(5)) // 5 minutes timeout
        .build()
}

/// Check if an error is retryable (network errors, timeouts, etc.)
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_timeout()
        || err.is_connect()
        || err.is_request()
        || (err.is_status() && err.status().is_some_and(|s| s.is_server_error()))
}

/// Send HTTP GET request with automatic retry on transient failures
async fn send_request_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<reqwest::Response> {
    send_request_with_retry_range(client, url, None).await
}

/// Send HTTP GET request with Range header support and automatic retry
async fn send_request_with_retry_range(
    client: &reqwest::Client,
    url: &str,
    start_pos: Option<u64>,
) -> Result<reqwest::Response> {
    let mut last_error = None;

    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = INITIAL_RETRY_DELAY * 2_u32.pow(attempt - 1);
            tokio::time::sleep(delay).await;
        }

        let mut request = client.get(url);

        // Add Range header if resuming
        if let Some(pos) = start_pos {
            if pos > 0 {
                request = request.header("Range", format!("bytes={pos}-"));
            }
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status();

                // Accept both 200 (full content) and 206 (partial content)
                if status.is_success() || status == reqwest::StatusCode::PARTIAL_CONTENT {
                    return Ok(response);
                }

                return Err(CrossError::DownloadFailed(format!(
                    "HTTP {status} for {url}"
                )));
            }
            Err(err) => {
                if !is_retryable_error(&err) || attempt == MAX_RETRIES {
                    // Non-retryable error or max retries reached
                    return Err(err.into());
                }
                last_error = Some(err);
            }
        }
    }

    // This should never be reached, but just in case
    Err(last_error.map_or_else(
        || CrossError::DownloadFailed("Unknown error".to_string()),
        Into::into,
    ))
}

/// Download to file with resume support and automatic retry
async fn download_with_resume(
    client: &reqwest::Client,
    url: &str,
    file_path: &Path,
    pb: &ProgressBar,
    already_downloaded: u64,
) -> Result<()> {
    // Set initial position if resuming
    if already_downloaded > 0 {
        pb.set_position(already_downloaded);
    }

    let mut downloaded = already_downloaded;
    let mut attempt = 0;
    'retry: loop {
        let response = send_request_with_retry_range(client, url, Some(downloaded)).await?;

        // Open file in append mode or create if doesn't exist
        let mut file = if downloaded > 0 {
            File::options()
                .append(true)
                .create(true)
                .open(file_path)
                .await?
        } else {
            File::create(file_path).await?
        };

        let mut stream = response.bytes_stream();

        while let Some(chunk_result) = stream.next().await {
            match chunk_result {
                Ok(chunk) => {
                    file.write_all(&chunk).await?;
                    downloaded += chunk.len() as u64;
                    pb.inc(chunk.len() as u64);
                }
                Err(err) => {
                    // Network error during streaming - need to retry
                    file.flush().await?;

                    if attempt >= MAX_RETRIES {
                        return Err(CrossError::DownloadFailed(format!(
                            "Max retries reached: {err}"
                        )));
                    }

                    attempt += 1;
                    let delay = INITIAL_RETRY_DELAY * 2_u32.pow(attempt - 1);
                    tokio::time::sleep(delay).await;
                    continue 'retry;
                }
            }
        }

        // Download completed successfully
        file.flush().await?;
        break;
    }

    Ok(())
}

/// Download a file from URL with progress indication, resume support and automatic retry
pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let client = create_http_client()?;

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Download to temporary file
    // Note: Can't use with_extension() because dest may contain dots (e.g., v0.7.7)
    let temp_path = dest
        .parent().map_or_else(|| {
            std::path::PathBuf::from(format!("{}.tmp", dest.file_name().unwrap().to_string_lossy()))
        }, |p| p.join(format!("{}.tmp", dest.file_name().unwrap().to_string_lossy())));

    // Check if partial file exists
    let already_downloaded = if temp_path.exists() {
        fs::metadata(&temp_path).await?.len()
    } else {
        0
    };

    // Get total size (try without Range first to get accurate size)
    let response = send_request_with_retry(&client, url).await?;
    let total_size = response.content_length();
    drop(response); // Close the connection

    // Create progress bar
    let pb = create_download_progress_bar(total_size);

    // Download with resume support
    download_with_resume(&client, url, &temp_path, &pb, already_downloaded).await?;

    pb.finish_with_message("Download complete");

    // Rename to final destination
    fs::rename(&temp_path, dest).await?;

    Ok(())
}

/// Download and extract an archive
pub async fn download_and_extract(
    url: &str,
    dest: &Path,
    format: Option<ArchiveFormat>,
    github_proxy: Option<&str>,
) -> Result<()> {
    let format = format
        .or_else(|| ArchiveFormat::from_url(url))
        .ok_or_else(|| CrossError::UnsupportedArchiveFormat(url.to_string()))?;

    // Apply GitHub proxy if configured
    let url = apply_github_proxy(url, github_proxy);

    // Get absolute path for destination
    let dest = if dest.is_absolute() {
        dest.to_path_buf()
    } else {
        std::env::current_dir()?.join(dest)
    };

    // Create parent directory
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Use temporary directory for extraction
    // Note: Can't use with_extension() because dest may contain dots (e.g., v0.7.7)
    let temp_dir = dest
        .parent()
        .unwrap()
        .join(format!("{}.tmp", dest.file_name().unwrap().to_string_lossy()));
    cleanup_and_create_dir(&temp_dir).await?;

    color::log_info(&format!(
        "Downloading \"{}\" to \"{}\"",
        color::green(&url),
        color::green(&dest.display().to_string())
    ));

    let start_time = std::time::Instant::now();

    // Download and extract based on format
    let result = match format {
        ArchiveFormat::TarGz => download_and_extract_tar_gz(&url, &temp_dir).await,
        ArchiveFormat::Zip => download_and_extract_zip(&url, &temp_dir).await,
    };

    // Clean up temp directory on failure
    if result.is_err() {
        fs::remove_dir_all(&temp_dir).await.ok();
        return result;
    }

    // Move extracted content to final destination
    finalize_extraction(&temp_dir, &dest).await?;

    let elapsed = start_time.elapsed();
    color::log_success(&format!(
        "Download and extraction successful (took {})",
        color::yellow(&format!("{}s", elapsed.as_secs()))
    ));

    Ok(())
}

/// Download archive file with resume support and progress tracking
async fn download_archive(url: &str, file_path: &Path) -> Result<()> {
    let client = create_http_client()?;

    // Check if partial file exists
    let already_downloaded = if file_path.exists() {
        fs::metadata(file_path).await?.len()
    } else {
        0
    };

    // Get total size for progress bar
    let response = send_request_with_retry(&client, url).await?;
    let total_size = response.content_length();
    drop(response); // Close the connection

    // Create download progress bar
    let download_pb = create_download_progress_bar(total_size);

    // Download with resume support
    download_with_resume(&client, url, file_path, &download_pb, already_downloaded).await?;

    download_pb.finish_with_message("Download complete");

    Ok(())
}

/// Download and extract a tar.gz archive with resume support and automatic retry
async fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<()> {
    use async_compression::tokio::bufread::GzipDecoder;
    use tokio::io::BufReader;
    use tokio_tar::ArchiveBuilder;

    // Download to {dest}.tar.gz file first (with resume support)
    // Note: Can't use with_extension() because dest may contain dots (e.g., v0.7.7)
    let archive_path = dest
        .parent()
        .unwrap()
        .join(format!("{}.tar.gz", dest.file_name().unwrap().to_string_lossy()));
    download_archive(url, &archive_path).await?;

    // Now extract the downloaded archive
    let extract_pb = create_extract_spinner();

    let file = File::open(&archive_path).await?;
    let buf_reader = BufReader::new(file);

    // Decompress and extract with permission preservation for executable files
    let decoder = GzipDecoder::new(buf_reader);
    let mut archive = ArchiveBuilder::new(decoder)
        .set_preserve_permissions(true)
        .build();

    let mut entries = archive
        .entries()
        .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

    while let Some(entry) = entries.next().await {
        let mut entry = entry.map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;
        entry
            .unpack_in(dest)
            .await
            .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;
        extract_pb.inc(1);
    }

    extract_pb.finish_with_message(format!("{} files extracted", extract_pb.position()));

    // Clean up archive file after extraction
    fs::remove_file(&archive_path).await.ok();

    Ok(())
}

/// Download and extract a ZIP archive with resume support and automatic retry
async fn download_and_extract_zip(url: &str, dest: &Path) -> Result<()> {
    // Download to {dest}.zip file
    // Note: Can't use with_extension() because dest may contain dots (e.g., v0.7.7)
    let zip_path = dest
        .parent()
        .unwrap()
        .join(format!("{}.zip", dest.file_name().unwrap().to_string_lossy()));
    download_archive(url, &zip_path).await?;

    // Extract ZIP with progress (creates its own progress bar with known total)
    extract_zip_archive(&zip_path, dest)?;

    // Clean up zip file after extraction
    fs::remove_file(&zip_path).await.ok();

    Ok(())
}

/// Extract ZIP archive from file with progress reporting
/// Based on zip crate's `extract_internal` implementation
fn extract_zip_archive(zip_path: &Path, dest: &Path) -> Result<()> {
    use std::fs;
    use std::io::Read;

    // Create destination directory
    fs::create_dir_all(dest)?;

    let file = fs::File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

    let total_files = archive.len();

    // Create progress bar with known total (shows speed and ETA)
    let pb = create_extract_progress_bar(total_files);

    // Collect files that need permission setting (set at the end)
    #[cfg(unix)]
    let mut files_by_unix_mode: Vec<(std::path::PathBuf, u32)> = Vec::new();

    for i in 0..total_files {
        let mut file = archive
            .by_index(i)
            .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

        let outpath = match file.enclosed_name() {
            Some(path) => dest.join(path),
            None => continue,
        };

        // Handle symlinks: read target first, then drop file handle before creating symlink
        #[allow(clippy::cast_possible_truncation)] // symlink targets are typically small
        let symlink_target = if file.is_symlink() && (cfg!(unix) || cfg!(windows)) {
            let mut target = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut target)
                .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;
            Some(target)
        } else if file.is_dir() {
            // Create directory and ensure it's writable for subsequent file extractions
            make_writable_dir_all(&outpath)?;
            continue;
        } else {
            None
        };

        // Drop file handle before creating symlink or re-opening for copy
        drop(file);

        if let Some(target) = symlink_target {
            // Create parent directory if needed
            if let Some(parent) = outpath.parent() {
                make_writable_dir_all(parent)?;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::symlink;
                let target_str = std::str::from_utf8(&target)
                    .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;
                // Remove existing file/symlink if present
                if outpath.symlink_metadata().is_ok() {
                    fs::remove_file(&outpath).ok();
                }
                symlink(target_str, &outpath)
                    .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;
            }
            #[cfg(not(unix))]
            {
                // On non-Unix, write target as file content
                if let Some(parent) = outpath.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&outpath, &target)?;
            }
        } else {
            // Regular file: re-open file handle and copy content
            let mut file = archive
                .by_index(i)
                .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

            if let Some(parent) = outpath.parent() {
                make_writable_dir_all(parent)?;
            }

            let mut outfile = fs::File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;

            // Collect permissions for later (don't set immediately)
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                files_by_unix_mode.push((outpath.clone(), mode));
            }
        }

        pb.inc(1);
    }

    // Set permissions at the end, in reverse path order
    // This ensures child permissions are set before parent becomes unwritable
    #[cfg(unix)]
    {
        use std::cmp::Reverse;
        use std::os::unix::fs::PermissionsExt;

        if files_by_unix_mode.len() > 1 {
            files_by_unix_mode.sort_by_key(|(path, _)| Reverse(path.clone()));
        }
        for (path, mode) in files_by_unix_mode {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
        }
    }

    pb.finish_with_message(format!("{total_files} files extracted"));
    Ok(())
}

/// Create directory and ensure it's writable (for subsequent file extractions)
fn make_writable_dir_all(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(path)?;
        let current_mode = metadata.permissions().mode();
        // Add owner rwx permissions to ensure directory is writable
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700 | current_mode))?;
    }
    Ok(())
}

/// Create a progress bar for download with steady tick
fn create_download_progress_bar(total_size: Option<u64>) -> ProgressBar {
    let pb = total_size.map_or_else(
        || {
            let pb = ProgressBar::new_spinner();
            pb.set_style(DOWNLOAD_SPINNER_STYLE.clone());
            pb
        },
        |size| {
            let pb = ProgressBar::new(size);
            pb.set_style(DOWNLOAD_BAR_STYLE.clone());
            pb
        },
    );
    pb.enable_steady_tick(TICK_INTERVAL);
    pb
}

/// Create a spinner for extraction progress with steady tick
fn create_extract_spinner() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(EXTRACT_SPINNER_STYLE.clone());
    pb.enable_steady_tick(TICK_INTERVAL);
    pb
}

/// Create a progress bar for extraction with known total (shows speed and ETA)
fn create_extract_progress_bar(total: usize) -> ProgressBar {
    let pb = ProgressBar::new(total as u64);
    pb.set_style(EXTRACT_BAR_STYLE.clone());
    pb.enable_steady_tick(TICK_INTERVAL);
    pb
}

/// Apply GitHub proxy to URL if configured
fn apply_github_proxy(url: &str, proxy: Option<&str>) -> String {
    match proxy {
        Some(proxy) if url.starts_with("https://github.com") => format!("{proxy}{url}"),
        _ => url.to_string(),
    }
}

/// Clean up existing directory and create new one
async fn cleanup_and_create_dir(path: &Path) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path).await.ok();
    }
    fs::create_dir_all(path).await?;
    Ok(())
}

/// Move extracted content to final destination, handling single top-level directory case
async fn finalize_extraction(temp_dir: &Path, dest: &Path) -> Result<()> {
    if dest.exists() {
        fs::remove_dir_all(dest).await.ok();
    }

    // Check if there's a single top-level directory
    let entries = collect_dir_entries(temp_dir).await?;

    if entries.len() == 1 && entries[0].file_type().await?.is_dir() {
        // Single directory - move it directly
        fs::rename(entries[0].path(), dest).await?;
        fs::remove_dir_all(temp_dir).await.ok();
    } else {
        // Multiple entries - move the whole temp directory
        fs::rename(temp_dir, dest).await?;
    }

    Ok(())
}

/// Collect directory entries
async fn collect_dir_entries(path: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries = Vec::new();
    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        entries.push(entry);
    }
    Ok(entries)
}

/// Check if a directory exists and has content
pub async fn dir_exists_and_not_empty(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }

    if let Ok(mut entries) = fs::read_dir(path).await {
        entries.next_entry().await.ok().flatten().is_some()
    } else {
        false
    }
}

/// Download cross-compiler if not already present
pub async fn download_cross_compiler(
    compiler_dir: &Path,
    download_url: &str,
    github_proxy: Option<&str>,
) -> Result<()> {
    if !dir_exists_and_not_empty(compiler_dir).await {
        download_and_extract(download_url, compiler_dir, None, github_proxy).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_format_detection() {
        assert_eq!(
            ArchiveFormat::from_url("foo.tar.gz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(
            ArchiveFormat::from_url("foo.tgz"),
            Some(ArchiveFormat::TarGz)
        );
        assert_eq!(ArchiveFormat::from_url("foo.zip"), Some(ArchiveFormat::Zip));
        assert_eq!(ArchiveFormat::from_url("foo.txt"), None);
        assert_eq!(ArchiveFormat::from_url("foo.tar.xz"), None); // Not supported
    }

    #[test]
    fn test_github_proxy() {
        assert_eq!(
            apply_github_proxy("https://github.com/foo/bar", Some("https://proxy.com/")),
            "https://proxy.com/https://github.com/foo/bar"
        );
        assert_eq!(
            apply_github_proxy("https://other.com/foo", Some("https://proxy.com/")),
            "https://other.com/foo"
        );
        assert_eq!(
            apply_github_proxy("https://github.com/foo/bar", None),
            "https://github.com/foo/bar"
        );
    }
}
