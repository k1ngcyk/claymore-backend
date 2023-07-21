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
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropFromSql {
    datadrop_id: Uuid,
    datadrop_name: String,
    datadrop_content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    job_id: Option<Uuid>,
    project_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    extra_data: Option<serde_json::Value>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_get_datadrop_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatadropInfoRequest>,
) -> Result<Json<CommonResponse>> {
    let project_id = sqlx::query!(
        r#"select project_id from datadrop where datadrop_id = $1"#,
        req.datadrop_id
    )
    .fetch_one(&ctx.db)
    .await?
    .project_id;

    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        project_id
    )
    .fetch_one(&ctx.db)
    .await?
    .team_id;

    let _member_record = sqlx::query!(
        r#"select * from team_member where team_id = $1 and user_id = $2"#,
        team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    let datadrop = sqlx::query_as!(
        DatadropFromSql,
        r#"select
            datadrop_id,
            datadrop_name,
            datadrop_content,
            job_id,
            project_id,
            extra_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from datadrop where datadrop_id = $1"#,
        req.datadrop_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "datadrop": datadrop,
        }),
    }))
}

async fn handle_get_datadrop_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatadropListRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        req.project_id
    )
    .fetch_one(&ctx.db)
    .await?
    .team_id;

    let _member_record = sqlx::query!(
        r#"select * from team_member where team_id = $1 and user_id = $2"#,
        team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    let datadrop_list = sqlx::query_as!(
        DatadropFromSql,
        r#"select
            datadrop_id,
            datadrop_name,
            datadrop_content,
            job_id,
            project_id,
            extra_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from datadrop where project_id = $1"#,
        req.project_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "datadropList": datadrop_list,
        }),
    }))
}
