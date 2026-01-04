//! Download and archive extraction utilities for cargo-cross

use crate::color;
use crate::error::{CrossError, Result};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::path::Path;
use std::sync::LazyLock;
use std::time::Duration;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

/// Shared tick interval for progress bars (100ms)
const TICK_INTERVAL: Duration = Duration::from_millis(100);

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
    reqwest::Client::builder().user_agent("cargo-cross").build()
}

/// Download a file from URL with progress indication
pub async fn download_file(url: &str, dest: &Path) -> Result<()> {
    let client = create_http_client()?;
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(CrossError::DownloadFailed(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let pb = create_download_progress_bar(response.content_length());

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Download to temporary file
    let temp_path = dest.with_extension("tmp");
    let mut file = File::create(&temp_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        pb.inc(chunk.len() as u64);
    }

    file.flush().await?;
    drop(file);

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
    let temp_dir = dest.with_extension("tmp");
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

/// Download and extract a tar.gz archive
async fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<()> {
    use async_compression::tokio::bufread::GzipDecoder;
    use tokio::io::BufReader;
    use tokio_tar::Archive;

    let client = create_http_client()?;
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(CrossError::DownloadFailed(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    // Create multi-progress for simultaneous progress bars
    // Create progress bars without steady tick first, add to MultiProgress, then enable tick
    // This prevents rendering race conditions when bars are added
    let mp = MultiProgress::with_draw_target(indicatif::ProgressDrawTarget::stderr_with_hz(10));
    let download_pb = mp.insert(0, create_download_progress_bar_no_tick(response.content_length()));
    let extract_pb = mp.insert(1, create_extract_spinner_no_tick());
    // Enable steady tick after both bars are registered with MultiProgress
    download_pb.enable_steady_tick(TICK_INTERVAL);
    extract_pb.enable_steady_tick(TICK_INTERVAL);

    // Stream download with progress tracking
    let stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(
        stream.map(|result| result.map_err(std::io::Error::other)),
    );
    let reader = ProgressReader::new(reader, download_pb.clone());
    let buf_reader = BufReader::new(reader);

    // Decompress and extract
    let decoder = GzipDecoder::new(buf_reader);
    let mut archive = Archive::new(decoder);

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

    download_pb.finish_with_message("Download complete");
    extract_pb.finish_with_message(format!("{} files extracted", extract_pb.position()));

    Ok(())
}

/// Download and extract a ZIP archive
async fn download_and_extract_zip(url: &str, dest: &Path) -> Result<()> {
    let client = create_http_client()?;
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(CrossError::DownloadFailed(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    // Create progress bar for download
    let download_pb = create_download_progress_bar(response.content_length());

    // Download to {dest}.zip file
    let zip_path = dest.with_extension("zip");

    // Clean up existing zip file
    if zip_path.exists() {
        fs::remove_file(&zip_path).await.ok();
    }

    let mut file = File::create(&zip_path).await?;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        download_pb.inc(chunk.len() as u64);
        file.write_all(&chunk).await?;
    }

    file.flush().await?;
    drop(file);

    download_pb.finish_with_message("Download complete");

    // Extract ZIP with progress (creates its own progress bar with known total)
    extract_zip_archive(&zip_path, dest)?;

    // Clean up zip file after extraction
    fs::remove_file(&zip_path).await.ok();

    Ok(())
}

/// Extract ZIP archive from file with progress reporting
/// Based on zip crate's extract_internal implementation
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

/// Create a progress bar for download (without steady tick - caller should enable after adding to MultiProgress)
fn create_download_progress_bar_no_tick(total_size: Option<u64>) -> ProgressBar {
    total_size.map_or_else(
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
    )
}

/// Create a progress bar for download with steady tick enabled
fn create_download_progress_bar(total_size: Option<u64>) -> ProgressBar {
    let pb = create_download_progress_bar_no_tick(total_size);
    pb.enable_steady_tick(TICK_INTERVAL);
    pb
}

/// Create a spinner for extraction progress (without steady tick)
fn create_extract_spinner_no_tick() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(EXTRACT_SPINNER_STYLE.clone());
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

/// Wrapper reader that tracks download progress
struct ProgressReader<R> {
    inner: R,
    progress: ProgressBar,
}

impl<R> ProgressReader<R> {
    const fn new(inner: R, progress: ProgressBar) -> Self {
        Self { inner, progress }
    }
}

impl<R: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for ProgressReader<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let before = buf.filled().len();
        let result = std::pin::Pin::new(&mut self.inner).poll_read(cx, buf);
        if matches!(&result, std::task::Poll::Ready(Ok(()))) {
            let after = buf.filled().len();
            self.progress.inc((after - before) as u64);
        }
        result
    }
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
