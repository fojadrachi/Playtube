//! Mit dem System starten. Windows: Eintrag unter HKCU\Software\Microsoft\Windows\CurrentVersion\Run
//! (nur für den eigenen Benutzer, ohne Adminrechte). Linux: `~/.config/autostart/playtube.desktop`
//! (XDG-Autostart). Playtube startet dann mit `--autostart` und bleibt auf Wunsch nur im Tray.

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const VALUE: &str = "Playtube";
pub const ARG: &str = "--autostart";
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Befehlszeile für den Run-Eintrag.
#[cfg(windows)]
pub fn command_line(exe: &std::path::Path) -> String {
    format!("\"{}\" {ARG}", exe.display())
}

/// Eintrag setzen bzw. entfernen. Nur für die installierte Playtube (eine Testkopie trägt sich nicht ein).
#[cfg(windows)]
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

#[cfg(windows)]
fn reg(args: &[&str]) -> bool {
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Befehl für den Exec-Eintrag der .desktop-Datei (Leerzeichen und Sonderzeichen in Anführung).
#[cfg(not(windows))]
pub fn command_line(exe: &std::path::Path) -> String {
    let path = exe.display().to_string();
    let escaped: String = path.chars().flat_map(|c| match c {
        '"' | '`' | '$' | '\\' => vec!['\\', c],
        c => vec![c],
    }).collect();
    format!("\"{escaped}\" {ARG}")
}

/// Installierte Playtube (AppImage oder Paket unter /usr bzw. /opt) - eine Testkopie aus dem
/// Build-Ordner trägt sich nicht ein.
#[cfg(not(windows))]
fn installed_exe() -> Option<std::path::PathBuf> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        return Some(appimage.into());
    }
    let exe = std::env::current_exe().ok()?;
    (exe.starts_with("/usr") || exe.starts_with("/opt")).then_some(exe)
}

#[cfg(not(windows))]
fn desktop_file() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))?;
    Some(base.join("autostart").join("playtube.desktop"))
}

#[cfg(not(windows))]
pub fn apply(enabled: bool) {
    let Some(file) = desktop_file() else { return };
    if !enabled {
        let _ = std::fs::remove_file(&file);
        return;
    }
    let Some(exe) = installed_exe() else { return };
    let text = format!(
        "[Desktop Entry]\nType=Application\nName=Playtube\nExec={}\nIcon=playtube\nX-GNOME-Autostart-enabled=true\nNoDisplay=false\n",
        command_line(&exe)
    );
    let ok = file.parent().is_some_and(|dir| std::fs::create_dir_all(dir).is_ok()) && std::fs::write(&file, text).is_ok();
    if !ok {
        crate::log::line("autostart: Eintrag konnte nicht gesetzt werden");
    }
}

/// Wurde Playtube vom Autostart gestartet?
pub fn started_by_autostart(args: &[String]) -> bool {
    args.iter().any(|a| a == ARG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(windows))]
    #[test]
    fn builds_a_desktop_exec_line() {
        let exe = std::path::Path::new("/home/a b/Apps/Playtube-2.16.0-x86_64.AppImage");
        assert_eq!(command_line(exe), r#""/home/a b/Apps/Playtube-2.16.0-x86_64.AppImage" --autostart"#);
        assert_eq!(command_line(std::path::Path::new("/x/\"$y")), r#""/x/\"\$y" --autostart"#);
    }

    #[cfg(windows)]
    #[test]
    fn builds_a_quoted_command_line() {
        let exe = std::path::Path::new(r"C:\Users\A B\AppData\Local\Programs\Playtube\Playtube.exe");
        assert_eq!(command_line(exe), r#""C:\Users\A B\AppData\Local\Programs\Playtube\Playtube.exe" --autostart"#);
        assert!(started_by_autostart(&["x".into(), "--autostart".into()]));
        assert!(!started_by_autostart(&["x".into()]));
    }
}
