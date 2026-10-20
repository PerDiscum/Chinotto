// No console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod artwork;
mod commands;
mod devices;
mod state;
mod tags;
mod write;

use tauri::Manager;

use state::AppState;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .register_asynchronous_uri_scheme_protocol("art", |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            std::thread::spawn(move || {
                let session = app.state::<AppState>().session();
                responder.respond(artwork::handle(session.as_deref(), &request));
            });
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_devices,
            commands::add_folder,
            commands::open_ipod,
            commands::close_ipod,
            commands::eject,
            commands::startup_folder,
            write::set_allow_changes,
            write::edit_tracks,
            write::delete_tracks,
            write::add_files,
            write::create_playlist,
            write::rename_playlist,
            write::delete_playlist,
            write::set_playlist_tracks,
        ])
        .setup(|app| {
            if let Ok(dir) = app.path().app_config_dir() {
                app.state::<AppState>().load_settings(dir.join("settings.json"));
            }
            devices::start_watcher(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Chinotto");
}
