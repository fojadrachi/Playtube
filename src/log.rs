//! Einfaches Diagnose-Log in %APPDATA%\Playtube\logs\playtube.log (die App hat kein
//! Konsolenfenster). Wird ab 1 MB neu begonnen.

use std::io::Write;

pub fn line(text: &str) {
    let dir = crate::config::data_dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("playtube.log");
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(file, "[{secs}] {text}");
    }
}
