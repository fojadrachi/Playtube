//! Diagnosedaten: neue Zeilen aus dem eigenen Log (src/log.rs) plus Version und Plattform an den
//! Playtube-Server (`POST /api/diag`, siehe playtube-jam) schicken, damit Fehler gefunden und
//! behoben werden können. Standardmäßig an, abschaltbar unter Einstellungen › Diagnose.
//!
//! Nicht gesendet: Cookies, Tokens, Anmeldedaten, Suchanfragen, Songtitel, Video-IDs, der
//! Windows-Benutzername und URL-Parameter. Die Debug-Zeilen (`music-debug`, `jam-debug`)
//! bleiben ganz weg. Der Server filtert ein zweites Mal.

use rand::RngExt;
use serde_json::json;

/// Höchstens so viele Zeilen pro Bericht (der Server nimmt 400).
const MAX_LINES: usize = 300;
const MAX_LINE_CHARS: usize = 480;

/// Neue zufällige Kennung dieser Installation (kein Bezug zu Konto oder Gerät).
pub fn new_install_id() -> String {
    hex::encode(rand::rng().random::<[u8; 12]>())
}

/// Alles entfernen, was nicht in einen Bericht gehört. Arbeitet Wort für Wort, ohne Regex-Crate.
pub fn sanitize(line: &str) -> String {
    let lower = line.to_lowercase();
    for marker in ["cookie", "authorization", "bearer "] {
        if let Some(pos) = lower.find(marker) {
            let head = line.get(..pos).unwrap_or("");
            return format!("{head}{marker} [entfernt]").chars().take(MAX_LINE_CHARS).collect();
        }
    }
    let mut out: Vec<String> = Vec::new();
    for word in line.split(' ') {
        out.push(clean_word(word));
    }
    let text = out.join(" ");
    text.chars().filter(|c| !c.is_control() || *c == '\t').take(MAX_LINE_CHARS).collect()
}

fn clean_word(word: &str) -> String {
    let lower = word.to_ascii_lowercase();
    // Windows-Benutzerordner: C:\Users\<Name>\… → C:\Users\[user]\…
    if let Some(pos) = lower.find("\\users\\") {
        let start = pos + "\\users\\".len();
        let rest = &word[start..];
        let tail = rest.find('\\').map(|i| &rest[i..]).unwrap_or("");
        return format!("{}[user]{tail}", &word[..start]);
    }
    // URL mit Parametern: nur Adresse und Pfad behalten.
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.contains("://") {
        return match word.find(['?', '#']) {
            Some(i) => format!("{}?[…]", &word[..i]),
            None => word.to_string(),
        };
    }
    if word.contains('@') && word.contains('.') {
        return "[e-mail]".into();
    }
    // key=value bzw. "key":"value" mit geheimem Schlüsselnamen.
    for secret in ["token", "key", "secret", "password", "passwort", "sapisid", "visitor"] {
        if let Some(pos) = lower.find(secret) {
            if let Some(eq) = word[pos..].find(['=', ':']).map(|i| pos + i) {
                return format!("{}[entfernt]", &word[..=eq]);
            }
        }
    }
    let core: String = word.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-').collect();
    if looks_like_video_id(&core) || looks_like_secret(&core) {
        return word.replace(&core, if core.len() == 11 { "[video]" } else { "[id]" });
    }
    word.to_string()
}

/// YouTube-Video-ID: 11 Zeichen, Groß- und Kleinbuchstaben gemischt plus Ziffer, `_` oder `-`.
pub fn looks_like_video_id(word: &str) -> bool {
    word.len() == 11
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && word.chars().any(|c| c.is_ascii_uppercase())
        && word.chars().any(|c| c.is_ascii_lowercase())
        && word.chars().any(|c| c.is_ascii_digit() || c == '_' || c == '-')
}

