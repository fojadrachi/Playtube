//! JavaScript für die YouTube-/YouTube-Music-Seiten: Wiedergabestand auslesen, Steuerbefehle,
//! Playlists lesen und Audioausgabe pro Tab. Die Skripte stammen unverändert aus der früheren
//! Python-Version (src/js/). Fremder Text landet nie im Skript: Befehle kommen aus einer festen
//! Liste, der einzige variable Teil ist eine geprüfte Zahl bzw. ein JSON-kodierter Gerätename.

use serde_json::Value;

pub const MEDIA_PROBE: &str = include_str!("js/media_probe.js");
pub const LIST_PLAYLISTS: &str = include_str!("js/list_playlists.js");
pub const QUEUE: &str = include_str!("js/queue.js");
const PLAY_QUEUE_ITEM: &str = include_str!("js/play_queue_item.js");
const CONTROL_PRELUDE: &str = include_str!("js/control_prelude.js");
const AUDIO_ROUTER: &str = include_str!("js/audio_router.js");
const AUDIO_TAP: &str = include_str!("js/audio_tap.js");

/// Playtube-Ton: greift den Ton der Seite ab und schickt ihn an Playtube (src/relay.rs).
pub fn audio_tap(enabled: bool) -> String {
    AUDIO_TAP.replace("__ENABLED__", if enabled { "true" } else { "false" })
}

/// Schaltet den Playtube-Ton auf der offenen Seite an/aus.
pub fn audio_tap_enable(enabled: bool) -> String {
    format!("(function(){{ if (window.__playtubeTap) {{ window.__playtubeTap.enable({enabled}); }} }})();")
}

/// Verhindert die Abfrage "Website verlassen?" (läuft beim Dokumentstart, vor der Seite).
pub const NO_LEAVE_PROMPT: &str = include_str!("js/no_leave_prompt.js");

pub const MUSIC_PLAYLIST_URL: &str = "https://music.youtube.com/watch?list=";

const NEXT: &str = "['ytmusic-player-bar .next-button', 'ytmusic-player-bar #next-button button', \
                    'tp-yt-paper-icon-button.next-button', '.ytp-next-button']";
const PREV: &str = "['ytmusic-player-bar .previous-button', 'ytmusic-player-bar #previous-button button', \
                    'tp-yt-paper-icon-button.previous-button', '.ytp-prev-button']";
const LIKE: &str = "['ytmusic-player-bar ytmusic-like-button-renderer #button-shape-like button', \
                    'like-button-view-model button', '#segmented-like-button button']";
const DISLIKE: &str = "['ytmusic-player-bar ytmusic-like-button-renderer #button-shape-dislike button', \
                       'dislike-button-view-model button', '#segmented-dislike-button button']";

/// Alle Steuerbefehle der Fernsteuerung (Whitelist).
pub const MEDIA_COMMANDS: &[&str] = &[
    "play_pause", "play", "pause", "next", "previous", "seek", "volume_change", "set_volume",
    "mute_toggle", "like", "dislike", "shuffle", "repeat",
];

/// Skript für einen Steuerbefehl; `None` bei unbekanntem Befehl oder nicht-endlichem Wert.
pub fn control(command: &str, value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    let body = match command {
        "play_pause" => "if (video) { video.paused ? video.play() : video.pause(); }".to_string(),
        "play" => "if (video && video.paused) { video.play(); }".to_string(),
        "pause" => "if (video && !video.paused) { video.pause(); }".to_string(),
        // YouTube (ohne Playlist) blendet "Nächster" teils aus -> Player-API.
        "next" => format!("if (!click({NEXT}) && has('nextVideo')) {{ player.nextVideo(); }}"),
        // Ohne Vorgänger springt "Zurück" an den Anfang.
        "previous" => format!("if (!click({PREV})) {{ if (video) {{ video.currentTime = 0; }} }}"),
        "seek" => format!(
            "if (video && isFinite(video.currentTime)) {{ var end = isFinite(video.duration) ? video.duration : video.currentTime + {value:?}; \
             video.currentTime = Math.max(0, Math.min(end, video.currentTime + {value:?})); }}"
        ),
        "volume_change" => format!("setVolume(getVolume() + {value:?});"),
        "set_volume" => format!("setVolume({value:?});"),
        "mute_toggle" => "if (isMuted()) { unmute(); } else { mute(); }".to_string(),
        "like" => format!("click({LIKE});"),
        "dislike" => format!("click({DISLIKE});"),
        "shuffle" => "if (isMusic) { click(['ytmusic-player-bar .shuffle', 'ytmusic-player-bar #shuffle-button', \
                      'ytmusic-player-bar .shuffle-button']); }"
            .to_string(),
        // YouTube Music: Wiederholen durchschalten; YouTube: Video in Schleife an/aus.
        "repeat" => "if (isMusic) { click(['ytmusic-player-bar .repeat', 'ytmusic-player-bar #repeat-button', \
                     'ytmusic-player-bar .repeat-button']); } else if (video) { video.loop = !video.loop; }"
            .to_string(),
        _ => return None,
    };
    Some(format!("(function() {{{CONTROL_PRELUDE}{body}\n}})();"))
}

