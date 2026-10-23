//! The `picturesque://` protocol: serves cached photo images to the web view.
//!
//! The UI builds URLs with `convertFileSrc("{size}/{photo_id}[/{view}]", "picturesque")`,
//! which percent-encodes the path (and uses `http://picturesque.localhost/`
//! on Windows).

use picturesque_app::api::{Api, ApiErrorCode};
use tauri::http::{Response, StatusCode, header};

/// Decodes `%XX` escapes; invalid escapes are kept as they are.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Builds the response for a request to `uri_path` (for example `/grid%2F12`).
pub fn image_response(api: &Api, uri_path: &str, if_none_match: Option<&str>) -> Response<Vec<u8>> {
    let path = percent_decode(uri_path.trim_start_matches('/'));
    let found = api.image_request(&path).and_then(|file| {
        std::fs::read(&file)
            .map(|bytes| (file, bytes))
            .map_err(|e| picturesque_app::api::ApiError::internal(e.to_string()))
    });
    match found {
        Ok((file, bytes)) => {
            let etag = format!(
                "\"{}\"",
                file.file_stem()
                    .map(|s| s.to_string_lossy())
                    .unwrap_or_default()
            );
            let builder = Response::builder()
                .header(header::ETAG, &etag)
                .header(header::CACHE_CONTROL, "private, no-cache");
            if if_none_match == Some(etag.as_str()) {
                builder.status(StatusCode::NOT_MODIFIED).body(Vec::new())
            } else {
                builder
                    .header(header::CONTENT_TYPE, "image/jpeg")
                    .body(bytes)
            }
        }
        Err(err) => {
            let status = match err.code {
                ApiErrorCode::NotFound => StatusCode::NOT_FOUND,
                ApiErrorCode::InvalidArguments | ApiErrorCode::UnknownCommand => {
                    StatusCode::BAD_REQUEST
                }
                ApiErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Response::builder()
                .status(status)
                .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                .body(err.message.into_bytes())
        }
    }
    .expect("valid response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("grid%2F12"), "grid/12");
        assert_eq!(percent_decode("a%20b%zz%2"), "a b%zz%2");
        assert_eq!(percent_decode("caf%C3%A9"), "café");
    }
}
