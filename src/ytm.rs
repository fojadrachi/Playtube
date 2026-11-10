//! Eigene Oberfläche für YouTube Music: Daten über dieselbe Schnittstelle, die music.youtube.com
//! selbst benutzt ("InnerTube", /youtubei/v1/…), mit dem Login aus dem YouTube-Music-Tab
//! (Cookies der WebView). Abgespielt wird weiterhin im echten YouTube-Music-Player – deshalb
//! bleiben Premium (keine Werbung, Qualität), eigene Uploads und der Verlauf erhalten.
//!
//! Die Antworten sind tief verschachtelt und ändern sich gelegentlich. Der Parser sucht deshalb
//! nicht nach festen Pfaden, sondern nach bekannten Bausteinen ("…ShelfRenderer" = Regal,
//! "musicTwoRowItemRenderer" = Karte, "musicResponsiveListItemRenderer" = Zeile).

use serde_json::{json, Value};
use std::time::Duration;

pub const ORIGIN: &str = "https://music.youtube.com";
/// Rückfall, falls die Seite (ytcfg) keine Version liefert.
pub const FALLBACK_CLIENT_VERSION: &str = "1.20251103.01.00";
const USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0";

// ---------------------------------------------------------------------------- Datenmodell

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageType {
    Album,
    Playlist,
    Artist,
    Other,
}

