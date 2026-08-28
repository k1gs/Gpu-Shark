//! Public GUI-to-runtime ABI client.
//!
//! The telemetry provider implementation is distributed as prebuilt runtime
//! DLLs. This module exposes only user-facing fields returned by the public ABI.

use libloading::{Library, Symbol};
use serde::Deserialize;
use std::ffi::CStr;
use std::fs;
use std::path::{Path, PathBuf};

mod embedded_public_runtime {
    include!(concat!(env!("OUT_DIR"), "/embedded_public_runtime.rs"));
}

#[derive(Deserialize, Debug, Clone)]
pub struct SysInfo {
    pub gpu_name: Option<String>,
    pub core: Option<f32>,
    pub hotspot: Option<f32>,
    pub vram_temp: Option<f32>,
    pub vram_used: Option<f32>,
    pub fan: Option<f32>,
    pub cpu_temp: Option<f32>,
    pub delta: Option<f32>,
    #[serde(default)]
    pub sensors: Vec<SensorReading>,
    #[serde(default)]
    pub perfcap_reason: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SensorReading {
    pub name: String,
    pub value: f32,
    pub unit: String,
}

type QueryFn = unsafe extern "C" fn(*mut u8, i32) -> i32;

pub struct DriverLibrary {
    library: Library,
    _native_provider: Library,
}

pub fn dll_library_path() -> &'static str {
    "gs.dll"
}

fn write_runtime_file(runtime_dir: &Path, name: &str, contents: &[u8]) -> Result<(), String> {
    let destination = runtime_dir.join(name);
    if fs::read(&destination).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }
    let temporary = runtime_dir.join(format!("{name}.new"));
    fs::write(&temporary, contents)
        .map_err(|error| format!("Cannot extract {}: {error}", temporary.display()))?;
    let previous = runtime_dir.join(format!("{name}.previous"));
    if previous.exists() {
        fs::remove_file(&previous)
            .map_err(|error| format!("Cannot prepare {}: {error}", previous.display()))?;
    }
    if destination.exists() {
        fs::rename(&destination, &previous).map_err(|error| {
            format!(
                "Cannot update {} (close every GPU Shark window and try again): {error}",
                destination.display()
            )
        })?;
    }
    fs::rename(&temporary, &destination).map_err(|error| {
        let _ = fs::rename(&previous, &destination);
        format!("Cannot activate {}: {error}", destination.display())
    })?;
    if previous.exists() {
        fs::remove_file(&previous)
            .map_err(|error| format!("Cannot finalize {}: {error}", previous.display()))?;
    }
    Ok(())
}

fn embedded_runtime_directory() -> Result<Option<PathBuf>, String> {
    use embedded_public_runtime::EMBEDDED_PUBLIC_RUNTIME;

    if EMBEDDED_PUBLIC_RUNTIME.is_empty() {
        return Ok(None);
    }
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA is unavailable for the standalone runtime".to_string())?;
    let runtime_dir = PathBuf::from(local_app_data)
        .join("GPU Shark")
        .join("runtime")
        .join(env!("CARGO_PKG_VERSION"));
    fs::create_dir_all(&runtime_dir)
        .map_err(|error| format!("Cannot create {}: {error}", runtime_dir.display()))?;
    for &(name, contents) in EMBEDDED_PUBLIC_RUNTIME {
        write_runtime_file(&runtime_dir, name, contents)?;
    }
    Ok(Some(runtime_dir))
}

pub fn load_driver_library() -> Result<DriverLibrary, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("Cannot locate executable: {error}"))?;
    let adjacent_library = executable
        .parent()
        .ok_or_else(|| "Executable has no parent directory".to_string())?
        .join(dll_library_path());
    let path = if adjacent_library.exists() {
        adjacent_library
    } else if let Some(runtime_dir) = embedded_runtime_directory()? {
        runtime_dir.join(dll_library_path())
    } else {
        adjacent_library
    };
    let native_provider_path = path.with_file_name("gsn.dll");
    let native_provider = unsafe { Library::new(&native_provider_path) }.map_err(|error| {
        format!(
            "Cannot load required component {}: {error}",
            native_provider_path.display()
        )
    })?;
    let library = unsafe { Library::new(&path) }
        .map_err(|error| format!("Cannot load {}: {error}", path.display()))?;
    Ok(DriverLibrary {
        library,
        _native_provider: native_provider,
    })
}

#[cfg(test)]
mod tests {
    use super::write_runtime_file;
    use std::fs;

    #[test]
    fn replaces_a_stale_embedded_runtime_file() {
        let directory =
            std::env::temp_dir().join(format!("gpu-shark-runtime-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("test runtime directory");
        fs::write(directory.join("gs.dll"), b"stale payload").expect("stale test payload");

        write_runtime_file(&directory, "gs.dll", b"fresh payload").expect("replace stale payload");

        assert_eq!(
            fs::read(directory.join("gs.dll")).unwrap(),
            b"fresh payload"
        );
        assert!(!directory.join("gs.dll.previous").exists());
        let _ = fs::remove_dir_all(&directory);
    }
}

pub fn fetch_data_from_dll(driver: &DriverLibrary) -> Result<SysInfo, Box<dyn std::error::Error>> {
    let library = &driver.library;
    unsafe {
        let query: Symbol<QueryFn> = library.get(b"q")?;
        let mut buffer = [0u8; 8192];
        let length = query(buffer.as_mut_ptr(), buffer.len() as i32);
        if length < 0 || length as usize >= buffer.len() {
            return Err("Telemetry response does not fit the public ABI buffer".into());
        }
        let json = CStr::from_bytes_until_nul(&buffer[..=length as usize])?.to_str()?;
        Ok(serde_json::from_str(json)?)
    }
}
