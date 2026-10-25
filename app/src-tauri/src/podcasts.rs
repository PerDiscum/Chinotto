//! Podcasts: shows followed by their RSS feed, and their episodes.
//!
//! Chinotto keeps a list of shows (in podcasts.json in the app's data folder)
//! and, for each, the episodes its feed lists. Nothing is downloaded until
//! asked: an episode goes into `<podcast folder>/<Show>/<date> - <Title>.mp3`.
//! A refresh marks episodes the feed didn't list before as new, which the
//! sidebar shows until the episode is downloaded or marked as seen.
//!
//! Only audio episodes are listed: Chinotto doesn't play video yet.

use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use quick_xml::events::Event as XmlEvent;
use quick_xml::Reader;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Podcasts {
    /// Where episodes are saved; `None` until the user picks it.
    pub folder: Option<PathBuf>,
    pub shows: Vec<Show>,
    #[serde(skip)]
    path: Option<PathBuf>,
    /// Downloads asked to stop ("<show>:<key>").
    #[serde(skip)]
    canceled: HashSet<String>,
}

/// What a stopped download reports; the window says nothing about it.
pub const CANCELED: &str = "Download canceled.";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Show {
    /// From the feed address it was first added with; never changes.
    pub id: String,
    pub feed: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub link: String,
    pub image: String,
    /// Unix seconds of the last successful refresh.
    pub checked: i64,
    /// Newest first.
    pub episodes: Vec<Episode>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Episode {
    /// The feed's guid (or the file address when there's none).
    pub guid: String,
    /// Short and stable, from the guid: used in ids and file names.
    pub key: String,
    pub title: String,
    pub description: String,
    /// Unix seconds; 0 when the feed doesn't say.
    pub published: i64,
    pub duration_ms: u64,
    pub url: String,
    pub mime: String,
    pub bytes: u64,
    pub season: u32,
    pub number: u32,
    /// The downloaded file, relative to the podcast folder.
    pub file: Option<String>,
    /// Appeared since the show was added and not yet downloaded or dismissed.
    pub new: bool,
    pub played: bool,
}

impl Podcasts {
    pub fn load(path: PathBuf) -> Self {
        let mut p: Podcasts = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        p.path = Some(path);
        p
    }

    fn save(&self) -> Result<(), String> {
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self).unwrap()).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    }

    fn show_mut(&mut self, id: &str) -> Result<&mut Show, String> {
        self.shows.iter_mut().find(|s| s.id == id).ok_or_else(|| "That podcast isn't in the list any more.".to_string())
    }

    /// Where a show's episodes (and its cover.jpg) go.
    fn show_dir(&self, show: &Show) -> Option<PathBuf> {
        Some(self.folder.as_ref()?.join(safe_name(&show.title, &show.id)))
    }
}

// ---- What the window sees ----

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PodcastsView {
    folder: Option<String>,
    shows: Vec<ShowView>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowView {
    #[serde(flatten)]
    show: Show,
    has_cover: bool,
    /// Per episode: is its file there?
    downloaded: Vec<bool>,
}

fn view(p: &Podcasts) -> PodcastsView {
    PodcastsView {
        folder: p.folder.as_ref().map(|f| f.display().to_string()),
        shows: p
            .shows
            .iter()
            .map(|s| {
                let dir = p.show_dir(s);
                ShowView {
                    has_cover: dir.as_ref().is_some_and(|d| d.join("cover.jpg").is_file()),
                    downloaded: s
                        .episodes
                        .iter()
                        .map(|e| e.file.as_ref().zip(p.folder.as_ref()).is_some_and(|(f, root)| root.join(f).is_file()))
                        .collect(),
                    show: s.clone(),
                }
            })
            .collect(),
    }
}

// ---- Commands ----

#[tauri::command]
pub fn podcasts_get(app: AppHandle) -> PodcastsView {
    view(&app.state::<AppState>().podcasts.lock().unwrap())
}

#[tauri::command]
pub fn podcasts_set_folder(folder: String, app: AppHandle) -> Result<PodcastsView, String> {
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).map_err(|e| format!("Can't use that folder: {e}"))?;
    let state = app.state::<AppState>();
    let mut p = state.podcasts.lock().unwrap();
    p.folder = Some(folder);
    p.save()?;
    Ok(view(&p))
}

