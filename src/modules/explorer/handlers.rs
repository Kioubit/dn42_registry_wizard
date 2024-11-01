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

    let mut m  = serde_json::map::Map::new();
    m.insert("commit".to_string(), serde_json::to_value(&u.commit_hash).unwrap_or_default());
    m.insert("roa".to_string(), serde_json::to_value(!u.roa_disabled).unwrap_or_default());
    m.insert("time".to_string(), serde_json::to_value(&u.etag).unwrap_or_default());
    let info_val = serde_json::to_value(&m).unwrap_or_default();
    let jr = serde_json::to_value(&u.index);
    if jr.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response();
    }
    let mut outer_map = serde_json::map::Map::with_capacity(2);
    outer_map.insert("i".to_string(), info_val);
    outer_map.insert("d".to_string(), jr.unwrap());
    let r = serde_json::to_string(&outer_map);
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

pub(super) async fn roa_handler_v4(State(u): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("text/plain"));
    let u = u.read().unwrap();
    if let Some(ref roa) = u.roa4  {
        (headers, roa.clone()).into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "").into_response()
    }
}

pub(super) async fn roa_handler_v6(State(u): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("text/plain"));
    let u = u.read().unwrap();
    if let Some(ref roa) = u.roa6  {
        (headers, roa.clone()).into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "").into_response()
    }
}

pub(super) async fn roa_handler_json(State(u): State<Arc<RwLock<AppState>>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Type", HeaderValue::from_static("application/json"));
    let u = u.read().unwrap();
    if let Some(ref roa) = u.roa_json  {
        (headers, roa.clone()).into_response()
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, "").into_response()
    }
}