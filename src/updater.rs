//! App-Updates wie beim NNN Launcher: neuestes GitHub-Release prüfen, Setup laden, SHA-256 prüfen,
//! still installieren. Das Setup beendet Playtube, installiert und startet es danach neu.
//!
//! Kleine Updates (gleiche Hauptversion) gehen ohne Setup: das Release enthält auch die nackte
//! `Playtube-<Version>-win64.exe`. Sie wird im Hintergrund geladen, geprüft und neben die laufende
//! Datei gelegt; Windows erlaubt, eine laufende .exe umzubenennen. Playtube läuft ungestört weiter,
//! die neue Version gilt ab dem nächsten Start (oder sofort über „Jetzt neu starten“).

use crate::config::VERSION;
use anyhow::{bail, Result};
use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use std::io::Write;

const LATEST: &str = "https://api.github.com/repos/fojadrachi/Playtube/releases/latest";
const SETUP_PREFIX: &str = "Playtube-Setup-";
const EXE_SUFFIX: &str = "-win64.exe";
/// Registry-Schlüssel des Setups (AppId aus installer/Playtube.iss) – für die angezeigte Version.
const UNINSTALL_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{87FBA502-2B84-4C42-A66B-2F03F490912D}_is1";
pub const RELEASES_PAGE: &str = "https://github.com/fojadrachi/Playtube/releases";