/// Follow a show: its feed's address, or an Apple Podcasts page.
#[tauri::command]
pub async fn podcast_add(url: String, app: AppHandle) -> Result<PodcastsView, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("Type or paste the podcast's feed address.".into());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let feed = resolve_feed_url(&url)?;
        let id = format!("{:016x}", fnv(feed.as_bytes()));
        if app2.state::<AppState>().podcasts.lock().unwrap().shows.iter().any(|s| s.id == id || s.feed == feed) {
            return Err("You're already following that podcast.".to_string());
        }
        let parsed = fetch_feed(&feed)?;
        let mut show = Show { id, feed, ..Default::default() };
        apply(&mut show, parsed, true);
        let state = app2.state::<AppState>();
        let mut p = state.podcasts.lock().unwrap();
        let dir = p.show_dir(&show);
        let image = show.image.clone();
        p.shows.push(show);
        p.save()?;
        drop(p);
        if let Some(dir) = dir {
            fetch_cover(&image, &dir);
        }
        Ok(view(&app2.state::<AppState>().podcasts.lock().unwrap()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Check one show's feed, or all of them, for new episodes. Shows whose feed
/// can't be read are left as they were and named in the error.
#[tauri::command]
pub async fn podcasts_refresh(id: Option<String>, app: AppHandle) -> Result<PodcastsView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let todo: Vec<(String, String, String)> = state
            .podcasts
            .lock()
            .unwrap()
            .shows
            .iter()
            .filter(|s| id.as_ref().is_none_or(|i| *i == s.id))
            .map(|s| (s.id.clone(), s.feed.clone(), s.title.clone()))
            .collect();
        let mut failed = Vec::new();
        for (sid, feed, title) in todo {
            match fetch_feed(&feed) {
                Ok(parsed) => {
                    let mut p = state.podcasts.lock().unwrap();
                    let Ok(show) = p.show_mut(&sid) else { continue };
                    apply(show, parsed, false);
                    let (image, dir) = (show.image.clone(), p.show_dir(&p.shows.iter().find(|s| s.id == sid).cloned().unwrap_or_default()));
                    p.save()?;
                    drop(p);
                    if let Some(dir) = dir.filter(|d| !d.join("cover.jpg").is_file()) {
                        fetch_cover(&image, &dir);
                    }
                }
                Err(e) => failed.push(format!("{title}: {e}")),
            }
        }
        if failed.is_empty() {
            Ok(view(&state.podcasts.lock().unwrap()))
        } else {
            Err(format!("Couldn't check every podcast. {}", failed.join(" · ")))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn podcast_remove(id: String, delete_files: bool, app: AppHandle) -> Result<PodcastsView, String> {
    let state = app.state::<AppState>();
    let mut p = state.podcasts.lock().unwrap();
    let Some(i) = p.shows.iter().position(|s| s.id == id) else { return Ok(view(&p)) };
    if delete_files {
        if let Some(root) = p.folder.clone() {
            for e in &p.shows[i].episodes {
                if let Some(f) = &e.file {
                    let _ = std::fs::remove_file(root.join(f));
                }
            }
            if let Some(dir) = p.show_dir(&p.shows[i]) {
                let _ = std::fs::remove_file(dir.join("cover.jpg"));
                let _ = std::fs::remove_dir(&dir); // only if now empty
            }
        }
    }
    p.shows.remove(i);
    p.save()?;
    Ok(view(&p))
}

/// Mark episodes as seen (no longer new) or as played/unplayed.
#[tauri::command]
pub fn podcast_mark(
    id: String,
    keys: Vec<String>,
    new: Option<bool>,
    played: Option<bool>,
    app: AppHandle,
) -> Result<PodcastsView, String> {
    let state = app.state::<AppState>();
    let mut p = state.podcasts.lock().unwrap();
    let show = p.show_mut(&id)?;
    for e in show.episodes.iter_mut().filter(|e| keys.is_empty() || keys.contains(&e.key)) {
        if let Some(n) = new {
            e.new = n;
        }
        if let Some(pl) = played {
            e.played = pl;
        }
    }
    p.save()?;
    Ok(view(&p))
}

/// Delete an episode's downloaded file (the episode stays in the list).
#[tauri::command]
pub fn podcast_delete_file(id: String, key: String, app: AppHandle) -> Result<PodcastsView, String> {
    let state = app.state::<AppState>();
    let mut p = state.podcasts.lock().unwrap();
    let root = p.folder.clone();
    let e = p.show_mut(&id)?.episodes.iter_mut().find(|e| e.key == key).ok_or("That episode isn't in the feed any more.")?;
    if let (Some(f), Some(root)) = (e.file.take(), root) {
        match std::fs::remove_file(root.join(&f)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                e.file = Some(f);
                return Err(format!("Couldn't delete the episode: {err}"));
            }
        }
    }
    p.save()?;
    Ok(view(&p))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: String,
    key: String,
    done: u64,
    total: u64,
}

/// Download one episode, reporting `podcast-progress` as it goes.
#[tauri::command]
pub async fn podcast_download(id: String, key: String, app: AppHandle) -> Result<PodcastsView, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let (url, dest, rel) = {
            let p = state.podcasts.lock().unwrap();
            let root = p.folder.clone().ok_or("Choose a folder for podcasts first.")?;
            let show = p.shows.iter().find(|s| s.id == id).ok_or("That podcast isn't in the list any more.")?;
            let e = show.episodes.iter().find(|e| e.key == key).ok_or("That episode isn't in the feed any more.")?;
            let dir = p.show_dir(show).unwrap();
            let name = episode_file_name(e);
            let dest = unique_path(&dir.join(name));
            let rel = dest.strip_prefix(&root).unwrap_or(&dest).to_string_lossy().into_owned();
            (e.url.clone(), dest, rel)
        };
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| format!("Couldn't make the show's folder: {e}"))?;
        let part = dest.with_extension(format!("{}.part", dest.extension().and_then(|x| x.to_str()).unwrap_or("")));
        let result = (|| -> Result<(), String> {
            let mut resp = agent(Duration::from_secs(60 * 60)).get(&url).call().map_err(net_error)?;
            let total = resp.body().content_length().unwrap_or(0);
            let mut reader = resp.body_mut().as_reader();
            let mut out = std::fs::File::create(&part).map_err(|e| format!("Couldn't save the episode: {e}"))?;
            let job = format!("{id}:{key}");
            state.podcasts.lock().unwrap().canceled.remove(&job);
            let mut buf = vec![0u8; 256 * 1024];
            let (mut done, mut last) = (0u64, std::time::Instant::now());
            loop {
                let n = reader.read(&mut buf).map_err(|e| format!("The download was cut off: {e}"))?;
                if n == 0 {
                    break;
                }
                if state.podcasts.lock().unwrap().canceled.remove(&job) {
                    return Err(CANCELED.into());
                }
                out.write_all(&buf[..n]).map_err(|e| format!("Couldn't save the episode: {e}"))?;
                done += n as u64;
                if last.elapsed() >= Duration::from_millis(200) {
                    last = std::time::Instant::now();
                    let _ = app.emit("podcast-progress", Progress { id: id.clone(), key: key.clone(), done, total });
                }
            }
            out.sync_all().map_err(|e| format!("Couldn't save the episode: {e}"))?;
            if done == 0 {
                return Err("The podcast's server sent an empty file.".into());
            }
            std::fs::rename(&part, &dest).map_err(|e| format!("Couldn't save the episode: {e}"))?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        let mut p = state.podcasts.lock().unwrap();
        if let Some(e) = p.show_mut(&id)?.episodes.iter_mut().find(|e| e.key == key) {
            e.file = Some(rel);
            e.new = false;
        }
        p.save()?;
        Ok(view(&p))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Stop a download; its partial file is deleted.
#[tauri::command]
pub fn podcast_cancel(id: String, key: String, app: AppHandle) {
    app.state::<AppState>().podcasts.lock().unwrap().canceled.insert(format!("{id}:{key}"));
}

/// The file of a downloaded episode, from its id "pod:<show>:<key>".
pub fn episode_file(app: &AppHandle, dbid: &str) -> Result<PathBuf, String> {
    let mut parts = dbid.splitn(3, ':').skip(1);
    let (sid, key) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    let state = app.state::<AppState>();
    let p = state.podcasts.lock().unwrap();
    let root = p.folder.clone().ok_or("No podcast folder")?;
    let show = p.shows.iter().find(|s| s.id == sid).ok_or("That podcast isn't in the list any more")?;
    let e = show.episodes.iter().find(|e| e.key == key).ok_or("That episode isn't in the feed any more")?;
    let f = root.join(e.file.as_ref().ok_or("That episode isn't downloaded")?);
    if !f.is_file() {
        return Err(format!("The episode's file is missing ({})", f.display()));
    }
    Ok(f)
}

/// A show's saved cover, for the libart protocol ("pod-<show>-<px>").
pub fn cover_file(app: &AppHandle, sid: &str) -> Option<PathBuf> {
    let state = app.state::<AppState>();
    let p = state.podcasts.lock().unwrap();
    let show = p.shows.iter().find(|s| s.id == sid)?;
    Some(p.show_dir(show)?.join("cover.jpg")).filter(|f| f.is_file())
}

// ---- On the iPod ----

/// An episode of a followed show that's on the open iPod.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnIpod {
    show: String,
    key: String,
    dbid: String,
    played: bool,
    bookmark_ms: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResult {
    added: usize,
    skipped: Vec<String>,
}

fn is_nano6(root: &Path) -> bool {
    root.join("iPod_Control/iTunes/iTunesCDB").is_file()
}

const NANO6: &str = "Podcasts can't go on the iPod nano 6G yet: its database is the experimental one.";

/// Is this iPod track that episode? By its file address, or by show and title.
fn same_episode(t: &chinotto_gpod::PodcastTrack, show: &Show, e: &Episode) -> bool {
    (!t.episode_url.is_empty() && t.episode_url == e.url) || (t.album == show.title && t.title == e.title)
}

/// Which followed episodes are on the iPod, and which were played there.
/// Episodes played on the iPod are marked played here too.
#[tauri::command]
pub async fn podcast_ipod_status(path: String, app: AppHandle) -> Result<Vec<OnIpod>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let root = PathBuf::from(&path);
        if is_nano6(&root) {
            return Err(NANO6.to_string());
        }
        let tracks = chinotto_gpod::Database::open(&root).map_err(|e| e.to_string())?.podcasts();
        let state = app.state::<AppState>();
        let mut p = state.podcasts.lock().unwrap();
        let mut out = Vec::new();
        let mut changed = false;
        for show in p.shows.iter_mut() {
            let snapshot = Show { episodes: Vec::new(), ..show.clone() };
            for e in show.episodes.iter_mut() {
                if let Some(t) = tracks.iter().find(|t| same_episode(t, &snapshot, e)) {
                    if t.played && !e.played {
                        e.played = true;
                        e.new = false;
                        changed = true;
                    }
                    out.push(OnIpod {
                        show: show.id.clone(),
                        key: e.key.clone(),
                        dbid: t.dbid.to_string(),
                        played: t.played,
                        bookmark_ms: t.bookmark_ms,
                    });
                }
            }
        }
        if changed {
            p.save()?;
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copy downloaded episodes onto the open iPod as podcasts. Ones already
/// there are skipped; formats the iPod can't play are converted as for songs.
#[tauri::command]
pub async fn podcast_send(path: String, id: String, keys: Vec<String>, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::write::blocking(app, move |app| {
        let root = PathBuf::from(&path);
        if is_nano6(&root) {
            return Err(NANO6.to_string());
        }
        let state = app.state::<AppState>();
        let (show, folder, cover) = {
            let p = state.podcasts.lock().unwrap();
            let show = p.shows.iter().find(|s| s.id == id).cloned().ok_or("That podcast isn't in the list any more.")?;
            let folder = p.folder.clone().ok_or("No podcast folder")?;
            let cover = p.show_dir(&show).and_then(|d| std::fs::read(d.join("cover.jpg")).ok());
            (show, folder, cover)
        };
        let conv =
            state.session().filter(|s| s.root == root).and_then(|s| s.sysinfo.identity()).map(|i| state.conversion(&i)).unwrap_or_default();
        let tmp = crate::convert::TempDir::new()?;
        let tmp_dir = tmp.0.clone();
        let emitter = app.clone();
        let result = crate::write::with_writer(&app, &root, move |db| {
            let existing = db.podcasts();
            let todo: Vec<&Episode> = show
                .episodes
                .iter()
                .filter(|e| keys.contains(&e.key) && e.file.is_some())
                .filter(|e| !existing.iter().any(|t| same_episode(t, &show, e)))
                .collect();
            let total = todo.len();
            let mut res = SendResult { added: 0, skipped: Vec::new() };
            let mut covers = Vec::new();
            for (i, e) in todo.into_iter().enumerate() {
                let _ = emitter
                    .emit("add-progress", crate::write::AddProgress { target: path.clone(), done: i, total, current: e.title.clone() });
                let src = folder.join(e.file.as_deref().unwrap_or_default());
                if !src.is_file() {
                    res.skipped.push(format!("{}: the downloaded file is missing", e.title));
                    continue;
                }
                let converted;
                let f = match conv.plan(&src, crate::convert::IPOD_PLAYS) {
                    Some(target) => match crate::convert::convert(&src, &tmp_dir, &target, i) {
                        Ok(c) => {
                            converted = c;
                            &converted
                        }
                        Err(err) => {
                            res.skipped.push(format!("{}: couldn't convert it ({err})", e.title));
                            continue;
                        }
                    },
                    None => &src,
                };
                if crate::tags::ipod_kind(f).is_none() {
                    res.skipped.push(format!("{}: this kind of file doesn't play on an iPod", e.title));
                    continue;
                }
                let mut info = match crate::tags::read(f) {
                    Ok(i) => i,
                    Err(err) => {
                        res.skipped.push(format!("{}: {err}", e.title));
                        continue;
                    }
                };
                // The feed's names, not whatever the file's tags say.
                let host = if show.author.is_empty() { show.title.clone() } else { show.author.clone() };
                info.title = Some(e.title.clone());
                info.album = Some(show.title.clone());
                info.artist = Some(host.clone());
                info.album_artist = Some(host);
                info.genre = Some("Podcast".into());
                info.compilation = false;
                info.track_nr = e.number as i32;
                info.track_count = 0;
                info.disc_nr = 0;
                info.disc_count = 0;
                if e.published > 0 {
                    info.year = civil_from_days(e.published.div_euclid(86_400)).0 as i32;
                }
                if info.length_ms <= 0 {
                    info.length_ms = e.duration_ms.min(i32::MAX as u64) as i32;
                }
                let pod = chinotto_gpod::PodcastInfo {
                    feed_url: show.feed.clone(),
                    episode_url: e.url.clone(),
                    description: e.description.clone(),
                    released: e.published,
                };
                match db.add_podcast(f, &info, &pod) {
                    Ok(dbid) => {
                        res.added += 1;
                        if let Some(img) = crate::tags::embedded_cover(f).or_else(|| cover.clone()) {
                            covers.push((dbid, img));
                        }
                    }
                    Err(err) => res.skipped.push(format!("{}: {err}", e.title)),
                }
            }
            let _ = emitter
                .emit("add-progress", crate::write::AddProgress { target: path.clone(), done: total, total, current: String::new() });
            crate::write::add_cover_images(db, covers, &mut res.skipped)?;
            if res.added == 0 && !res.skipped.is_empty() {
                return Err(res.skipped.join("\n"));
            }
            Ok(res)
        });
        drop(tmp);
        result
    })
    .await
}

/// Take a show's played episodes off the iPod, and with `from_computer`
/// delete their downloaded files too.
#[tauri::command]
pub async fn podcast_remove_played(path: String, id: String, from_computer: bool, app: AppHandle) -> Result<tauri::ipc::Response, String> {
    crate::write::blocking(app, move |app| {
        let root = PathBuf::from(&path);
        if is_nano6(&root) {
            return Err(NANO6.to_string());
        }
        let state = app.state::<AppState>();
        let show =
            state.podcasts.lock().unwrap().shows.iter().find(|s| s.id == id).cloned().ok_or("That podcast isn't in the list any more.")?;
        let removed_keys = std::sync::Arc::new(Mutex::new(Vec::<String>::new()));
        let keys_out = removed_keys.clone();
        let show2 = show.clone();
        let response = crate::write::with_writer(&app, &root, move |db| {
            let mut ids = Vec::new();
            for t in db.podcasts().into_iter().filter(|t| t.played) {
                let ep = show2.episodes.iter().find(|e| same_episode(&t, &show2, e));
                let ours = ep.is_some() || (!t.feed_url.is_empty() && t.feed_url == show2.feed);
                if ours {
                    ids.push(t.dbid);
                    if let Some(e) = ep {
                        keys_out.lock().unwrap().push(e.key.clone());
                    }
                }
            }
            db.remove_tracks(&ids).map_err(|e| e.to_string())?;
            Ok(ids.len())
        })?;
        // The iPod is saved; now the computer's copies, if asked.
        let keys = removed_keys.lock().unwrap().clone();
        let mut p = state.podcasts.lock().unwrap();
        let root_folder = p.folder.clone();
        if let Ok(s) = p.show_mut(&id) {
            for e in s.episodes.iter_mut().filter(|e| keys.contains(&e.key)) {
                e.played = true;
                e.new = false;
                if from_computer {
                    if let (Some(f), Some(r)) = (e.file.take(), root_folder.as_ref()) {
                        let _ = std::fs::remove_file(r.join(f));
                    }
                }
            }
        }
        p.save()?;
        Ok(response)
    })
    .await
}

// ---- Fetching ----

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(Some(timeout))
        .user_agent(concat!("Chinotto/", env!("CARGO_PKG_VERSION"), " (podcast client)"))
        .build()
        .into()
}

fn net_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(404) => "The address wasn't found (404). Check it, or the podcast may have moved.".into(),
        ureq::Error::StatusCode(401 | 403) => "The podcast's server refused access. A private feed's address may have expired.".into(),
        ureq::Error::StatusCode(c) => format!("The podcast's server answered with an error ({c})."),
        ureq::Error::HostNotFound => "That address's server can't be found. Check the address and your internet connection.".into(),
        ureq::Error::Timeout(_) => "The podcast's server took too long to answer.".into(),
        other => format!("Couldn't reach the podcast: {other}"),
    }
}

fn get_bytes(url: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut resp = agent(Duration::from_secs(60)).get(url).call().map_err(net_error)?;
    resp.body_mut().with_config().limit(limit).read_to_vec().map_err(|e| format!("Couldn't read the podcast's feed: {e}"))
}

/// An Apple Podcasts page ("podcasts.apple.com/…/id123") stands for its feed,
/// which Apple's public lookup gives. Anything else is taken as the feed.
fn resolve_feed_url(url: &str) -> Result<String, String> {
    let url = if url.contains("://") { url.to_string() } else { format!("https://{url}") };
    let url = url.replacen("feed://", "https://", 1).replacen("pcast://", "https://", 1);
    if !url.contains("podcasts.apple.com/") {
        return Ok(url);
    }
    let id = url
        .split(['/', '?', '&'])
        .find_map(|p| p.strip_prefix("id").filter(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit())))
        .ok_or("That Apple Podcasts link has no podcast number in it. Paste the show's page link, or its feed address.")?;
    let body = get_bytes(&format!("https://itunes.apple.com/lookup?id={id}&entity=podcast"), 1 << 20)?;
    let v: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    v["results"][0]["feedUrl"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| "Apple doesn't list a feed for that podcast (it may be Apple-only).".to_string())
}

