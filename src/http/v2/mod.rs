use axum::extract::{Extension, Path};
use axum::routing::get;
use axum::{Json, Router};

use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result, ResultExt};
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use axum::extract::{Query, State};
use log::info;
use regex::Regex;
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

mod generators;
mod templates;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/v2/ping", get(handle_ping))
        .merge(generators::router())
        .merge(templates::router())
}

async fn handle_ping(ctx: State<ApiContext>) -> Result<Json<CommonResponse>> {
    Ok(Json(CommonResponse {
        code: 200,
        message: "pong".to_string(),
        data: json!({}),
    }))
}
