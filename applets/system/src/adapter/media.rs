// SPDX-License-Identifier: GPL-3.0-only

//! Media controls for every MPRIS player, from upstream's audio applet subscription
//! (`crate::upstream::audio::mpris_subscription`). Upstream shows only the active player; we list
//! all of them (its marked `Players` update) and send each player's requests to that player,
//! with the same calls upstream uses (`play`, `pause`, `next`, `previous`, and `raise` through
//! the player's MediaPlayer2 object).
//!
//! Art, in this order: upstream's cover (local `file://` covers only), a web cover upstream
//! ignores (downloaded here and cached), the player's application icon from its desktop entry,
//! and the note icon in the view. The web cover and the app icon are looked up here through the
//! player's own D-Bus proxy, so upstream's code stays unchanged.

use crate::upstream::audio::mpris_subscription::{
    self as up, ListedPlayer, MprisRequest, MprisUpdate, PlayerStatus,
};
use cosmic::iced::Subscription;
use mpris2_zbus::player::{PlaybackStatus, Player};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub fn subscription() -> Subscription<MprisUpdate> {
    up::mpris_subscription("system-applet-mpris")
}

/// The play/pause button: which icon it shows and whether it does anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayPause {
    /// True shows play (the player is paused or stopped), false shows pause.
    pub play: bool,
    pub enabled: bool,
}

/// One row: art, title, "artist · app", and three buttons that are always there, disabled when
/// the player can't do it, so rows line up.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRow {
    /// The player's bus name; requests are addressed by it.
    pub id: String,
    /// A cover: upstream's local one, or a downloaded web cover.
    pub art: Option<PathBuf>,
    /// The application's icon, shown when there's no cover.
    pub app_icon: Option<AppIcon>,
    pub title: String,
    pub artist: Option<String>,
    pub app: Option<String>,
    pub previous: bool,
    pub play_pause: PlayPause,
    pub next: bool,
}

/// What a row needs from upstream's `PlayerStatus`, as plain data (`PlayerStatus` holds a live
/// D-Bus proxy).
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub id: String,
    pub art: Option<PathBuf>,
    pub title: Option<String>,
    pub artists: Option<Vec<String>>,
    pub status: PlaybackStatus,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_go_previous: bool,
    pub can_go_next: bool,
    pub identity: Option<String>,
}

impl From<&ListedPlayer> for PlayerInfo {
    fn from(player: &ListedPlayer) -> Self {
        let s: &PlayerStatus = &player.status;
        Self {
            id: s.player.inner().destination().to_string(),
            art: s.icon.clone(),
            title: s.title.as_ref().map(|t| t.to_string()),
            artists: s
                .artists
                .as_ref()
                .map(|a| a.iter().map(|x| x.to_string()).collect()),
            status: s.status,
            can_play: s.can_play,
            can_pause: s.can_pause,
            can_go_previous: s.can_go_previous,
            can_go_next: s.can_go_next,
            identity: player.identity.clone(),
        }
    }
}

/// Upstream's `is_play`: `Some(true)` for play, `Some(false)` for pause, `None` when the player
/// supports neither in its current state.
fn is_play(info: &PlayerInfo) -> Option<bool> {
    match info.status {
        PlaybackStatus::Playing => info.can_pause.then_some(false),
        PlaybackStatus::Paused | PlaybackStatus::Stopped => info.can_play.then_some(true),
    }
}

pub fn row(info: &PlayerInfo) -> PlayerRow {
    let play = is_play(info);
    PlayerRow {
        id: info.id.clone(),
        art: info.art.clone(),
        app_icon: None,
        title: info.title.as_deref().map(str::to_owned).unwrap_or_default(),
        artist: info
            .artists
            .as_ref()
            .map(|a| a.join(", "))
            .filter(|a| !a.is_empty()),
        app: info.identity.clone().filter(|i| !i.is_empty()),
        previous: info.can_go_previous,
        play_pause: PlayPause {
            play: play.unwrap_or(!matches!(info.status, PlaybackStatus::Playing)),
            enabled: play.is_some(),
        },
        next: info.can_go_next,
    }
}

/// Rows in the order given (upstream's: by bus name), for players that are playing or paused.
/// Upstream's `find_active` gives a stopped player a score of 0, so the built-in applet never
/// shows one; this keeps e.g. a browser without media (registered on MPRIS, but stopped) out.
pub fn rows(infos: &[PlayerInfo]) -> Vec<PlayerRow> {
    infos
        .iter()
        .filter(|info| !matches!(info.status, PlaybackStatus::Stopped))
        .map(row)
        .collect()
}

/// An application icon: a theme icon name, or a file named in the desktop entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIcon {
    Name(String),
    Path(PathBuf),
}

