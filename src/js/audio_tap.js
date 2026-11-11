(function() {
    // Playtube-Ton: Der Ton der Seite wird hier abgegriffen (Web Audio) und an Playtube geschickt,
    // das ihn selbst ausgibt (src/relay.rs). So kommt der Ton aus Playtube.exe statt aus dem
    // Hilfsprozess msedgewebview2.exe: Discord überträgt ihn beim Teilen des Fensters, und Sonar &
    // Co. zeigen "Playtube" an. Ist die Funktion aus, läuft der Ton wie gewohnt über die Seite.
    if (window.__playtubeTap) { return; }
    var DEFAULT_ON = __ENABLED__;
    var state = { enabled: DEFAULT_ON, ctx: null, taps: [] };
    window.__playtubeTap = state;

    function post(text) {
        try { if (window.ipc && window.ipc.postMessage) { window.ipc.postMessage(text); } } catch (e) {}
    }

    function context() {
        if (state.ctx) { return state.ctx; }
        var options = { sampleRate: 48000, latencyHint: 'interactive' };
        try { options.sinkId = { type: 'none' }; } catch (e) {}
        try { state.ctx = new AudioContext(options); }
        catch (e) { state.ctx = new AudioContext({ sampleRate: 48000 }); }
        return state.ctx;
    }

    // Float -> 16 Bit, als Base64 (kompakt für die Übergabe an Playtube).
    function encode(left, right, gain) {
        var n = left.length;
        var bytes = new Uint8Array(n * 4);
        for (var i = 0; i < n; i++) {
            var l = Math.max(-1, Math.min(1, left[i] * gain)) * 32767;
            var r = Math.max(-1, Math.min(1, right[i] * gain)) * 32767;
            var a = l | 0, b = r | 0;
            bytes[i * 4] = a & 255; bytes[i * 4 + 1] = (a >> 8) & 255;
            bytes[i * 4 + 2] = b & 255; bytes[i * 4 + 3] = (b >> 8) & 255;
        }
        var text = '';
        for (var j = 0; j < bytes.length; j += 8192) {
            text += String.fromCharCode.apply(null, bytes.subarray(j, j + 8192));
        }
        return btoa(text);
    }

    function tap(el) {
        if (el.__playtubeTap || !state.enabled) { return; }
        var ctx = context();
        var source;
        try { source = ctx.createMediaElementSource(el); } catch (e) { return; }
        // 2048 Frames (~43 ms) je Stück: weniger Aufrufe, robuster gegen kurze Ruckler der Seite.
        var proc = ctx.createScriptProcessor(2048, 2, 2);
        var sink = ctx.createMediaStreamDestination();
        source.connect(proc);
        proc.connect(sink);
        proc.onaudioprocess = function(event) {
            var input = event.inputBuffer;
            if (!state.enabled) { return; }
            var left = input.getChannelData(0);
            var right = input.numberOfChannels > 1 ? input.getChannelData(1) : left;
            // Stille (Pause) nicht übertragen - Playtube füllt Lücken selbst mit Stille.
            var loud = false;
            for (var i = 0; i < left.length; i += 16) { if (left[i] !== 0 || right[i] !== 0) { loud = true; break; } }
            if (!loud) { return; }
            // Lautstärke/Stumm der Seite wendet Chrome schon vor diesem Knoten an.
            post('pcm:' + ctx.sampleRate + ':' + encode(left, right, 1));
        };
        el.__playtubeTap = { source: source, proc: proc };
        state.taps.push(el.__playtubeTap);
        if (ctx.state === 'suspended') { ctx.resume(); }
    }

    function scan() {
        var media = document.querySelectorAll('video, audio');
        for (var i = 0; i < media.length; i++) { tap(media[i]); }
    }

    // Jede Wiedergabe erfassen (auch Elemente, die erst später entstehen).
    document.addEventListener('play', function(e) {
        if (e.target && (e.target.tagName === 'VIDEO' || e.target.tagName === 'AUDIO')) {
            tap(e.target);
            if (state.ctx && state.ctx.state === 'suspended') { state.ctx.resume(); }
        }
    }, true);

    // An/aus zur Laufzeit (Einstellung). Schon erfasste Elemente gehen beim Ausschalten wieder
    // direkt über die Seite raus.
    state.enable = function(on) {
        state.enabled = !!on;
        if (state.enabled) {
            if (state.ctx && state.ctx.setSinkId) { try { state.ctx.setSinkId({ type: 'none' }); } catch (e) {} }
            for (var i = 0; i < state.taps.length; i++) {
                try { state.taps[i].source.disconnect(); state.taps[i].source.connect(state.taps[i].proc); } catch (e) {}
            }
            scan();
        } else if (state.ctx) {
            if (state.ctx.setSinkId) { try { state.ctx.setSinkId(''); } catch (e) {} }
            for (var k = 0; k < state.taps.length; k++) {
                try { state.taps[k].source.disconnect(); state.taps[k].source.connect(state.ctx.destination); } catch (e) {}
            }
        }
        return state.enabled;
    };
})();
