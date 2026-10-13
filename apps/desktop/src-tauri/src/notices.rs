//! Confirm dialogs and toasts drawn by the renderer (`AppAlerts.tsx`), and the HUD.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::oneshot;

use crate::{events, overlay, window};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfirm {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancel_label: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub destructive: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppToast {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: ToastKind,
}

static PENDING: Mutex<Option<HashMap<String, oneshot::Sender<bool>>>> = Mutex::new(None);

/// How long a confirm waits for an answer before counting as cancelled.
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);

pub struct Confirm<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub confirm_label: &'a str,
    pub cancel_label: &'a str,
    pub destructive: bool,
}

/// Shows the window and asks; false when cancelled, timed out or there is no window.
pub async fn ask_confirm(ask: Confirm<'_>) -> bool {
    window::show_main(None);
    window::wait_until_loaded(Duration::from_secs(5)).await;
    let id = uuid::Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();
    PENDING
        .lock()
        .unwrap()
        .get_or_insert_with(HashMap::new)
        .insert(id.clone(), tx);
    events::emit(
        "app:confirm",
        AppConfirm {
            id: id.clone(),
            title: ask.title.into(),
            description: ask.description.into(),
            confirm_label: Some(ask.confirm_label.into()),
            cancel_label: Some(ask.cancel_label.into()),
            destructive: ask.destructive,
        },
    );
    let answer = tokio::time::timeout(CONFIRM_TIMEOUT, rx).await;
    if let Some(map) = PENDING.lock().unwrap().as_mut() {
        map.remove(&id);
    }
    matches!(answer, Ok(Ok(true)))
}

#[derive(Deserialize)]
pub struct ConfirmReply {
    pub id: String,
    pub ok: bool,
}

pub fn reply_confirm(reply: ConfirmReply) {
    let tx = PENDING.lock().unwrap().as_mut().and_then(|map| map.remove(&reply.id));
    if let Some(tx) = tx {
        let _ = tx.send(reply.ok);
    }
}

pub fn toast(title: impl Into<String>, description: Option<String>, kind: ToastKind) {
    events::emit(
        "app:toast",
        AppToast {
            title: title.into(),
            description,
            kind,
        },
    );
}

pub fn hud(title: &str) {
    overlay::show(title);
}
