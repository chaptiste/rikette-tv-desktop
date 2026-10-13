//! Rikette's desktop app: a tray recorder that keeps a replay buffer and saves clips.

mod auth;
mod autostart;
mod billing;
mod brand;
mod capture;
mod clip_audio;
mod clips;
mod events;
mod hotkey;
mod ipc;
mod library;
mod logging;
mod media;
mod media_protocol;
mod notices;
mod overlay;
mod paths;
mod protocol;
mod release_notes;
mod settings;
mod splash;
mod tray;
mod updater;
mod upload;
mod window;

use std::sync::Mutex;
use std::time::Duration;

use tauri::RunEvent;
use tauri_plugin_deep_link::DeepLinkExt;

use capture::engine;

/// A protocol link that arrived while the launch splash was checking for updates.
static PENDING_URL: Mutex<Option<String>> = Mutex::new(None);

pub fn run() {
    logging::install();

    let app = tauri::Builder::default()
        // First, so a second launch hands over before anything else starts.
        .plugin(tauri_plugin_single_instance::init(|_app, argv, _cwd| {
            let url = protocol::url_from_args(argv);
            if splash::focus() {
                if url.is_some() {
                    *PENDING_URL.lock().unwrap() = url;
                }
                return;
            }
            if let Some(url) = url {
                protocol::handle(&url);
            }
            window::show_main(None);
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(autostart::plugin())
        .plugin(hotkey::plugin())
        .plugin(updater::plugin())
        .register_asynchronous_uri_scheme_protocol(media_protocol::SCHEME, |_ctx, request, responder| {
            tauri::async_runtime::spawn(async move {
                responder.respond(media_protocol::handle(request).await);
            });
        })
        .invoke_handler(ipc::handler())
        .on_menu_event(|_app, event| {
            clips::on_menu_event(event.id().as_ref());
        })
        .setup(|app| {
            events::init(app.handle().clone());
            logging::header(&version());
            #[cfg(any(windows, target_os = "linux"))]
            if cfg!(debug_assertions) {
                // Installed builds register the scheme in the installer.
                if let Err(error) = app.deep_link().register_all() {
                    log::warn!("[rikette] protocol registration failed: {error}");
                }
            }
            app.deep_link().on_open_url(|event| {
                for url in event.urls() {
                    protocol::handle(url.as_str());
                }
            });
            tauri::async_runtime::spawn(boot());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build the app");

    app.run(|_app, event| {
        if let RunEvent::ExitRequested { code, api, .. } = event {
            // Closing (or freeing) the last window keeps the app in the tray.
            if code.is_none() && !window::is_quitting() {
                api.prevent_exit();
            }
        }
    });
}

/// The app version, from `apps/desktop/package.json` (see `tauri.conf.json`).
pub fn version() -> String {
    events::app().package_info().version.to_string()
}

/// Shut the capture process down cleanly, then exit.
pub fn quit() {
    window::mark_quitting();
    window::hide_main();
    tauri::async_runtime::spawn(async {
        hotkey::unregister();
        engine::shutdown().await;
        events::app().exit(0);
    });
}

async fn boot() {
    // Like Discord: check for an update before opening, unless started with the OS.
    // RIKETTE_SPLASH_PREVIEW=1 shows the splash in debug builds too.
    let preview = std::env::var_os("RIKETTE_SPLASH_PREVIEW").is_some();
    let with_splash = (!cfg!(debug_assertions) || preview) && !autostart::opened_at_login();
    if with_splash {
        splash::show();
        if updater::install_on_launch(splash::set_status).await {
            return;
        }
        splash::set_status("Starting…", None);
    }

    release_notes::track_installed_version();
    auth::load_session();
    let settings = settings::load();
    if !cfg!(debug_assertions) && settings.autostart {
        if let Err(error) = autostart::apply(true) {
            log::warn!("[rikette] startup registration failed: {error:#}");
        }
    }
    ipc::apply_side_effects(None, &settings);
    tauri::async_runtime::spawn(async {
        if let Err(error) = library::open() {
            log::warn!("[rikette] library open failed: {error:#}");
            return;
        }
        clips::sync_output_folder().await;
        events::clips_changed();
    });
    engine::init();
    if let Err(error) = tray::create() {
        log::error!("[rikette] tray: {error}");
    }
    if let Err(error) = window::create_main() {
        log::error!("[rikette] window: {error}");
    }
    window::wait_until_loaded(Duration::from_secs(3)).await;
    if with_splash {
        splash::close();
    }
    updater::start(!with_splash);
    upload::start_queue();
    tauri::async_runtime::spawn(async {
        if let Err(error) = auth::fetch_me().await {
            log::warn!("[rikette] session restore failed: {error:#}");
        }
    });
    let launch_url = PENDING_URL
        .lock()
        .unwrap()
        .take()
        .or_else(|| protocol::url_from_args(std::env::args()));
    if let Some(url) = launch_url {
        protocol::handle(&url);
    }
    // Start capture once the window has painted, so starting OBS never delays it.
    engine::start(settings::load()).await;
}
