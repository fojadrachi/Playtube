(function() {
    // Liest die aktuelle Wiedergabeliste (YouTube Music: Warteschlange im Player; YouTube: Playlist
    // neben dem Video). Liefert JSON-Text; src/remote.rs (sanitize_queue) bereinigt die Daten.
    function text(el) { return el ? (el.textContent || '').replace(/\s+/g, ' ').trim() : ''; }
    function runs(formatted) {
        if (!formatted) { return ''; }
        if (typeof formatted.simpleText === 'string') { return formatted.simpleText; }
        return (formatted.runs || []).map(function(r) { return r.text || ''; }).join('');
    }
    function lastThumb(data) {
        var list = data && data.thumbnail && data.thumbnail.thumbnails;
        return (list && list.length) ? list[list.length - 1].url : '';
    }
    function img(el) {
        var i = el.querySelector('img');
        var src = i ? (i.getAttribute('src') || '') : '';
        return /^https:\/\//i.test(src) ? src : '';
    }
    var out = [];
    if (location.hostname.indexOf('music.youtube.com') !== -1) {
        var items = document.querySelectorAll('ytmusic-player-queue ytmusic-player-queue-item');
        for (var i = 0; i < items.length; i++) {
            var it = items[i];
            var data = it.data || (it.__data && it.__data.data) || null;
            out.push({
                title: text(it.querySelector('.song-title')) || runs(data && data.title),
                subtitle: text(it.querySelector('.byline')) || runs(data && data.shortBylineText),
                thumbnail: img(it) || lastThumb(data),
                current: it.hasAttribute('selected') || it.getAttribute('play-button-state') === 'playing' || it.getAttribute('play-button-state') === 'paused'
            });
        }
    } else {
        var panel = document.querySelector('ytd-watch-flexy ytd-playlist-panel-renderer #items') || document.querySelector('ytd-playlist-panel-renderer #items');
        var rows = panel ? panel.querySelectorAll('ytd-playlist-panel-video-renderer') : [];
        for (var j = 0; j < rows.length; j++) {
            var row = rows[j];
            var d = row.data || (row.__data && row.__data.data) || null;
            out.push({
                title: text(row.querySelector('#video-title')) || runs(d && d.title),
                subtitle: text(row.querySelector('#byline')) || runs(d && d.shortBylineText),
                thumbnail: img(row) || lastThumb(d),
                current: row.hasAttribute('selected')
            });
        }
    }
    return JSON.stringify(out);
})();
