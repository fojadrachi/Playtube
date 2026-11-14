(function() {
    // Cover einer YouTube-Music-Playlist (fuer die Playlist-Tasten am Stream Dock). Die Seitenleiste
    // hat keine Bilder - deshalb die Playlist-Seite mit dem Login dieses Tabs laden und das Bild aus
    // og:image bzw. dem Kopfbereich der Seitendaten lesen. __ID__ ist vorher in src/remote.rs geprueft.
    // Synchrone Anfrage, weil WebView2 auf Promises nicht wartet; dauert nur einen Augenblick.
    var id = "__ID__";
    try {
        var xhr = new XMLHttpRequest();
        xhr.open('GET', '/playlist?list=' + encodeURIComponent(id), false);
        xhr.send();
        if (xhr.status !== 200) { return JSON.stringify(null); }
        var html = xhr.responseText || '';
        var meta = /<meta property="og:image" content="([^"]+)"/.exec(html);
        if (meta && /^https:\/\//.test(meta[1])) { return JSON.stringify(meta[1].replace(/&amp;/g, '&')); }
        // Seitendaten stehen hex-kodiert im HTML ("\x22thumbnails\x22...").
        var data = html.replace(/\\x([0-9a-fA-F]{2})/g, function(_, h) { return String.fromCharCode(parseInt(h, 16)); })
                       .replace(/\\\//g, '/').replace(/\\u0026/g, '&');
        var start = data.search(/(musicResponsiveHeaderRenderer|musicDetailHeaderRenderer|musicEditablePlaylistDetailHeaderRenderer)/);
        var part = start >= 0 ? data.slice(start, start + 20000) : data;
        var list = /"thumbnails":\[([^\]]+)\]/.exec(part);
        if (!list) { return JSON.stringify(null); }
        var urls = list[1].match(/https:\/\/[^"\\]+/g) || [];
        return JSON.stringify(urls.length ? urls[urls.length - 1] : null);
    } catch (e) {
        return JSON.stringify(null);
    }
})();
