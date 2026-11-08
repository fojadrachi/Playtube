//! Nur EINE Playtube-Instanz pro Datenordner: zwei Instanzen auf demselben WebView2-Profil
//! stören sich (Login, fehlende Icons). Ein zweiter Start holt das laufende Fenster nach vorn
//! bzw. reicht eine Jam-Einladung (`playtube://jam/<CODE>`) an die laufende Instanz weiter.

use std::io::Write;

fn pipe_name() -> String {
    let folder = crate::config::data_dir().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    format!(r"\\.\pipe\Playtube.SingleInstance.{folder}")
}

/// Nachricht an die laufende Instanz: `show` oder `jam <CODE>`.
pub fn message(jam_code: Option<&str>) -> String {
    match jam_code {
        Some(code) => format!("jam {code}\n"),
        None => "show\n".to_string(),
    }
}

/// Liest eine Nachricht der zweiten Instanz: `None` = nur anzeigen, `Some(code)` = Jam beitreten.
pub fn parse_message(text: &str) -> Option<String> {
    let line = text.lines().next().unwrap_or("").trim();
    line.strip_prefix("jam ").and_then(crate::jam::normalize_code)
}

/// Meldet sich bei einer laufenden Instanz. `true` = es läuft schon eine, diese hier soll sich
/// beenden.
pub fn notify_running(jam_code: Option<&str>) -> bool {
    match std::fs::OpenOptions::new().write(true).open(pipe_name()) {
        Ok(mut pipe) => {
            let _ = pipe.write_all(message(jam_code).as_bytes());
            true
        }
        Err(_) => false,
    }
}

/// Wartet auf weitere Starts und ruft dann `on_show` auf (mit Jam-Code, falls die zweite
/// Instanz über eine Einladung gestartet wurde). Liefert `false`, wenn die Pipe schon existiert
/// (eine andere Instanz ist gerade gleichzeitig gestartet).
pub fn listen(runtime: &tokio::runtime::Handle, on_show: impl Fn(Option<String>) + Send + Sync + 'static) -> bool {
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::ServerOptions;
    let name = pipe_name();
    let _guard = runtime.enter();
    let Ok(mut server) = ServerOptions::new().first_pipe_instance(true).reject_remote_clients(true).create(&name) else {
        return false;
    };
    runtime.spawn(async move {
        loop {
            if server.connect().await.is_ok() {
                let mut buffer = [0u8; 64];
                let read = server.read(&mut buffer).await.unwrap_or(0);
                on_show(parse_message(&String::from_utf8_lossy(&buffer[..read])));
            }
            let _ = server.disconnect();
            match ServerOptions::new().reject_remote_clients(true).create(&name) {
                Ok(next) => server = next,
                Err(_) => return,
            }
        }
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_jam_codes_to_the_running_instance() {
        assert_eq!(parse_message(&message(Some("ABC234"))).as_deref(), Some("ABC234"));
        assert_eq!(parse_message(&message(None)), None);
        assert_eq!(parse_message("jam ABC10O\n"), None);
        assert_eq!(parse_message(""), None);
    }
}
