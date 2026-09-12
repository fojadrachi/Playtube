"""JavaScript, das periodisch in die YouTube- bzw. YouTube-Music-Seite injiziert wird,
um Titel/Interpret/Wiedergabestatus fuer die Discord-Rich-Presence auszulesen."""

MEDIA_PROBE_JS = r"""
(function() {
    function txt(sel) {
        var el = document.querySelector(sel);
        return el ? el.textContent.trim() : null;
    }
    // Wichtig: getAttribute('src') statt .src - bei einem noch nicht (nach-)geladenen
    // <img> (leeres/fehlendes src-Attribut, z.B. waehrend Lazy-Loading) loest die .src
    // Property faelschlich auf die aktuelle Seiten-URL auf statt null/"" zu liefern.
    function imgSrc(sel) {
        var el = document.querySelector(sel);
        var raw = el ? el.getAttribute('src') : null;
        return (raw && /^https?:\/\//i.test(raw)) ? raw : null;
    }
    // Video-ID aus der URL (?v=... Parameter) - damit laesst sich die Thumbnail-URL
    // ueber YouTubes CDN immer zuverlaessig selbst bauen, unabhaengig davon, ob
    // gerade ein passendes <img>/<link> im DOM zu finden ist (das <link
    // rel="image_src">, auf das sich der Code frueher verlassen hat, fehlt auf
    // vielen aktuellen YouTube-Seiten schlicht).
    function videoIdFromUrl(url) {
        try {
            return new URL(url).searchParams.get('v');
        } catch (e) { return null; }
    }
    function ytThumbnailUrl(videoId) {
        return videoId ? ('https://i.ytimg.com/vi/' + videoId + '/hqdefault.jpg') : null;
    }
    // "1:23" oder "1:02:03" -> Sekunden.
    function parseClock(str) {
        if (!str) { return null; }
        var parts = String(str).trim().split(':').map(function(p) { return parseInt(p, 10); });
        if (!parts.length) { return null; }
        for (var i = 0; i < parts.length; i++) { if (isNaN(parts[i])) { return null; } }
        var seconds = 0;
        for (var j = 0; j < parts.length; j++) { seconds = seconds * 60 + parts[j]; }
        return seconds;
    }
    function numAttr(el, name) {
        if (!el) { return null; }
        var raw = el.getAttribute(name);
        var n = raw !== null ? parseFloat(raw) : NaN;
        return isFinite(n) ? n : null;
    }
    // WICHTIG: video.currentTime/video.duration sind bei YouTube Music NICHT
    // verlaesslich pro Titel - bei nahtlosem (gapless) Songwechsel laeuft darunter
    // teils ein durchgehender Buffer weiter, dessen currentTime/duration beim
    // Songwechsel NICHT auf den neuen Titel zurueckgesetzt wird, sondern einfach ueber
    // mehrere Songs hinweg weiterzaehlt (empirisch bestaetigt: nach einem Songwechsel
    // wurde die verbleibende Spielzeit des VORHERIGEN Titels als Start-Offset des NEUEN
    // uebernommen, und die "Dauer" wuchs bei jedem Poll weiter statt konstant zu
    // bleiben). Deshalb wird die sichtbar angezeigte Fortschrittsanzeige ausgelesen
    // (ARIA-Attribute des Sliders in Sekunden, sonst der Zeit-Text im Player) - die
    // zeigt garantiert den Fortschritt des GERADE LAUFENDEN Titels, weil der Nutzer sie
    // ja selbst so sieht.
    function musicTimeInfo() {
        var slider = document.querySelector('#progress-bar, tp-yt-paper-slider#progress-bar');
        var current = numAttr(slider, 'aria-valuenow');
        var total = numAttr(slider, 'aria-valuemax');
        if (current !== null && total !== null && total > 0) {
            return { current: current, total: total };
        }
        var el = document.querySelector('.time-info.ytmusic-player-bar, ytmusic-player-bar .time-info');
        var parts = el ? (el.textContent || '').split('/') : [];
        if (parts.length === 2) {
            var c = parseClock(parts[0]), t = parseClock(parts[1]);
            if (c !== null && t !== null) { return { current: c, total: t }; }
        }
        return null;
    }
    function youtubeTimeInfo() {
        var current = parseClock(txt('.ytp-time-current'));
        var total = parseClock(txt('.ytp-time-duration'));
        if (current !== null && total !== null) { return { current: current, total: total }; }
        return null;
    }

    var video = document.querySelector('video');
    var isMusic = location.hostname.indexOf('music.youtube.com') !== -1;
    var videoId = videoIdFromUrl(location.href);
    var title = null, subtitle = null, thumbnail = null;

    if (isMusic) {
        title = txt('.title.ytmusic-player-bar') || txt('ytmusic-player-bar .title');
        subtitle = txt('.byline.ytmusic-player-bar') || txt('ytmusic-player-bar .byline');
        thumbnail = imgSrc('ytmusic-player-bar img, .image.ytmusic-player-bar img') || ytThumbnailUrl(videoId);
    } else {
        var t = document.title.replace(/ - YouTube$/, '');
        title = t || null;
        subtitle = txt('ytd-video-owner-renderer #channel-name a')
            || txt('#owner #channel-name a')
            || txt('#channel-name a')
            || txt('ytd-channel-name#channel-name a');
        var imgY = document.querySelector('link[rel="image_src"]');
        thumbnail = (imgY ? imgY.href : null) || ytThumbnailUrl(videoId);
    }

    var timeInfo = isMusic ? musicTimeInfo() : youtubeTimeInfo();
    var currentTime = timeInfo ? timeInfo.current : (video ? video.currentTime : 0);
    var duration = timeInfo ? timeInfo.total : ((video && isFinite(video.duration)) ? video.duration : 0);

    // WICHTIG: QtWebEngine's runJavaScript()-Bruecke liefert bei einem direkt
    // zurueckgegebenen JS-Objekt zuverlaessig nur einen leeren String statt des
    // Objekts (Zahlen/Strings funktionieren, Objekte nicht) - deshalb hier als
    // JSON-String zurueckgeben und in Python mit json.loads() wieder parsen
    // (siehe BrowserTab._on_media_probe_result in browser.py).
    return JSON.stringify({
        isMusic: isMusic,
        title: title,
        subtitle: subtitle,
        thumbnail: thumbnail,
        url: location.href,
        playing: video ? (!video.paused && !video.ended && video.readyState > 2) : false,
        currentTime: currentTime,
        duration: duration,
        hasVideo: !!video
    });
})();
"""

TOGGLE_PLAYBACK_JS = r"""
(function() {
    var video = document.querySelector('video');
    if (video) { video.paused ? video.play() : video.pause(); }
})();
"""

NEXT_TRACK_JS = r"""
(function() {
    var btn = document.querySelector('.next-button, ytmusic-player-bar #next-button button, tp-yt-paper-icon-button.next-button');
    if (btn) { btn.click(); }
})();
"""

PREV_TRACK_JS = r"""
(function() {
    var btn = document.querySelector('.previous-button, ytmusic-player-bar #previous-button button, tp-yt-paper-icon-button.previous-button');
    if (btn) { btn.click(); }
})();
"""
