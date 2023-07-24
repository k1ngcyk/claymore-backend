use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/datadrop", get(handle_get_datadrop_info))
        .route("/datadrop/list", get(handle_get_datadrop_list))
        .route("/datadrop/content", put(handle_modify_datadrop))
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
    page: Option<i64>,
    page_size: Option<i64>,
    order_by: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropModifyRequest {
    datadrop_id: Uuid,
    datadrop_content: String,
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

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropFullFromSql {
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
    job_name: String,
    generator_id: Uuid,
    generator_name: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommentFromSql {
    comment_id: Uuid,
    user_id: Uuid,
    user_name: String,
    datadrop_id: Uuid,
    comment_content: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
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

    let comment = sqlx::query_as!(
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
        req.datadrop_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let feedback = sqlx::query_as!(
        FeedbackFromSql,
        r#"select
            feedback_id,
            user_id,
            datadrop_id,
            feedback_content,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from feedback where datadrop_id = $1"#,
        req.datadrop_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "datadrop": datadrop,
            "comment": comment,
            "feedback": feedback,
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

    let page_size = req.page_size.unwrap_or(10);
    let page = req.page.unwrap_or(1);
    let offset = (page - 1) * page_size;
    let order_by = req.order_by.unwrap_or("created_at".to_string());
    let available_order_by = vec!["created_at", "updated_at", "job_id"];
    if !available_order_by.contains(&order_by.as_str()) {
        return Err(Error::unprocessable_entity([(
            "order_by".to_string(),
            "invalid order_by".to_string(),
        )]));
    }

    let datadrop_list = sqlx::query_as!(
        DatadropFullFromSql,
        r#"select
            datadrop_id,
            datadrop_name,
            datadrop_content,
            datadrop.job_id,
            datadrop.project_id,
            extra_data,
            job.job_name,
            generator.generator_id,
            generator_name,
            datadrop.created_at "created_at: Timestamptz",
            datadrop.updated_at "updated_at: Timestamptz"
        from datadrop
        left join job on datadrop.job_id = job.job_id
        left join generator on job.generator_id = generator.generator_id
        where datadrop.project_id = $1 order by $2 desc limit $3 offset $4"#,
        req.project_id,
        order_by,
        page_size,
        offset
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

async fn handle_modify_datadrop(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<DatadropBody<DatadropModifyRequest>>,
) -> Result<Json<CommonResponse>> {
    let project_id = sqlx::query!(
        r#"select project_id from datadrop where datadrop_id = $1"#,
        req.datadrop.datadrop_id
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
        r#"update datadrop set datadrop_content = $1 where datadrop_id = $2 returning
        datadrop_id,
        datadrop_name,
        datadrop_content,
        job_id,
        project_id,
        extra_data,
        created_at "created_at: Timestamptz",
        updated_at "updated_at: Timestamptz"
        "#,
        req.datadrop.datadrop_content,
        req.datadrop.datadrop_id
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
