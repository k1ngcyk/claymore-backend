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
        .route("/job", post(handle_new_job))
        // .route("/job/list", get(handle_get_job_list))
        .route("/jop/operate", post(handle_operate_job))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct JobBody<T> {
    job: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewJobRequest {
    job_name: String,
    project_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    generator_id: Option<Uuid>,
    target_count: i32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobInfoRequest {
    job_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobListRequest {
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobOperateRequest {
    job_id: Uuid,
    job_status: i32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobFromSql {
    job_id: Uuid,
    job_name: String,
    project_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    generator_id: Option<Uuid>,
    target_count: i32,
    job_status: i32,
    prompt_chain: Option<serde_json::Value>,
    temperature: Option<f64>,
    word_count: Option<i32>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_job(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<JobBody<NewJobRequest>>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        req.job.project_id
    )
    .fetch_one(&ctx.db)
    .await?
    .team_id;

    let member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from team_member where team_id = $1 and user_id = $2"#,
        team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    if member_record.user_level != 0 {
        return Err(Error::Unauthorized);
    }

    let job = sqlx::query_as!(
        JobFromSql,
        // language=PostgreSQL
        r#"insert into job (project_id, job_name, generator_id, target_count) values ($1, $2, $3, $4) returning
            job_id,
            job_name,
            project_id,
            generator_id,
            target_count,
            job_status,
            prompt_chain,
            temperature,
            word_count,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        req.job.project_id,
        req.job.job_name,
        req.job.generator_id,
        req.job.target_count
    )
    .fetch_one(&ctx.db)
    .await?;

    // TODO: MQ

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "jobId": job.job_id,
        }),
    }))
}

async fn handle_operate_job(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<JobBody<JobOperateRequest>>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from job where job_id = $1)"#,
        req.job.job_id
    )
    .fetch_one(&ctx.db)
    .await?
    .team_id;

    let member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from team_member where team_id = $1 and user_id = $2"#,
        team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Unauthorized)?;

    if member_record.user_level != 0 {
        return Err(Error::Unauthorized);
    }

    sqlx::query!(
        // language=PostgreSQL
        r#"update job set job_status = $1 where job_id = $2"#,
        req.job.job_status,
        req.job.job_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}
