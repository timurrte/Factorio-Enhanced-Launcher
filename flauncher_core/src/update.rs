use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::config::{get_installed_version, FactorioConfig};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ReleaseBranch {
    pub alpha: Option<String>,
    pub demo: Option<String>,
    pub expansion: Option<String>,
    pub headless: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct LatestReleases {
    pub stable: ReleaseBranch,
    pub experimental: ReleaseBranch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Distro {
    Win64Manual,
    Osx,
    Linux64,
}

impl Distro {
    pub fn as_str(&self) -> &'static str {
        match self {
            Distro::Win64Manual => "win64-manual",
            Distro::Osx => "osx",
            Distro::Linux64 => "linux64",
        }
    }

    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            Distro::Win64Manual
        }
        #[cfg(target_os = "macos")]
        {
            Distro::Osx
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            Distro::Linux64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildType {
    Alpha,
    Expansion,
}

impl BuildType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BuildType::Alpha => "alpha",
            BuildType::Expansion => "expansion",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BuildType::Alpha => "Base Game (Alpha)",
            BuildType::Expansion => "Space Age (Expansion)",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadProgress {
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub percentage: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStage {
    Idle,
    Checking,
    Downloading(DownloadProgress),
    Extracting(String),
    Installing,
    Completed(String),
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    TarXz,
    TarGz,
    Dmg,
}

/// Fetch the latest releases JSON from the official Factorio API
pub async fn fetch_latest_releases() -> Result<LatestReleases, String> {
    let client = reqwest::Client::builder()
        .user_agent("Factorio-Enhanced-Launcher")
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .get("https://factorio.com/api/latest-releases")
        .send()
        .await
        .map_err(|e| format!("Failed to reach Factorio releases API: {e}"))?;

    if !res.status().is_success() {
        return Err(format!("Factorio API returned status: {}", res.status()));
    }

    res.json::<LatestReleases>()
        .await
        .map_err(|e| format!("Failed to parse release response: {e}"))
}

/// Construct the authenticated download URL
/// Format: https://www.factorio.com/get-download/{version}/{build}/{distro}?username={username}&token={token}
pub fn construct_download_url(
    version: &str,
    build: BuildType,
    distro: Distro,
    username: &str,
    token: &str,
) -> Result<reqwest::Url, String> {
    if username.trim().is_empty() {
        return Err("Factorio Username cannot be empty.".into());
    }
    if token.trim().is_empty() {
        return Err("Factorio Service Token cannot be empty.".into());
    }

    let base = format!(
        "https://www.factorio.com/get-download/{}/{}/{}",
        version.trim(),
        build.as_str(),
        distro.as_str()
    );

    let mut url = reqwest::Url::parse(&base).map_err(|e| format!("Invalid base URL: {e}"))?;
    url.query_pairs_mut()
        .append_pair("username", username.trim())
        .append_pair("token", token.trim());

    Ok(url)
}

/// Download Factorio update archive with real-time progress reporting
pub async fn download_update<F>(
    download_url: reqwest::Url,
    destination_file: &Path,
    mut on_progress: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let client = reqwest::Client::builder()
        .user_agent("Factorio-Enhanced-Launcher")
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client
        .get(download_url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {e}"))?;

    // Check if we were redirected to factorio.com/login due to missing/invalid token or username
    let final_url = response.url().as_str();
    if final_url.contains("/login") {
        return Err(
            "Authentication failed: Factorio rejected credentials and redirected to login. Please verify your Username and Service Token."
                .into(),
        );
    }

    if !response.status().is_success() {
        return Err(format!("Download failed with HTTP status: {}", response.status()));
    }

    // Verify response content-type isn't HTML (which indicates an error page from factorio.com)
    if let Some(ct) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct_str = ct.to_str().unwrap_or_default();
        if ct_str.starts_with("text/html") {
            return Err("Server returned HTML page instead of download. Please check your credentials or selected version.".into());
        }
    }

    let total_bytes = response.content_length().unwrap_or(0);
    let mut downloaded_bytes: u64 = 0;

    if let Some(parent) = destination_file.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create destination dir: {e}"))?;
    }

    let mut file = tokio::fs::File::create(destination_file)
        .await
        .map_err(|e| format!("Failed to create temporary file {}: {e}", destination_file.display()))?;

    let mut stream = response.bytes_stream();
    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| format!("Error during download streaming: {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Failed to write downloaded data: {e}"))?;
        downloaded_bytes += chunk.len() as u64;

        let percentage = if total_bytes > 0 {
            (downloaded_bytes as f64 / total_bytes as f64) * 100.0
        } else {
            0.0
        };

        on_progress(DownloadProgress {
            downloaded_bytes,
            total_bytes,
            percentage,
        });
    }

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush downloaded file: {e}"))?;

    Ok(())
}

