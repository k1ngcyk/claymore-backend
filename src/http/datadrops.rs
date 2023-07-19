use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/datadrop", get(handle_get_datadrop_info))
        .route("/datadrop/list", get(handle_get_datadrop_list))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DatadropBody<T> {
    datadrop: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewDatadropRequest {
    datadrop_id: Uuid,
    datadrop_content: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropInfoRequest {
    datadrop_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropListRequest {
    datadrop_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropFromSql {
    datadrop_id: Uuid,
    user_id: Uuid,
    datadrop_content: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_get_datadrop_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatadropInfoRequest>,
) -> Result<Json<CommonResponse>> {
    // TODO
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_get_datadrop_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatadropListRequest>,
) -> Result<Json<CommonResponse>> {
    // TODO
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}
