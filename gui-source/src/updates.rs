//! Release update discovery against the public GitHub repository.
//!
//! The check performs a single anonymous HTTPS request carrying only the
//! application user agent. No machine data, identifiers or telemetry leave
//! the device, and nothing is downloaded or installed automatically.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::ffi::c_void;
use std::fs;
use std::io::{Read, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use windows_sys::Win32::Networking::WinHttp::{
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER,
    WINHTTP_QUERY_STATUS_CODE, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};

const API_HOST: &str = "api.github.com";
const API_PATH: &str = "/repos/k1gs/Gpu-Shark/releases/latest";
const MAX_RESPONSE: usize = 1024 * 1024;
const MAX_PACKAGE: u64 = 64 * 1024 * 1024;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PACKAGE_NAME: &str = "GPU-Shark-win-x64.zip";

fn wstr(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

#[derive(Clone, Debug)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate,
    Available { latest: String, url: String },
    Downloading,
    ReadyToInstall { exe_path: String, url: String },
    InstallFailed { detail: String, url: String },
    Failed(String),
}

#[derive(Deserialize)]
pub struct LatestRelease {
    pub tag_name: String,
    pub html_url: String,
    #[serde(default)]
    pub assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct PreparedUpdate {
    pub exe_path: PathBuf,
    pub page_url: String,
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn core_and_prerelease(version: &str) -> (Vec<u64>, Option<&str>) {
    let version = version.trim().trim_start_matches('v');
    match version.split_once('-') {
        Some((core, prerelease)) => (numeric_core(core), Some(prerelease)),
        None => (numeric_core(version), None),
    }
}

fn numeric_core(core: &str) -> Vec<u64> {
    core.split('.')
        .map(|part| part.trim().parse::<u64>().unwrap_or(0))
        .collect()
}

fn compare_cores(candidate: &[u64], current: &[u64]) -> Ordering {
    let length = candidate.len().max(current.len());
    for index in 0..length {
        let left = candidate.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left.cmp(&right);
        }
    }
    Ordering::Equal
}

/// True when `candidate` is strictly newer than `current`. A release without
/// a pre-release identifier outranks one with it, and `beta.N` identifiers
/// compare numerically.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    let (candidate_core, candidate_pre) = core_and_prerelease(candidate);
    let (current_core, current_pre) = core_and_prerelease(current);
    match compare_cores(&candidate_core, &current_core) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match (candidate_pre, current_pre) {
            (None, None) => false,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(candidate), Some(current)) => prerelease_newer(candidate, current),
        },
    }
}

fn prerelease_newer(candidate: &str, current: &str) -> bool {
    let candidate = split_prerelease(candidate);
    let current = split_prerelease(current);
    for index in 0..candidate.len().max(current.len()) {
        let left = candidate.get(index);
        let right = current.get(index);
        match (left, right) {
            (None, None) => continue,
            (Some(_), None) => return false,
            (None, Some(_)) => return true,
            (Some(left), Some(right)) => {
                let left_number = left.parse::<u64>();
                let right_number = right.parse::<u64>();
                match (left_number, right_number) {
                    (Ok(left), Ok(right)) => {
                        if left != right {
                            return left > right;
                        }
                    }
                    _ => {
                        if left != right {
                            return left > right;
                        }
                    }
                }
            }
        }
    }
    false
}

fn split_prerelease(value: &str) -> Vec<&str> {
    value
        .trim()
        .split(|character: char| character == '.' || character == '_')
        .filter(|part| !part.is_empty())
        .collect()
}

pub fn fetch_latest() -> Result<LatestRelease, String> {
    let body = request(API_HOST, API_PATH, "application/vnd.github+json", 12_000)?;
    serde_json::from_slice::<LatestRelease>(&body)
        .map_err(|error| format!("Invalid update response: {error}"))
}

struct SessionGuard(*mut c_void);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

fn find_package(release: &LatestRelease) -> Option<(&ReleaseAsset, &ReleaseAsset)> {
    let package = release
        .assets
        .iter()
        .find(|asset| asset.name == PACKAGE_NAME)
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.ends_with(".zip"))
        })?;
    let sums = release
        .assets
        .iter()
        .find(|asset| asset.name.eq_ignore_ascii_case("SHA256SUMS.txt"))
        .or_else(|| {
            release
                .assets
                .iter()
                .find(|asset| asset.name.to_ascii_uppercase().contains("SHA256"))
        })?;
    Some((package, sums))
}

