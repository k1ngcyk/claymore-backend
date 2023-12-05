use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new().route("/comment", post(handle_new_comment))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CommentBody<T> {
    comment: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewCommentRequest {
    datadrop_id: Uuid,
    comment_content: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentInfoRequest {
    comment_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentListRequest {
    datadrop_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentFromSql {
    comment_id: Uuid,
    user_id: Uuid,
    datadrop_id: Uuid,
    comment_content: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
    user_name: String,
}

async fn handle_new_comment(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<CommentBody<NewCommentRequest>>,
) -> Result<Json<CommonResponse>> {
    let project_id = sqlx::query!(
        r#"select project_id from datadrop where datadrop_id = $1"#,
        req.comment.datadrop_id
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
    .ok_or_else(|| Error::Forbidden)?;

    if req.comment.comment_content == "" || req.comment.comment_content.len() > 500 {
        return Err(Error::unprocessable_entity([(
            "commentContent",
            "comment content should be non-empty and less than 500 characters",
        )]));
    }

    let comment = sqlx::query!(
        // language=PostgreSQL
        r#"insert into comment (user_id, datadrop_id, comment_content) values ($1, $2, $3) returning comment_id"#,
        auth_user.user_id,
        req.comment.datadrop_id,
        req.comment.comment_content,
    )
    .fetch_one(&ctx.db)
    .await?;

    let comments = sqlx::query_as!(
        CommentFromSql,
        r#"select
            "user".user_name,
            comment_id,
            comment.user_id,
            datadrop_id,
            comment_content,
            comment.created_at "created_at: Timestamptz",
            comment.updated_at "updated_at: Timestamptz"
        from comment
        left join "user" on comment.user_id = "user".user_id
        where datadrop_id = $1"#,
        req.comment.datadrop_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "commentId": comment.comment_id,
            "comment": comments,
        }),
    }))
}
