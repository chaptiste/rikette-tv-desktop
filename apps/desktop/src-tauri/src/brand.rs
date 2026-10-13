//! Names and URLs shared with `packages/shared/src/brand.ts`.

pub const APP_NAME: &str = "Rikette";
pub const APP_ORIGIN: &str = "https://rikette.tv";
pub const CLIP_FOLDER_NAME: &str = "Rikette";

/// Worker started by `pnpm --filter web dev`; debug builds talk to it.
pub const DEV_ORIGIN: &str = "http://127.0.0.1:8787";

pub fn is_app_protocol_url(url: &str) -> bool {
    url.starts_with("rikette://") || url.starts_with("echoframe://")
}