fn request(
    url_host: &str,
    url_path: &str,
    accept: &str,
    receive_timeout_ms: u32,
) -> Result<Vec<u8>, String> {
    let agent = wstr(&format!("GPU-Shark/{}", current_version()));
    let host = wstr(url_host);
    let verb = wstr("GET");
    let path = wstr(url_path);
    let headers = wstr(&format!(
        "User-Agent: GPU-Shark update check\r\nAccept: {accept}\r\n"
    ));
    unsafe {
        let session = WinHttpOpen(
            agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err("WinHTTP session failed".into());
        }
        let _guard = SessionGuard(session);
        if WinHttpSetTimeouts(session, 8_000, 8_000, 8_000, receive_timeout_ms as i32) == 0 {
            return Err("WinHTTP timeouts failed".into());
        }
        let connection = WinHttpConnect(session, host.as_ptr(), 443, 0);
        if connection.is_null() {
            return Err("HTTPS connection failed".into());
        }
        let request = WinHttpOpenRequest(
            connection,
            verb.as_ptr(),
            path.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            return Err("HTTPS request failed".into());
        }
        if WinHttpSendRequest(
            request,
            headers.as_ptr(),
            (headers.len() - 1) as u32,
            std::ptr::null(),
            0,
            0,
            0,
        ) == 0
        {
            return Err("Could not send request".into());
        }
        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            return Err("Could not receive response".into());
        }
        let mut status = 0u32;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        if WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            (&mut status as *mut u32).cast(),
            &mut status_size,
            std::ptr::null_mut(),
        ) == 0
        {
            return Err("Invalid response status".into());
        }
        if status != 200 {
            return Err(format!("Request returned HTTP {status}"));
        }
        let mut response = Vec::new();
        let mut limit = MAX_RESPONSE.max(MAX_PACKAGE as usize);
        loop {
            let mut chunk = [0u8; 65536];
            let mut read = 0u32;
            if WinHttpReadData(
                request,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err("Could not read response".into());
            }
            if read == 0 {
                break;
            }
            response.extend_from_slice(&chunk[..read as usize]);
            if response.len() > limit {
                return Err("Response is too large".into());
            }
            limit -= read as usize;
        }
        Ok(response)
    }
}

/// Returns `(host, path)` for an https URL. Only https URLs are accepted so a
/// crafted release asset cannot downgrade the transport.
fn split_https_url(url: &str) -> Result<(String, String), String> {
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "Only https URLs are supported".to_string())?;
    match rest.split_once('/') {
        Some((host, path)) => Ok((host.to_owned(), format!("/{path}"))),
        None => Ok((rest.to_owned(), "/".to_owned())),
    }
}

pub fn download_file(url: &str, destination: &Path) -> Result<(), String> {
    let (host, path) = split_https_url(url)?;
    let body = request(&host, &path, "application/octet-stream", 60_000)?;
    if body.len() as u64 > MAX_PACKAGE {
        return Err("Package is too large".into());
    }
    let mut file = fs::File::create(destination)
        .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
    file.write_all(&body)
        .map_err(|error| format!("Could not write {}: {error}", destination.display()))?;
    file.sync_all()
        .map_err(|error| format!("Could not flush {}: {error}", destination.display()))?;
    Ok(())
}

fn expected_sha256(sums: &[u8], package_name: &str) -> Result<String, String> {
    let text = String::from_utf8_lossy(sums);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        let Some(hash) = parts.next() else {
            continue;
        };
        let Some(name) = parts.next() else {
            continue;
        };
        if name == package_name && hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(hash.to_ascii_uppercase());
        }
    }
    Err(format!("SHA256SUMS has no entry for {package_name}"))
}

pub fn verify_sha256(file: &Path, expected: &str) -> Result<(), String> {
    let mut hasher = Sha256::new();
    let mut source = fs::File::open(file)
        .map_err(|error| format!("Could not open {}: {error}", file.display()))?;
    let mut buffer = [0u8; 65536];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("Could not read {}: {error}", file.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:X}", hasher.finalize());
    if actual != expected.to_ascii_uppercase() {
        return Err(format!(
            "Checksum mismatch: expected {expected}, got {actual}"
        ));
    }
    Ok(())
}

