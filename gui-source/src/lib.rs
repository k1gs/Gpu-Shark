//! Public GUI-to-runtime ABI client.
//!
//! The telemetry provider implementation is distributed as prebuilt runtime
//! DLLs. This module exposes only user-facing fields returned by the public ABI.

use libloading::{Library, Symbol};
use serde::Deserialize;
use std::ffi::CStr;

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
    pub error: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SensorReading {
    pub name: String,
    pub value: f32,
    pub unit: String,
}

type QueryFn = unsafe extern "C" fn(*mut u8, i32) -> i32;

pub fn dll_library_path() -> &'static str {
    "gs.dll"
}

pub fn load_driver_library() -> Result<Library, String> {
    let executable =
        std::env::current_exe().map_err(|error| format!("Cannot locate executable: {error}"))?;
    let path = executable
        .parent()
        .ok_or_else(|| "Executable has no parent directory".to_string())?
        .join(dll_library_path());
    unsafe { Library::new(&path) }
        .map_err(|error| format!("Cannot load {}: {error}", path.display()))
}

pub fn fetch_data_from_dll(library: &Library) -> Result<SysInfo, Box<dyn std::error::Error>> {
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