fn fetch_feed(url: &str) -> Result<Feed, String> {
    let body = get_bytes(url, 64 << 20)?;
    let feed = parse_feed(&body)?;
    if feed.title.is_empty() && feed.items.is_empty() {
        return Err("That address isn't a podcast feed. Look for the show's RSS link.".into());
    }
    Ok(feed)
}

/// Save the show's picture as cover.jpg (at most 1400 px). Best effort.
fn fetch_cover(url: &str, dir: &Path) {
    if url.is_empty() {
        return;
    }
    let Ok(bytes) = get_bytes(url, 20 << 20) else { return };
    let Ok(img) = crate::cover::decode(&bytes) else { return };
    let img = if img.width() > 1400 || img.height() > 1400 { img.thumbnail(1400, 1400) } else { img };
    let mut out = Vec::new();
    if image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90).encode_image(&img.to_rgb8()).is_ok()
        && std::fs::create_dir_all(dir).is_ok()
    {
        let _ = std::fs::write(dir.join("cover.jpg"), out);
    }
}

/// Bring a show up to date with its feed. Episodes keep their downloaded file
/// and flags; ones the feed no longer lists stay if they were downloaded.
fn apply(show: &mut Show, feed: Feed, first: bool) {
    show.title = if feed.title.is_empty() { show.feed.clone() } else { feed.title };
    show.author = feed.author;
    show.description = feed.description;
    show.link = feed.link;
    if !feed.image.is_empty() {
        show.image = feed.image;
    }
    if let Some(moved) = feed.new_feed_url.filter(|u| u.starts_with("http")) {
        show.feed = moved;
    }
    show.checked = now();
    let old: std::collections::HashMap<String, Episode> = show.episodes.drain(..).map(|e| (e.guid.clone(), e)).collect();
    let mut seen = HashSet::new();
    let mut list = Vec::new();
    for item in feed.items {
        if !seen.insert(item.guid.clone()) {
            continue;
        }
        let prev = old.get(&item.guid);
        list.push(Episode {
            key: format!("{:012x}", fnv(item.guid.as_bytes()) & 0xffff_ffff_ffff),
            file: prev.and_then(|e| e.file.clone()),
            // When a show is first added, only its latest episode counts as new.
            new: prev.map(|e| e.new).unwrap_or(!first),
            played: prev.is_some_and(|e| e.played),
            guid: item.guid,
            title: item.title,
            description: item.description,
            published: item.published,
            duration_ms: item.duration_ms,
            url: item.url,
            mime: item.mime,
            bytes: item.bytes,
            season: item.season,
            number: item.number,
        });
    }
    for (guid, e) in old {
        if !seen.contains(&guid) && e.file.is_some() {
            list.push(e);
        }
    }
    list.sort_by_key(|e| std::cmp::Reverse(e.published));
    if first {
        if let Some(e) = list.first_mut() {
            e.new = true;
        }
    }
    show.episodes = list;
}

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// ---- File names ----