/// What the adapter looked up for one player beyond upstream's status.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Extras {
    /// The player's cover address, when it is a web address.
    pub web_art_url: Option<String>,
    /// That cover, downloaded.
    pub web_art: Option<PathBuf>,
    pub app_icon: Option<AppIcon>,
}

/// A lookup to run: the player, and what the last lookup found (to skip a repeat download).
pub struct Lookup {
    pub id: String,
    pub player: Player,
    pub previous: Option<Extras>,
}

/// Every running player, in upstream's order, plus our extras per player.
#[derive(Debug, Default)]
pub struct Model {
    players: Vec<ListedPlayer>,
    extras: HashMap<String, Extras>,
    pending: HashSet<String>,
}

impl Model {
    pub fn update(&mut self, update: MprisUpdate) {
        match update {
            MprisUpdate::Players(players) => {
                self.players = players;
                let ids: HashSet<String> = self.players.iter().map(player_id).collect();
                self.extras.retain(|id, _| ids.contains(id));
            }
            MprisUpdate::Finished => {
                self.players.clear();
                self.extras.clear();
            }
            // Upstream's active player; our rows come from the full list.
            MprisUpdate::Setup | MprisUpdate::Player(_) => {}
        }
    }

    /// Players to look up after an update: those never looked up, and those without a local
    /// cover (their web cover may have changed with the track). One lookup per player at a time.
    pub fn lookups(&mut self) -> Vec<Lookup> {
        let mut out = Vec::new();
        for p in &self.players {
            let id = player_id(p);
            if self.pending.contains(&id) {
                continue;
            }
            let previous = self.extras.get(&id).cloned();
            if previous.is_some() && p.status.icon.is_some() {
                continue;
            }
            self.pending.insert(id.clone());
            out.push(Lookup {
                id,
                player: p.status.player.clone(),
                previous,
            });
        }
        out
    }

    pub fn set_extras(&mut self, id: String, extras: Extras) {
        self.pending.remove(&id);
        if self.players.iter().any(|p| player_id(p) == id) {
            self.extras.insert(id, extras);
        }
    }

    pub fn rows(&self) -> Vec<PlayerRow> {
        let infos: Vec<PlayerInfo> = self.players.iter().map(PlayerInfo::from).collect();
        rows(&infos)
            .into_iter()
            .map(|mut row| {
                if let Some(extras) = self.extras.get(&row.id) {
                    with_extras(&mut row, extras);
                }
                row
            })
            .collect()
    }

    /// Upstream's `MprisRequest` handling, for the player with this bus name.
    pub fn request(&self, id: &str, request: MprisRequest) {
        let Some(player) = self
            .players
            .iter()
            .find(|p| p.status.player.inner().destination().as_str() == id)
        else {
            return;
        };
        let player = player.status.player.clone();
        tokio::spawn(async move {
            let result: Result<(), String> = match request {
                MprisRequest::Play => player.play().await.map_err(|e| e.to_string()),
                MprisRequest::Pause => player.pause().await.map_err(|e| e.to_string()),
                MprisRequest::Next => player.next().await.map_err(|e| e.to_string()),
                MprisRequest::Previous => player.previous().await.map_err(|e| e.to_string()),
                MprisRequest::Raise => match player.media_player().await {
                    Ok(media_player) => media_player.raise().await.map_err(|e| e.to_string()),
                    Err(err) => Err(err.to_string()),
                },
            };
            if let Err(err) = result {
                tracing::error!(?request, %err, "media request failed");
            }
        });
    }
}

fn player_id(p: &ListedPlayer) -> String {
    p.status.player.inner().destination().to_string()
}

/// Upstream's local cover wins; then the web cover; the app icon is kept as the fallback.
fn with_extras(row: &mut PlayerRow, extras: &Extras) {
    if row.art.is_none() {
        row.art = extras.web_art.clone();
    }
    row.app_icon = extras.app_icon.clone();
}

/// Reads the cover address and the desktop entry through the player's proxy, and downloads a
/// web cover unless the previous lookup already has it.
pub async fn lookup(player: Player, previous: Option<Extras>) -> Extras {
    let art_url = match player.metadata().await {
        Ok(metadata) => metadata.art_url(),
        Err(_) => None,
    };
    let web_art_url = art_url.filter(|u| is_web_url(u));
    let web_art = match &web_art_url {
        Some(url) => match previous
            .as_ref()
            .filter(|p| p.web_art_url.as_deref() == Some(url.as_str()))
            .and_then(|p| p.web_art.clone())
        {
            Some(path) => Some(path),
            None => fetch_art(url).await,
        },
        None => None,
    };
    let app_icon = match previous.as_ref().and_then(|p| p.app_icon.clone()) {
        Some(icon) => Some(icon),
        None => match player.media_player().await {
            Ok(media_player) => match media_player.desktop_entry().await {
                Ok(entry) if !entry.is_empty() => Some(app_icon(&entry)),
                _ => None,
            },
            Err(_) => None,
        },
    };
    Extras {
        web_art_url,
        web_art,
        app_icon,
    }
}