/// Wohin ein Klick führt: abspielen (Song/Playlist) oder eine Seite öffnen.
#[derive(Clone, Debug, PartialEq)]
pub enum Nav {
    Play { video_id: Option<String>, playlist_id: Option<String> },
    Browse { browse_id: String, params: Option<String>, page: PageType },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub title: String,
    pub subtitle: String,
    pub thumb: Option<String>,
    pub duration: Option<String>,
    /// Klick auf die Karte/Zeile
    pub open: Option<Nav>,
    /// Abspiel-Knopf (z. B. ganze Playlist)
    pub play: Option<Nav>,
    /// Interpret: rundes Bild
    pub round: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shelf {
    pub title: String,
    pub entries: Vec<Entry>,
    /// Songzeilen statt Karten
    pub rows: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub title: String,
    pub subtitle: String,
    pub description: String,
    pub thumb: Option<String>,
    pub shelves: Vec<Shelf>,
    /// "Abspielen" für die ganze Seite (Album/Playlist)
    pub play: Option<Nav>,
}

// ---------------------------------------------------------------------------- Hilfen

/// Text aus `{"runs":[{"text":…}]}` oder `{"simpleText":…}`.
pub fn text(v: &Value) -> String {
    if let Some(s) = v["simpleText"].as_str() {
        return s.to_string();
    }
    v["runs"].as_array().map(|runs| runs.iter().filter_map(|r| r["text"].as_str()).collect::<String>()).unwrap_or_default()
}

/// Letztes (größtes) Bild aus dem ersten `thumbnails`-Feld unterhalb von `v`.
pub fn thumb(v: &Value) -> Option<String> {
    fn find(v: &Value, depth: u32) -> Option<String> {
        if depth > 8 {
            return None;
        }
        match v {
            Value::Object(map) => {
                if let Some(list) = map.get("thumbnails").and_then(Value::as_array) {
                    return list.last().and_then(|t| t["url"].as_str()).map(str::to_string);
                }
                map.values().find_map(|v| find(v, depth + 1))
            }
            Value::Array(list) => list.iter().find_map(|v| find(v, depth + 1)),
            _ => None,
        }
    }
    find(v, 0).map(|url| if url.starts_with("//") { format!("https:{url}") } else { url })
}

/// Bildadresse auf `px` Pixel umschreiben (Google-Bilddienst: `=w60-h60…` bzw. `=s192`).
pub fn sized_thumb(url: &str, px: u32) -> String {
    if !url.contains("googleusercontent.com") && !url.contains("ggpht.com") {
        return url.to_string();
    }
    match url.rfind('=') {
        Some(pos) => {
            let options = &url[pos + 1..];
            let rest: Vec<&str> = options.split('-').filter(|p| !(p.starts_with('w') || p.starts_with('h') || p.starts_with('s')) || p.len() < 2 || !p[1..].chars().all(|c| c.is_ascii_digit())).collect();
            let mut out = format!("{}=w{px}-h{px}", &url[..pos]);
            for part in rest {
                out.push('-');
                out.push_str(part);
            }
            out
        }
        None => format!("{url}=w{px}-h{px}"),
    }
}

fn page_type(endpoint: &Value) -> PageType {
    match endpoint["browseEndpointContextSupportedConfigs"]["browseEndpointContextMusicConfig"]["pageType"].as_str() {
        Some("MUSIC_PAGE_TYPE_ALBUM" | "MUSIC_PAGE_TYPE_AUDIOBOOK" | "MUSIC_PAGE_TYPE_SINGLE") => PageType::Album,
        Some("MUSIC_PAGE_TYPE_PLAYLIST") => PageType::Playlist,
        Some("MUSIC_PAGE_TYPE_ARTIST" | "MUSIC_PAGE_TYPE_USER_CHANNEL" | "MUSIC_PAGE_TYPE_LIBRARY_ARTIST") => PageType::Artist,
        _ => PageType::Other,
    }
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Ziel aus einem `navigationEndpoint`/`playNavigationEndpoint`.
pub fn nav(endpoint: &Value) -> Option<Nav> {
    if let Some(w) = endpoint.get("watchEndpoint") {
        let video_id = opt_str(&w["videoId"]);
        let playlist_id = opt_str(&w["playlistId"]);
        return (video_id.is_some() || playlist_id.is_some()).then_some(Nav::Play { video_id, playlist_id });
    }
    if let Some(w) = endpoint.get("watchPlaylistEndpoint") {
        return opt_str(&w["playlistId"]).map(|id| Nav::Play { video_id: None, playlist_id: Some(id) });
    }
    if let Some(b) = endpoint.get("browseEndpoint") {
        let browse_id = opt_str(&b["browseId"])?;
        return Some(Nav::Browse { browse_id, params: opt_str(&b["params"]), page: page_type(b) });
    }
    None
}

fn first_run_nav(v: &Value) -> Option<Nav> {
    v["runs"].as_array()?.iter().find_map(|r| r.get("navigationEndpoint").and_then(nav))
}

fn overlay_play(v: &Value) -> Option<Nav> {
    nav(&v["overlay"]["musicItemThumbnailOverlayRenderer"]["content"]["musicPlayButtonRenderer"]["playNavigationEndpoint"])
        .or_else(|| nav(&v["thumbnailOverlay"]["musicItemThumbnailOverlayRenderer"]["content"]["musicPlayButtonRenderer"]["playNavigationEndpoint"]))
}

// ---------------------------------------------------------------------------- Bausteine

/// Karte (Album, Playlist, Interpret, Video …)
fn two_row(r: &Value) -> Entry {
    let open = r.get("navigationEndpoint").and_then(nav).or_else(|| first_run_nav(&r["title"]));
    let round = matches!(open, Some(Nav::Browse { page: PageType::Artist, .. }));
    Entry {
        title: text(&r["title"]),
        subtitle: text(&r["subtitle"]),
        thumb: thumb(&r["thumbnailRenderer"]),
        duration: None,
        play: overlay_play(r),
        open,
        round,
    }
}

/// Zeile (Song, Video, oder in Suchergebnissen auch Album/Interpret/Playlist)
fn list_item(r: &Value) -> Entry {
    let columns: Vec<String> = r["flexColumns"]
        .as_array()
        .map(|cols| cols.iter().map(|c| text(&c["musicResponsiveListItemFlexColumnRenderer"]["text"])).collect())
        .unwrap_or_default();
    let title_runs = &r["flexColumns"][0]["musicResponsiveListItemFlexColumnRenderer"]["text"];
    let video_id = opt_str(&r["playlistItemData"]["videoId"]);
    let play = overlay_play(r).or_else(|| first_run_nav(title_runs).filter(|n| matches!(n, Nav::Play { .. }))).or_else(|| {
        video_id.clone().map(|id| Nav::Play { video_id: Some(id), playlist_id: None })
    });
    let open = r.get("navigationEndpoint").and_then(nav).or_else(|| first_run_nav(title_runs)).or_else(|| play.clone());
    let round = matches!(open, Some(Nav::Browse { page: PageType::Artist, .. }));
    let duration = r["fixedColumns"][0]["musicResponsiveListItemFixedColumnRenderer"]["text"].clone();
    Entry {
        title: columns.first().cloned().unwrap_or_default(),
        subtitle: columns.iter().skip(1).filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>().join(" • "),
        thumb: thumb(&r["thumbnail"]),
        duration: Some(text(&duration)).filter(|d| !d.is_empty()),
        open,
        play,
        round,
    }
}

/// "Top-Ergebnis" der Suche
fn card_shelf(r: &Value) -> Entry {
    let open = r.get("onTap").and_then(nav).or_else(|| first_run_nav(&r["title"]));
    Entry {
        title: text(&r["title"]),
        subtitle: text(&r["subtitle"]),
        thumb: thumb(&r["thumbnail"]),
        duration: None,
        play: nav(&r["thumbnailOverlay"]["musicItemThumbnailOverlayRenderer"]["content"]["musicPlayButtonRenderer"]["playNavigationEndpoint"])
            .or_else(|| first_run_nav(&r["title"]).filter(|n| matches!(n, Nav::Play { .. }))),
        round: matches!(open, Some(Nav::Browse { page: PageType::Artist, .. })),
        open,
    }
}

fn item(v: &Value) -> Option<Entry> {
    if let Some(r) = v.get("musicTwoRowItemRenderer") {
        return Some(two_row(r));
    }
    if let Some(r) = v.get("musicResponsiveListItemRenderer") {
        return Some(list_item(r));
    }
    if let Some(r) = v.get("musicMultiRowListItemRenderer") {
        return Some(Entry {
            title: text(&r["title"]),
            subtitle: text(&r["subtitle"]),
            thumb: thumb(&r["thumbnail"]),
            open: r.get("onTap").and_then(nav).or_else(|| first_run_nav(&r["title"])),
            ..Default::default()
        });
    }
    None
}

fn shelf_title(header: &Value) -> String {
    for key in ["musicCarouselShelfBasicHeaderRenderer", "gridHeaderRenderer", "musicShelfHeaderRenderer"] {
        let t = text(&header[key]["title"]);
        if !t.is_empty() {
            return t;
        }
    }
    String::new()
}

fn entries(contents: &Value) -> Vec<Entry> {
    contents.as_array().map(|items| items.iter().filter_map(item).filter(|e| !e.title.is_empty()).collect()).unwrap_or_default()
}

/// Sucht rekursiv alle Regale.
fn collect_shelves(v: &Value, out: &mut Vec<Shelf>) {
    match v {
        Value::Object(map) => {
            for (key, value) in map {
                match key.as_str() {
                    "musicCarouselShelfRenderer" => {
                        push(out, Shelf { title: shelf_title(&value["header"]), entries: entries(&value["contents"]), rows: is_row(value) });
                    }
                    "musicShelfRenderer" | "musicPlaylistShelfRenderer" => {
                        let mut title = text(&value["title"]);
                        if title.is_empty() {
                            title = shelf_title(&value["header"]);
                        }
                        push(out, Shelf { title, entries: entries(&value["contents"]), rows: true });
                    }
                    "gridRenderer" => {
                        push(out, Shelf { title: shelf_title(&value["header"]), entries: entries(&value["items"]), rows: false });
                    }
                    "musicCardShelfRenderer" => {
                        let mut list = vec![card_shelf(value)];
                        list.extend(entries(&value["contents"]));
                        push(out, Shelf { title: "\u{2605}".into(), entries: list, rows: true });
                    }
                    "itemSectionRenderer" => {
                        // Neue Suche: jedes Ergebnis ist ein eigener Abschnitt - zusammenfassen.
                        let found = entries(&value["contents"]);
                        if found.is_empty() {
                            collect_shelves(value, out);
                        } else if let Some(last) = out.last_mut().filter(|s| s.title.is_empty() && s.rows) {
                            last.entries.extend(found);
                        } else {
                            out.push(Shelf { title: String::new(), entries: found, rows: true });
                        }
                    }
                    // Kopfbereich und Menüs nicht als Regale lesen
                    "header" | "menu" | "musicResponsiveHeaderRenderer" | "musicImmersiveHeaderRenderer" | "musicDetailHeaderRenderer" => {}
                    _ => collect_shelves(value, out),
                }
            }
        }
        Value::Array(list) => list.iter().for_each(|v| collect_shelves(v, out)),
        _ => {}
    }
}

fn is_row(shelf: &Value) -> bool {
    shelf["contents"][0].get("musicResponsiveListItemRenderer").is_some()
}

fn push(out: &mut Vec<Shelf>, shelf: Shelf) {
    if !shelf.entries.is_empty() {
        out.push(shelf);
    }
}

fn find_key<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Object(map) => map.get(key).or_else(|| map.values().find_map(|v| find_key(v, key))),
        Value::Array(list) => list.iter().find_map(|v| find_key(v, key)),
        _ => None,
    }
}