fn episode_file_name(e: &Episode) -> String {
    let date = if e.published > 0 { format!("{} - ", ymd(e.published)) } else { String::new() };
    let base = safe_name(&format!("{date}{}", e.title), &e.key);
    format!("{base}.{}", extension(&e.url, &e.mime))
}

fn extension(url: &str, mime: &str) -> &'static str {
    let path = url.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    for ext in ["mp3", "m4a", "aac", "mp4", "ogg", "oga", "opus", "flac", "wav", "aif", "aiff"] {
        if path.ends_with(&format!(".{ext}")) {
            return match ext {
                "mp3" => "mp3",
                "m4a" | "mp4" => "m4a",
                "aac" => "aac",
                "ogg" | "oga" => "ogg",
                "opus" => "opus",
                "flac" => "flac",
                "wav" => "wav",
                _ => "aif",
            };
        }
    }
    match mime {
        "audio/x-m4a" | "audio/mp4" | "audio/m4a" | "audio/aac" => "m4a",
        "audio/ogg" | "audio/vorbis" => "ogg",
        "audio/opus" => "opus",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        _ => "mp3",
    }
}

/// A name that's safe on any file system, at most ~120 characters.
fn safe_name(s: &str, fallback: &str) -> String {
    let cleaned: String = s.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '-' } else { c }).collect();
    let mut out = cleaned.trim().trim_matches('.').trim().to_string();
    if out.chars().count() > 120 {
        out = out.chars().take(120).collect::<String>().trim_end().to_string();
    }
    if out.is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

