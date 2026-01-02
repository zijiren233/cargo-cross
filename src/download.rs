//! Download and archive extraction utilities for cargo-cross

use crate::color;
use crate::error::{CrossError, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use tokio::fs::{self, File};
use tokio::io::AsyncWriteExt;

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

    let pb = create_progress_bar(response.content_length());

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
        url,
        dest.display()
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
        "Download and extraction successful (took {}s)",
        elapsed.as_secs()
    ));

    Ok(())
}

/// Download and extract a tar.gz archive
async fn download_and_extract_tar_gz(url: &str, dest: &Path) -> Result<()> {
    use async_compression::futures::bufread::GzipDecoder;
    use futures_util::io::BufReader;

    let client = create_http_client()?;
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        return Err(CrossError::DownloadFailed(format!(
            "HTTP {} for {}",
            response.status(),
            url
        )));
    }

    let pb = create_progress_bar(response.content_length());

    // Stream the response with progress tracking
    let stream = response.bytes_stream();
    let reader = tokio_util::io::StreamReader::new(
        stream.map(|result| result.map_err(std::io::Error::other)),
    );
    let reader = ProgressReader::new(reader, pb.clone());

    // Convert tokio AsyncRead to futures AsyncRead
    let reader = tokio_util::compat::TokioAsyncReadCompatExt::compat(reader);
    let buf_reader = BufReader::new(reader);

    // Decompress and extract (async-tar uses futures AsyncRead directly)
    let decoder = GzipDecoder::new(buf_reader);
    let archive = async_tar::Archive::new(decoder);
    archive
        .unpack(dest)
        .await
        .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

    pb.finish_with_message("Extraction complete");
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

    let pb = create_progress_bar(response.content_length());

    // Download to memory (ZIP needs random access)
    let mut data = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pb.inc(chunk.len() as u64);
        data.extend_from_slice(&chunk);
    }

    pb.finish_with_message("Download complete, extracting...");

    // Extract ZIP
    extract_zip_archive(&data, dest)?;

    Ok(())
}

/// Extract ZIP archive from bytes
fn extract_zip_archive(data: &[u8], dest: &Path) -> Result<()> {
    let cursor = std::io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

    // Use the built-in extract method which handles symlinks correctly
    archive
        .extract(dest)
        .map_err(|e| CrossError::ExtractionFailed(e.to_string()))?;

    Ok(())
}

/// Create a progress bar based on content length
fn create_progress_bar(total_size: Option<u64>) -> ProgressBar {
    total_size.map_or_else(|| {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {bytes}")
                .unwrap(),
        );
        pb
    }, |size| {
        let pb = ProgressBar::new(size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );
        pb
    })
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
