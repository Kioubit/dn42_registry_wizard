use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/modules/explorer/static/"]
struct StaticFiles;

pub(super) struct StaticFile<T>(pub T);
impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.0.into();

        match StaticFiles::get(path.as_str()) {
            Some(content) => {
                let body = Body::from(content.data);
                let mime = mime_guess::from_path(path).first_or_octet_stream();
                Response::builder()
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .header(header::CACHE_CONTROL, "max-age=3600, public, stale-if-error=86400")
                    .body(body)
                    .unwrap()
            }
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not found"))
                .unwrap(),
        }
    }
}