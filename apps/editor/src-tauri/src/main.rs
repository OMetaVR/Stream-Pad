//! StreamPad editor — Tauri shell around the React UI in `../src`. Every command the UI may call is
//! registered here; `src/lib/backend.tauri.ts` is the only caller.

mod apps;
mod config_cmd;
mod covers;
mod daemon;
mod env;
mod error;
mod ini;
mod kde;
mod service;
mod system;
mod window;

use tauri::Manager;

fn main() {
    env::apply_webkit_workarounds();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

    let result = tauri::Builder::default()
        // Must be the first plugin: a second launch focuses this window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| window::focus_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .manage(apps::AppsCache::default())
        .setup(|app| {
            let handle = app.handle();
            app.manage(daemon::DaemonClient::start(handle.clone()));
            window::allow_asset_dirs(handle);
            window::create_main(handle)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            config_cmd::load_config,
            config_cmd::save_config,
            config_cmd::config_path,
            covers::import_cover,
            covers::import_cover_bytes,
            covers::import_image_url,
            apps::list_apps,
            kde::list_kde_shortcuts,
            daemon::daemon_status,
            daemon::daemon_request,
            service::start_daemon,
            service::get_autostart,
            service::set_autostart,
            system::open_path,
            system::compile_macro,
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        eprintln!("streampad: {e}");
        std::process::exit(1);
    }
}
