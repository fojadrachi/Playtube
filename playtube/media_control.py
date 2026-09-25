"""JavaScript fuer die Fernsteuerung (siehe remote_control.py): baut pro Befehl ein kleines
Skript, das im YouTube- bzw. YouTube-Music-Tab ausgefuehrt wird.

Es werden ausschliesslich feste Skripte aus einer Whitelist erzeugt; der einzige
variable Teil ist ein bereits validierter Zahlenwert (Sekunden/Lautstaerke), der als
Zahl eingesetzt wird - fremder Text landet nie im Skript."""
from __future__ import annotations

import math

# Gemeinsame Helfer: der YouTube-Player (#movie_player) bietet auf youtube.com UND
# music.youtube.com eine API fuer Lautstaerke/Stummschaltung/Titelwechsel; das
# <video>-Element dient als Fallback, falls die API (noch) nicht verfuegbar ist.
_PRELUDE = r"""
    var isMusic = location.hostname.indexOf('music.youtube.com') !== -1;
    var video = document.querySelector('video');
    var player = document.getElementById('movie_player');
    function has(fn) { return player && typeof player[fn] === 'function'; }
    function click(selectors) {
        for (var i = 0; i < selectors.length; i++) {
            var el = document.querySelector(selectors[i]);
            if (el) { el.click(); return true; }
        }
        return false;
    }
    function clamp(v) { return Math.max(0, Math.min(100, Math.round(v))); }
    function getVolume() {
        if (has('getVolume')) { return player.getVolume(); }
        return video ? video.volume * 100 : 0;
    }
    function unmute() {
        if (has('unMute')) { player.unMute(); } else if (video) { video.muted = false; }
    }
    function mute() {
        if (has('mute')) { player.mute(); } else if (video) { video.muted = true; }
    }
    function isMuted() {
        if (has('isMuted')) { return player.isMuted(); }
        return video ? video.muted : false;
    }
    function setVolume(v) {
        v = clamp(v);
        if (has('setVolume')) { player.setVolume(v); }
        else if (video) { video.volume = v / 100; }
        if (v > 0) { unmute(); }
    }
"""

_NEXT_SELECTORS = (
    "['ytmusic-player-bar .next-button', 'ytmusic-player-bar #next-button button', "
    "'tp-yt-paper-icon-button.next-button', '.ytp-next-button']"
)
_PREV_SELECTORS = (
    "['ytmusic-player-bar .previous-button', 'ytmusic-player-bar #previous-button button', "
    "'tp-yt-paper-icon-button.previous-button', '.ytp-prev-button']"
)
_LIKE_SELECTORS = (
    "['ytmusic-player-bar ytmusic-like-button-renderer #button-shape-like button', "
    "'like-button-view-model button', '#segmented-like-button button']"
)
_DISLIKE_SELECTORS = (
    "['ytmusic-player-bar ytmusic-like-button-renderer #button-shape-dislike button', "
    "'dislike-button-view-model button', '#segmented-dislike-button button']"
)

# Befehl -> Skriptkoerper. "{value}" wird durch den validierten Zahlenwert ersetzt.
_BODIES: dict[str, str] = {
    "play_pause": "if (video) { video.paused ? video.play() : video.pause(); }",
    "play": "if (video && video.paused) { video.play(); }",
    "pause": "if (video && !video.paused) { video.pause(); }",
    # YouTube (ohne Playlist) blendet den "Naechster"-Knopf teils aus -> Player-API.
    "next": f"if (!click({_NEXT_SELECTORS}) && has('nextVideo')) {{ player.nextVideo(); }}",
    # Ohne Vorgaenger springt "Zurueck" wie bei ueblichen Playern an den Anfang.
    "previous": (
        f"if (!click({_PREV_SELECTORS})) {{"
        " if (video) { video.currentTime = 0; } }"
    ),
    "seek": (
        "if (video && isFinite(video.currentTime)) {"
        " var end = isFinite(video.duration) ? video.duration : video.currentTime + {value};"
        " video.currentTime = Math.max(0, Math.min(end, video.currentTime + {value})); }"
    ),
    "volume_change": "setVolume(getVolume() + {value});",
    "set_volume": "setVolume({value});",
    "mute_toggle": "if (isMuted()) { unmute(); } else { mute(); }",
    "like": f"click({_LIKE_SELECTORS});",
    "dislike": f"click({_DISLIKE_SELECTORS});",
    "shuffle": (
        "if (isMusic) { click(['ytmusic-player-bar .shuffle', "
        "'ytmusic-player-bar #shuffle-button', 'ytmusic-player-bar .shuffle-button']); }"
    ),
    # YouTube Music: Wiederholen-Modus durchschalten; YouTube: Video in Schleife an/aus.
    "repeat": (
        "if (isMusic) { click(['ytmusic-player-bar .repeat', "
        "'ytmusic-player-bar #repeat-button', 'ytmusic-player-bar .repeat-button']); }"
        " else if (video) { video.loop = !video.loop; }"
    ),
}