/// Never overwrite: "name.mp3", then "name (2).mp3", …
fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("episode");
    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("mp3");
    (2..).map(|n| p.with_file_name(format!("{stem} ({n}).{ext}"))).find(|q| !q.exists()).unwrap()
}

fn fnv(b: &[u8]) -> u64 {
    b.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))
}

fn ymd(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    format!("{y:04}-{m:02}-{d:02}")
}

// ---- Reading feeds ----

#[derive(Debug, Default)]
struct Feed {
    title: String,
    author: String,
    description: String,
    link: String,
    image: String,
    new_feed_url: Option<String>,
    items: Vec<Item>,
}

#[derive(Debug, Default)]
struct Item {
    guid: String,
    title: String,
    description: String,
    published: i64,
    duration_ms: u64,
    url: String,
    mime: String,
    bytes: u64,
    season: u32,
    number: u32,
}

/// Read an RSS 2.0 podcast feed (with Apple's itunes: tags). Video episodes
/// and items without a file are left out.
fn parse_feed(xml: &[u8]) -> Result<Feed, String> {
    let mut r = Reader::from_reader(xml);
    let mut feed = Feed::default();
    let mut path: Vec<String> = Vec::new();
    let mut item: Option<Item> = None;
    let mut summary = String::new(); // itunes:summary / content:encoded, if there's no description
    let mut text = String::new();
    let mut buf = Vec::new();
    let mut saw_rss = false;
    loop {
        let ev = r.read_event_into(&mut buf).map_err(|e| format!("The feed isn't valid XML: {e}"))?;
        match ev {
            XmlEvent::Start(ref e) | XmlEvent::Empty(ref e) => {
                let empty = matches!(ev, XmlEvent::Empty(_));
                let name = e.name().as_ref().to_string();
                saw_rss |= name == "rss" || name == "channel";
                let attr = |key: &str| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == key)
                        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.trim().to_string()))
                        .unwrap_or_default()
                };
                let in_item = item.is_some();
                match name.as_str() {
                    "item" if !empty => {
                        item = Some(Item::default());
                        summary.clear();
                    }
                    "enclosure" if in_item => {
                        let it = item.as_mut().unwrap();
                        if it.url.is_empty() {
                            it.url = attr("url");
                            it.mime = attr("type").to_ascii_lowercase();
                            it.bytes = attr("length").parse().unwrap_or(0);
                        }
                    }
                    "itunes:image" if !in_item && feed.image.is_empty() => feed.image = attr("href"),
                    _ => {}
                }
                if !empty {
                    path.push(name);
                    text.clear();
                }
            }
            XmlEvent::Text(t) => {
                text.push_str(&t.xml10_content());
            }
            XmlEvent::CData(t) => text.push_str(&t.xml10_content()),
            XmlEvent::GeneralRef(r) => {
                // Entities quick-xml leaves to us: &amp; &#38; and friends.
                text.push_str(&decode_entity(&r.xml10_content()));
            }
            XmlEvent::End(_) => {
                let name = path.pop().unwrap_or_default();
                let parent = path.last().map(String::as_str).unwrap_or("");
                let value = std::mem::take(&mut text).trim().to_string();
                if let Some(it) = item.as_mut() {
                    match name.as_str() {
                        "title" if parent == "item" => it.title = value,
                        "guid" => it.guid = value,
                        "pubDate" => it.published = parse_date(&value).unwrap_or(0),
                        "description" if parent == "item" => it.description = plain_text(&value),
                        "itunes:summary" | "content:encoded" if summary.is_empty() => summary = plain_text(&value),
                        "itunes:duration" => it.duration_ms = parse_duration(&value),
                        "itunes:season" => it.season = value.parse().unwrap_or(0),
                        "itunes:episode" => it.number = value.parse().unwrap_or(0),
                        "item" => {
                            let mut it = item.take().unwrap();
                            if it.description.is_empty() {
                                it.description = std::mem::take(&mut summary);
                            }
                            if it.guid.is_empty() {
                                it.guid = it.url.clone();
                            }
                            let video = it.mime.starts_with("video/")
                                || [".mp4v", ".m4v", ".mov"]
                                    .iter()
                                    .any(|x| it.url.to_ascii_lowercase().split('?').next().unwrap_or("").ends_with(x));
                            if !it.url.is_empty() && !video {
                                if it.title.is_empty() {
                                    it.title = "Untitled episode".into();
                                }
                                feed.items.push(it);
                            }
                        }
                        _ => {}
                    }
                } else {
                    match (name.as_str(), parent) {
                        ("title", "channel") => feed.title = value,
                        ("link", "channel") => feed.link = value,
                        ("description", "channel") => feed.description = plain_text(&value),
                        ("itunes:summary", "channel") if feed.description.is_empty() => feed.description = plain_text(&value),
                        ("itunes:author", "channel") => feed.author = value,
                        ("url", "image") if feed.image.is_empty() => feed.image = value,
                        ("itunes:new-feed-url", "channel") => feed.new_feed_url = Some(value),
                        _ => {}
                    }
                }
            }
            XmlEvent::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    if !saw_rss {
        return Err("That address isn't a podcast feed. Look for the show's RSS link.".into());
    }
    Ok(feed)
}

