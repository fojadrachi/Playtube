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