#[derive(Clone, Debug, Default)]
pub struct UpdateInfo {
    pub available: bool,
    pub latest: String,
    pub url: String,
    pub size: u64,
    pub sha256: Option<String>,
    /// Nackte Programmdatei für Updates ohne Setup (leer, wenn das Release keine hat).
    pub exe_url: String,
    pub exe_sha256: Option<String>,
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
    let exe = release["assets"].as_array().into_iter().flatten().find(|a| {
        let name = a["name"].as_str().unwrap_or_default();
        name.starts_with("Playtube-") && !name.starts_with(SETUP_PREFIX) && name.ends_with(EXE_SUFFIX)
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
        exe_url: exe.and_then(|a| a["browser_download_url"].as_str()).unwrap_or_default().to_string(),
        exe_sha256: exe
            .and_then(|a| a["digest"].as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .map(str::to_lowercase),
    }
}

/// Kleines Update: gleiche Hauptversion und eine geprüfte Programmdatei im Release.
pub fn is_minor(info: &UpdateInfo, current: &str) -> bool {
    info.available
        && parse_version(&info.latest).0 == parse_version(current).0
        && info.exe_url.starts_with("https://github.com/")
        && info.exe_sha256.is_some()
}

/// Lädt `url` nach `file`, prüft SHA-256 (falls bekannt) und meldet den Fortschritt (0-100).
async fn download(url: &str, file: &std::path::Path, size: u64, sha256: Option<&str>, progress: impl Fn(i32)) -> Result<()> {
    let res = client().get(url).send().await?.error_for_status()?;
    let total = res.content_length().unwrap_or(size).max(1);
    let mut stream = res.bytes_stream();
    let mut out = std::fs::File::create(file)?;
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
    if let Some(expected) = sha256 {
        if hex::encode(hasher.finalize()) != expected {
            let _ = std::fs::remove_file(file);
            bail!("Prüfsumme des Updates stimmt nicht – abgebrochen.");
        }
    }
    Ok(())
}

/// Kleines Update einspielen, während Playtube läuft: neue Datei laden und gegen die laufende
/// tauschen. Gilt ab dem nächsten Start.
pub async fn install_in_place(info: &UpdateInfo) -> Result<()> {
    if !is_minor(info, VERSION) {
        bail!("Kein kleines Update");
    }
    let current = std::env::current_exe()?;
    let new = current.with_extension("exe.new");
    download(&info.exe_url, &new, 0, info.exe_sha256.as_deref(), |_| {}).await?;
    swap_in(&current, &new)?;
    set_registered_version(&info.latest);
    Ok(())
}

/// Tauscht `current` (darf laufen) gegen `new`: alte Datei wird zu `<name>.old`, neue an ihre Stelle.
/// Geht der zweite Schritt schief, kommt die alte Datei zurück.
pub fn swap_in(current: &std::path::Path, new: &std::path::Path) -> Result<()> {
    let old = current.with_extension("exe.old");
    let _ = std::fs::remove_file(&old);
    std::fs::rename(current, &old)?;
    if let Err(e) = std::fs::rename(new, current) {
        let _ = std::fs::rename(&old, current);
        let _ = std::fs::remove_file(new);
        return Err(e.into());
    }
    Ok(())
}

/// Reste eines Updates aufräumen (die alte Datei läuft nach dem Neustart nicht mehr).
pub fn cleanup() {
    if let Ok(current) = std::env::current_exe() {
        let _ = std::fs::remove_file(current.with_extension("exe.old"));
        let _ = std::fs::remove_file(current.with_extension("exe.new"));
    }
}

/// Version in „Apps & Features“ nachziehen (sonst stünde dort weiter die alte).
fn set_registered_version(version: &str) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let _ = std::process::Command::new("reg")
        .args(["add", UNINSTALL_KEY, "/v", "DisplayVersion", "/t", "REG_SZ", "/d", version, "/f"])
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

/// Neu starten: neue Instanz, die wartet, bis diese beendet ist (sonst gäbe sie an uns ab).
pub fn relaunch() -> Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new(exe).args(["--wait-pid", &std::process::id().to_string()]).spawn()?;
    Ok(())
}

/// `--wait-pid <pid>` aus den Startargumenten: höchstens 15 s warten, bis der Prozess weg ist.
pub fn wait_for_previous(args: &[String]) {
    let Some(pid) = args.iter().position(|a| a == "--wait-pid").and_then(|i| args.get(i + 1)).and_then(|p| p.parse::<u32>().ok()) else {
        return;
    };
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            WaitForSingleObject(handle, 15_000);
            let _ = CloseHandle(handle);
        }
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
    download(&info.url, &file, info.size, info.sha256.as_deref(), progress).await?;
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
    fn finds_the_plain_exe_for_minor_updates() {
        let release = serde_json::json!({
            "tag_name": "v2.8.0",
            "assets": [
                {"name": "Playtube-Setup-2.8.0.exe", "browser_download_url": "https://github.com/x/Setup.exe", "size": 9, "digest": "sha256:AA"},
                {"name": "Playtube-2.8.0-win64.exe", "browser_download_url": "https://github.com/x/Playtube.exe", "size": 8, "digest": "sha256:BB"}
            ]
        });
        let info = from_release(&release, "2.7.0");
        assert_eq!(info.exe_url, "https://github.com/x/Playtube.exe");
        assert_eq!(info.exe_sha256.as_deref(), Some("bb"));
        assert!(is_minor(&info, "2.7.0"));
        // Neue Hauptversion: wie bisher über das Setup.
        assert!(!is_minor(&from_release(&serde_json::json!({"tag_name": "v3.0.0", "assets": release["assets"]}), "2.7.0"), "2.7.0"));
        // Ohne Programmdatei im Release: Setup.
        let only_setup = serde_json::json!({"tag_name": "v2.8.0", "assets": [release["assets"][0]]});
        assert!(!is_minor(&from_release(&only_setup, "2.7.0"), "2.7.0"));
    }

    #[test]
    fn swaps_a_file_that_is_in_use() {
        let dir = std::env::temp_dir().join(format!("playtube-swap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let current = dir.join("Playtube.exe");
        let new = dir.join("Playtube.exe.new");
        std::fs::write(&current, b"alt").unwrap();
        std::fs::write(&new, b"neu").unwrap();
        // Offen halten wie eine laufende .exe (Lesen/Löschen-Freigabe wie beim Programmstart).
        let _open = std::fs::File::open(&current).unwrap();
        swap_in(&current, &new).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"neu");
        assert_eq!(std::fs::read(dir.join("Playtube.exe.old")).unwrap(), b"alt");
        assert!(!new.exists());
        drop(_open);
        let _ = std::fs::remove_dir_all(&dir);
    }

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
