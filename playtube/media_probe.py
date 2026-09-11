"""JavaScript, das periodisch in die YouTube- bzw. YouTube-Music-Seite injiziert wird,
um Titel/Interpret/Wiedergabestatus fuer die Discord-Rich-Presence auszulesen."""

MEDIA_PROBE_JS = r"""
(function() {
    function txt(sel) {
        var el = document.querySelector(sel);
        return el ? el.textContent.trim() : null;
    }
    var video = document.querySelector('video');
    var isMusic = location.hostname.indexOf('music.youtube.com') !== -1;
    var title = null, subtitle = null, thumbnail = null;

    if (isMusic) {
        title = txt('.title.ytmusic-player-bar') || txt('ytmusic-player-bar .title');
        subtitle = txt('.byline.ytmusic-player-bar') || txt('ytmusic-player-bar .byline');
        var imgM = document.querySelector('ytmusic-player-bar img, .image.ytmusic-player-bar img');
        thumbnail = imgM ? imgM.src : null;
    } else {
        var t = document.title.replace(/ - YouTube$/, '');
        title = t || null;
        subtitle = txt('ytd-video-owner-renderer #channel-name a')
            || txt('#owner #channel-name a')
            || txt('#channel-name a')
            || txt('ytd-channel-name#channel-name a');
        var imgY = document.querySelector('link[rel="image_src"]');
        thumbnail = imgY ? imgY.href : null;
    }

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
        currentTime: video ? video.currentTime : 0,
        duration: (video && isFinite(video.duration)) ? video.duration : 0,
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