fn is_web_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

const ART_TIMEOUT: Duration = Duration::from_secs(10);
const ART_MAX_BYTES: usize = 5 * 1024 * 1024;
/// Downloaded covers kept; the oldest go first.
const ART_KEEP: usize = 200;

fn art_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(base.join("at.hoeggi.CosmicAppletSystem").join("art"))
}

/// The cache file name for a cover address: a hash of it, with the image's extension.
fn art_file_name(url: &str, extension: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    format!("{:016x}.{extension}", hasher.finish())
}

fn extension_for(content_type: Option<&str>) -> Option<&'static str> {
    let essence = content_type?.split(';').next()?.trim();
    match essence {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        _ => None,
    }
}

/// Downloads a web cover into the cache, or returns the cached file.
async fn fetch_art(url: &str) -> Option<PathBuf> {
    let dir = art_dir()?;
    for extension in ["jpg", "png", "webp", "gif"] {
        let path = dir.join(art_file_name(url, extension));
        if path.is_file() {
            return Some(path);
        }
    }
    let client = reqwest::Client::builder()
        .timeout(ART_TIMEOUT)
        .build()
        .ok()?;
    let response = client.get(url).send().await.ok()?.error_for_status().ok()?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let extension = extension_for(content_type.as_deref())?;
    if response
        .content_length()
        .is_some_and(|n| n as usize > ART_MAX_BYTES)
    {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    if bytes.len() > ART_MAX_BYTES {
        return None;
    }
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(art_file_name(url, extension));
    let partial = path.with_extension("part");
    std::fs::write(&partial, &bytes).ok()?;
    std::fs::rename(&partial, &path).ok()?;
    prune_art(&dir);
    Some(path)
}

/// Keeps the newest [`ART_KEEP`] covers.
fn prune_art(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    if files.len() <= ART_KEEP {
        return;
    }
    files.sort();
    for (_, path) in files.iter().take(files.len() - ART_KEEP) {
        _ = std::fs::remove_file(path);
    }
}

/// The application's icon from its desktop entry (e.g. "vlc" or "spotify"): the `Icon=` of the
/// first matching `.desktop` file in the XDG data directories, else the entry name itself as a
/// theme icon name.
fn app_icon(entry: &str) -> AppIcon {
    let file = format!("{entry}.desktop");
    for dir in data_dirs() {
        if let Ok(contents) = std::fs::read_to_string(dir.join("applications").join(&file))
            && let Some(icon) = desktop_icon(&contents)
        {
            return icon;
        }
    }
    AppIcon::Name(entry.to_owned())
}

fn data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    match std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        Some(home) if home.is_absolute() => dirs.push(home),
        _ => {
            if let Some(home) = std::env::var_os("HOME") {
                dirs.push(PathBuf::from(home).join(".local/share"));
            }
        }
    }
    let system = std::env::var("XDG_DATA_DIRS")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    dirs.extend(
        system
            .split(':')
            .filter(|d| !d.is_empty())
            .map(PathBuf::from),
    );
    dirs
}

