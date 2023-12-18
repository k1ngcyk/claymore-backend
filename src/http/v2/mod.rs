use axum::routing::get;
use axum::{Json, Router};

use crate::http::ApiContext;
use crate::http::Result;
use axum::extract::State;
use serde_json::json;

use crate::http::CommonResponse;

mod chats;
mod databases;
mod evaluators;
mod files;
mod generators;
mod invoices;
mod modules;
mod templates;
mod workspaces;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/v2/ping", get(handle_ping))
        .merge(generators::router())
        .merge(templates::router())
        .merge(files::router())
        .merge(chats::router())
        .merge(evaluators::router())
        .merge(modules::router())
        .merge(workspaces::router())
        .merge(databases::router())
}

async fn handle_ping(ctx: State<ApiContext>) -> Result<Json<CommonResponse>> {
    Ok(Json(CommonResponse {
        code: 200,
        message: "pong".to_string(),
        data: json!({}),
    }))
}
