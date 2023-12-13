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
    Router::new().route("/workspace/list", get(handle_list_workspace))
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WorkspaceBody<T> {
    workspace: T,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WorkspaceFromSql {
    workspace_id: Uuid,
    workspace_name: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_list_workspace(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let workspaces = sqlx::query_as!(
        WorkspaceFromSql,
        // language=PostgreSQL
        r#"select
            workspace_id,
            workspace_name,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from workspace_v2
        where owner_id = $1
        order by created_at desc"#,
        auth_user.user_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "workspaces": workspaces,
        }),
    }))
}
