use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::IntoResponse;
use crate::modules::explorer::{static_files, AppState};

pub(super) async fn root_handler(uri: Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_owned();
    if path.is_empty() {
        path = String::from("index.html");
    }
    static_files::StaticFile(path)
}

pub(super) async fn index_handler(request_headers: HeaderMap, State(u): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let client_etag = request_headers.get("if-none-match")
        .and_then(|v| v.to_str().ok()).unwrap_or_default();
    let u = u.read().unwrap();
    let etag = u.etag.clone();
    if etag == client_etag {
        return (StatusCode::NOT_MODIFIED, "Not Modified").into_response();
    }
    let r = serde_json::to_string(&u.index);
    drop(u);

    if r.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response();
    }
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Cache-Control", HeaderValue::from_static("max-age=3600, public, must-revalidate"));
    headers.insert("ETag", HeaderValue::from_str(&etag).unwrap());


    if let Ok(r) = r {
        (headers, r).into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
    }
}

pub(super) async fn get_object(request_headers: HeaderMap, Query(params): Query<HashMap<String, String>>, State(u): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let client_etag = request_headers.get("if-none-match")
        .and_then(|v| v.to_str().ok()).unwrap_or_default();

    let object_name = params.get("name");
    let object_type = params.get("type");
    if object_name.is_none() || object_type.is_none() {
        return (StatusCode::BAD_REQUEST, "bad request").into_response();
    }
    let object_name = object_name.unwrap();
    let object_type = object_type.unwrap();

    let u = u.read().unwrap();
    let etag = u.etag.clone();
    if etag == client_etag {
        return (StatusCode::NOT_MODIFIED, "Not Modified").into_response();
    }

    let category_map = u.objects.get(object_type);
    if category_map.is_none() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "category not found").into_response();
    }
    let target = category_map.unwrap().iter().find(|x| x.object.filename == *object_name);
    if target.is_none() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "object not found").into_response();
    }
    let js = serde_json::to_string(target.unwrap());
    if js.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    headers.insert("Cache-Control", HeaderValue::from_static("max-age=1800, public, must-revalidate"));
    headers.insert("ETag", HeaderValue::from_str(&etag).unwrap());

    (headers, js.unwrap()).into_response()
}