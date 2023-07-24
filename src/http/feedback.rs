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
    Router::new().route("/feedback", post(handle_new_feedback))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FeedbackBody<T> {
    feedback: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewFeedbackRequest {
    datadrop_id: Uuid,
    feedback_content: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackInfoRequest {
    feedback_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackListRequest {
    datadrop_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeedbackFromSql {
    feedback_id: Uuid,
    user_id: Uuid,
    datadrop_id: Uuid,
    feedback_content: serde_json::Value,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_feedback(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<FeedbackBody<NewFeedbackRequest>>,
) -> Result<Json<CommonResponse>> {
    let project_id = sqlx::query!(
        r#"select project_id from datadrop where datadrop_id = $1"#,
        req.feedback.datadrop_id
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
        // language=PostgreSQL
        r#"select user_level from team_member where team_id = $1 and user_id = $2"#,
        team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    let feedback = sqlx::query!(
        // language=PostgreSQL
        r#"insert into feedback (user_id, datadrop_id, feedback_content) values ($1, $2, $3)
            on conflict (user_id, datadrop_id) do update set feedback_content = $3 returning feedback_id
        "#,
        auth_user.user_id,
        req.feedback.datadrop_id,
        req.feedback.feedback_content,
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "feedbackId": feedback.feedback_id,
        }),
    }))
}
