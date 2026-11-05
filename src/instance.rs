//! Nur EINE Playtube-Instanz pro Datenordner: zwei Instanzen auf demselben WebView2-Profil
//! stören sich (Login, fehlende Icons). Ein zweiter Start holt das laufende Fenster nach vorn.

use std::io::Write;

fn pipe_name() -> String {
    let folder = crate::config::data_dir().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    format!(r"\\.\pipe\Playtube.SingleInstance.{folder}")
}

/// Meldet sich bei einer laufenden Instanz ("show"). `true` = es läuft schon eine, diese hier
/// soll sich beenden.
pub fn notify_running() -> bool {
    match std::fs::OpenOptions::new().write(true).open(pipe_name()) {
        Ok(mut pipe) => {
            let _ = pipe.write_all(b"show\n");
            true
        }
        Err(_) => false,
    }
}

/// Wartet auf weitere Starts und ruft dann `on_show` auf. Liefert `false`, wenn die Pipe schon
/// existiert (eine andere Instanz ist gerade gleichzeitig gestartet).
pub fn listen(runtime: &tokio::runtime::Handle, on_show: impl Fn() + Send + Sync + 'static) -> bool {
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
                let mut buffer = [0u8; 16];
                let _ = server.read(&mut buffer).await;
                on_show();
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