fn decode_entity(name: &str) -> String {
    let named = match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some('\u{a0}'),
        _ => None,
    };
    if let Some(c) = named {
        return c.to_string();
    }
    let num = name
        .strip_prefix("#x")
        .or_else(|| name.strip_prefix("#X"))
        .map(|h| u32::from_str_radix(h, 16))
        .or_else(|| name.strip_prefix('#').map(|d| d.parse::<u32>()));
    match num.and_then(|r| r.ok()).and_then(char::from_u32) {
        Some(c) => c.to_string(),
        None => format!("&{name};"),
    }
}

/// Descriptions are often HTML: keep the words and paragraph breaks.
fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => {
                let mut tag = String::new();
                for t in chars.by_ref() {
                    if t == '>' {
                        break;
                    }
                    tag.push(t);
                }
                let t = tag.trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_ascii_lowercase();
                if matches!(t.as_str(), "br" | "p" | "div" | "li" | "h1" | "h2" | "h3" | "h4") {
                    out.push('\n');
                }
            }
            '&' => {
                let mut ent = String::new();
                while let Some(&n) = chars.peek() {
                    if n == ';' || ent.len() > 8 || n.is_whitespace() {
                        break;
                    }
                    ent.push(n);
                    chars.next();
                }
                if chars.peek() == Some(&';') {
                    chars.next();
                    out.push_str(&decode_entity(&ent));
                } else {
                    out.push('&');
                    out.push_str(&ent);
                }
            }
            _ => out.push(c),
        }
    }
    // Tidy whitespace: single spaces, at most one blank line.
    let mut tidy = String::new();
    for para in out.split('\n').map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")) {
        if para.is_empty() {
            if !tidy.ends_with("\n\n") && !tidy.is_empty() {
                tidy.push('\n');
            }
        } else {
            if !tidy.is_empty() && !tidy.ends_with('\n') {
                tidy.push('\n');
            }
            tidy.push_str(&para);
        }
    }
    tidy.trim().to_string()
}

