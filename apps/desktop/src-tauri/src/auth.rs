//! The signed-in session: a bearer token kept encrypted on disk (DPAPI on Windows), and the
//! cached `/api/me`.

use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use anyhow::{bail, Context, Result};
use reqwest::{Method, RequestBuilder, StatusCode};
use serde_json::Value;

use crate::{events, paths, settings};

static TOKEN: Mutex<Option<String>> = Mutex::new(None);
static ME: Mutex<Option<Value>> = Mutex::new(None);

fn session_file() -> PathBuf {
    paths::data_dir().join("session.v2.bin")
}

/// Where the Electron build kept its token, encrypted with Chromium's safeStorage.
fn electron_session_file() -> PathBuf {
    paths::data_dir().join("session.bin")
}

pub fn http() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(format!("Rikette/{}", crate::version()))
            .build()
            .expect("http client")
    })
}

pub fn access_token() -> Option<String> {
    TOKEN.lock().unwrap().clone()
}

pub fn cached_me() -> Option<Value> {
    ME.lock().unwrap().clone()
}

pub fn load_session() {
    let token = fs::read(session_file())
        .ok()
        .and_then(|bytes| secret::open(&bytes))
        .or_else(migrate_electron_session);
    *TOKEN.lock().unwrap() = token.filter(|t| !t.is_empty());
}

fn migrate_electron_session() -> Option<String> {
    let bytes = fs::read(electron_session_file()).ok()?;
    let token = secret::open_electron(&bytes)?;
    persist_session(&token);
    let _ = fs::remove_file(electron_session_file());
    log::info!("[rikette] carried the sign-in over from the previous build");
    Some(token)
}

fn persist_session(token: &str) {
    *TOKEN.lock().unwrap() = Some(token.to_string());
    let result = fs::create_dir_all(paths::data_dir()).and_then(|_| fs::write(session_file(), secret::seal(token)));
    if let Err(error) = result {
        log::warn!("[rikette] could not save the session: {error}");
    }
}

pub fn clear_session() {
    *TOKEN.lock().unwrap() = None;
    *ME.lock().unwrap() = None;
    let _ = fs::remove_file(session_file());
    let _ = fs::remove_file(electron_session_file());
    notify_auth_changed();
}

pub async fn accept_session_token(token: &str) -> Result<Value> {
    persist_session(token.trim());
    match fetch_me().await? {
        Some(me) => Ok(me),
        None => {
            clear_session();
            bail!("Session was rejected")
        }
    }
}

pub async fn fetch_me() -> Result<Option<Value>> {
    if access_token().is_none() {
        *ME.lock().unwrap() = None;
        notify_auth_changed();
        return Ok(None);
    }
    let res = api(Method::GET, "/api/me").send().await?;
    if res.status() == StatusCode::UNAUTHORIZED {
        clear_session();
        return Ok(None);
    }
    if !res.status().is_success() {
        bail!("GET /api/me {}", res.status().as_u16());
    }
    let me: Value = res.json().await.context("parse /api/me")?;
    *ME.lock().unwrap() = Some(me.clone());
    notify_auth_changed();
    Ok(Some(me))
}

pub async fn dev_login(email: &str) -> Result<Value> {
    let res = api(Method::POST, "/api/dev/login")
        .json(&serde_json::json!({ "email": email }))
        .send()
        .await?;
    if res.status() == StatusCode::NOT_FOUND {
        bail!("Dev login is disabled on this API");
    }
    if !res.status().is_success() {
        bail!("Dev login failed ({})", res.status().as_u16());
    }
    let body: Value = res.json().await?;
    let token = body["token"].as_str().context("no token in dev login")?;
    accept_session_token(token).await
}

/// A request to the API, with the session token when there is one.
pub fn api(method: Method, path: &str) -> RequestBuilder {
    let url = format!("{}{path}", settings::load().api_base());
    let request = http().request(method, url);
    match access_token() {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

pub fn notify_auth_changed() {
    events::emit("auth:changed", cached_me());
}

#[cfg(windows)]
mod secret {
    use aes_gcm::aead::Aead;
    use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
    use base64::Engine;
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};

    fn blob(bytes: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        }
    }

    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        bytes
    }

    fn protect(bytes: &[u8]) -> Option<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptProtectData(&blob(bytes), None, None, None, None, 0, &mut out).ok()?;
            Some(take(out))
        }
    }

    fn unprotect(bytes: &[u8]) -> Option<Vec<u8>> {
        let mut out = CRYPT_INTEGER_BLOB::default();
        unsafe {
            CryptUnprotectData(&blob(bytes), None, None, None, None, 0, &mut out).ok()?;
            Some(take(out))
        }
    }

    pub fn seal(token: &str) -> Vec<u8> {
        protect(token.as_bytes()).unwrap_or_else(|| token.as_bytes().to_vec())
    }

    pub fn open(bytes: &[u8]) -> Option<String> {
        String::from_utf8(unprotect(bytes)?).ok()
    }

    /// Chromium's os_crypt format: "v10" + 12-byte nonce + AES-256-GCM, keyed by the
    /// DPAPI-protected key in `Local State`.
    pub fn open_electron(bytes: &[u8]) -> Option<String> {
        let Some(sealed) = bytes.strip_prefix(b"v10") else {
            return String::from_utf8(bytes.to_vec()).ok();
        };
        let state = std::fs::read_to_string(crate::paths::data_dir().join("Local State")).ok()?;
        let state: serde_json::Value = serde_json::from_str(&state).ok()?;
        let wrapped = base64::engine::general_purpose::STANDARD
            .decode(state["os_crypt"]["encrypted_key"].as_str()?)
            .ok()?;
        let key = unprotect(wrapped.strip_prefix(b"DPAPI")?)?;
        let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
        if sealed.len() < 12 {
            return None;
        }
        let (nonce, data) = sealed.split_at(12);
        let plain = cipher.decrypt(Nonce::from_slice(nonce), data).ok()?;
        String::from_utf8(plain).ok()
    }
}

#[cfg(not(windows))]
mod secret {
    pub fn seal(token: &str) -> Vec<u8> {
        token.as_bytes().to_vec()
    }

    pub fn open(bytes: &[u8]) -> Option<String> {
        String::from_utf8(bytes.to_vec()).ok()
    }

    /// The macOS build was never shipped; its Keychain-wrapped tokens are not carried over.
    pub fn open_electron(bytes: &[u8]) -> Option<String> {
        if bytes.starts_with(b"v10") {
            return None;
        }
        String::from_utf8(bytes.to_vec()).ok()
    }
}