MEDIA_COMMANDS = frozenset(_BODIES)

# Liest die Playlists aus der Seitenleiste von YouTube Music. Die Eintraege sind keine
# normalen Links: Ziel steht in den Polymer-Daten des Elements
# (data.navigationEndpoint.browseEndpoint.browseId = "VL<Playlist-ID>"); Links
# ("browse/VL<id>", "playlist?list=<id>") dienen nur als Fallback. Liefert einen
# JSON-String (Objekte kommen ueber die runJavaScript()-Bruecke nicht zuverlaessig an,
# siehe media_probe.py); die Daten gelten als fremd und werden in
# remote_control.sanitize_playlists bereinigt.
LIST_PLAYLISTS_JS = r"""
(function() {
    var out = [];
    function idFromBrowseId(browseId) {
        return (typeof browseId === 'string' && browseId.indexOf('VL') === 0) ? browseId.slice(2) : null;
    }
    function idFromHref(href) {
        var m = /[?&]list=([A-Za-z0-9_-]+)/.exec(href || '') || /browse\/VL([A-Za-z0-9_-]+)/.exec(href || '');
        return m ? m[1] : null;
    }
    function textOf(formatted) {
        if (!formatted) { return ''; }
        if (typeof formatted.simpleText === 'string') { return formatted.simpleText; }
        return (formatted.runs || []).map(function(r) { return r.text || ''; }).join('');
    }
    var entries = document.querySelectorAll('ytmusic-guide-entry-renderer');
    for (var i = 0; i < entries.length; i++) {
        var el = entries[i];
        var data = el.data || (el.__data && el.__data.data) || null;
        var endpoint = data && data.navigationEndpoint;
        var id = idFromBrowseId(endpoint && endpoint.browseEndpoint && endpoint.browseEndpoint.browseId);
        if (!id) {
            var link = el.querySelector('[href]');
            id = idFromHref(link ? link.getAttribute('href') : '');
        }
        if (!id) { continue; }
        var titleEl = el.querySelector('.title');
        var title = textOf(data && data.formattedTitle) || (titleEl ? titleEl.textContent : '');
        out.push({ id: id, title: (title || '').trim() });
    }
    return JSON.stringify(out);
})();
"""

MUSIC_PLAYLIST_URL = "https://music.youtube.com/watch?list={playlist_id}"


def build_control_js(command: str, value: float = 0) -> str:
    """Liefert das Skript fuer `command`. Unbekannte Befehle oder nicht-endliche Werte
    -> ValueError (die Wertebereiche prueft vorher remote_control.parse_request)."""
    body = _BODIES.get(command)
    if body is None:
        raise ValueError(f"unbekannter Befehl: {command}")
    number = float(value)
    if not math.isfinite(number):
        raise ValueError("value muss eine endliche Zahl sein")
    # repr() einer endlichen float ist immer ein gueltiges JS-Zahlenliteral (z.B. -10.0).
    return "(function() {" + _PRELUDE + body.replace("{value}", repr(number)) + "\n})();"
