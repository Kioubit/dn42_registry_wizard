use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "src/modules/explorer/static/"]
struct StaticFiles;

pub(super) struct StaticFile<T> {
    pub path: T,
    pub if_none_match : Option<HeaderValue>,
}
impl<T> IntoResponse for StaticFile<T>
where
    T: Into<String>,
{
    fn into_response(self) -> Response {
        let path = self.path.into();

        match StaticFiles::get(path.as_str()) {
            Some(content) => {
                let body = Body::from(content.data);
                let hash = content.metadata.sha256_hash()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();
                let etag = format!(r#""{}""#, hash);

                if self.if_none_match.as_ref().and_then(|h| h.to_str().ok()) == Some(&etag) {
                    return StatusCode::NOT_MODIFIED.into_response();
                }

                let mime = mime_guess::from_path(path).first_or_octet_stream();
                Response::builder()
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .header(header::ETAG, etag)
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