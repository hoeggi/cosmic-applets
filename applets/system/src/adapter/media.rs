// SPDX-License-Identifier: GPL-3.0-only

//! Media controls for every MPRIS player, from upstream's audio applet subscription
//! (`crate::upstream::audio::mpris_subscription`). Upstream shows only the active player; we list
//! all of them (its marked `Players` update) and send each player's requests to that player,
//! with the same calls upstream uses (`play`, `pause`, `next`, `previous`, and `raise` through
//! the player's MediaPlayer2 object).

use crate::upstream::audio::mpris_subscription::{
    self as up, ListedPlayer, MprisRequest, MprisUpdate, PlayerStatus,
};
use cosmic::iced::Subscription;
use mpris2_zbus::player::PlaybackStatus;
use std::path::PathBuf;

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
    pub art: Option<PathBuf>,
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

/// Upstream's title and artist trimming in its popup (30 / 25 characters plus "...").
fn trim(text: &str, max: usize, keep: usize) -> String {
    if text.chars().count() > max {
        let mut out = text.chars().take(keep).collect::<String>();
        out.push_str("...");
        out
    } else {
        text.to_owned()
    }
}

pub fn row(info: &PlayerInfo) -> PlayerRow {
    let play = is_play(info);
    PlayerRow {
        id: info.id.clone(),
        art: info.art.clone(),
        title: info
            .title
            .as_deref()
            .map(|t| trim(t, 32, 30))
            .unwrap_or_default(),
        artist: info
            .artists
            .as_ref()
            .map(|a| trim(&a.join(", "), 27, 25))
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

/// Rows in the order given (upstream's: by bus name).
pub fn rows(infos: &[PlayerInfo]) -> Vec<PlayerRow> {
    infos.iter().map(row).collect()
}

/// Every running player, in upstream's order.
#[derive(Debug, Default)]
pub struct Model {
    players: Vec<ListedPlayer>,
}

impl Model {
    pub fn update(&mut self, update: MprisUpdate) {
        match update {
            MprisUpdate::Players(players) => self.players = players,
            MprisUpdate::Finished => self.players.clear(),
            // Upstream's active player; our rows come from the full list.
            MprisUpdate::Setup | MprisUpdate::Player(_) => {}
        }
    }

    pub fn rows(&self) -> Vec<PlayerRow> {
        let infos: Vec<PlayerInfo> = self.players.iter().map(PlayerInfo::from).collect();
        rows(&infos)
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
    fn one_row_per_player_in_upstreams_order() {
        let list = rows(&[
            info("org.mpris.MediaPlayer2.spotify", PlaybackStatus::Playing),
            info(
                "org.mpris.MediaPlayer2.vlc.instance101",
                PlaybackStatus::Paused,
            ),
            info(
                "org.mpris.MediaPlayer2.vlc.instance202",
                PlaybackStatus::Stopped,
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
    fn text_like_upstream() {
        let mut i = info("p", PlaybackStatus::Playing);
        i.title = Some("A very long track title that keeps on going".into());
        i.artists = Some(vec!["One".into(), "Two".into()]);
        i.identity = Some(String::new());
        let r = row(&i);
        assert_eq!(r.title.chars().count(), 33);
        assert!(r.title.ends_with("..."));
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