/// Seite (Start, Suche, Album, Playlist, Interpret, Bibliothek) aus einer Antwort.
pub fn parse_page(response: &Value) -> Page {
    let mut page = Page::default();
    for key in ["musicResponsiveHeaderRenderer", "musicImmersiveHeaderRenderer", "musicVisualHeaderRenderer", "musicDetailHeaderRenderer"] {
        if let Some(h) = find_key(response, key) {
            page.title = text(&h["title"]);
            let parts: Vec<String> = [text(&h["straplineTextOne"]), text(&h["subtitle"]), text(&h["secondSubtitle"])]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect();
            page.subtitle = parts.join(" • ");
            page.description = text(&h["description"]["musicDescriptionShelfRenderer"]["description"]);
            if page.description.is_empty() {
                page.description = text(&h["description"]);
            }
            page.thumb = thumb(&h["thumbnail"]).or_else(|| thumb(&h["foregroundThumbnail"]));
            break;
        }
    }
    collect_shelves(&response["contents"], &mut page.shelves);
    // Ganze Seite abspielen: erster Song mit Playlist-ID (Album, Playlist).
    page.play = page
        .shelves
        .iter()
        .filter(|s| s.rows)
        .flat_map(|s| &s.entries)
        .find_map(|e| match &e.play {
            Some(Nav::Play { playlist_id: Some(_), .. }) => e.play.clone(),
            _ => None,
        });
    page
}

