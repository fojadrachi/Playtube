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
    pub update_interval_seconds: u64,
    pub show_idle_presence: bool,
}

impl Default for Discord {
    fn default() -> Self {
        Self {
            enabled: true,
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
    /// Kleine Updates (gleiche Hauptversion) still laden und direkt einspielen.
    pub auto_install: bool,
}

impl Default for Updates {
    fn default() -> Self {
        Self { enabled: true, check_interval_hours: 6, auto_install: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Audio {
    pub youtube_output: String,
    pub music_output: String,
    /// Tonausgabe "Für Discord-Streams": Ton über Playtube.exe statt über die Edge-Engine
    /// (Discord-Fenster-Streams, Sonar zeigt "Playtube"). Standard: aus = normale Tonausgabe.
    pub stream_mode: bool,
}

impl Default for Audio {
    fn default() -> Self {
        Self { youtube_output: String::new(), music_output: String::new(), stream_mode: false }
    }
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

/// Playtube Jam: gemeinsam hören über den Jam-Server (siehe src/jam.rs).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Jam {
    /// Anzeigename im Jam (leer = Windows-Benutzername)
    pub name: String,
    pub server: String,
    /// Zuletzt besuchter Jam: mit dem Token geht es nach einem Neustart als derselbe
    /// Teilnehmer (bzw. Host) weiter.
    pub last_code: String,
    pub last_token: String,
}

impl Default for Jam {
    fn default() -> Self {
        Self { name: String::new(), server: crate::jam::DEFAULT_SERVER.into(), last_code: String::new(), last_token: String::new() }
    }
}

/// Diagnosedaten (src/diag.rs): Standard an, abschaltbar in den Einstellungen.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Diagnostics {
    pub enabled: bool,
    /// Zufällige Kennung dieser Installation (wird beim ersten Senden erzeugt)
    pub install_id: String,
    /// Zeitstempel (Unix-Sekunden) der letzten gesendeten Logzeile
    pub last_sent: u64,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self { enabled: true, install_id: String::new(), last_sent: 0 }
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
    pub jam: Jam,
    pub diagnostics: Diagnostics,
    /// YouTube Music in der Playtube-Ansicht (Spotify-Stil) statt der Original-Seite
    pub music_view: bool,
    /// "youtube" oder "music"
    pub start_tab: String,
    /// Mit Windows starten
    pub autostart: bool,
    /// Beim Autostart nur im Tray starten (kein Fenster)
    pub autostart_tray: bool,
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
            jam: Jam::default(),
            diagnostics: Diagnostics::default(),
            music_view: true,
            start_tab: "youtube".into(),
            autostart: false,
            autostart_tray: true,
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
            if config.jam.server.trim().is_empty() {
                config.jam.server = crate::jam::DEFAULT_SERVER.into();
            }
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
        assert_eq!(config.start_tab, "music");
        assert_eq!(config.lan_remote.port, crate::lan::DEFAULT_PORT);
        assert_eq!(config.lan_remote.devices[0].name, "Pixel");
        assert!(config.updates.enabled);
        // Diagnose ist Opt-out: ohne Eintrag an.
        assert!(config.diagnostics.enabled);
        assert!(config.diagnostics.install_id.is_empty());
    }
}