/// `Icon=` in the `[Desktop Entry]` group.
fn desktop_icon(contents: &str) -> Option<AppIcon> {
    let mut in_entry = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry && let Some(value) = line.strip_prefix("Icon=") {
            let value = value.trim();
            if value.is_empty() {
                return None;
            }
            return Some(if value.starts_with('/') {
                AppIcon::Path(PathBuf::from(value))
            } else {
                AppIcon::Name(value.to_owned())
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(id: &str, status: PlaybackStatus) -> PlayerInfo {
        PlayerInfo {
            id: id.into(),
            art: None,
            title: Some("Song".into()),
            artists: Some(vec!["Artist".into()]),
            status,
            can_play: true,
            can_pause: true,
            can_go_previous: true,
            can_go_next: true,
            identity: Some("App".into()),
        }
    }

    #[test]
    fn desktop_icon_from_the_entry_group() {
        let file = "[Desktop Entry]\nName=VLC\nIcon=vlc\n[Desktop Action new]\nIcon=other\n";
        assert_eq!(desktop_icon(file), Some(AppIcon::Name("vlc".into())));
        let file = "[Desktop Action x]\nIcon=nope\n[Desktop Entry]\nIcon=/opt/app/icon.png\n";
        assert_eq!(
            desktop_icon(file),
            Some(AppIcon::Path("/opt/app/icon.png".into()))
        );
        assert_eq!(desktop_icon("[Desktop Entry]\nName=X\n"), None);
    }

    #[test]
    fn web_covers_get_a_stable_file_name() {
        assert!(is_web_url("https://i.scdn.co/image/ab67"));
        assert!(!is_web_url("file:///home/me/cover.jpg"));
        let a = art_file_name("https://i.scdn.co/image/ab67", "jpg");
        assert_eq!(a, art_file_name("https://i.scdn.co/image/ab67", "jpg"));
        assert_ne!(a, art_file_name("https://i.scdn.co/image/cd89", "jpg"));
        assert!(a.ends_with(".jpg"));
        assert_eq!(extension_for(Some("image/jpeg")), Some("jpg"));
        assert_eq!(
            extension_for(Some("image/png; charset=binary")),
            Some("png")
        );
        assert_eq!(extension_for(Some("text/html")), None);
    }

    #[test]
    fn local_cover_wins_then_web_cover_then_app_icon() {
        let extras = Extras {
            web_art_url: Some("https://x/1".into()),
            web_art: Some("/cache/1.jpg".into()),
            app_icon: Some(AppIcon::Name("spotify".into())),
        };
        let mut local = row(&info("a", PlaybackStatus::Playing));
        local.art = Some("/music/cover.png".into());
        with_extras(&mut local, &extras);
        assert_eq!(local.art.as_deref(), Some(Path::new("/music/cover.png")));
        let mut web = row(&info("b", PlaybackStatus::Playing));
        with_extras(&mut web, &extras);
        assert_eq!(web.art.as_deref(), Some(Path::new("/cache/1.jpg")));
        assert_eq!(web.app_icon, Some(AppIcon::Name("spotify".into())));
    }

    #[test]
    fn stopped_players_are_hidden() {
        // A browser registers on MPRIS while it runs, stopped and without a title.
        let mut browser = info(
            "org.mpris.MediaPlayer2.chromium.instance1",
            PlaybackStatus::Stopped,
        );
        browser.title = None;
        let list = rows(&[
            browser,
            info("org.mpris.MediaPlayer2.vlc", PlaybackStatus::Playing),
        ]);
        let ids: Vec<_> = list.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["org.mpris.MediaPlayer2.vlc"]);
    }

    #[test]
    fn one_row_per_player_in_upstreams_order() {
        let list = rows(&[
            info("org.mpris.MediaPlayer2.spotify", PlaybackStatus::Playing),
            info(
                "org.mpris.MediaPlayer2.vlc.instance101",
                PlaybackStatus::Paused,
            ),
            info(
                "org.mpris.MediaPlayer2.vlc.instance202",
                PlaybackStatus::Paused,
            ),
        ]);
        let ids: Vec<_> = list.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "org.mpris.MediaPlayer2.spotify",
                "org.mpris.MediaPlayer2.vlc.instance101",
                "org.mpris.MediaPlayer2.vlc.instance202",
            ]
        );
        assert_eq!(
            list[0].play_pause,
            PlayPause {
                play: false,
                enabled: true
            }
        );
        assert_eq!(
            list[1].play_pause,
            PlayPause {
                play: true,
                enabled: true
            }
        );
    }

    #[test]
    fn unsupported_buttons_are_disabled_not_hidden() {
        let mut i = info("p", PlaybackStatus::Playing);
        i.can_go_previous = false;
        i.can_go_next = false;
        i.can_pause = false;
        let r = row(&i);
        assert!(!r.previous && !r.next);
        assert_eq!(
            r.play_pause,
            PlayPause {
                play: false,
                enabled: false
            }
        );
        // Paused without can_play: the play icon, disabled.
        let mut i = info("p", PlaybackStatus::Paused);
        i.can_play = false;
        assert_eq!(
            row(&i).play_pause,
            PlayPause {
                play: true,
                enabled: false
            }
        );
    }

    #[test]
    fn text_is_passed_on_whole() {
        // The view cuts long text with "…" to fit before the buttons; the adapter doesn't trim.
        let mut i = info("p", PlaybackStatus::Playing);
        i.title = Some("A very long track title that keeps on going".into());
        i.artists = Some(vec!["One".into(), "Two".into()]);
        i.identity = Some(String::new());
        let r = row(&i);
        assert_eq!(r.title, "A very long track title that keeps on going");
        assert_eq!(r.artist.as_deref(), Some("One, Two"));
        assert_eq!(r.app, None);
        i.artists = None;
        i.title = None;
        let r = row(&i);
        assert_eq!((r.title.as_str(), r.artist), ("", None));
    }

    #[test]
    fn empty_list_means_no_rows() {
        assert!(rows(&[]).is_empty());
        let mut model = Model::default();
        model.update(MprisUpdate::Players(Vec::new()));
        assert!(model.rows().is_empty());
        model.update(MprisUpdate::Finished);
        assert!(model.rows().is_empty());
    }
}
