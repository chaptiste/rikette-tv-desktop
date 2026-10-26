//! Commands behind `window.echoframe` (see `src/tauri/bridge.ts` and `src/preload/api.ts`).

use serde::Serialize;
use serde_json::Value;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::capture::{canvas, engine};
use crate::clips::{self, DiscordExportDone, DiscordExportRequest, LibraryClip, TrimClipRequest};
use crate::library::ClipFolder;
use crate::notices::{self, ConfirmReply};
use crate::release_notes::{self, VersionChange};
use crate::settings::{self, AppSettings};
use crate::{auth, autostart, billing, clip_audio, events, hotkey, updater, upload};

type CmdResult<T> = Result<T, String>;

fn fail(error: anyhow::Error) -> String {
    format!("{error:#}")
}

#[tauri::command]
pub fn app_version() -> String {
    crate::version()
}

#[tauri::command]
pub fn settings_get() -> AppSettings {
    settings::load()
}

#[tauri::command]
pub async fn app_check_updates() {
    updater::check_now_or_prompt(true).await;
}

#[tauri::command]
pub fn app_version_change() -> Option<VersionChange> {
    release_notes::take_version_change()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdateResult {
    settings: AppSettings,
    hotkey_registered: bool,
    capture: engine::CaptureStatus,
}

#[tauri::command]
pub async fn settings_update(patch: Value) -> CmdResult<SettingsUpdateResult> {
    if let Some(enabled) = patch.get("autostart").and_then(Value::as_bool) {
        // Keep the previous preference if startup registration fails.
        autostart::apply(enabled).map_err(fail)?;
    }
    let prev = settings::load();
    let next = settings::update(&patch).map_err(fail)?;
    apply_side_effects(Some(&prev), &next);
    Ok(SettingsUpdateResult {
        settings: settings::load(),
        hotkey_registered: hotkey::is_registered(),
        capture: engine::status(),
    })
}

pub fn apply_side_effects(prev: Option<&AppSettings>, next: &AppSettings) {
    if prev.is_none_or(|p| p.hotkey != next.hotkey) {
        hotkey::register(&next.hotkey);
    }
    let Some(prev) = prev else { return };
    if prev.mic_volume != next.mic_volume {
        engine::set_mic_volume(next.mic_volume);
    }
    if engine::capture_changed(prev, next) {
        engine::schedule_restart(next.clone());
    }
}

#[tauri::command]
pub fn settings_hotkey_status() -> bool {
    hotkey::is_registered()
}

#[tauri::command]
pub fn settings_displays() -> Vec<canvas::ListedDisplay> {
    canvas::list_displays()
}

#[tauri::command]
pub async fn settings_audio_devices() -> capture_protocol::AudioDevices {
    engine::list_audio_devices().await
}

#[tauri::command]
pub async fn settings_windows() -> Vec<capture_protocol::WindowInfo> {
    engine::list_windows().await
}

#[tauri::command]
pub async fn settings_pick_output_folder() -> AppSettings {
    let current = settings::load();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = events::app().dialog().file().set_title("Clip output folder");
    dialog = dialog.set_directory(&current.output_folder);
    if let Some(win) = events::main_window() {
        dialog = dialog.set_parent(&win);
    }
    dialog.pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let Ok(Some(folder)) = rx.await else { return current };
    let Ok(path) = folder.into_path() else { return current };
    let path = path.to_string_lossy().into_owned();
    let patch = serde_json::json!({ "outputFolder": path });
    let Ok(next) = settings::update(&patch) else { return current };
    if next.output_folder != current.output_folder {
        tauri::async_runtime::spawn(async {
            clips::sync_output_folder().await;
            events::clips_changed();
        });
    }
    next
}

#[tauri::command]
pub fn capture_status() -> engine::CaptureStatus {
    engine::status()
}

#[tauri::command]
pub async fn capture_pause() -> engine::CaptureStatus {
    engine::pause().await;
    engine::status()
}

#[tauri::command]
pub async fn capture_resume() -> engine::CaptureStatus {
    engine::resume().await;
    engine::status()
}

#[tauri::command]
pub fn capture_restart() -> engine::CaptureStatus {
    engine::restart();
    engine::status()
}

/// Async so it runs off the main thread: the first save creates the HUD window, and
/// creating a webview inside a sync command deadlocks WebView2 on Windows.
#[tauri::command]
pub async fn clips_save() {
    hotkey::request_save_clip(hotkey::SaveReason::Ui);
}

#[tauri::command]
pub async fn clips_list() -> CmdResult<Vec<LibraryClip>> {
    clips::list_library_clips().await.map_err(fail)
}

#[tauri::command]
pub async fn clips_get(id: String) -> CmdResult<Option<LibraryClip>> {
    clips::get_library_clip(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_delete(id: String) -> CmdResult<bool> {
    clips::confirm_delete_clip(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_delete_local(id: String) -> CmdResult<bool> {
    clips::confirm_delete_local_file(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_remove_cloud(id: String) -> CmdResult<bool> {
    clips::remove_from_cloud(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_reveal(id: String) -> CmdResult<()> {
    clips::reveal_clip(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_rename(id: String, title: String) -> CmdResult<LibraryClip> {
    clips::rename_clip(&id, &title).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_menu(id: String) -> CmdResult<()> {
    clips::popup_clip_menu(&id).await.map_err(fail)
}

#[tauri::command]
pub fn folders_list() -> CmdResult<Vec<ClipFolder>> {
    clips::list_folders().map_err(fail)
}

#[tauri::command]
pub fn folders_create(name: String) -> CmdResult<ClipFolder> {
    clips::create_folder(&name).map_err(fail)
}

#[tauri::command]
pub fn folders_rename(id: String, name: String) -> CmdResult<ClipFolder> {
    clips::rename_folder(&id, &name).map_err(fail)
}

#[tauri::command]
pub async fn folders_delete(id: String) -> CmdResult<bool> {
    clips::confirm_delete_folder(&id).await.map_err(fail)
}

#[tauri::command]
pub fn clips_move(ids: Vec<String>, folder_id: Option<String>) -> CmdResult<()> {
    clips::move_clips(&ids, folder_id.as_deref()).map_err(fail)
}

#[tauri::command]
pub async fn clips_audio(id: String) -> CmdResult<Vec<clip_audio::ClipAudioTrack>> {
    clip_audio::get_clip_audio(&id).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_trim(request: TrimClipRequest) -> CmdResult<LibraryClip> {
    clips::trim_clip(request).await.map_err(fail)
}

#[tauri::command]
pub async fn clips_export_discord(request: DiscordExportRequest) -> CmdResult<DiscordExportDone> {
    clips::export_for_discord(request).await.map_err(fail)
}

#[tauri::command]
pub fn clips_discord_drag(id: String) {
    clips::start_discord_drag(&id);
}

#[tauri::command]
pub fn clips_discord_reveal(id: String) -> CmdResult<()> {
    clips::reveal_discord_export(&id).map_err(fail)
}

#[tauri::command]
pub async fn clips_copy_link(id: String) -> CmdResult<String> {
    let clip = clips::get_library_clip(&id).await.map_err(fail)?;
    let url = clip.and_then(|c| c.public_url).ok_or("No cloud link")?;
    events::app().clipboard().write_text(url.clone()).map_err(|e| e.to_string())?;
    clips::toast_link_copied();
    Ok(url)
}

#[tauri::command]
pub async fn clips_upload(id: String) -> CmdResult<Option<String>> {
    upload::share_clip(&id, false).await.map_err(fail)
}

#[tauri::command]
pub fn auth_me() -> Option<Value> {
    auth::cached_me()
}

#[tauri::command]
pub async fn auth_refresh() -> CmdResult<Option<Value>> {
    auth::fetch_me().await.map_err(fail)
}

#[tauri::command]
pub fn auth_sign_in() -> CmdResult<()> {
    let url = format!("{}/login?desktop=1", settings::load().api_base());
    events::app().opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_dev_login(email: Option<String>) -> CmdResult<Value> {
    let email = email.filter(|e| e.contains('@')).unwrap_or_else(|| "dev@echoframe.local".into());
    let me = auth::dev_login(&email).await.map_err(fail)?;
    tauri::async_runtime::spawn(upload::drain());
    Ok(me)
}

#[tauri::command]
pub fn auth_sign_out() {
    auth::clear_session();
}

#[tauri::command]
pub async fn billing_checkout(interval: String) -> CmdResult<()> {
    billing::start_checkout(&interval).await.map_err(fail)
}

#[tauri::command]
pub async fn billing_portal() -> CmdResult<()> {
    billing::open_portal().await.map_err(fail)
}

#[tauri::command]
pub fn app_confirm_result(reply: ConfirmReply) {
    notices::reply_confirm(reply);
}

pub fn handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        app_version,
        settings_get,
        app_check_updates,
        app_version_change,
        settings_update,
        settings_hotkey_status,
        settings_displays,
        settings_audio_devices,
        settings_windows,
        settings_pick_output_folder,
        capture_status,
        capture_pause,
        capture_resume,
        capture_restart,
        clips_save,
        clips_list,
        clips_get,
        clips_delete,
        clips_delete_local,
        clips_remove_cloud,
        clips_reveal,
        clips_rename,
        clips_menu,
        folders_list,
        folders_create,
        folders_rename,
        folders_delete,
        clips_move,
        clips_audio,
        clips_trim,
        clips_export_discord,
        clips_discord_drag,
        clips_discord_reveal,
        clips_copy_link,
        clips_upload,
        auth_me,
        auth_refresh,
        auth_sign_in,
        auth_dev_login,
        auth_sign_out,
        billing_checkout,
        billing_portal,
        app_confirm_result,
    ]
}