/// Playtube Jam: Stand des Players (Video-ID, Position, Pause, Werbung, Fehler).
pub const JAM_PROBE: &str = include_str!("js/jam_probe.js");

/// Playtube Jam: an Position `seconds` springen (nur endliche Zahlen).
pub fn jam_seek(seconds: f64) -> Option<String> {
    seconds.is_finite().then(|| {
        format!(
            "(function() {{ var p = document.getElementById('movie_player'); var s = {:?};              if (p && typeof p.seekTo === 'function') {{ p.seekTo(s, true); return; }}              var v = p && p.querySelector('video') || document.querySelector('video'); if (v) {{ v.currentTime = s; }} }})();",
            seconds.max(0.0)
        )
    })
}

/// Playtube Jam: abspielen bzw. pausieren über die Player-API (es kann mehrere <video> geben).
pub fn jam_play(play: bool) -> String {
    let (method, prop) = if play { ("playVideo", "play") } else { ("pauseVideo", "pause") };
    format!(
        "(function() {{ var p = document.getElementById('movie_player');          if (p && typeof p.{method} === 'function') {{ p.{method}(); return; }}          var v = p && p.querySelector('video') || document.querySelector('video'); if (v) {{ v.{prop}(); }} }})();"
    )
}

/// Spielt Eintrag Nr. `index` der Wiedergabeliste ab (Reihenfolge wie [`QUEUE`]).
pub fn play_queue_item(index: u32) -> String {
    PLAY_QUEUE_ITEM.replace("__INDEX__", &index.to_string())
}

/// Legt die Medienelemente der Seite auf das Gerät `device_name` (leer = Systemstandard).
pub fn audio_router(device_name: &str) -> String {
    AUDIO_ROUTER.replace("__TARGET__", &Value::String(device_name.to_string()).to_string())
}

/// Für Skripte, die beim Dokumentstart laufen: erst nach DOMContentLoaded ausführen.
pub fn in_document(source: &str) -> String {
    format!(
        "(function(){{function go(){{{source}}}\
         if(document.readyState==='loading'){{document.addEventListener('DOMContentLoaded',go);}}else{{go();}}}})();"
    )
}

/// WebView2 liefert das Ergebnis JSON-kodiert; unsere Skripte geben selbst JSON-Text zurück,
/// daher wird bei Bedarf ein zweites Mal dekodiert.
pub fn decode_result(raw: &str) -> Option<Value> {
    let value: Value = serde_json::from_str(raw).ok()?;
    match value {
        Value::String(inner) => Some(serde_json::from_str(&inner).unwrap_or(Value::String(inner))),
        Value::Null => None,
        other => Some(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_builds_a_wrapped_script() {
        for command in MEDIA_COMMANDS {
            let script = control(command, 5.0).unwrap();
            assert!(script.starts_with("(function() {"), "{command}");
            assert!(!script.contains("{value}"));
        }
    }

    #[test]
    fn value_is_inserted_as_number() {
        assert!(control("set_volume", 42.0).unwrap().contains("setVolume(42.0)"));
        assert!(control("seek", -10.0).unwrap().contains("+ -10.0"));
    }

    #[test]
    fn rejects_unknown_command_and_non_finite_value() {
        assert!(control("alert", 0.0).is_none());
        assert!(control("seek", f64::INFINITY).is_none());
        assert!(control("seek", f64::NAN).is_none());
    }

    #[test]
    fn jam_seek_takes_only_numbers() {
        assert!(jam_seek(12.5).unwrap().contains("var s = 12.5;"));
        assert!(jam_seek(-3.0).unwrap().contains("var s = 0.0;"));
        assert!(jam_play(true).contains("p.playVideo()"));
        assert!(jam_play(false).contains("p.pauseVideo()"));
        assert!(jam_seek(f64::NAN).is_none());
    }

    #[test]
    fn audio_tap_switch_is_a_boolean() {
        assert!(audio_tap(true).contains("var DEFAULT_ON = true;"));
        assert!(audio_tap(false).contains("var DEFAULT_ON = false;"));
        assert!(!audio_tap(true).contains("__ENABLED__"));
        assert!(audio_tap_enable(false).contains("enable(false)"));
    }

    #[test]
    fn leave_prompt_script_blocks_beforeunload() {
        assert!(NO_LEAVE_PROMPT.contains("'beforeunload'"));
        assert!(NO_LEAVE_PROMPT.contains("onbeforeunload"));
    }

    #[test]
    fn queue_index_is_a_number() {
        let script = play_queue_item(7);
        assert!(script.contains("var index = 7;"));
        assert!(!script.contains("__INDEX__"));
    }

    #[test]
    fn router_target_is_json_escaped() {
        let script = audio_router("Lautsprecher \"A\" </script>");
        assert!(script.contains(r#"var TARGET = "Lautsprecher \"A\" </script>";"#));
        assert!(!script.contains("__TARGET__"));
    }

    #[test]
    fn decodes_double_encoded_results() {
        assert_eq!(decode_result(r#""{\"a\":1}""#), Some(serde_json::json!({"a": 1})));
        assert_eq!(decode_result("null"), None);
        assert_eq!(decode_result("kaputt"), None);
    }
}
