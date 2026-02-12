use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};

const IDIOMORPH_JS: &[u8] = include_bytes!("static_files/idiomorph.min.js");
const PICO_CSS: &[u8] = include_bytes!("static_files/pico.min.css");
const SSR_JS: &[u8] = include_bytes!("static_files/ssr.js");

pub async fn get(Path(name): Path<String>) -> Response {
    let Some((name, ext)) = name.split_once(".") else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let ct = match ext {
        "css" => (header::CONTENT_TYPE, "text/css"),
        "js" => (header::CONTENT_TYPE, "text/javascript"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    let name = match name.split_once("@") {
        Some((b, _)) => b,
        None => name,
    };

    match name {
        "idiomorph" => ([ct], IDIOMORPH_JS).into_response(),
        "pico" => ([ct], PICO_CSS).into_response(),
        "ssr" => ([ct], SSR_JS).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
