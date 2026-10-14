//! Saving clips from the replay buffer and everything the library does with them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use anyhow::{anyhow, bail, Result};
use capture_protocol::{CaptureTarget, TargetKind};
use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::Mutex as AsyncMutex;

use crate::capture::{canvas, engine};
use crate::library::{self, ClipFolder, CloudStatus, LocalClip};
use crate::media::discord_plan::{self, DiscordSource, DISCORD_LIMIT_BYTES};
use crate::media::ffmpeg::{self, SizedEncode};
use crate::notices::{self, Confirm, ToastKind};
use crate::upload::{self, QuotaError};
use crate::{clip_audio, events, media_protocol, paths, settings, window};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryClip {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub duration_ms: i64,
    pub width: i64,
    pub height: i64,
    pub bytes: i64,
    pub game_title: Option<String>,
    pub cloud_status: CloudStatus,
    pub cloud_id: Option<String>,
    pub public_url: Option<String>,
    pub local_file: bool,
    pub video_url: String,
    pub thumb_url: Option<String>,
    pub folder_id: Option<String>,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub async fn save_current_clip() -> Result<()> {
    let settings = settings::load();
    let status = engine::status();
    if status.paused {
        bail!("Buffering is paused");
    }
    if !status.ready {
        bail!(status.error.clone().unwrap_or_else(|| "Capture is not running".into()));
    }

    let raw = engine::save_replay().await?;
    let created_at = now_ms();
    let game_title = game_name(&status.capture_target);
    let folder = Path::new(&settings.output_folder).join(sanitize_name(&game_title));
    tokio::fs::create_dir_all(&folder).await?;
    let dest = folder.join(format!("{}.mp4", clip_stem(created_at)));
    let id = uuid::Uuid::new_v4().to_string();
    let canvas = canvas::for_settings(&settings);

    tokio::fs::copy(&raw, &dest).await?;
    if raw != dest {
        let _ = tokio::fs::remove_file(&raw).await;
    }
    let bytes = tokio::fs::metadata(&dest).await?.len() as i64;
    let row = LocalClip {
        thumb_path: Some(thumb_path(&id)),
        id,
        cloud_id: None,
        path: dest.to_string_lossy().into_owned(),
        created_at,
        duration_ms: (status.buffer_sec as i64 * 1000).max(1000),
        width: canvas.output_width as i64,
        height: canvas.output_height as i64,
        bytes,
        title: Some(default_clip_title(created_at)),
        game_title: Some(game_title),
        cloud_status: CloudStatus::Local,
        folder_id: None,
    };
    library::insert_clip(&row)?;
    events::clips_changed();
    log::info!("[rikette] saved {}", row.path);
    upload::hold(&row.id);
    tauri::async_runtime::spawn(finalize_saved_clip(row));
    Ok(())
}

fn thumb_path(id: &str) -> String {
    paths::thumb_file(id).to_string_lossy().into_owned()
}

/// Mint the link right away (so it is on the clipboard), polish the file, then upload.
async fn finalize_saved_clip(row: LocalClip) {
    let id = row.id.clone();
    let share = async {
        if crate::auth::access_token().is_none() {
            return;
        }
        if let Err(error) = upload::share_clip(&id, true).await {
            if let Some(quota) = error.downcast_ref::<QuotaError>() {
                log::warn!("[rikette] quota, keeping local: {quota}");
                toast_quota(&quota.code);
            } else {
                log::warn!("[rikette] cloud share failed, keeping local: {error:#}");
            }
        }
    };
    let ((), ()) = tokio::join!(polish_local_clip(&row), share);
    upload::release(&id);
}

async fn polish_local_clip(row: &LocalClip) {
    let path = PathBuf::from(&row.path);
    let tmp = sibling(&path, "faststart");
    match ffmpeg::remux_faststart(&path, &tmp).await {
        Ok(()) => {
            if let Err(error) = replace_file(&tmp, &path).await {
                log::warn!("[rikette] remux replace failed: {error:#}");
            }
        }
        Err(error) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            log::warn!("[rikette] remux failed: {error:#}");
        }
    }
    let _ = tokio::fs::create_dir_all(paths::thumbs_dir()).await;
    if let Some(thumb) = row.thumb_path.as_deref() {
        if let Err(error) = ffmpeg::write_thumbnail(&path, Path::new(thumb)).await {
            log::warn!("[rikette] thumbnail failed: {error:#}");
        }
    }
    match ffmpeg::probe_media(&path).await {
        Ok(info) => {
            let bytes = tokio::fs::metadata(&path).await.map(|m| m.len() as i64).unwrap_or(row.bytes);
            let _ = library::update_clip(&row.id, |c| {
                c.duration_ms = info.duration_ms;
                c.width = info.width as i64;
                c.height = info.height as i64;
                c.bytes = bytes;
            });
            events::clips_changed();
            log::info!("[rikette] polished {} {}x{} {}ms", row.path, info.width, info.height, info.duration_ms);
        }
        Err(error) => log::warn!("[rikette] probe failed: {error:#}"),
    }
}

/// `.<stem>.<tag>.mp4` next to the clip; dot files are never imported.
fn sibling(path: &Path, tag: &str) -> PathBuf {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("clip");
    path.with_file_name(format!(".{stem}.{tag}.mp4"))
}

pub async fn list_library_clips() -> Result<Vec<LibraryClip>> {
    tauri::async_runtime::spawn(refresh_library());
    let mut clips = Vec::new();
    for row in library::list_clips()? {
        clips.push(to_library_clip(settle_thumb(row).await).await);
    }
    Ok(clips)
}

pub async fn get_library_clip(id: &str) -> Result<Option<LibraryClip>> {
    let Some(row) = library::get_clip(id)? else { return Ok(None) };
    Ok(Some(to_library_clip(settle_thumb(row).await).await))
}

