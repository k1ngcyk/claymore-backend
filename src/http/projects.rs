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
    Router::new().route("/project", post(handle_new_project))
    // .route("/project/list", get(handle_get_project_list))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ProjectBody<T> {
    project: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewProjectRequest {
    project_name: String,
    team_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectInfoRequest {
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectListRequest {
    team_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectFromSql {
    project_id: Uuid,
    team_id: Uuid,
    project_name: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_project(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ProjectBody<NewProjectRequest>>,
) -> Result<Json<CommonResponse>> {
    let member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from team_member where team_id = $1 and user_id = $2"#,
        req.project.team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    if member_record.user_level != 0 {
        return Err(Error::Unauthorized);
    }

    let project = sqlx::query_as!(
        ProjectFromSql,
        // language=PostgreSQL
        r#"insert into project (project_name, team_id) values ($1, $2) returning
            project_id,
            project_name,
            team_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        req.project.project_name,
        req.project.team_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 0,
        message: "success".to_string(),
        data: json!({
            "project_id": project.project_id,
            "created_at": project.created_at,
            "updated_at": project.updated_at,
        }),
    }))
}
