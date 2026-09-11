"""JavaScript-"Shim", der QtWebEngine (echtes, aber unbranded Chromium) fuer
JS-seitige Fingerprint-Checks wie echtes Google Chrome aussehen laesst.

Hintergrund: Google blockiert Logins aus WebViews/Embedded-Browsern serverseitig
("Dieser Browser oder diese App ist unter Umstaenden nicht sicher"). Die dazu
verwendeten Signale sind u.a.:
  - Fehlendes `window.chrome` Objekt (QtWebEngine ist ein Open-Source-Chromium-Build
    ohne Google-Branding, hat das normalerweise nicht).
  - `navigator.userAgentData` (JS Client Hints), das bei QtWebEngine "Chromium" statt
    "Google Chrome" als Marke meldet.
  - Fehlende/leere `navigator.plugins` (echtes Chrome hat u.a. immer den eingebauten
    PDF-Viewer als Plugin gelistet).

Die HTTP-Header-Variante (Sec-CH-UA) wird zusaetzlich in browser.py gesetzt - Google
prueft offenbar beides, JS-seitig und Header-seitig, daher muessen beide konsistent
"Google Chrome" melden.

Dies aendert nur, WIE sich der eingebettete Browser bei Feature-Detection meldet, es
manipuliert keine fremden Konten und automatisiert keine Logins - es ermoeglicht dem
Nutzer lediglich, sich in der eigenen App mit dem eigenen Google-Konto anzumelden,
so wie es in vielen anderen Desktop-Clients (z.B. Mail-/Chat-Sammler-Apps) ueblich ist.
"""

CHROME_MAJOR = "140"
CHROME_FULL = "140.0.0.0"

CHROME_SHIM_JS = f"""
(function() {{
    try {{
        Object.defineProperty(navigator, 'webdriver', {{ get: () => undefined, configurable: true }});
    }} catch (e) {{}}

    try {{
        if (typeof window.chrome === 'undefined') {{ window.chrome = {{}}; }}
        window.chrome.runtime = window.chrome.runtime || {{}};
        window.chrome.loadTimes = window.chrome.loadTimes || function() {{
            var now = Date.now() / 1000;
            return {{
                commitLoadTime: now, connectionInfo: 'h2',
                finishDocumentLoadTime: now, finishLoadTime: now,
                firstPaintAfterLoadTime: 0, firstPaintTime: now,
                navigationType: 'Other', npnNegotiatedProtocol: 'h2',
                requestTime: now - 1, startLoadTime: now - 1,
                wasAlternateProtocolAvailable: false,
                wasFetchedViaSpdy: true, wasNpnNegotiated: true
            }};
        }};
        window.chrome.csi = window.chrome.csi || function() {{
            return {{ onloadT: Date.now(), pageT: Date.now(), startE: Date.now(), tran: 15 }};
        }};
        window.chrome.app = window.chrome.app || {{
            isInstalled: false,
            InstallState: {{ DISABLED: 'disabled', INSTALLED: 'installed', NOT_INSTALLED: 'not_installed' }},
            RunningState: {{ CANNOT_RUN: 'cannot_run', READY_TO_RUN: 'ready_to_run', RUNNING: 'running' }}
        }};
    }} catch (e) {{}}

    try {{
        var brands = [
            {{ brand: 'Not(A:Brand', version: '99' }},
            {{ brand: 'Google Chrome', version: '{CHROME_MAJOR}' }},
            {{ brand: 'Chromium', version: '{CHROME_MAJOR}' }}
        ];
        var fullBrands = [
            {{ brand: 'Not(A:Brand', version: '99.0.0.0' }},
            {{ brand: 'Google Chrome', version: '{CHROME_FULL}' }},
            {{ brand: 'Chromium', version: '{CHROME_FULL}' }}
        ];
        var uaData = {{
            brands: brands,
            mobile: false,
            platform: 'Windows',
            getHighEntropyValues: function(hints) {{
                var full = {{
                    architecture: 'x86', bitness: '64', brands: brands,
                    fullVersionList: fullBrands, mobile: false, model: '',
                    platform: 'Windows', platformVersion: '15.0.0',
                    uaFullVersion: '{CHROME_FULL}', wow64: false
                }};
                var result = {{}};
                (hints || Object.keys(full)).forEach(function(k) {{
                    if (k in full) {{ result[k] = full[k]; }}
                }});
                return Promise.resolve(result);
            }},
            toJSON: function() {{ return {{ brands: brands, mobile: false, platform: 'Windows' }}; }}
        }};
        Object.defineProperty(navigator, 'userAgentData', {{ get: () => uaData, configurable: true }});
    }} catch (e) {{}}

    try {{
        var fakePlugin = {{
            name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer',
            description: 'Portable Document Format', length: 1
        }};
        var fakePlugins = [fakePlugin];
        fakePlugins.item = function(i) {{ return fakePlugins[i]; }};
        fakePlugins.namedItem = function() {{ return fakePlugin; }};
        fakePlugins.refresh = function() {{}};
        Object.defineProperty(navigator, 'plugins', {{ get: () => fakePlugins, configurable: true }});
        Object.defineProperty(navigator, 'mimeTypes', {{ get: () => [], configurable: true }});
    }} catch (e) {{}}

    try {{
        Object.defineProperty(navigator, 'languages', {{
            get: () => ['de-DE', 'de', 'en-US', 'en'], configurable: true
        }});
    }} catch (e) {{}}

    try {{
        var origQuery = window.navigator.permissions && window.navigator.permissions.query;
        if (origQuery) {{
            window.navigator.permissions.query = function(params) {{
                if (params && params.name === 'notifications') {{
                    return Promise.resolve({{ state: Notification.permission }});
                }}
                return origQuery(params);
            }};
        }}
    }} catch (e) {{}}
}})();
"""
