//! config.json im Datenordner %APPDATA%\Playtube - dasselbe Format wie die frühere
//! Python-Version, damit Einstellungen, gekoppelte Handys und das Login-Profil erhalten bleiben.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const APP_NAME: &str = "Playtube";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Discord {
    pub enabled: bool,
    pub client_id: String,
    pub update_interval_seconds: u64,
    pub show_idle_presence: bool,
}

impl Default for Discord {
    fn default() -> Self {
        Self {
            enabled: true,
            // Discord-Anwendung "Playtube" (öffentliche ID, kein Geheimnis).
            client_id: "1548023494976086127".into(),
            update_interval_seconds: 15,
            show_idle_presence: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Updates {
    pub enabled: bool,
    pub check_interval_hours: u64,
}

impl Default for Updates {
    fn default() -> Self {
        Self { enabled: true, check_interval_hours: 6 }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub youtube_output: String,
    pub music_output: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteControl {
    pub enabled: bool,
}

impl Default for RemoteControl {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairedDevice {
    pub name: String,
    pub token_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct LanRemote {
    pub enabled: bool,
    pub port: u16,
    pub devices: Vec<PairedDevice>,
}

impl Default for LanRemote {
    fn default() -> Self {
        Self { enabled: false, port: crate::lan::DEFAULT_PORT, devices: Vec::new() }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowSize {
    pub width: u32,
    pub height: u32,
}

impl Default for WindowSize {
    fn default() -> Self {
        Self { width: 1366, height: 860 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub app_name: String,
    pub discord: Discord,
    pub updates: Updates,
    pub audio: Audio,
    pub remote_control: RemoteControl,
    pub lan_remote: LanRemote,
    /// "youtube" oder "music"
    pub start_tab: String,
    /// "de", "en" oder leer (= Windows-Sprache)
    pub language: String,
    pub home_youtube: String,
    pub home_music: String,
    pub window: WindowSize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: APP_NAME.into(),
            discord: Discord::default(),
            updates: Updates::default(),
            audio: Audio::default(),
            remote_control: RemoteControl::default(),
            lan_remote: LanRemote::default(),
            start_tab: "youtube".into(),
            language: String::new(),
            home_youtube: "https://www.youtube.com/".into(),
            home_music: "https://music.youtube.com/".into(),
            window: WindowSize::default(),
        }
    }
}

/// %APPDATA%\Playtube (bzw. PLAYTUBE_DATA_DIR zum Testen).
pub fn data_dir() -> PathBuf {
    let dir = std::env::var_os("PLAYTUBE_DATA_DIR").map(PathBuf::from).unwrap_or_else(|| {
        let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
        base.join(APP_NAME)
    });
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// WebView2-Profil (Login, Cookies) - derselbe Ordner wie bei der Python-Version.
pub fn webview_dir() -> PathBuf {
    let dir = data_dir().join("webprofile").join("webview2");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str::<Config>(&text).ok()) {
        Some(mut config) => {
            config.discord.update_interval_seconds = config.discord.update_interval_seconds.clamp(15, 120);
            config.updates.check_interval_hours = config.updates.check_interval_hours.clamp(1, 168);
            if config.lan_remote.port == 0 {
                config.lan_remote.port = crate::lan::DEFAULT_PORT;
            }
            config
        }
        None => {
            let config = Config::default();
            save(&config);
            config
        }
    }
}

pub fn save(config: &Config) {
    if let Ok(text) = serde_json::to_string_pretty(config) {
        let path = config_path();
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_python_config_and_keeps_defaults() {
        let text = r#"{"discord": {"enabled": false}, "start_tab": "music",
            "lan_remote": {"enabled": true, "devices": [{"name": "Pixel", "token_sha256": "ab"}]},
            "unbekannt": 1}"#;
        let config: Config = serde_json::from_str(text).unwrap();
        assert!(!config.discord.enabled);
        assert_eq!(config.discord.client_id, "1548023494976086127");
        assert_eq!(config.start_tab, "music");
        assert_eq!(config.lan_remote.port, crate::lan::DEFAULT_PORT);
        assert_eq!(config.lan_remote.devices[0].name, "Pixel");
        assert!(config.updates.enabled);
    }
}