/// Adresse für den YouTube-Music-Player.
pub fn watch_url(nav: &Nav) -> Option<String> {
    let Nav::Play { video_id, playlist_id } = nav else { return None };
    let enc = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect::<String>();
    match (video_id, playlist_id) {
        (Some(v), Some(p)) => Some(format!("{ORIGIN}/watch?v={}&list={}", enc(v), enc(p))),
        (Some(v), None) => Some(format!("{ORIGIN}/watch?v={}", enc(v))),
        (None, Some(p)) => Some(format!("{ORIGIN}/watch?list={}", enc(p))),
        (None, None) => None,
    }
}

// ---------------------------------------------------------------------------- Anmeldung und Abruf

/// Zugang mit dem Login des YouTube-Music-Tabs.
#[derive(Clone, Debug, Default)]
pub struct Auth {
    /// `Cookie`-Kopfzeile (aus der WebView)
    pub cookie: String,
    /// Wert des Cookies SAPISID (bzw. __Secure-3PAPISID) - nur angemeldet
    pub sapisid: Option<String>,
    pub client_version: String,
    pub visitor_data: Option<String>,
    pub hl: String,
    pub gl: String,
    pub session_index: String,
    pub on_behalf_of: Option<String>,
}

impl Auth {
    pub fn logged_in(&self) -> bool {
        self.sapisid.is_some()
    }

    /// Aus den Cookies der WebView (Name, Wert) und den Angaben aus `ytcfg` der Seite.
    pub fn from_parts(cookies: &[(String, String)], ytcfg: &Value) -> Self {
        let cookie = cookies.iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join("; ");
        let find = |name: &str| cookies.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
        let sapisid = find("SAPISID").or_else(|| find("__Secure-3PAPISID"));
        Self {
            cookie,
            sapisid,
            client_version: opt_str(&ytcfg["INNERTUBE_CLIENT_VERSION"]).unwrap_or_else(|| FALLBACK_CLIENT_VERSION.into()),
            visitor_data: opt_str(&ytcfg["VISITOR_DATA"]),
            hl: opt_str(&ytcfg["HL"]).unwrap_or_else(|| "de".into()),
            gl: opt_str(&ytcfg["GL"]).unwrap_or_else(|| "DE".into()),
            session_index: ytcfg["SESSION_INDEX"].as_u64().map(|n| n.to_string()).or_else(|| opt_str(&ytcfg["SESSION_INDEX"])).unwrap_or_else(|| "0".into()),
            on_behalf_of: opt_str(&ytcfg["DELEGATED_SESSION_ID"]),
        }
    }