/// Detect archive format based on file signatures (magic bytes) and filename extensions
pub fn detect_archive_format(file_path: &Path) -> Result<ArchiveFormat, String> {
    let mut file = File::open(file_path).map_err(|e| format!("Could not open archive: {e}"))?;
    let mut magic = [0u8; 6];
    let bytes_read = file.read(&mut magic).unwrap_or(0);

    if bytes_read >= 4 && &magic[0..4] == b"PK\x03\x04" {
        return Ok(ArchiveFormat::Zip);
    }
    if bytes_read >= 6 && &magic[0..6] == b"\xfd7zXZ\x00" {
        return Ok(ArchiveFormat::TarXz);
    }
    if bytes_read >= 2 && &magic[0..2] == b"\x1f\x8b" {
        return Ok(ArchiveFormat::TarGz);
    }

    let filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_lowercase();

    if filename.ends_with(".zip") {
        return Ok(ArchiveFormat::Zip);
    }
    if filename.ends_with(".tar.xz") || filename.ends_with(".xz") {
        return Ok(ArchiveFormat::TarXz);
    }
    if filename.ends_with(".tar.gz") || filename.ends_with(".tgz") {
        return Ok(ArchiveFormat::TarGz);
    }
    if filename.ends_with(".dmg") {
        return Ok(ArchiveFormat::Dmg);
    }

    Err(format!(
        "Unknown or unsupported archive format for file: {}",
        file_path.display()
    ))
}

/// Extract .zip archives (used on Windows and standalone zips)
pub fn extract_zip(archive_path: &Path, extract_to: &Path) -> Result<(), String> {
    fs::create_dir_all(extract_to).map_err(|e| e.to_string())?;
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid zip archive: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| e.to_string())?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_to.join(path),
            None => continue,
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| e.to_string())?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
            }
        }
    }
    Ok(())
}

/// Extract .tar.xz archives (used on Linux and macOS tarballs)
pub fn extract_tar_xz(archive_path: &Path, extract_to: &Path) -> Result<(), String> {
    fs::create_dir_all(extract_to).map_err(|e| e.to_string())?;
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let decompressor = xz2::read::XzDecoder::new(file);
    let mut tar_archive = tar::Archive::new(decompressor);
    tar_archive
        .unpack(extract_to)
        .map_err(|e| format!("Failed to unpack tar.xz: {e}"))?;
    Ok(())
}

/// Extract .tar.gz archives
pub fn extract_tar_gz(archive_path: &Path, extract_to: &Path) -> Result<(), String> {
    fs::create_dir_all(extract_to).map_err(|e| e.to_string())?;
    let file = File::open(archive_path).map_err(|e| e.to_string())?;
    let decompressor = flate2::read::GzDecoder::new(file);
    let mut tar_archive = tar::Archive::new(decompressor);
    tar_archive
        .unpack(extract_to)
        .map_err(|e| format!("Failed to unpack tar.gz: {e}"))?;
    Ok(())
}

