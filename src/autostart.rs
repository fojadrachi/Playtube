//! Mit Windows starten: Eintrag unter HKCU\Software\Microsoft\Windows\CurrentVersion\Run (nur für den eigenen Benutzer, ohne Adminrechte).
//! Playtube startet dann mit `--autostart` und bleibt auf Wunsch nur im Tray.

use std::os::windows::process::CommandExt;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE: &str = "Playtube";
pub const ARG: &str = "--autostart";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Befehlszeile für den Run-Eintrag.
pub fn command_line(exe: &std::path::Path) -> String {
    format!("\"{}\" {ARG}", exe.display())
}

/// Eintrag setzen bzw. entfernen. Nur für die installierte Playtube (eine Testkopie trägt sich nicht ein).
pub fn apply(enabled: bool) {
    if !crate::updater::is_installed() {
        return;
    }
    let ok = if enabled {
        let Ok(exe) = std::env::current_exe() else { return };
        reg(&["add", RUN_KEY, "/v", VALUE, "/t", "REG_SZ", "/d", &command_line(&exe), "/f"])
    } else {
        // Fehlt der Eintrag, meldet reg einen Fehler - das ist hier egal.
        reg(&["delete", RUN_KEY, "/v", VALUE, "/f"]) || true
    };
    if !ok {
        crate::log::line("autostart: Eintrag konnte nicht gesetzt werden");
    }
}

fn reg(args: &[&str]) -> bool {
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Wurde Playtube vom Autostart gestartet?
pub fn started_by_autostart(args: &[String]) -> bool {
    args.iter().any(|a| a == ARG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_quoted_command_line() {
        let exe = std::path::Path::new(r"C:\Users\A B\AppData\Local\Programs\Playtube\Playtube.exe");
        assert_eq!(command_line(exe), r#""C:\Users\A B\AppData\Local\Programs\Playtube\Playtube.exe" --autostart"#);
        assert!(started_by_autostart(&["x".into(), "--autostart".into()]));
        assert!(!started_by_autostart(&["x".into()]));
    }
}