    fn context(&self) -> Value {
        let mut client = json!({ "clientName": "WEB_REMIX", "clientVersion": self.client_version, "hl": self.hl, "gl": self.gl });
        if let Some(v) = &self.visitor_data {
            client["visitorData"] = json!(v);
        }
        let mut context = json!({ "client": client });
        if let Some(id) = &self.on_behalf_of {
            context["user"] = json!({ "onBehalfOfUser": id });
        }
        context
    }
}

/// `SAPISIDHASH <zeit>_<sha1(zeit SAPISID origin)>` - so meldet sich auch die Seite selbst an.
pub fn sapisid_hash(sapisid: &str, timestamp: u64, origin: &str) -> String {
    use sha1::{Digest, Sha1};
    let digest = Sha1::digest(format!("{timestamp} {sapisid} {origin}").as_bytes());
    format!("SAPISIDHASH {timestamp}_{}", hex::encode(digest))
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().timeout(Duration::from_secs(15)).build().unwrap_or_default()
}

pub async fn call(auth: &Auth, endpoint: &str, mut body: Value) -> anyhow::Result<Value> {
    body["context"] = auth.context();
    let mut request = client()
        .post(format!("{ORIGIN}/youtubei/v1/{endpoint}?prettyPrint=false"))
        .header("Content-Type", "application/json")
        .header("Origin", ORIGIN)
        .header("X-Origin", ORIGIN)
        .header("Referer", format!("{ORIGIN}/"))
        .header("User-Agent", USER_AGENT)
        .header("X-Goog-AuthUser", &auth.session_index)
        .json(&body);
    if !auth.cookie.is_empty() {
        request = request.header("Cookie", &auth.cookie);
    }
    if let Some(sapisid) = &auth.sapisid {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        request = request.header("Authorization", sapisid_hash(sapisid, now, ORIGIN));
    }
    if let Some(v) = &auth.visitor_data {
        request = request.header("X-Goog-Visitor-Id", v);
    }
    let response = request.send().await?;
    anyhow::ensure!(response.status().is_success(), "YouTube Music: HTTP {}", response.status());
    Ok(response.json().await?)
}

pub async fn browse(auth: &Auth, browse_id: &str, params: Option<&str>) -> anyhow::Result<Page> {
    let mut body = json!({ "browseId": browse_id });
    if let Some(p) = params {
        body["params"] = json!(p);
    }
    Ok(parse_page(&call(auth, "browse", body).await?))
}

pub async fn search(auth: &Auth, query: &str) -> anyhow::Result<Page> {
    let mut page = parse_page(&call(auth, "search", json!({ "query": query })).await?);
    page.title = query.to_string();
    Ok(page)
}