/// Extract macOS .dmg archives by mounting with hdiutil, copying contents, and detaching
pub fn extract_dmg(archive_path: &Path, extract_to: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        fs::create_dir_all(extract_to).map_err(|e| e.to_string())?;
        let mount_point = extract_to.join("_dmg_mount");
        fs::create_dir_all(&mount_point).map_err(|e| e.to_string())?;

        let status = std::process::Command::new("hdiutil")
            .arg("attach")
            .arg("-nobrowse")
            .arg("-readonly")
            .arg(archive_path)
            .arg("-mountpoint")
            .arg(&mount_point)
            .status()
            .map_err(|e| format!("Failed to run hdiutil: {e}"))?;

        if !status.success() {
            let _ = fs::remove_dir_all(&mount_point);
            return Err("Failed to attach macOS DMG image with hdiutil".into());
        }

        // Find Factorio.app inside mounted DMG
        let mut found_app = None;
        if let Ok(entries) = fs::read_dir(&mount_point) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    found_app = Some(path);
                    break;
                }
            }
        }

        let copy_result = if let Some(app_path) = found_app {
            let target_app = extract_to.join("Factorio.app");
            copy_and_overwrite_all(&app_path, &target_app)
        } else {
            copy_and_overwrite_all(&mount_point, extract_to)
        };

        // Detach DMG volume cleanly
        let _ = std::process::Command::new("hdiutil")
            .arg("detach")
            .arg(&mount_point)
            .arg("-force")
            .status();

        let _ = fs::remove_dir_all(&mount_point);
        copy_result?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (archive_path, extract_to);
        Err("DMG extraction is only supported on macOS".into())
    }
}

/// Extract any supported Factorio archive format (.zip, .tar.xz, .tar.gz, .dmg)
pub fn extract_archive(archive_path: &Path, extract_to: &Path) -> Result<(), String> {
    let format = detect_archive_format(archive_path)?;
    match format {
        ArchiveFormat::Zip => extract_zip(archive_path, extract_to),
        ArchiveFormat::TarXz => extract_tar_xz(archive_path, extract_to),
        ArchiveFormat::TarGz => extract_tar_gz(archive_path, extract_to),
        ArchiveFormat::Dmg => extract_dmg(archive_path, extract_to),
    }
}

/// Recursively copy all files and directories from src to dst, overwriting existing files
pub fn copy_and_overwrite_all(src: &Path, dst: &Path) -> Result<(), String> {
    if !dst.exists() {
        fs::create_dir_all(dst)
            .map_err(|e| format!("Failed to create destination directory {}: {e}", dst.display()))?;
    }

    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read {}: {e}", src.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let entry_path = entry.path();
        let file_name = entry.file_name();
        let target_path = dst.join(&file_name);

        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_dir() {
            copy_and_overwrite_all(&entry_path, &target_path)?;
        } else {
            if target_path.exists() {
                let _ = fs::remove_file(&target_path);
            }
            fs::copy(&entry_path, &target_path).map_err(|e| {
                format!(
                    "Failed to copy {} to {}: {e}",
                    entry_path.display(),
                    target_path.display()
                )
            })?;

            #[cfg(unix)]
            {
                if let Ok(meta) = entry_path.metadata() {
                    let _ = fs::set_permissions(&target_path, meta.permissions());
                }
            }
        }
    }
    Ok(())
}

/// Find the payload directory inside the extracted archive (e.g. Factorio.app or directory with bin/ & data/)
pub fn find_extracted_factorio_root(dir: &Path) -> Result<PathBuf, String> {
    // If the directory itself contains Factorio contents
    if dir.join("Contents").exists() || dir.join("bin").exists() {
        return Ok(dir.to_path_buf());
    }

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    return Ok(path);
                }
                if path.join("Contents").exists() || path.join("bin").exists() {
                    return Ok(path);
                }
            }
        }
    }

    Ok(dir.to_path_buf())
}

