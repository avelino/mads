use axum::{
    body::Body,
    extract::Path,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "assets/"]
pub struct Assets;

pub async fn file(Path(path): Path<String>) -> Response {
    if path.is_empty() || path.contains("..") || path.starts_with('/') {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(content) = Assets::get(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mut response = Body::from(content.data.into_owned()).into_response();
    let headers = response.headers_mut();
    if let Ok(value) = header::HeaderValue::from_str(mime.as_ref()) {
        headers.insert(header::CONTENT_TYPE, value);
    }
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stylesheet_is_embedded() {
        assert!(Assets::get("app.css").is_some());
    }

    #[test]
    fn the_script_is_embedded() {
        assert!(Assets::get("app.js").is_some());
    }
}
