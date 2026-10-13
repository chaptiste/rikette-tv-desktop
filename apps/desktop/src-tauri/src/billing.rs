//! Stripe checkout and billing portal, opened in the browser.

use anyhow::{bail, Result};
use reqwest::{Method, StatusCode};
use serde_json::Value;
use tauri_plugin_opener::OpenerExt;

use crate::{auth, events};

pub async fn start_checkout(interval: &str) -> Result<()> {
    if interval != "month" && interval != "year" {
        bail!("Invalid interval");
    }
    let res = auth::api(Method::POST, "/api/stripe/checkout")
        .json(&serde_json::json!({ "interval": interval }))
        .send()
        .await?;
    open_stripe_url(res, "Checkout").await
}

pub async fn open_portal() -> Result<()> {
    let res = auth::api(Method::POST, "/api/stripe/portal")
        .json(&serde_json::json!({}))
        .send()
        .await?;
    open_stripe_url(res, "Billing portal").await
}

async fn open_stripe_url(res: reqwest::Response, label: &str) -> Result<()> {
    let status = res.status();
    let body: Value = res.json().await.unwrap_or(Value::Null);
    match status {
        StatusCode::UNAUTHORIZED => bail!("Signed out"),
        StatusCode::CONFLICT => bail!("Subscribe first"),
        StatusCode::SERVICE_UNAVAILABLE => bail!("Billing is not configured on this API"),
        _ => {}
    }
    let Some(url) = body["url"].as_str().filter(|_| status.is_success()) else {
        let reason = body["message"]
            .as_str()
            .or(body["code"].as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("{label} failed ({})", status.as_u16()));
        bail!(reason);
    };
    events::app().opener().open_url(url, None::<&str>)?;
    Ok(())
}