/// Start-, Bibliotheks- und Lieblingsseiten.
pub const HOME: &str = "FEmusic_home";
pub const LIBRARY_PLAYLISTS: &str = "FEmusic_liked_playlists";
pub const LIKED_SONGS: &str = "VLLM";

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let text = match name {
            "home" => include_str!("../tests/fixtures/ytm_home.json"),
            "search" => include_str!("../tests/fixtures/ytm_search.json"),
            "playlist" => include_str!("../tests/fixtures/ytm_playlist.json"),
            _ => include_str!("../tests/fixtures/ytm_album.json"),
        };
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn reads_home_shelves() {
        let page = parse_page(&fixture("home"));
        assert!(page.shelves.len() >= 3, "{}", page.shelves.len());
        let first = &page.shelves[0];
        assert_eq!(first.title, "Schnellauswahl");
        assert!(first.rows, "Schnellauswahl sind Songzeilen");
        assert!(matches!(first.entries[0].play, Some(Nav::Play { video_id: Some(_), .. })));
        let cards = page.shelves.iter().find(|s| !s.rows).expect("Kartenregal");
        let card = &cards.entries[0];
        assert!(!card.title.is_empty() && card.thumb.is_some());
        assert!(matches!(card.open, Some(Nav::Browse { .. })), "{:?}", card.open);
    }

    #[test]
    fn reads_search_results() {
        let page = parse_page(&fixture("search"));
        let top = &page.shelves[0];
        assert_eq!(top.title, "\u{2605}");
        assert!(top.entries[0].title.to_lowercase().contains("bohemian"), "{}", top.entries[0].title);
        let all: usize = page.shelves.iter().map(|s| s.entries.len()).sum();
        assert!(all >= 15, "{all}");
        let album = page.shelves.iter().flat_map(|s| &s.entries).find(|e| matches!(e.open, Some(Nav::Browse { page: PageType::Album, .. })));
        assert!(album.is_some(), "ein Album in den Ergebnissen");
    }

    #[test]
    fn reads_playlist_page() {
        let page = parse_page(&fixture("playlist"));
        assert_eq!(page.title, "80er Pop");
        assert!(page.subtitle.contains("Playlist"));
        assert!(page.thumb.is_some());
        let tracks = &page.shelves[0];
        assert!(tracks.rows && tracks.entries.len() >= 50, "{}", tracks.entries.len());
        let song = &tracks.entries[0];
        assert_eq!(song.title, "Total Eclipse of the Heart");
        assert!(song.duration.is_some());
        assert!(matches!(&page.play, Some(Nav::Play { playlist_id: Some(_), .. })));
    }

    #[test]
    fn reads_album_page() {
        let page = parse_page(&fixture("album"));
        assert_eq!(page.title, "A Night At The Opera");
        assert!(page.subtitle.starts_with("Queen"), "{}", page.subtitle);
        assert!(page.shelves[0].rows);
        assert_eq!(page.shelves[0].entries[0].title, "Death On Two Legs (Dedicated To...)");
        let url = watch_url(page.play.as_ref().unwrap()).unwrap();
        assert!(url.starts_with("https://music.youtube.com/watch?v=") && url.contains("&list=OLAK"), "{url}");
    }

    #[test]
    fn builds_watch_urls_safely() {
        let nav = Nav::Play { video_id: Some("abc&evil=1".into()), playlist_id: None };
        assert_eq!(watch_url(&nav).unwrap(), "https://music.youtube.com/watch?v=abcevil1");
        assert_eq!(watch_url(&Nav::Play { video_id: None, playlist_id: Some("PL1".into()) }).unwrap(), "https://music.youtube.com/watch?list=PL1");
        assert!(watch_url(&Nav::Browse { browse_id: "x".into(), params: None, page: PageType::Other }).is_none());
    }

    #[test]
    fn resizes_google_images() {
        assert_eq!(sized_thumb("https://lh3.googleusercontent.com/abc=w60-h60-l90-rj", 226), "https://lh3.googleusercontent.com/abc=w226-h226-l90-rj");
        assert_eq!(sized_thumb("https://yt3.ggpht.com/abc=s192", 300), "https://yt3.ggpht.com/abc=w300-h300");
        assert_eq!(sized_thumb("https://i.ytimg.com/vi/x/hqdefault.jpg", 300), "https://i.ytimg.com/vi/x/hqdefault.jpg");
    }

    #[test]
    fn signs_like_the_website() {
        // Bekannter Wert: sha1("1700000000 abc https://music.youtube.com")
        let hash = sapisid_hash("abc", 1_700_000_000, ORIGIN);
        assert!(hash.starts_with("SAPISIDHASH 1700000000_"));
        assert_eq!(hash.len(), "SAPISIDHASH 1700000000_".len() + 40);
    }

    #[test]
    fn auth_from_cookies() {
        let cookies = vec![("SID".to_string(), "x".to_string()), ("__Secure-3PAPISID".to_string(), "p".to_string())];
        let auth = Auth::from_parts(&cookies, &json!({ "INNERTUBE_CLIENT_VERSION": "1.2", "HL": "en", "SESSION_INDEX": 1 }));
        assert_eq!(auth.cookie, "SID=x; __Secure-3PAPISID=p");
        assert_eq!(auth.sapisid.as_deref(), Some("p"));
        assert_eq!(auth.client_version, "1.2");
        assert_eq!(auth.hl, "en");
        assert_eq!(auth.session_index, "1");
        assert!(!Auth::from_parts(&[], &json!({})).logged_in());
    }
}
