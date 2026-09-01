use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const SETTINGS_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiLanguage {
    #[default]
    English,
    Russian,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentTheme {
    #[default]
    Green,
    Blue,
    Purple,
    Orange,
    Windows,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemperatureUnit {
    #[default]
    Celsius,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AppSettings {
    pub schema_version: u32,
    pub language: UiLanguage,
    pub theme: UiTheme,
    pub refresh_interval_ms: u64,
    pub accent: AccentTheme,
    pub temperature_unit: TemperatureUnit,
    pub track_all_maxima: bool,
    pub autostart: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            language: UiLanguage::English,
            theme: UiTheme::Light,
            refresh_interval_ms: 1_000,
            accent: AccentTheme::Green,
            temperature_unit: TemperatureUnit::Celsius,
            track_all_maxima: true,
            autostart: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != SETTINGS_SCHEMA_VERSION {
            return Err(format!(
                "unsupported settings schema version {}",
                self.schema_version
            ));
        }
        if !matches!(self.refresh_interval_ms, 500 | 1_000 | 2_000) {
            return Err(format!(
                "unsupported refresh interval {} ms",
                self.refresh_interval_ms
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct LoadOutcome {
    pub settings: AppSettings,
    pub warning: Option<String>,
}

pub fn settings_path() -> Result<PathBuf, String> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "LOCALAPPDATA is unavailable; defaults are active".to_string())?;
    Ok(PathBuf::from(local_app_data)
        .join("GPU Shark")
        .join("settings.json"))
}

pub fn load() -> LoadOutcome {
    match settings_path() {
        Ok(path) => load_from(&path),
        Err(warning) => LoadOutcome {
            settings: AppSettings::default(),
            warning: Some(warning),
        },
    }
}

pub fn save(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    save_to(&path, settings)
}

fn load_from(path: &Path) -> LoadOutcome {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return LoadOutcome {
                settings: AppSettings::default(),
                warning: None,
            };
        }
        Err(error) => {
            return recovered(format!("Could not read settings: {error}"));
        }
    };
    match serde_json::from_slice::<AppSettings>(&bytes) {
        Ok(settings) => match settings.validate() {
            Ok(()) => LoadOutcome {
                settings,
                warning: None,
            },
            Err(error) => recovered(format!("Invalid settings were ignored: {error}")),
        },
        Err(error) => recovered(format!("Malformed settings were ignored: {error}")),
    }
}

fn recovered(warning: String) -> LoadOutcome {
    LoadOutcome {
        settings: AppSettings::default(),
        warning: Some(warning),
    }
}

fn save_to(path: &Path, settings: &AppSettings) -> Result<(), String> {
    settings.validate()?;
    let parent = path
        .parent()
        .ok_or_else(|| "settings path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    let json = serde_json::to_vec_pretty(settings)
        .map_err(|error| format!("Could not serialize settings: {error}"))?;
    let temporary = path.with_extension("json.new");
    let previous = path.with_extension("json.previous");
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|error| format!("Could not reset {}: {error}", temporary.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| {
            format!(
                "Could not open temporary settings file {}: {error}",
                temporary.display()
            )
        })?;
    file.write_all(&json).map_err(|error| {
        format!(
            "Could not write temporary settings file {}: {error}",
            temporary.display()
        )
    })?;
    file.sync_all().map_err(|error| {
        format!(
            "Could not flush temporary settings file {}: {error}",
            temporary.display()
        )
    })?;
    drop(file);
    if previous.exists() {
        fs::remove_file(&previous)
            .map_err(|error| format!("Could not reset {}: {error}", previous.display()))?;
    }
    if path.exists() {
        fs::rename(path, &previous)
            .map_err(|error| format!("Could not prepare settings update: {error}"))?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::rename(&previous, path);
        return Err(format!("Could not activate settings: {error}"));
    }
    if previous.exists() {
        fs::remove_file(&previous)
            .map_err(|error| format!("Could not finalize settings: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn directory(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpu-shark-settings-{name}-{}", std::process::id()))
    }

    #[test]
    fn missing_settings_use_defaults_without_warning() {
        let directory = directory("missing");
        let _ = fs::remove_dir_all(&directory);
        let outcome = load_from(&directory.join("settings.json"));
        assert_eq!(outcome.settings, AppSettings::default());
        assert_eq!(outcome.settings.language, UiLanguage::English);
        assert!(outcome.warning.is_none());
    }

    #[test]
    fn settings_round_trip_and_replace_atomically() {
        let directory = directory("roundtrip");
        let _ = fs::remove_dir_all(&directory);
        let path = directory.join("settings.json");
        let settings = AppSettings {
            language: UiLanguage::English,
            refresh_interval_ms: 500,
            accent: AccentTheme::Purple,
            ..AppSettings::default()
        };
        save_to(&path, &AppSettings::default()).expect("initial settings");
        save_to(&path, &settings).expect("replacement settings");
        let loaded = load_from(&path);
        assert_eq!(loaded.settings, settings);
        assert!(loaded.warning.is_none());
        assert!(!path.with_extension("json.new").exists());
        assert!(!path.with_extension("json.previous").exists());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn malformed_or_invalid_settings_recover_to_defaults() {
        let directory = directory("recovery");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("test settings directory");
        let path = directory.join("settings.json");
        fs::write(&path, b"{not-json").expect("malformed fixture");
        let malformed = load_from(&path);
        assert_eq!(malformed.settings, AppSettings::default());
        assert!(malformed.warning.is_some());

        fs::write(&path, br#"{"schema_version":1,"refresh_interval_ms":250}"#)
            .expect("invalid fixture");
        let invalid = load_from(&path);
        assert_eq!(invalid.settings, AppSettings::default());
        assert!(invalid.warning.is_some());
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn unknown_fields_are_forward_compatible() {
        let directory = directory("unknown-field");
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("test settings directory");
        let path = directory.join("settings.json");
        fs::write(&path, br#"{"schema_version":1,"future_option":true}"#).expect("future fixture");
        let outcome = load_from(&path);
        assert_eq!(outcome.settings, AppSettings::default());
        assert!(outcome.warning.is_none());
        let _ = fs::remove_dir_all(directory);
    }
}