pub async fn rename_clip(id: &str, raw_title: &str) -> Result<LibraryClip> {
    let row = library::get_clip(id)?.ok_or_else(|| anyhow!("Clip not found"))?;
    let title = collapse_spaces(raw_title);
    if title.is_empty() {
        bail!("Title is empty");
    }
    if title.chars().count() > 120 {
        bail!("Title is too long");
    }
    let mut dest = PathBuf::from(&row.path);
    if exists(&dest).await {
        let dir = dest.parent().map(Path::to_path_buf).unwrap_or_default();
        let next = unique_path(&dir, &sanitize_name(&title), ".mp4", Some(&dest)).await;
        if next != dest {
            rename_case_safe(&dest, &next).await?;
            dest = next;
        }
    }
    let updated = library::update_clip(id, |c| {
        c.title = Some(title.clone());
        c.path = dest.to_string_lossy().into_owned();
    })?;
    events::clips_changed();
    if let Some(cloud_id) = updated.cloud_id.clone() {
        tauri::async_runtime::spawn(async move {
            if let Err(error) = upload::patch_cloud_title(&cloud_id, &title).await {
                log::warn!("[rikette] cloud rename failed: {error:#}");
            }
        });
    }
    Ok(to_library_clip(updated).await)
}

fn reveal(path: &Path) -> Result<()> {
    events::app().opener().reveal_item_in_dir(path)?;
    Ok(())
}

pub async fn reveal_clip(id: &str) -> Result<()> {
    let row = library::get_clip(id)?.ok_or_else(|| anyhow!("Clip not found"))?;
    if !exists(Path::new(&row.path)).await {
        bail!("No local file");
    }
    reveal(Path::new(&row.path))
}

async fn delete_clip(id: &str) -> Result<()> {
    let Some(row) = library::get_clip(id)? else { return Ok(()) };
    if let Some(cloud_id) = row.cloud_id.as_deref() {
        upload::wait_for_idle(id).await;
        if let Err(error) = upload::delete_cloud_clip(cloud_id).await {
            log::warn!("[rikette] cloud delete failed: {error:#}");
        }
    }
    let _ = tokio::fs::remove_file(&row.path).await;
    if let Some(thumb) = row.thumb_path.as_deref() {
        let _ = tokio::fs::remove_file(thumb).await;
    }
    clip_audio::clear_cache(id).await;
    library::delete_clip(id)?;
    events::clips_changed();
    refresh_me_later();
    Ok(())
}

