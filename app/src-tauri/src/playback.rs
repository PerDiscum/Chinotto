//! Playback commands: the window asks to play a song (by dbid) from the open
//! iPod; the player reports back through `player-status`, `player-ended` and
//! `player-error` events.

use std::path::PathBuf;

use chinotto_player::{Event, Player, State};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatusView {
    state: &'static str,
    position_ms: u64,
    duration_ms: Option<u64>,
}

pub fn start(app: &AppHandle) -> Player {
    let app = app.clone();
    Player::new(move |e| match e {
        Event::Status(s) => {
            let state = match s.state {
                State::Playing => "playing",
                State::Paused => "paused",
                State::Stopped => "stopped",
            };
            let _ = app.emit("player-status", StatusView { state, position_ms: s.position_ms, duration_ms: s.duration_ms });
        }
        Event::Ended => {
            let _ = app.emit("player-ended", ());
        }
        Event::Error(msg) => {
            let _ = app.emit("player-error", msg);
        }
    })
}

/// The music file of a track on the open iPod.
pub(crate) fn track_file(app: &AppHandle, dbid: u64) -> Result<PathBuf, String> {
    let session = app.state::<AppState>().session().ok_or("No iPod is open")?;
    let t = session.db.tracks.iter().find(|t| t.dbid == dbid).ok_or("That song isn't on the iPod any more")?;
    let loc = t.strings.location.as_deref().ok_or("The iPod doesn't say where this song's file is")?;
    // ":iPod_Control:Music:F12:ABCD.mp3" → <root>/iPod_Control/Music/F12/ABCD.mp3
    let rel: PathBuf = loc.split(':').filter(|p| !p.is_empty()).collect();
    let path = session.root.join(rel);
    if !path.is_file() {
        return Err(format!("The song's file is missing from the iPod ({})", path.display()));
    }
    Ok(path)
}

#[tauri::command]
pub fn play_track(dbid: String, start_ms: Option<u64>, app: AppHandle) -> Result<(), String> {
    let file = if dbid.starts_with("lib:") {
        crate::libcommands::track_file(&app, &dbid)?
    } else if dbid.starts_with("scratch:") {
        crate::scratch::track_file(&app, &dbid)?
    } else {
        track_file(&app, dbid.parse().map_err(|_| "bad id")?)?
    };
    app.state::<AppState>().player().load(file, start_ms.unwrap_or(0), true);
    Ok(())
}

#[tauri::command]
pub fn player_play(app: AppHandle) {
    app.state::<AppState>().player().play();
}

#[tauri::command]
pub fn player_pause(app: AppHandle) {
    app.state::<AppState>().player().pause();
}

#[tauri::command]
pub fn player_stop(app: AppHandle) {
    app.state::<AppState>().player().stop();
}

#[tauri::command]
pub fn player_seek(ms: u64, app: AppHandle) {
    app.state::<AppState>().player().seek(ms);
}

#[tauri::command]
pub fn player_volume(volume: f32, app: AppHandle) {
    app.state::<AppState>().player().set_volume(volume);
}