/// "1:02:03", "62:03", "3723" or "3723.5" → milliseconds.
fn parse_duration(s: &str) -> u64 {
    let mut secs = 0f64;
    for part in s.trim().split(':') {
        match part.trim().parse::<f64>() {
            Ok(v) if v >= 0.0 => secs = secs * 60.0 + v,
            _ => return 0,
        }
    }
    (secs * 1000.0) as u64
}

/// RFC 822/2822 dates as feeds write them ("Wed, 02 Oct 2024 08:00:00 +0000",
/// also "GMT"/"EST", two-digit years, missing seconds or weekday).
fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    let s = s.split_once(',').map(|(_, r)| r).unwrap_or(s);
    let mut it = s.split_whitespace();
    let day: i64 = it.next()?.parse().ok()?;
    let mon = it.next()?.to_ascii_lowercase();
    let month = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"]
        .iter()
        .position(|m| mon.starts_with(m))? as i64
        + 1;
    let mut year: i64 = it.next()?.parse().ok()?;
    if year < 100 {
        year += if year < 50 { 2000 } else { 1900 };
    }
    let (mut h, mut mi, mut sec) = (0, 0, 0);
    let mut zone = "";
    if let Some(t) = it.next() {
        if t.contains(':') {
            let mut tp = t.split(':');
            h = tp.next()?.parse().ok()?;
            mi = tp.next().unwrap_or("0").parse().ok()?;
            sec = tp.next().unwrap_or("0").parse::<f64>().ok()? as i64;
            zone = it.next().unwrap_or("");
        } else {
            zone = t;
        }
    }
    let offset_min: i64 = match zone.to_ascii_uppercase().as_str() {
        "" | "GMT" | "UT" | "UTC" | "Z" => 0,
        "EST" => -300,
        "EDT" => -240,
        "CST" => -360,
        "CDT" => -300,
        "MST" => -420,
        "MDT" => -360,
        "PST" => -480,
        "PDT" => -420,
        z if (z.starts_with('+') || z.starts_with('-')) && z.len() >= 5 => {
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let digits: String = z[1..].chars().filter(|c| c.is_ascii_digit()).collect();
            let hh: i64 = digits.get(0..2)?.parse().ok()?;
            let mm: i64 = digits.get(2..4).unwrap_or("0").parse().ok()?;
            sign * (hh * 60 + mm)
        }
        _ => 0,
    };
    Some(days_from_civil(year, month, day) * 86_400 + h * 3600 + mi * 60 + sec - offset_min * 60)
}