/// Apply the extracted files to overwrite the target installation
pub fn apply_update_to_installation(payload_dir: &Path, config: &FactorioConfig) -> Result<(), String> {
    let install_dir = config.install_dir();

    #[cfg(target_os = "macos")]
    {
        if payload_dir.join("Contents").exists()
            && install_dir.extension().and_then(|e| e.to_str()) == Some("app")
        {
            copy_and_overwrite_all(payload_dir, &install_dir)?;
        } else if let Some(app_in_payload) = fs::read_dir(payload_dir).ok().and_then(|mut r| {
            r.find_map(|e| {
                let p = e.ok()?.path();
                if p.extension().and_then(|ext| ext.to_str()) == Some("app") {
                    Some(p)
                } else {
                    None
                }
            })
        }) {
            copy_and_overwrite_all(&app_in_payload, &install_dir)?;
        } else {
            copy_and_overwrite_all(payload_dir, &install_dir)?;
        }

        // Ensure factorio binary is executable
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&config.bin_path, fs::Permissions::from_mode(0o755));
    }

    #[cfg(not(target_os = "macos"))]
    {
        copy_and_overwrite_all(payload_dir, &install_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&config.bin_path, fs::Permissions::from_mode(0o755));
        }
    }

    Ok(())
}

/// Complete end-to-end update process:
/// 1. Download authenticated update archive to a temporary file
/// 2. Extract update archive into temporary directory
/// 3. Overwrite local installation directory
/// 4. Clean up temporary files
/// 5. Return updated version string
pub async fn perform_update<F>(
    config: &FactorioConfig,
    version: &str,
    build: BuildType,
    username: &str,
    token: &str,
    mut stage_cb: F,
) -> Result<String, String>
where
    F: FnMut(UpdateStage),
{
    let temp_root = std::env::temp_dir().join(format!(
        "factorio_update_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));

    if temp_root.exists() {
        let _ = fs::remove_dir_all(&temp_root);
    }
    fs::create_dir_all(&temp_root).map_err(|e| format!("Failed to create temp dir: {e}"))?;

    let distro = Distro::current();
    let download_url = match construct_download_url(version, build, distro, username, token) {
        Ok(u) => u,
        Err(e) => {
            let _ = fs::remove_dir_all(&temp_root);
            stage_cb(UpdateStage::Failed(e.clone()));
            return Err(e);
        }
    };

    let download_file = match distro {
        Distro::Win64Manual => temp_root.join("factorio_update.zip"),
        Distro::Linux64 => temp_root.join("factorio_update.tar.xz"),
        Distro::Osx => temp_root.join("factorio_update.dmg"),
    };

    // 1. Download
    let dl_res = download_update(download_url, &download_file, |prog| {
        stage_cb(UpdateStage::Downloading(prog));
    })
    .await;

    if let Err(e) = dl_res {
        let _ = fs::remove_dir_all(&temp_root);
        stage_cb(UpdateStage::Failed(e.clone()));
        return Err(e);
    }

    // 2. Extract
    stage_cb(UpdateStage::Extracting("Extracting update archive...".into()));
    let extract_dir = temp_root.join("extracted");
    if let Err(e) = extract_archive(&download_file, &extract_dir) {
        let _ = fs::remove_dir_all(&temp_root);
        stage_cb(UpdateStage::Failed(format!("Extraction error: {e}")));
        return Err(format!("Extraction error: {e}"));
    }

    // 3. Apply to installation
    stage_cb(UpdateStage::Installing);
    let payload = match find_extracted_factorio_root(&extract_dir) {
        Ok(p) => p,
        Err(e) => {
            let _ = fs::remove_dir_all(&temp_root);
            stage_cb(UpdateStage::Failed(e.clone()));
            return Err(e);
        }
    };

    if let Err(e) = apply_update_to_installation(&payload, config) {
        let _ = fs::remove_dir_all(&temp_root);
        stage_cb(UpdateStage::Failed(format!("Installation error: {e}")));
        return Err(format!("Installation error: {e}"));
    }

    // 4. Clean up temporary files
    let _ = fs::remove_dir_all(&temp_root);

    // 5. Detect and return new installed version
    let updated_version = get_installed_version(config).unwrap_or_else(|| version.to_string());
    stage_cb(UpdateStage::Completed(updated_version.clone()));

    Ok(updated_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_download_url() {
        let url = construct_download_url(
            "2.0.10",
            BuildType::Alpha,
            Distro::Win64Manual,
            "test_user",
            "test_token_123",
        )
        .expect("Valid URL");

        assert_eq!(
            url.as_str(),
            "https://www.factorio.com/get-download/2.0.10/alpha/win64-manual?username=test_user&token=test_token_123"
        );
    }

    #[test]
    fn test_construct_download_url_expansion() {
        let url = construct_download_url(
            "latest",
            BuildType::Expansion,
            Distro::Osx,
            "space_engineer",
            "sec_tok",
        )
        .expect("Valid URL");

        assert_eq!(
            url.as_str(),
            "https://www.factorio.com/get-download/latest/expansion/osx?username=space_engineer&token=sec_tok"
        );
    }

    #[test]
    fn test_construct_download_url_empty_credentials() {
        assert!(construct_download_url("latest", BuildType::Alpha, Distro::Linux64, "", "token").is_err());
        assert!(construct_download_url("latest", BuildType::Alpha, Distro::Linux64, "user", "").is_err());
    }

    #[test]
    fn test_zip_extraction_and_copy() {
        let tmp = std::env::temp_dir().join(format!("test_zip_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let zip_file_path = tmp.join("test.zip");
        {
            let file = File::create(&zip_file_path).unwrap();
            let mut zip_writer = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();
            zip_writer.start_file("factorio/bin/test.txt", options).unwrap();
            std::io::Write::write_all(&mut zip_writer, b"hello factorio").unwrap();
            zip_writer.finish().unwrap();
        }

        assert_eq!(detect_archive_format(&zip_file_path).unwrap(), ArchiveFormat::Zip);

        let extract_dir = tmp.join("extract");
        extract_archive(&zip_file_path, &extract_dir).unwrap();

        assert_eq!(
            fs::read_to_string(extract_dir.join("factorio/bin/test.txt")).unwrap(),
            "hello factorio"
        );

        let target_dir = tmp.join("target");
        let payload = find_extracted_factorio_root(&extract_dir).unwrap();
        copy_and_overwrite_all(&payload, &target_dir).unwrap();

        assert_eq!(
            fs::read_to_string(target_dir.join("bin/test.txt")).unwrap(),
            "hello factorio"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_tar_gz_extraction() {
        let tmp = std::env::temp_dir().join(format!("test_targz_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let tar_gz_path = tmp.join("test.tar.gz");
        {
            let file = File::create(&tar_gz_path).unwrap();
            let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);

            let mut header = tar::Header::new_gnu();
            header.set_path("factorio/bin/factorio").unwrap();
            header.set_size(14);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append(&header, &b"dummy_factorio"[..]).unwrap();
            tar.finish().unwrap();
        }

        assert_eq!(detect_archive_format(&tar_gz_path).unwrap(), ArchiveFormat::TarGz);

        let extract_dir = tmp.join("extract");
        extract_archive(&tar_gz_path, &extract_dir).unwrap();

        assert_eq!(
            fs::read_to_string(extract_dir.join("factorio/bin/factorio")).unwrap(),
            "dummy_factorio"
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_tar_xz_extraction() {
        let tmp = std::env::temp_dir().join(format!("test_tarxz_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let tar_xz_path = tmp.join("test.tar.xz");
        {
            let file = File::create(&tar_xz_path).unwrap();
            let enc = xz2::write::XzEncoder::new(file, 6);
            let mut tar = tar::Builder::new(enc);

            let mut header = tar::Header::new_gnu();
            header.set_path("factorio/bin/factorio").unwrap();
            header.set_size(14);
            header.set_mode(0o755);
            header.set_cksum();
            tar.append(&header, &b"dummy_factorio"[..]).unwrap();
            tar.finish().unwrap();
        }

        assert_eq!(detect_archive_format(&tar_xz_path).unwrap(), ArchiveFormat::TarXz);

        let extract_dir = tmp.join("extract");
        extract_archive(&tar_xz_path, &extract_dir).unwrap();

        assert_eq!(
            fs::read_to_string(extract_dir.join("factorio/bin/factorio")).unwrap(),
            "dummy_factorio"
        );

        let _ = fs::remove_dir_all(&tmp);
    }
}
