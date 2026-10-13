//! `rikette://` links from the website: sign-in tokens and the return from Stripe.

use crate::{auth, brand, upload, window};

pub fn url_from_args<I: IntoIterator<Item = String>>(args: I) -> Option<String> {
    args.into_iter().find(|arg| brand::is_app_protocol_url(arg))
}

pub fn handle(url: &str) {
    if !brand::is_app_protocol_url(url) {
        return;
    }
    let Ok(parsed) = tauri::Url::parse(url) else {
        log::warn!("[rikette] bad protocol url {url}");
        return;
    };
    let host = parsed
        .host_str()
        .map(str::to_string)
        .unwrap_or_else(|| parsed.path().trim_start_matches('/').split('/').next().unwrap_or("").to_string());
    if host == "billing" {
        tauri::async_runtime::spawn(async {
            if let Err(error) = auth::fetch_me().await {
                log::warn!("[rikette] billing refresh failed: {error:#}");
            }
            window::show_main(Some("/account"));
            upload::drain().await;
        });
        return;
    }
    let Some(token) = parsed.query_pairs().find(|(k, _)| k == "token").map(|(_, v)| v.into_owned()) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        match auth::accept_session_token(&token).await {
            Ok(_) => {
                window::show_main(None);
                upload::drain().await;
            }
            Err(error) => log::error!("[rikette] protocol login failed: {error:#}"),
        }
    });
}