/// Lange zufällig wirkende Zeichenketten (Tokens, Playlist-IDs) ab 20 Zeichen.
fn looks_like_secret(word: &str) -> bool {
    word.len() >= 20 && word.chars().any(|c| c.is_ascii_digit()) && word.chars().any(|c| c.is_ascii_alphabetic())
}

/// Zeilen des Logs (`[secs] text`) nach `since` (Unix-Sekunden). Gibt (Zeilen, neuester Zeitstempel).
pub fn collect(log_text: &str, since: u64) -> (Vec<String>, u64) {
    let mut latest = since;
    let mut lines = Vec::new();
    for raw in log_text.lines() {
        let Some(rest) = raw.strip_prefix('[') else { continue };
        let Some((secs, text)) = rest.split_once("] ") else { continue };
        let Ok(secs) = secs.parse::<u64>() else { continue };
        if secs <= since {
            continue;
        }
        latest = latest.max(secs);
        // Debug-Zeilen enthalten Titel und den ganzen Player-Zustand.
        if text.starts_with("music-debug") || text.starts_with("jam-debug") {
            continue;
        }
        lines.push(format!("[{secs}] {}", sanitize(text)));
    }
    let skip = lines.len().saturating_sub(MAX_LINES);
    (lines.split_off(skip), latest)
}

pub fn is_error(line: &str) -> bool {
    let lower = line.to_lowercase();
    ["panic", "fehlgeschlagen", "failed", "fehler", "error", "nicht erstellbar", "nicht verfuegbar"].iter().any(|w| lower.contains(w))
}

/// Vermerk zum Bericht: manuell gesendet (ggf. ohne neue Zeilen) oder leer. Ohne Zeilen braucht
/// der Server einen Vermerk, sonst lehnt er den Bericht als leer ab.
pub fn note(manual: bool, no_new_lines: bool) -> &'static str {
    match (manual, no_new_lines) {
        (true, true) => "manuell gesendet (keine neuen Zeilen)",
        (true, false) => "manuell gesendet",
        _ => "",
    }
}

/// Bericht als JSON (Format siehe playtube-jam/src/diag.js).
pub fn report(install_id: &str, lines: &[String], note: &str) -> serde_json::Value {
    let errors: Vec<&String> = lines.iter().filter(|l| is_error(l)).collect();
    json!({
        "app": "pc",
        "version": crate::config::VERSION,
        "platform": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        "os": windows_version(),
        "installId": install_id,
        "lines": lines,
        "errors": errors,
        "note": note,
        "includesTitles": false,
    })
}

/// z. B. "Windows 10.0.26300 (25H2)" aus HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion.
fn windows_version() -> String {
    let build = registry_string("CurrentBuild").unwrap_or_default();
    let display = registry_string("DisplayVersion").unwrap_or_default();
    match (build.is_empty(), display.is_empty()) {
        (true, _) => "Windows".into(),
        (false, true) => format!("Windows 10.0.{build}"),
        (false, false) => format!("Windows 10.0.{build} ({display})"),
    }
}

fn registry_string(name: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let mut buf = [0u16; 64];
    let mut size = (buf.len() * 2) as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
            &HSTRING::from(name),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
}

pub fn log_path() -> std::path::PathBuf {
    crate::config::data_dir().join("logs").join("playtube.log")
}

