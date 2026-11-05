//! App-Updates wie beim NNN Launcher: neuestes GitHub-Release prüfen, Setup laden, SHA-256 prüfen,
//! still installieren. Das Setup beendet Playtube, installiert und startet es danach neu.

use crate::config::VERSION;
use anyhow::{bail, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;

const LATEST: &str = "https://api.github.com/repos/fojadrachi/Playtube/releases/latest";
const SETUP_PREFIX: &str = "Playtube-Setup-";
pub const RELEASES_PAGE: &str = "https://github.com/fojadrachi/Playtube/releases";

#[derive(Clone, Debug, Default)]
pub struct UpdateInfo {
    pub available: bool,
    pub latest: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
}

pub fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut it = v.trim_start_matches(['v', 'V']).split(['.', '-', '+']).map(|p| p.parse().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(format!("Playtube/{VERSION}"))
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default()
}

/// Wertet die Antwort von /releases/latest aus.
pub fn from_release(release: &serde_json::Value, current: &str) -> UpdateInfo {
    let tag = release["tag_name"].as_str().unwrap_or_default();
    let asset = release["assets"].as_array().into_iter().flatten().find(|a| {
        let name = a["name"].as_str().unwrap_or_default();
        name.starts_with(SETUP_PREFIX) && name.to_lowercase().ends_with(".exe")
    });
    let newer = parse_version(tag) > parse_version(current);
    UpdateInfo {
        available: newer && asset.is_some(),
        latest: tag.trim_start_matches(['v', 'V']).to_string(),
        url: asset.and_then(|a| a["browser_download_url"].as_str()).unwrap_or_default().to_string(),
        size: asset.and_then(|a| a["size"].as_u64()).unwrap_or(0),
        sha256: asset
            .and_then(|a| a["digest"].as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_lowercase),
    }
}

pub async fn check() -> Result<UpdateInfo> {
    let res = client().get(LATEST).header("Accept", "application/vnd.github+json").send().await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo::default());
    }
    let release: serde_json::Value = res.error_for_status()?.json().await?;
    Ok(from_release(&release, VERSION))
}

/// Lädt das Setup (Fortschritt 0-100 über `progress`), prüft es und startet die stille
/// Installation. Danach muss sich die App beenden.
pub async fn install(info: &UpdateInfo, progress: impl Fn(i32)) -> Result<()> {
    if !info.available || !info.url.starts_with("https://github.com/") {
        bail!("Kein gültiges Update verfügbar");
    }
    let dir = crate::config::data_dir().join("updates");
    std::fs::create_dir_all(&dir)?;
    // Reste früherer Updates aufräumen.
    for entry in std::fs::read_dir(&dir)?.flatten() {
        let _ = std::fs::remove_file(entry.path());
    }
    let file = dir.join(format!("{SETUP_PREFIX}{}.exe", info.latest));
    let res = client().get(&info.url).send().await?.error_for_status()?;
    let total = res.content_length().unwrap_or(info.size).max(1);
    let mut stream = res.bytes_stream();
    let mut out = std::fs::File::create(&file)?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.write_all(&chunk)?;
        hasher.update(&chunk);
        done += chunk.len() as u64;
        progress(((done * 100) / total).min(100) as i32);
    }
    out.flush()?;
    drop(out);
    if let Some(expected) = &info.sha256 {
        if &hex::encode(hasher.finalize()) != expected {
            let _ = std::fs::remove_file(&file);
            bail!("Prüfsumme des Updates stimmt nicht – abgebrochen.");
        }
    }
    std::process::Command::new(&file)
        .args(["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/CLOSEAPPLICATIONS"])
        .spawn()?;
    Ok(())
}

/// Per Setup installiert (in %LOCALAPPDATA%\Programs) - nur dann kann sich die App selbst
/// aktualisieren.
pub fn is_installed() -> bool {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default().to_lowercase();
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_lowercase().starts_with(&format!("{local}\\programs")))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn compares_versions() {
        assert!(parse_version("v2.0.0") > parse_version("1.0.0"));
        assert!(parse_version("2.10.0") > parse_version("2.9.9"));
        assert_eq!(parse_version("v1.0.0-edge"), (1, 0, 0));
    }

    #[test]
    fn picks_setup_asset_and_digest() {
        let release = json!({
            "tag_name": "v2.1.0", "body": "Neu",
            "assets": [
                {"name": "Playtube-v2.1.0-win64.zip", "browser_download_url": "https://github.com/x/zip", "size": 1},
                {"name": "Playtube-Setup-2.1.0.exe", "browser_download_url": "https://github.com/x/setup", "size": 9,
                 "digest": "sha256:ABCD"}
            ]
        });
        let info = from_release(&release, "2.0.0");
        assert!(info.available);
        assert_eq!(info.latest, "2.1.0");
        assert_eq!(info.url, "https://github.com/x/setup");
        assert_eq!(info.sha256.as_deref(), Some("abcd"));
        assert!(!from_release(&release, "2.1.0").available);
        assert!(!from_release(&json!({"tag_name": "v9.0.0", "assets": []}), "2.0.0").available);
    }
}
