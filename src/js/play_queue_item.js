(function() {
    // Spielt Eintrag Nr. __INDEX__ der Wiedergabeliste ab (gleiche Reihenfolge wie queue.js).
    var index = __INDEX__;
    if (location.hostname.indexOf('music.youtube.com') !== -1) {
        var items = document.querySelectorAll('ytmusic-player-queue ytmusic-player-queue-item');
        var it = items[index];
        if (!it) { return false; }
        var button = it.querySelector('ytmusic-play-button-renderer');
        if (button) { button.click(); } else { it.click(); }
        return true;
    }
    var panel = document.querySelector('ytd-watch-flexy ytd-playlist-panel-renderer #items') || document.querySelector('ytd-playlist-panel-renderer #items');
    var rows = panel ? panel.querySelectorAll('ytd-playlist-panel-video-renderer') : [];
    var row = rows[index];
    if (!row) { return false; }
    var link = row.querySelector('a#wc-endpoint') || row.querySelector('a');
    if (link) { link.click(); } else { row.click(); }
    return true;
})();