fn refresh_me_later() {
    tauri::async_runtime::spawn(async {
        let _ = crate::auth::fetch_me().await;
    });
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub async fn confirm_delete_clip(id: &str) -> Result<bool> {
    let Some(row) = library::get_clip(id)? else { return Ok(false) };
    let description = if row.cloud_id.is_some() {
        format!("{}\nThis removes the file, kills the public link, and refunds quota.", file_name(&row.path))
    } else {
        format!("{}\nThis deletes the file from disk.", file_name(&row.path))
    };
    let ok = notices::ask_confirm(Confirm {
        title: "Delete clip",
        description: &description,
        confirm_label: "Delete",
        cancel_label: "Cancel",
        destructive: true,
    })
    .await;
    if ok {
        delete_clip(id).await?;
    }
    Ok(ok)
}

pub async fn remove_from_cloud(id: &str) -> Result<bool> {
    let Some(row) = library::get_clip(id)? else { return Ok(false) };
    let Some(cloud_id) = row.cloud_id.clone() else { return Ok(false) };
    let ok = notices::ask_confirm(Confirm {
        title: "Remove from cloud",
        description: "The local file stays. The public link dies and quota is refunded.",
        confirm_label: "Remove",
        cancel_label: "Cancel",
        destructive: true,
    })
    .await;
    if !ok {
        return Ok(false);
    }
    upload::wait_for_idle(id).await;
    upload::delete_cloud_clip(&cloud_id).await?;
    if exists(Path::new(&row.path)).await {
        library::update_clip(id, |c| {
            c.cloud_id = None;
            c.cloud_status = CloudStatus::Local;
        })?;
    } else {
        library::delete_clip(id)?;
    }
    events::clips_changed();
    refresh_me_later();
    Ok(true)
}

pub async fn confirm_delete_local_file(id: &str) -> Result<bool> {
    let Some(row) = library::get_clip(id)? else { return Ok(false) };
    if !row.is_live_cloud() || !exists(Path::new(&row.path)).await {
        return Ok(false);
    }
    let description = format!(
        "{}\nRemoves the MP4 on disk. The public link stays. The cloud copy is compressed, not original quality, and you can't trim without a local file.",
        file_name(&row.path)
    );
    let ok = notices::ask_confirm(Confirm {
        title: "Delete local file",
        description: &description,
        confirm_label: "Delete local file",
        cancel_label: "Cancel",
        destructive: true,
    })
    .await;
    if !ok {
        return Ok(false);
    }
    let _ = tokio::fs::remove_file(&row.path).await;
    clip_audio::clear_cache(id).await;
    events::clips_changed();
    Ok(true)
}

/// Menu item ids are `clip:<action>:<clip id>[:<folder id>]`.
const MENU_PREFIX: &str = "clip:";

pub async fn popup_clip_menu(id: &str) -> Result<()> {
    let Some(row) = library::get_clip(id)? else { return Ok(()) };
    let Some(win) = events::main_window() else { return Ok(()) };
    let app = events::app();
    let signed_in = crate::auth::access_token().is_some();
    let local = exists(Path::new(&row.path)).await;
    let item = |action: &str, label: &str, enabled: bool| {
        MenuItem::with_id(app, format!("{MENU_PREFIX}{action}:{id}"), label, enabled, None::<&str>)
    };
    let menu = Menu::new(app)?;
    menu.append(&item("edit", "Edit", true)?)?;
    if row.cloud_id.is_some() {
        menu.append(&item("copy-link", "Copy link", true)?)?;
    }
    if signed_in && (row.cloud_id.is_none() || matches!(row.cloud_status, CloudStatus::Error | CloudStatus::Local)) {
        let label = if row.cloud_status == CloudStatus::Error { "Retry upload" } else { "Upload" };
        menu.append(&item("upload", label, true)?)?;
    }
    if local {
        menu.append(&item("discord", "Export for Discord", !DISCORD_BUSY.load(Ordering::SeqCst))?)?;
        menu.append(&item("reveal", "Show in folder", true)?)?;
    }
    let folders = library::list_folders()?;
    if !folders.is_empty() {
        let submenu = Submenu::new(app, "Move to folder", true)?;
        for folder in &folders {
            submenu.append(&CheckMenuItem::with_id(
                app,
                format!("{MENU_PREFIX}move:{id}:{}", folder.id),
                &folder.name,
                true,
                row.folder_id.as_deref() == Some(folder.id.as_str()),
                None::<&str>,
            )?)?;
        }
        submenu.append(&PredefinedMenuItem::separator(app)?)?;
        submenu.append(&CheckMenuItem::with_id(
            app,
            format!("{MENU_PREFIX}move:{id}:"),
            "Unsorted",
            true,
            row.folder_id.is_none(),
            None::<&str>,
        )?)?;
        menu.append(&submenu)?;
    }
    let mut cloud_actions = Vec::new();
    if row.is_live_cloud() && local {
        cloud_actions.push(item("delete-local", "Delete local file", true)?);
    }
    if row.cloud_id.is_some() {
        cloud_actions.push(item("remove-cloud", "Remove from cloud", true)?);
    }
    if !cloud_actions.is_empty() {
        menu.append(&PredefinedMenuItem::separator(app)?)?;
        for action in &cloud_actions {
            menu.append(action)?;
        }
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&item("delete", "Delete", true)?)?;
    win.as_ref().window().popup_menu(&menu)?;
    Ok(())
}

/// Runs a clip menu item; false when the id is not one of ours.
pub fn on_menu_event(menu_id: &str) -> bool {
    let Some(rest) = menu_id.strip_prefix(MENU_PREFIX) else { return false };
    let mut parts = rest.splitn(3, ':');
    let action = parts.next().unwrap_or("").to_string();
    let id = parts.next().unwrap_or("").to_string();
    let extra = parts.next().map(str::to_string);
    tauri::async_runtime::spawn(async move {
        let result: Result<()> = async {
            match action.as_str() {
                "edit" => events::emit("app:navigate", format!("/editor/{id}")),
                "copy-link" => {
                    if let Some(cloud_id) = library::get_clip(&id)?.and_then(|c| c.cloud_id) {
                        let url = format!("{}/c/{cloud_id}", settings::load().api_base());
                        events::app().clipboard().write_text(url)?;
                        toast_link_copied();
                    }
                }
                "upload" => match upload::share_clip(&id, false).await {
                    Ok(_) => {}
                    Err(error) => match error.downcast_ref::<QuotaError>() {
                        Some(quota) => toast_quota(&quota.code),
                        None => return Err(error),
                    },
                },
                "discord" => {
                    // Failures already toast.
                    let _ = export_for_discord(DiscordExportRequest { id, ..Default::default() }).await;
                }
                "reveal" => reveal_clip(&id).await?,
                "move" => {
                    let folder = extra.filter(|f| !f.is_empty());
                    move_clips(std::slice::from_ref(&id), folder.as_deref())?;
                }
                "delete-local" => drop(confirm_delete_local_file(&id).await?),
                "remove-cloud" => drop(remove_from_cloud(&id).await?),
                "delete" => drop(confirm_delete_clip(&id).await?),
                _ => {}
            }
            Ok(())
        }
        .await;
        if let Err(error) = result {
            notices::toast(format!("{error:#}"), None, ToastKind::Error);
        }
    });
    true
}

pub fn list_folders() -> Result<Vec<ClipFolder>> {
    library::list_folders()
}

pub fn create_folder(raw: &str) -> Result<ClipFolder> {
    let name = valid_folder_name(raw, None)?;
    let folder = ClipFolder { id: uuid::Uuid::new_v4().to_string(), name, created_at: now_ms() };
    library::insert_folder(&folder)?;
    events::clips_changed();
    Ok(folder)
}

pub fn rename_folder(id: &str, raw: &str) -> Result<ClipFolder> {
    let folder = library::list_folders()?
        .into_iter()
        .find(|f| f.id == id)
        .ok_or_else(|| anyhow!("Folder not found"))?;
    let name = valid_folder_name(raw, Some(id))?;
    library::rename_folder(id, &name)?;
    events::clips_changed();
    Ok(ClipFolder { name, ..folder })
}

pub async fn confirm_delete_folder(id: &str) -> Result<bool> {
    let Some(folder) = library::list_folders()?.into_iter().find(|f| f.id == id) else {
        return Ok(false);
    };
    let count = library::list_clips()?
        .iter()
        .filter(|row| row.folder_id.as_deref() == Some(id))
        .count();
    let title = format!("Delete \"{}\"", folder.name);
    let description = match count {
        0 => "This folder is empty.".to_string(),
        1 => "Only the folder goes. Its clip moves back to Unsorted.".to_string(),
        n => format!("Only the folder goes. Its {n} clips move back to Unsorted."),
    };
    let ok = notices::ask_confirm(Confirm {
        title: &title,
        description: &description,
        confirm_label: "Delete folder",
        cancel_label: "Cancel",
        destructive: true,
    })
    .await;
    if ok {
        library::delete_folder(id)?;
        events::clips_changed();
    }
    Ok(ok)
}

pub fn move_clips(ids: &[String], folder_id: Option<&str>) -> Result<()> {
    if let Some(folder_id) = folder_id {
        if !library::list_folders()?.iter().any(|f| f.id == folder_id) {
            bail!("Folder not found");
        }
    }
    library::set_clips_folder(ids, folder_id)?;
    events::clips_changed();
    Ok(())
}

const FOLDER_NAME_MAX: usize = 40;

fn valid_folder_name(raw: &str, ignore_id: Option<&str>) -> Result<String> {
    let name = collapse_spaces(raw);
    if name.is_empty() {
        bail!("Give the folder a name");
    }
    if name.chars().count() > FOLDER_NAME_MAX {
        bail!("Folder names are {FOLDER_NAME_MAX} characters at most");
    }
    let lower = name.to_lowercase();
    let taken = library::list_folders()?
        .iter()
        .any(|f| Some(f.id.as_str()) != ignore_id && f.name.to_lowercase() == lower);
    if taken {
        bail!("You already have a folder called \"{name}\"");
    }
    Ok(name)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrimMode {
    Overwrite,
    SaveAsNew,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimClipRequest {
    pub id: String,
    pub start_sec: f64,
    pub end_sec: f64,
    pub mode: TrimMode,
    #[serde(default)]
    pub volumes: Option<Vec<Option<f64>>>,
}

fn volumes_for(count: usize, requested: Option<&[Option<f64>]>) -> Vec<f64> {
    (0..count)
        .map(|index| {
            requested
                .and_then(|v| v.get(index).copied().flatten())
                .filter(|v| v.is_finite())
                .map_or(1.0, |v| v.clamp(0.0, 2.0))
        })
        .collect()
}

pub async fn trim_clip(request: TrimClipRequest) -> Result<LibraryClip> {
    let source = library::get_clip(&request.id)?.ok_or_else(|| anyhow!("Clip not found"))?;
    let source_path = PathBuf::from(&source.path);
    if !exists(&source_path).await {
        bail!("No local file");
    }
    if matches!(request.mode, TrimMode::Overwrite) && source.cloud_id.is_some() {
        upload::wait_for_idle(&source.id).await;
    }
    let start = request.start_sec.max(0.0);
    let end = request.end_sec.max(start + 0.25);
    let tmp = sibling(&source_path, "trim");
    let _ = tokio::fs::remove_file(&tmp).await;
    let count = ffmpeg::count_audio_streams(&source_path).await?;
    let volumes = volumes_for(count, request.volumes.as_deref());

    let first = async {
        ffmpeg::trim_with_volumes(&source_path, &tmp, start, end, &volumes).await?;
        if ffmpeg::probe_media(&tmp).await?.duration_ms < 200 {
            bail!("copy trim too short");
        }
        Ok(())
    }
    .await;
    if let Err(error) = first {
        if ffmpeg::volumes_need_encode(&volumes) {
            return Err(error);
        }
        log::warn!("[rikette] trim failed, reencoding: {error:#}");
        let _ = tokio::fs::remove_file(&tmp).await;
        ffmpeg::trim_reencode(&source_path, &tmp, start, end).await?;
    }

    if matches!(request.mode, TrimMode::SaveAsNew) {
        let created_at = now_ms();
        let dir = source_path.parent().map(Path::to_path_buf).unwrap_or_default();
        let dest = dir.join(format!("{}.mp4", clip_stem(created_at)));
        let id = uuid::Uuid::new_v4().to_string();
        let thumb = paths::thumb_file(&id);
        tokio::fs::rename(&tmp, &dest).await?;
        let _ = tokio::fs::create_dir_all(paths::thumbs_dir()).await;
        if let Err(error) = ffmpeg::write_thumbnail(&dest, &thumb).await {
            log::warn!("[rikette] thumbnail failed: {error:#}");
        }
        let info = ffmpeg::probe_media(&dest).await?;
        let row = LocalClip {
            id,
            cloud_id: None,
            path: dest.to_string_lossy().into_owned(),
            thumb_path: Some(thumb.to_string_lossy().into_owned()),
            created_at,
            duration_ms: info.duration_ms,
            width: info.width as i64,
            height: info.height as i64,
            bytes: tokio::fs::metadata(&dest).await?.len() as i64,
            title: Some(default_clip_title(created_at)),
            game_title: source.game_title.clone(),
            cloud_status: CloudStatus::Local,
            folder_id: source.folder_id.clone(),
        };
        library::insert_clip(&row)?;
        events::clips_changed();
        return Ok(to_library_clip(row).await);
    }

    replace_file(&tmp, &source_path).await?;
    clip_audio::clear_cache(&source.id).await;
    let thumb = paths::thumb_file(&source.id);
    let _ = tokio::fs::create_dir_all(paths::thumbs_dir()).await;
    match ffmpeg::write_thumbnail(&source_path, &thumb).await {
        Ok(()) => {
            let thumb = thumb.to_string_lossy().into_owned();
            if source.thumb_path.as_deref() != Some(thumb.as_str()) {
                library::update_clip(&source.id, |c| c.thumb_path = Some(thumb))?;
            }
        }
        Err(error) => log::warn!("[rikette] thumbnail failed: {error:#}"),
    }
    let info = ffmpeg::probe_media(&source_path).await?;
    let bytes = tokio::fs::metadata(&source_path).await?.len() as i64;
    let in_cloud = source.cloud_id.is_some();
    let row = library::update_clip(&source.id, |c| {
        c.duration_ms = info.duration_ms;
        c.width = info.width as i64;
        c.height = info.height as i64;
        c.bytes = bytes;
        if in_cloud {
            c.cloud_status = CloudStatus::Uploading;
        }
    })?;
    events::clips_changed();
    if in_cloud {
        let id = source.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = upload::replace_cloud_object(&id).await {
                log::warn!("[rikette] cloud overwrite failed: {error:#}");
            }
        });
    }
    Ok(to_library_clip(row).await)
}

/// Lives in the output folder so people can find it, but is never imported into the library.
const DISCORD_EXPORT_DIR: &str = "Discord exports";
static DISCORD_BUSY: AtomicBool = AtomicBool::new(false);

struct DiscordExport {
    path: PathBuf,
    /// A small copy of the thumbnail; Windows draws drag images at full pixel size.
    drag_image: Option<PathBuf>,
}

/// Finished exports by clip id. The page names a clip, never a path, when it starts a drag.
static DISCORD_EXPORTS: Mutex<Option<HashMap<String, DiscordExport>>> = Mutex::new(None);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordExportRequest {
    pub id: String,
    pub start_sec: Option<f64>,
    pub end_sec: Option<f64>,
    pub volumes: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscordExportDone {
    pub path: String,
    pub bytes: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportProgress {
    id: String,
    title: String,
    percent: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiscordReady {
    id: String,
    title: String,
    bytes: u64,
    thumb_url: Option<String>,
}

struct DiscordBusy;

impl Drop for DiscordBusy {
    fn drop(&mut self) {
        DISCORD_BUSY.store(false, Ordering::SeqCst);
    }
}

pub async fn export_for_discord(request: DiscordExportRequest) -> Result<DiscordExportDone> {
    if DISCORD_BUSY.swap(true, Ordering::SeqCst) {
        notices::toast("A Discord export is already running", None, ToastKind::Warning);
        bail!("A Discord export is already running");
    }
    let _busy = DiscordBusy;
    let source = library::get_clip(&request.id)?.ok_or_else(|| anyhow!("Clip not found"))?;
    let source_path = PathBuf::from(&source.path);
    if !exists(&source_path).await {
        bail!("No local file");
    }
    let title = clip_title(&source);
    let folder = Path::new(&settings::load().output_folder).join(DISCORD_EXPORT_DIR);
    let dest = folder.join(format!("{}.mp4", sanitize_name(&title)));
    let tmp = folder.join(format!(".{}.discord.mp4", source.id));
    let pass_log = std::env::temp_dir().join(format!("rikette-discord-{}", source.id));

    let emit_progress = |percent: Option<u32>| {
        events::emit(
            "clips:export-progress",
            ExportProgress { id: source.id.clone(), title: title.clone(), percent },
        );
    };
    let result = async {
        emit_progress(Some(0));
        tokio::fs::create_dir_all(&folder).await?;
        let info = ffmpeg::probe_media(&source_path).await?;
        let full = info.duration_ms as f64 / 1000.0;
        let start = request.start_sec.unwrap_or(0.0).max(0.0).min((full - 0.25).max(0.0));
        let end = request.end_sec.unwrap_or(full).min(full).max(start + 0.25);
        let count = ffmpeg::count_audio_streams(&source_path).await?;
        let volumes: Vec<f64> = (0..count)
            .map(|i| {
                request
                    .volumes
                    .as_ref()
                    .and_then(|v| v.get(i).copied().flatten())
                    .filter(|v| v.is_finite())
                    .map_or(1.0, |v| v.clamp(0.0, 2.0))
            })
            .collect();
        let mut plan = discord_plan::plan_discord_export(
            DiscordSource { duration_sec: end - start, width: info.width, height: info.height, fps: info.fps },
            DISCORD_LIMIT_BYTES,
        )
        .map_err(|message| anyhow!(message))?;
        let mut bytes = 0;
        let mut last_percent = None;
        // Two-pass usually lands within a few percent; shrink and retry if it overshoots.
        for attempt in 0..3 {
            let _ = tokio::fs::remove_file(&tmp).await;
            let mut progress = |ratio: f64| {
                let percent = (ratio * 100.0).floor() as u32;
                if last_percent != Some(percent) {
                    last_percent = Some(percent);
                    emit_progress(Some(percent));
                }
            };
            ffmpeg::encode_for_size(
                &source_path,
                &tmp,
                SizedEncode { start_sec: start, end_sec: end, volumes: &volumes, plan, pass_log: &pass_log },
                if attempt == 0 { Some(&mut progress) } else { None },
            )
            .await?;
            bytes = tokio::fs::metadata(&tmp).await?.len();
            if bytes <= DISCORD_LIMIT_BYTES {
                break;
            }
            if attempt == 2 {
                bail!("Could not get this clip under 20 MB. Trim it and retry.");
            }
            let video_kbps = (plan.video_kbps as f64 * ((DISCORD_LIMIT_BYTES as f64 * 0.9) / bytes as f64)).floor() as i64;
            log::warn!("[rikette] discord export {bytes} bytes, retrying at {video_kbps}k");
            plan.video_kbps = video_kbps;
        }
        replace_file(&tmp, &dest).await?;
        log::info!("[rikette] discord export {} {bytes} bytes {plan:?}", dest.display());
        let thumb = match source.thumb_path.as_deref().map(PathBuf::from) {
            Some(path) if exists(&path).await => Some(path),
            _ => None,
        };
        let thumb_url = thumb.as_ref().map(|_| media_protocol::thumb_url(&source.id, now_ms()));
        let drag_image = match &thumb {
            Some(thumb) => {
                let out = std::env::temp_dir().join(format!("rikette-drag-{}.png", source.id));
                match ffmpeg::write_drag_image(thumb, &out).await {
                    Ok(()) => Some(out),
                    Err(error) => {
                        log::warn!("[rikette] discord drag image failed: {error:#}");
                        None
                    }
                }
            }
            None => None,
        };
        DISCORD_EXPORTS
            .lock()
            .unwrap()
            .get_or_insert_with(HashMap::new)
            .insert(source.id.clone(), DiscordExport { path: dest.clone(), drag_image });
        events::emit(
            "clips:discord-ready",
            DiscordReady { id: source.id.clone(), title: title.clone(), bytes, thumb_url },
        );
        Ok(DiscordExportDone { path: dest.to_string_lossy().into_owned(), bytes })
    }
    .await;

    if let Err(error) = &result {
        let _ = tokio::fs::remove_file(&tmp).await;
        log::warn!("[rikette] discord export failed: {error:#}");
        let message = format!("{error:#}");
        let description = (message == discord_plan::TOO_LONG).then_some(message);
        notices::toast("Discord export failed", description, ToastKind::Error);
    }
    for suffix in ["-0.log", "-0.log.mbtree", "-0.log.temp", "-0.log.mbtree.temp"] {
        let _ = tokio::fs::remove_file(format!("{}{suffix}", pass_log.display())).await;
    }
    emit_progress(None);
    result
}

/// Hands a finished export to the OS drag, so it can be dropped straight into Discord.
pub fn start_discord_drag(id: &str) {
    let exports = DISCORD_EXPORTS.lock().unwrap();
    let Some(done) = exports.as_ref().and_then(|m| m.get(id)) else { return };
    if !done.path.exists() {
        return;
    }
    let file = done.path.clone();
    let icon = match &done.drag_image {
        Some(path) if path.exists() => drag::Image::File(path.clone()),
        _ => drag::Image::Raw(include_bytes!("../icons/128x128.png").to_vec()),
    };
    let Some(win) = events::main_window() else { return };
    let target = win.clone();
    let _ = win.run_on_main_thread(move || {
        if let Err(error) = drag::start_drag(
            &target,
            drag::DragItem::Files(vec![file]),
            icon,
            |_, _| {},
            drag::Options::default(),
        ) {
            log::warn!("[rikette] discord drag failed: {error}");
        }
    });
}

pub fn reveal_discord_export(id: &str) -> Result<()> {
    let exports = DISCORD_EXPORTS.lock().unwrap();
    let done = exports
        .as_ref()
        .and_then(|m| m.get(id))
        .filter(|d| d.path.exists())
        .ok_or_else(|| anyhow!("Export not found"))?;
    reveal(&done.path)
}

/// One scan at a time: two overlapping scans would both import the same new files.
static SYNC: AsyncMutex<()> = AsyncMutex::const_new(());
static REFRESHING: AtomicBool = AtomicBool::new(false);

/// Import mp4s dropped into the output folder. True when any were added.
pub async fn sync_output_folder() -> bool {
    let _lock = SYNC.lock().await;
    let root = PathBuf::from(settings::load().output_folder);
    let Ok(mut entries) = tokio::fs::read_dir(&root).await else { return false };
    let mut added = import_folder_mp4s(&root, None).await;
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        if !is_dir || name.starts_with('.') || name == DISCORD_EXPORT_DIR {
            continue;
        }
        added += import_folder_mp4s(&entry.path(), Some(name)).await;
    }
    added > 0
}

/// Reconcile the library with the disk off the list path; the page reloads if anything changed.
async fn refresh_library() {
    if REFRESHING.swap(true, Ordering::SeqCst) {
        return;
    }
    let pruned = prune_missing().await;
    let added = sync_output_folder().await;
    if pruned || added {
        events::clips_changed();
    }
    REFRESHING.store(false, Ordering::SeqCst);
}

pub fn toast_quota(code: &str) {
    let (title, description) = if code == "STORAGE_CAP" {
        ("Storage cap reached", "Delete clips or manage billing in Account.")
    } else {
        ("Cloud clip limit reached", "Upgrade to Pro for 150 GB. Clip is saved locally.")
    };
    notices::toast(title, Some(description.into()), ToastKind::Warning);
    window::show_main(Some("/account"));
}

pub fn toast_link_copied() {
    notices::toast("Link copied", None, ToastKind::Success);
}

pub fn toast_error(message: &str) {
    log::error!("[rikette] {message}");
    notices::hud("Save failed");
    notices::toast(message, None, ToastKind::Error);
}

async fn prune_missing() -> bool {
    let mut pruned = false;
    for row in library::list_clips().unwrap_or_default() {
        if exists(Path::new(&row.path)).await || row.is_live_cloud() {
            continue;
        }
        if let Some(thumb) = row.thumb_path.as_deref() {
            let _ = tokio::fs::remove_file(thumb).await;
        }
        if library::delete_clip(&row.id).is_ok() {
            pruned = true;
        }
    }
    pruned
}

fn mtime_ms(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
}

async fn to_library_clip(row: LocalClip) -> LibraryClip {
    let meta = tokio::fs::metadata(&row.path).await.ok();
    let local_file = meta.is_some();
    let bust = meta.as_ref().and_then(mtime_ms).unwrap_or(row.created_at);
    let thumb_at = match row.thumb_path.as_deref() {
        Some(path) => tokio::fs::metadata(path).await.ok().as_ref().and_then(mtime_ms),
        None => None,
    };
    let thumb_url = match (thumb_at, row.cloud_id.as_deref()) {
        // Keyed on the thumbnail's own mtime: thumbs are cached, and a re-render rewrites the file.
        (Some(at), _) => Some(media_protocol::thumb_url(&row.id, at)),
        (None, Some(cloud_id)) if row.is_live_cloud() => {
            Some(format!("{}?v={}", cloud_file_url(cloud_id, "thumb.jpg"), row.created_at))
        }
        _ => None,
    };
    let api = settings::load().api_base().to_string();
    let video_url = match row.cloud_id.as_deref() {
        Some(cloud_id) if !local_file => cloud_file_url(cloud_id, "video.mp4"),
        _ => media_protocol::video_url(&row.id, bust),
    };
    LibraryClip {
        title: clip_title(&row),
        public_url: row.cloud_id.as_ref().map(|c| format!("{api}/c/{c}")),
        local_file,
        video_url,
        thumb_url,
        id: row.id,
        created_at: row.created_at,
        duration_ms: row.duration_ms,
        width: row.width,
        height: row.height,
        bytes: row.bytes,
        game_title: row.game_title,
        cloud_status: row.cloud_status,
        cloud_id: row.cloud_id,
        folder_id: row.folder_id,
    }
}

fn cloud_file_url(cloud_id: &str, file: &str) -> String {
    let api = settings::load().api_base().to_string();
    let cdn = if api.contains("localhost") || api.contains("127.0.0.1") {
        format!("{api}/cdn")
    } else {
        let re = regex_lite::Regex::new(r"^(https?://)(?:www\.)?").unwrap();
        re.replace(&api, "${1}cdn.").into_owned()
    };
    format!("{cdn}/clips/{cloud_id}/{file}")
}

fn clip_title(row: &LocalClip) -> String {
    match row.title.as_deref().map(str::trim) {
        Some(title) if !title.is_empty() => title.to_string(),
        _ => default_clip_title(row.created_at),
    }
}

async fn import_folder_mp4s(folder: &Path, game_title: Option<String>) -> usize {
    let Ok(mut entries) = tokio::fs::read_dir(folder).await else { return 0 };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.to_lowercase().ends_with(".mp4") && !name.starts_with('.') {
            names.push(name);
        }
    }
    let mut added = 0;
    for name in names {
        let path = folder.join(&name);
        let path_str = path.to_string_lossy().into_owned();
        if matches!(library::get_clip_by_path(&path_str), Ok(Some(_))) {
            continue;
        }
        match import_one(&path, &name, game_title.clone()).await {
            Ok(true) => added += 1,
            Ok(false) => {}
            Err(error) => log::warn!("[rikette] skip import {path_str}: {error:#}"),
        }
    }
    added
}

async fn import_one(path: &Path, name: &str, game_title: Option<String>) -> Result<bool> {
    let info = ffmpeg::probe_media(path).await?;
    if info.duration_ms < 200 {
        return Ok(false);
    }
    let meta = tokio::fs::metadata(path).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let thumb = paths::thumb_file(&id);
    let _ = tokio::fs::create_dir_all(paths::thumbs_dir()).await;
    let sidecar = path.with_extension("jpg");
    let thumb_result = if exists(&sidecar).await {
        tokio::fs::copy(&sidecar, &thumb).await.map(drop).map_err(anyhow::Error::from).map(|_| {
            let sidecar = sidecar.clone();
            tauri::async_runtime::spawn(async move {
                let _ = tokio::fs::remove_file(sidecar).await;
            });
        })
    } else {
        ffmpeg::write_thumbnail(path, &thumb).await
    };
    if let Err(error) = thumb_result {
        log::warn!("[rikette] thumbnail failed: {error:#}");
    }
    let created_at = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .or_else(|| mtime_ms(&meta))
        .unwrap_or_else(now_ms);
    library::insert_clip(&LocalClip {
        id,
        cloud_id: None,
        path: path.to_string_lossy().into_owned(),
        thumb_path: Some(thumb.to_string_lossy().into_owned()),
        created_at,
        duration_ms: info.duration_ms,
        width: info.width as i64,
        height: info.height as i64,
        bytes: meta.len() as i64,
        title: Some(title_from_import(name, created_at)),
        game_title,
        cloud_status: CloudStatus::Local,
        folder_id: None,
    })?;
    Ok(true)
}

fn title_from_import(name: &str, created_at: i64) -> String {
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let generated = regex_lite::Regex::new(r"^Rikette_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}").unwrap();
    if generated.is_match(stem) {
        return default_clip_title(created_at);
    }
    let title = stem.replace('_', " ").trim().to_string();
    if title.is_empty() { default_clip_title(created_at) } else { title }
}

fn game_name(target: &CaptureTarget) -> String {
    if target.kind == TargetKind::Display {
        return "Desktop".into();
    }
    pretty_game_name(&target.title, &target.exe)
}

fn pretty_game_name(title: &str, exe: &str) -> String {
    let api = regex_lite::Regex::new(r"(?i)\s+-\s+(Direct3D\s*\d+|DX\d+|Vulkan|OpenGL|Unreal Engine.*)$").unwrap();
    let bits = regex_lite::Regex::new(r"(?i)\s+\((?:64|32)[\s-]*bit\)$").unwrap();
    let mut name = title.trim().to_string();
    name = api.replace(&name, "").into_owned();
    name = bits.replace(&name, "").into_owned();
    if name.is_empty() {
        let ext = regex_lite::Regex::new(r"(?i)\.(exe|bin)$").unwrap();
        name = ext.replace(exe, "").into_owned();
    }
    let name = name.trim();
    if name.is_empty() { "Unknown".into() } else { name.to_string() }
}

/// A name Windows accepts as a file or folder name.
pub fn sanitize_name(raw: &str) -> String {
    let replaced: String = raw
        .chars()
        .map(|c| match c {
            ':' => '-',
            '<' | '>' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if (c as u32) < 0x20 => ' ',
            c => c,
        })
        .collect();
    let mut name = collapse_spaces(&replaced)
        .trim_end_matches(['.', ' '])
        .to_string();
    let reserved = regex_lite::Regex::new(r"(?i)^(con|prn|aux|nul|com[1-9]|lpt[1-9])$").unwrap();
    if reserved.is_match(&name) {
        name = format!("_{name}");
    }
    if name.chars().count() > 80 {
        name = name.chars().take(80).collect::<String>().trim_end_matches(['.', ' ']).to_string();
    }
    if name.is_empty() { "Unknown".into() } else { name }
}

fn collapse_spaces(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "Oct 13, 10:43 AM", like `Intl.DateTimeFormat('en-US')` in the Electron build.
fn default_clip_title(at_ms: i64) -> String {
    local(at_ms).format("%b %-d, %-I:%M %p").to_string()
}

fn clip_stem(at_ms: i64) -> String {
    local(at_ms).format("Rikette_%Y-%m-%d_%H-%M-%S").to_string()
}

fn local(at_ms: i64) -> chrono::DateTime<chrono::Local> {
    chrono::DateTime::from_timestamp_millis(at_ms)
        .unwrap_or_default()
        .with_timezone(&chrono::Local)
}

async fn unique_path(dir: &Path, stem: &str, ext: &str, ignore: Option<&Path>) -> PathBuf {
    let ignore = ignore.map(|p| p.to_string_lossy().to_lowercase());
    let mut candidate = dir.join(format!("{stem}{ext}"));
    let mut n = 2;
    while exists(&candidate).await && Some(candidate.to_string_lossy().to_lowercase()) != ignore {
        candidate = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    candidate
}

async fn rename_case_safe(from: &Path, to: &Path) -> Result<()> {
    if from == to {
        return Ok(());
    }
    if from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase() {
        let tmp = PathBuf::from(format!("{}.tmp-rename", from.display()));
        tokio::fs::rename(from, &tmp).await?;
        tokio::fs::rename(&tmp, to).await?;
        return Ok(());
    }
    tokio::fs::rename(from, to).await?;
    Ok(())
}

/// Thumbnails live in the data folder under the clip id; move older sidecar `.jpg`s there.
async fn settle_thumb(row: LocalClip) -> LocalClip {
    let dest = paths::thumb_file(&row.id);
    let dest_str = dest.to_string_lossy().into_owned();
    let video = PathBuf::from(&row.path);
    let sidecar = video.with_extension("jpg");
    if row.thumb_path.as_deref() == Some(dest_str.as_str()) && exists(&dest).await {
        unlink_if_sidecar(&sidecar, &dest).await;
        return row;
    }
    let _ = tokio::fs::create_dir_all(paths::thumbs_dir()).await;
    let sources = row.thumb_path.iter().map(PathBuf::from).chain(std::iter::once(sidecar.clone()));
    for source in sources {
        if source == dest || !exists(&source).await {
            continue;
        }
        match tokio::fs::copy(&source, &dest).await {
            Ok(_) => {
                if source.parent() == video.parent() {
                    let _ = tokio::fs::remove_file(&source).await;
                }
                break;
            }
            Err(error) => log::warn!("[rikette] move thumb failed: {error}"),
        }
    }
    if !exists(&dest).await && exists(&video).await {
        if let Err(error) = ffmpeg::write_thumbnail(&video, &dest).await {
            log::warn!("[rikette] thumbnail failed: {error:#}");
        }
    }
    unlink_if_sidecar(&sidecar, &dest).await;
    if row.thumb_path.as_deref() != Some(dest_str.as_str()) {
        return library::update_clip(&row.id, |c| c.thumb_path = Some(dest_str.clone())).unwrap_or(row);
    }
    row
}

async fn unlink_if_sidecar(sidecar: &Path, keep: &Path) {
    if sidecar != keep && exists(sidecar).await {
        let _ = tokio::fs::remove_file(sidecar).await;
    }
}

async fn exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Swap `tmp` into `dest`, retrying while a player or scanner still holds the file open.
async fn replace_file(tmp: &Path, dest: &Path) -> Result<()> {
    let bak = PathBuf::from(format!("{}.bak", dest.display()));
    let _ = tokio::fs::remove_file(&bak).await;
    let mut last = None;
    for attempt in 0..8u64 {
        let _ = tokio::fs::rename(dest, &bak).await;
        match tokio::fs::rename(tmp, dest).await {
            Ok(()) => {
                let _ = tokio::fs::remove_file(&bak).await;
                return Ok(());
            }
            Err(error) => {
                last = Some(error);
                let _ = tokio::fs::rename(&bak, dest).await;
                tokio::time::sleep(Duration::from_millis(50 * (attempt + 1))).await;
            }
        }
    }
    Err(last.map(anyhow::Error::from).unwrap_or_else(|| anyhow!("replace failed")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_safe_on_windows() {
        assert_eq!(sanitize_name("Halo: Infinite"), "Halo- Infinite");
        assert_eq!(sanitize_name("a<b>c?.."), "a b c");
        assert_eq!(sanitize_name("CON"), "_CON");
        assert_eq!(sanitize_name("   "), "Unknown");
        assert_eq!(sanitize_name(&"x".repeat(100)).len(), 80);
    }

    #[test]
    fn game_names_drop_renderer_suffixes() {
        assert_eq!(pretty_game_name("Valorant - Direct3D 11", "valorant.exe"), "Valorant");
        assert_eq!(pretty_game_name("Game (64-bit)", "game.exe"), "Game");
        assert_eq!(pretty_game_name("", "cs2.exe"), "cs2");
        assert_eq!(pretty_game_name("", ""), "Unknown");
    }

    #[test]
    fn imported_names_become_titles() {
        assert_eq!(title_from_import("My_great_clip.mp4", 0), "My great clip");
        assert_eq!(title_from_import("Rikette_2026-01-02_03-04-05.mp4", 0), default_clip_title(0));
    }

    #[test]
    fn trim_volumes_default_to_one_and_clamp() {
        assert_eq!(volumes_for(3, Some(&[Some(0.5), Some(9.0)])), vec![0.5, 2.0, 1.0]);
        assert_eq!(volumes_for(1, None), vec![1.0]);
    }
}