fn extract_zip(zip: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination)
        .map_err(|error| format!("Could not create {}: {error}", destination.display()))?;
    let command = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        zip.display(),
        destination.display()
    );
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &command,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Could not run extraction: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Extraction failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn find_exe(directory: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("Could not list {}: {error}", directory.display()))?;
    let mut subdirectories = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("Could not list {}: {error}", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            subdirectories.push(path);
        } else if path.file_name().is_some_and(|name| {
            name.eq_ignore_ascii_case("GPU-Shark.exe")
                || name.eq_ignore_ascii_case("gpu-shark-gui.exe")
        }) {
            return Ok(path);
        }
    }
    for subdirectory in subdirectories {
        if let Ok(path) = find_exe(&subdirectory) {
            return Ok(path);
        }
    }
    Err(format!(
        "Package has no GPU-Shark.exe in {}",
        directory.display()
    ))
}

/// Downloads the newest verified release package and unpacks it locally.
/// The package is accepted only when its SHA-256 matches SHA256SUMS.txt.
pub fn prepare_install(update_root: &Path) -> Result<PreparedUpdate, String> {
    let release = fetch_latest()?;
    let (package, sums) =
        find_package(&release).ok_or_else(|| "The release has no verifiable package".to_owned())?;
    let tag = release.tag_name.trim().trim_start_matches('v');
    let directory = update_root.join(tag);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let zip_path = directory.join(&package.name);
    let sums_path = directory.join(&sums.name);
    download_file(&package.browser_download_url, &zip_path)?;
    download_file(&sums.browser_download_url, &sums_path)?;
    let sums_body = fs::read(&sums_path)
        .map_err(|error| format!("Could not read {}: {error}", sums_path.display()))?;
    let expected = expected_sha256(&sums_body, &package.name)?;
    verify_sha256(&zip_path, &expected)?;
    let extract_dir = directory.join("pkg");
    extract_zip(&zip_path, &extract_dir)?;
    let exe_path = find_exe(&extract_dir)?;
    Ok(PreparedUpdate {
        exe_path,
        page_url: release.html_url,
    })
}

/// Replaces the running executable with the prepared one. The previous file
/// is kept as `*.exe.old` until the next successful launch so a failed copy
/// can be rolled back.
pub fn apply_prepared(new_exe: &Path) -> Result<(), String> {
    let current =
        std::env::current_exe().map_err(|error| format!("Cannot locate executable: {error}"))?;
    let backup = current.with_extension("exe.old");
    let _ = fs::remove_file(&backup);
    fs::rename(&current, &backup)
        .map_err(|error| format!("Could not back up the current executable: {error}"))?;
    if let Err(error) = fs::copy(new_exe, &current) {
        let _ = fs::rename(&backup, &current);
        return Err(format!("Could not place the update: {error}"));
    }
    Ok(())
}

pub fn cleanup_old_install() {
    if let Ok(current) = std::env::current_exe() {
        let _ = fs::remove_file(current.with_extension("exe.old"));
    }
}

pub fn update_root() -> Result<PathBuf, String> {
    let local =
        std::env::var_os("LOCALAPPDATA").ok_or_else(|| "LOCALAPPDATA is unavailable".to_owned())?;
    Ok(PathBuf::from(local).join("GPU Shark").join("updates"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_semver_is_detected() {
        assert!(is_newer("0.2.5", "0.2.4"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(is_newer("0.3.0-beta.1", "0.2.4-beta.9"));
        assert!(!is_newer("0.2.4-beta.2", "0.2.5"));
    }

    #[test]
    fn stable_releases_outrank_prereleases() {
        assert!(is_newer("0.2.4", "0.2.4-beta.2"));
        assert!(!is_newer("0.2.4-beta.1", "0.2.4"));
    }

    #[test]
    fn beta_identifiers_compare_numerically() {
        assert!(is_newer("0.2.4-beta.2", "0.2.4-beta.1"));
        assert!(is_newer("0.2.4-beta.10", "0.2.4-beta.9"));
        assert!(!is_newer("0.2.4-beta.2", "0.2.4-beta.10"));
        assert!(is_newer("0.2.4-rc.1", "0.2.4-beta.9"));
    }

    #[test]
    fn equal_versions_and_garbage_do_not_report_updates() {
        assert!(!is_newer("0.2.4-beta.1", "v0.2.4-beta.1"));
        assert!(!is_newer("0.2.4", "0.2.4"));
        assert!(!is_newer("", "0.2.4"));
        assert!(!is_newer("garbage", "garbage"));
    }

    #[test]
    fn current_version_is_exposed() {
        assert!(!current_version().is_empty());
    }
}