// Howard Hinnant's civil-date algorithms.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:content="http://purl.org/rss/1.0/modules/content/">
<channel>
  <title>Test &amp; Show</title>
  <link>https://example.com</link>
  <description><![CDATA[<p>A <b>show</b> about tests.</p><p>Second line.</p>]]></description>
  <itunes:author>Someone</itunes:author>
  <itunes:image href="https://example.com/art.jpg"/>
  <image><url>https://example.com/other.jpg</url><title>x</title></image>
  <item>
    <title>Episode 2: More</title>
    <guid isPermaLink="false">ep-2</guid>
    <pubDate>Tue, 08 Oct 2024 09:30:00 -0400</pubDate>
    <enclosure url="https://cdn.example.com/ep2.mp3?x=1" length="1234" type="audio/mpeg"/>
    <itunes:duration>1:02:03</itunes:duration>
    <itunes:episode>2</itunes:episode>
    <content:encoded><![CDATA[Fallback &amp; notes]]></content:encoded>
  </item>
  <item>
    <title>Video one</title>
    <guid>vid</guid>
    <enclosure url="https://cdn.example.com/v.mp4" length="1" type="video/mp4"/>
  </item>
  <item>
    <title>Episode 1</title>
    <pubDate>01 Oct 24 08:00 GMT</pubDate>
    <enclosure url="https://cdn.example.com/ep1.m4a" length="99" type="audio/x-m4a"/>
    <itunes:duration>754</itunes:duration>
    <description>First &lt;i&gt;one&lt;/i&gt;</description>
  </item>
</channel>
</rss>"#;

    #[test]
    fn reads_a_feed() {
        let f = parse_feed(FEED.as_bytes()).unwrap();
        assert_eq!(f.title, "Test & Show");
        assert_eq!(f.author, "Someone");
        assert_eq!(f.image, "https://example.com/art.jpg");
        assert_eq!(f.description, "A show about tests.\nSecond line.");
        assert_eq!(f.items.len(), 2, "the video is left out");
        let e2 = &f.items[0];
        assert_eq!((e2.guid.as_str(), e2.number, e2.bytes, e2.duration_ms), ("ep-2", 2, 1234, 3_723_000));
        assert_eq!(e2.description, "Fallback & notes");
        assert_eq!(e2.published, 1_728_394_200); // 2024-10-08 13:30 UTC
        let e1 = &f.items[1];
        assert_eq!(e1.guid, "https://cdn.example.com/ep1.m4a");
        assert_eq!(e1.description, "First one");
        assert_eq!(e1.duration_ms, 754_000);
        assert_eq!(ymd(e1.published), "2024-10-01");
    }

    #[test]
    fn rejects_web_pages() {
        assert!(parse_feed(b"<html><body>hi</body></html>").is_err());
    }

    #[test]
    fn new_episodes() {
        let mut show = Show { id: "x".into(), feed: "f".into(), ..Default::default() };
        apply(&mut show, parse_feed(FEED.as_bytes()).unwrap(), true);
        assert_eq!(show.episodes.iter().map(|e| e.new).collect::<Vec<_>>(), [true, false], "only the latest is new at first");
        show.episodes[0].new = false;
        show.episodes[1].file = Some("Test/ep1.m4a".into());
        let more = FEED.replace("<item>\n    <title>Episode 2", "<item><title>Episode 3</title><guid>ep-3</guid><pubDate>Wed, 09 Oct 2024 09:00:00 GMT</pubDate><enclosure url=\"https://cdn.example.com/ep3.mp3\" type=\"audio/mpeg\"/></item>\n  <item>\n    <title>Episode 2");
        apply(&mut show, parse_feed(more.as_bytes()).unwrap(), false);
        assert_eq!(
            show.episodes.iter().map(|e| (e.title.as_str(), e.new)).collect::<Vec<_>>(),
            [("Episode 3", true), ("Episode 2: More", false), ("Episode 1", false)]
        );
        assert_eq!(show.episodes[2].file.as_deref(), Some("Test/ep1.m4a"), "downloads are kept");
    }

    #[test]
    fn names_and_dates() {
        assert_eq!(safe_name("A/B: C?", "k"), "A-B- C-");
        assert_eq!(safe_name(" .. ", "k"), "k");
        assert_eq!(extension("https://x/y.MP3?z", ""), "mp3");
        assert_eq!(extension("https://x/download", "audio/x-m4a"), "m4a");
        assert_eq!(parse_date("Mon, 1 Jan 2024 00:00:00 +0100"), Some(1_704_063_600));
        assert_eq!(parse_duration("62:03"), 3_723_000);
        assert_eq!(parse_duration("bad"), 0);
        assert_eq!(civil_from_days(days_from_civil(2000, 2, 29)), (2000, 2, 29));
    }
}