/// Bericht senden. Ergebnis: Bericht-ID des Servers.
pub async fn send(server: &str, body: serde_json::Value) -> anyhow::Result<String> {
    let url = format!("{}/api/diag", server.trim_end_matches('/'));
    let res = crate::jam::http_client(server).await.post(url).json(&body).send().await?;
    anyhow::ensure!(res.status().is_success(), "HTTP {}", res.status());
    let value: serde_json::Value = res.json().await?;
    Ok(value["id"].as_str().unwrap_or("").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_private_data() {
        let cases = [
            ("Cookie: SAPISID=abc; HSID=def", "cookie [entfernt]"),
            ("jam: lade dQw4w9WgXcQ", "jam: lade [video]"),
            ("jam: Titel dQw4w9WgXcQ spielt hier nicht an", "jam: Titel [video] spielt hier nicht an"),
            ("Pfad C:\\Users\\Christian\\AppData\\Roaming\\Playtube", "Pfad C:\\Users\\[user]\\AppData\\Roaming\\Playtube"),
            ("GET https://music.youtube.com/search?q=lieblingssong", "GET https://music.youtube.com/search?[…]"),
            ("token=abcdef123 weiter", "token=[entfernt] weiter"),
            ("{\"last_token\":\"xyz\"}", "{\"last_token\":[entfernt]"),
            ("Mail jemand@example.com", "Mail [e-mail]"),
            ("Playlist PLabcdefghij1234567890XYZ", "Playlist [id]"),
        ];
        for (input, expected) in cases {
            assert_eq!(sanitize(input), expected, "{input}");
        }
        // Normale Wörter bleiben (auch lange deutsche).
        assert_eq!(sanitize("update: Prüfung fehlgeschlagen: Verbindung abgelehnt"), "update: Prüfung fehlgeschlagen: Verbindung abgelehnt");
        assert_eq!(sanitize("Verbindungen"), "Verbindungen");
    }

    #[test]
    fn video_ids_are_recognized() {
        assert!(looks_like_video_id("dQw4w9WgXcQ"));
        assert!(looks_like_video_id("abc_DEFghij"));
        assert!(!looks_like_video_id("Verbindunge"));
        assert!(!looks_like_video_id("12345678901"));
    }

    #[test]
    fn collects_only_new_lines_without_debug() {
        let log = "[100] alt\n[200] jam: Verbindung fehlgeschlagen: x\n[201] music-debug: {\"title\":\"Song\"}\nkaputt\n[202] neu";
        let (lines, latest) = collect(log, 100);
        assert_eq!(lines, vec!["[200] jam: Verbindung fehlgeschlagen: x", "[202] neu"]);
        assert_eq!(latest, 202);
        let (none, latest) = collect(log, 202);
        assert!(none.is_empty());
        assert_eq!(latest, 202);
    }

    #[test]
    fn keeps_only_the_newest_lines() {
        let log: String = (1..=1000).map(|i| format!("[{i}] Zeile {i}\n")).collect();
        let (lines, _) = collect(&log, 0);
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines[0], "[701] Zeile 701");
    }

    #[test]
    fn manual_report_without_lines_has_note() {
        assert_eq!(note(true, true), "manuell gesendet (keine neuen Zeilen)");
        assert_eq!(note(true, false), "manuell gesendet");
        assert_eq!(note(false, true), "");
        let r = report("abc123abc123", &[], note(true, true));
        assert!(r["lines"].as_array().unwrap().is_empty());
        assert!(!r["note"].as_str().unwrap().is_empty());
    }

    #[test]
    fn builds_report() {
        let lines = vec!["[1] PANIC: boom".to_string(), "[2] ok".to_string()];
        let r = report("abc123abc123", &lines, "");
        assert_eq!(r["app"], "pc");
        assert_eq!(r["errors"].as_array().unwrap().len(), 1);
        assert_eq!(r["includesTitles"], false);
        assert_eq!(new_install_id().len(), 24);
    }
}

/// Echter Versand gegen einen laufenden Worker:
/// `$env:DIAG_TEST_SERVER="http://127.0.0.1:8787"; cargo test sends_to_server -- --ignored`
#[cfg(test)]
#[test]
#[ignore]
fn sends_to_server() {
    let server = std::env::var("DIAG_TEST_SERVER").expect("DIAG_TEST_SERVER");
    let lines = vec![r"[1] test: Verbindung fehlgeschlagen: C:\Users\Jemand\x".to_string()];
    let rt = tokio::runtime::Runtime::new().unwrap();
    let id = rt.block_on(send(&server, report(&new_install_id(), &lines, "cargo test"))).unwrap();
    assert!(id.starts_with("r:"), "{id}");
}
