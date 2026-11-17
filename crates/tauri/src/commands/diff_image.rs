use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use git::domain::OperationId;
use git::streaming::store::DiffImageSideKind;
use tauri::http::header;
use tauri::http::{Request, Response, StatusCode};
use tauri::{Manager, Runtime, UriSchemeContext, UriSchemeResponder};

use crate::state::AppState;

pub const SCHEME: &str = "gitau-image";

pub fn url(
    webview: &tauri::Webview,
    operation_id: u64,
    section_id: u32,
    side: DiffImageSideKind,
) -> tauri::Result<String> {
    let side = match side {
        DiffImageSideKind::Old => "old",
        DiffImageSideKind::New => "new",
    };
    webview.convert_file_src(
        format!("{}-{operation_id}-{section_id}-{side}", boot_nonce()),
        Some(SCHEME),
    )
}

/// Operation ids restart at 1 every launch, so responses include a per-process
/// nonce in the URL. That keeps the webview's disk cache from serving a
/// previous launch's image for a reused id.
fn boot_nonce() -> u64 {
    static NONCE: OnceLock<u64> = OnceLock::new();
    *NONCE.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        nanos ^ (u64::from(std::process::id())).wrapping_mul(0x9E37_79B9_7F4A_7C15)
    })
}

pub fn serve<R: Runtime>(
    ctx: UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let Some((operation_id, section_id, side)) =
        parse_token(request.uri().path().trim_start_matches('/'))
    else {
        responder.respond(not_found());
        return;
    };

    let backend = Arc::clone(&ctx.app_handle().state::<AppState>().backend);
    tauri::async_runtime::spawn(async move {
        let response = match backend
            .read_diff_image_side(operation_id, section_id, side)
            .await
        {
            Ok(Some((mime, bytes))) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CONTENT_LENGTH, bytes.len().to_string())
                .header(
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                )
                .body(bytes)
                .expect("valid image response"),
            Ok(None) => not_found(),
            Err(error) => {
                log::warn!("serving diff image failed: {error}");
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Vec::new())
                    .expect("valid error response")
            }
        };
        responder.respond(response);
    });
}

fn parse_token(token: &str) -> Option<(OperationId, u32, DiffImageSideKind)> {
    let mut parts = token.split('-');
    if parts.next()?.parse::<u64>().ok()? != boot_nonce() {
        return None;
    }
    let operation_id = parts.next()?.parse().ok()?;
    let section_id = parts.next()?.parse().ok()?;
    let side = match parts.next()? {
        "old" => DiffImageSideKind::Old,
        "new" => DiffImageSideKind::New,
        _ => return None,
    };
    if parts.next().is_some() {
        return None;
    }
    Some((OperationId(operation_id), section_id, side))
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Vec::new())
        .expect("valid not found response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_side_tokens() {
        let nonce = boot_nonce();
        assert_eq!(
            parse_token(&format!("{nonce}-12-3-old")),
            Some((OperationId(12), 3, DiffImageSideKind::Old))
        );
        assert_eq!(
            parse_token(&format!("{nonce}-12-3-new")),
            Some((OperationId(12), 3, DiffImageSideKind::New))
        );
        assert_eq!(parse_token("12-3-old"), None);
        assert_eq!(parse_token(&format!("{nonce}-12-3")), None);
        assert_eq!(parse_token(&format!("{nonce}-12-3-sideways")), None);
        assert_eq!(parse_token(&format!("{nonce}-12-3-old-extra")), None);
        assert_eq!(parse_token(&format!("{nonce}-x-3-old")), None);
    }
}
