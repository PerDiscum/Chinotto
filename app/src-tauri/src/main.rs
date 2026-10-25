// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod burn;
mod commands;
mod convert;
mod cover;
mod devices;
mod export;
mod libcommands;
mod library;
mod msc;
mod mtp;
mod openmg;
mod playback;
mod podcasts;
mod rip;
mod scratch;
mod setup;
mod state;
mod tags;
mod tagwrite;
mod write;

use tauri::Manager;

use state::AppState;

fn main() {
    tauri::Builder::default()
        // A second launch (e.g. "Open With Chinotto" on Linux) hands its files to this one.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let files = scratch::files_in_args(&args);
            if !files.is_empty() {
                scratch::add(app, &files);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .register_asynchronous_uri_scheme_protocol("art", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let session = app.state::<AppState>().session();
                responder.respond(artwork::handle(session.as_deref(), &request));
            });
        })
        .register_asynchronous_uri_scheme_protocol("libart", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || responder.respond(libcommands::handle_art(&app, &request)));
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            podcasts::podcasts_get,
            podcasts::podcasts_set_folder,
            podcasts::podcast_add,
            podcasts::podcasts_refresh,
            podcasts::podcast_remove,
            podcasts::podcast_mark,
            podcasts::podcast_delete_file,
            podcasts::podcast_download,
            podcasts::podcast_cancel,
            podcasts::podcast_ipod_status,
            podcasts::podcast_send,
            podcasts::podcast_remove_played,
            playback::reveal_song,
            commands::add_folder,
            commands::open_ipod,
            setup::setup_info,
            msc::msc_inspect,
            mtp::mtp_open,
            mtp::mtp_close,
            mtp::mtp_info,
            openmg::openmg_view,
            openmg::openmg_reorder,
            rip::cd_list,
            rip::cd_rip,
            rip::cd_eject,
            commands::open_licenses,
            rip::cd_music_info,
            burn::burn_status,
            burn::burn_inspect,
            burn::burn_image,
            msc::msc_save,
            msc::msc_state,
            msc::msc_allow,
            msc::msc_set_conversion,
            msc::msc_add,
            msc::msc_delete,
            setup::setup_ipod,
            commands::close_ipod,
            commands::eject,
            commands::startup_folder,
            playback::play_track,
            playback::player_play,
            playback::player_pause,
            playback::player_stop,
            playback::player_seek,
            playback::player_volume,
            write::set_allow_changes,
            write::edit_tracks,
            write::delete_tracks,
            write::add_files,
            write::create_playlist,
            write::rename_playlist,
            write::delete_playlist,
            write::set_playlist_tracks,
            write::set_artwork,
            write::remove_artwork,
            write::image_preview,
            write::save_cover,
            write::paste_image,
            commands::device_icons,
            commands::system_locales,
            write::set_conversion,
            export::export_name,
            export::export_songs,
            libcommands::list_libraries,
            libcommands::create_library,
            libcommands::update_library,
            libcommands::delete_library,
            libcommands::scan_library,
            libcommands::remove_unavailable,
            libcommands::library_view,
            libcommands::lib_create_playlist,
            libcommands::lib_rename_playlist,
            libcommands::lib_delete_playlist,
            libcommands::lib_set_playlist_tracks,
            libcommands::lib_remove_tracks,
            libcommands::lib_edit_tracks,
            libcommands::save_file_cover,
            scratch::scratch_view,
            scratch::scratch_add,
            scratch::scratch_remove,
            commands::set_device_icon,
        ])
        .setup(|app| {
            app.state::<AppState>().set_player(playback::start(app.handle()));
            if let Ok(dir) = app.path().app_config_dir() {
                app.state::<AppState>().load_settings(dir.join("settings.json"));
            }
            if let Ok(dir) = app.path().app_data_dir() {
                *app.state::<AppState>().libraries.lock().unwrap() = libcommands::Libraries::load(dir.join("libraries"));
                *app.state::<AppState>().podcasts.lock().unwrap() = podcasts::Podcasts::load(dir.join("podcasts.json"));
            }
            libcommands::start_hourly(app.handle().clone());
            // Songs given when Chinotto was started (Linux, Windows).
            let files = scratch::files_in_args(&std::env::args().collect::<Vec<_>>());
            if !files.is_empty() {
                scratch::add(app.handle(), &files);
            }
            devices::start_watcher(app.handle().clone());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while running Chinotto")
        .run(|_app, _event| {
            // macOS: songs opened with Chinotto (double-click, Open With, dropped on the Dock icon).
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &_event {
                let files: Vec<std::path::PathBuf> = urls.iter().filter_map(|u| u.to_file_path().ok()).collect();
                if !files.is_empty() {
                    scratch::add(_app, &files);
                }
            }
        });
}
