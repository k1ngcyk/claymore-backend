use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use crate::queue;
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/job", post(handle_new_job).get(handle_get_job_info))
        .route("/job/list", get(handle_get_job_list))
        .route("/job/operate", post(handle_operate_job))
        .route("/job/candidate", get(handle_get_job_candidate))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct JobBody<T> {
    job: T,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq, Debug)]
#[repr(i32)]
pub enum JobStatus {
    Pending,
    Running,
    Finished,
    Paused,
}

#[derive(serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq, Debug)]
#[repr(i32)]
enum JobOperation {
    Start,
    Pause,
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
    job_operation: JobOperation,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobCandidateRequest {
    job_id: Uuid,
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
    job_status: JobStatus,
    prompt_chain: Option<serde_json::Value>,
    temperature: Option<f64>,
    word_count: Option<i32>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
    finished_count: Option<i64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobFullFromSql {
    job_id: Uuid,
    job_name: String,
    project_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    generator_id: Option<Uuid>,
    target_count: i32,
    job_status: JobStatus,
    prompt_chain: Option<serde_json::Value>,
    temperature: Option<f64>,
    word_count: Option<i32>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
    model_name: Option<String>,
    generator_name: String,
    finished_count: Option<i64>,
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

    if req.job.generator_id.is_none() {
        return Err(Error::unprocessable_entity([(
            "generatorId",
            "generatorId is required",
        )]));
    }

    if req.job.job_name == "" {
        return Err(Error::unprocessable_entity([(
            "jobName",
            "jobName is required",
        )]));
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
            job_status "job_status: JobStatus",
            prompt_chain,
            temperature,
            word_count,
            (select count(*) from datadrop where job_id = job.job_id and datadrop_content is not null) as finished_count,
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

    let channel = queue::make_channel(&ctx.config.rabbitmq_url).await;
    let _result = queue::publish_message(
        &channel,
        json!({
            "job_id": job.job_id,
            "generator_id": job.generator_id,
        }),
    )
    .await;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "jobId": job.job_id,
        }),
    }))
}

async fn handle_get_job_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<JobListRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        req.project_id
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

    let jobs = sqlx::query_as!(
        JobFromSql,
        // language=PostgreSQL
        r#"select
            job_id,
            job_name,
            project_id,
            generator_id,
            target_count,
            job_status "job_status: JobStatus",
            prompt_chain,
            temperature,
            word_count,
            (select count(*) from datadrop where job_id = job.job_id and datadrop_content is not null) as finished_count,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from job where project_id = $1"#,
        req.project_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "jobs": jobs,
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

    let current_status = sqlx::query!(
        // language=PostgreSQL
        r#"select job_status "job_status: JobStatus"
        from job where job_id = $1"#,
        req.job.job_id
    )
    .fetch_one(&ctx.db)
    .await?
    .job_status;

    if req.job.job_operation == JobOperation::Start {
        if current_status != JobStatus::Paused {
            return Err(Error::unprocessable_entity([
                ("jobOperation", "invalid job operation"),
                ("jobStatus", "This job is running or finished"),
            ]));
        } else {
            sqlx::query!(
                // language=PostgreSQL
                r#"update job set job_status = $1 where job_id = $2"#,
                JobStatus::Pending as i32,
                req.job.job_id
            )
            .execute(&ctx.db)
            .await?;

            let generator_id = sqlx::query!(
                // language=PostgreSQL
                r#"select generator_id from job where job_id = $1"#,
                req.job.job_id
            )
            .fetch_one(&ctx.db)
            .await?
            .generator_id;

            queue::publish_message(
                &queue::make_channel(&ctx.config.rabbitmq_url).await,
                json!({
                    "job_id": req.job.job_id,
                    "generator_id": generator_id,
                }),
            )
            .await;
        }
    } else if req.job.job_operation == JobOperation::Pause {
        if current_status == JobStatus::Finished || current_status == JobStatus::Paused {
            return Err(Error::unprocessable_entity([
                ("jobOperation", "invalid job operation"),
                ("jobStatus", "This job is finished or paused"),
            ]));
        } else {
            sqlx::query!(
                // language=PostgreSQL
                r#"update job set job_status = $1 where job_id = $2"#,
                JobStatus::Paused as i32,
                req.job.job_id
            )
            .execute(&ctx.db)
            .await?;
        }
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_get_job_candidate(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<JobCandidateRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from job where job_id = $1)"#,
        req.job_id
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

    let candidates = sqlx::query_as!(
        DatadropFromSql,
        // language=PostgreSQL
        r#"select
            datadrop_id,
            datadrop_name,
            datadrop_content,
            job_id,
            project_id,
            extra_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from datadrop where job_id = $1"#,
        req.job_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "candidates": candidates,
        }),
    }))
}

async fn handle_get_job_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<JobInfoRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from job where job_id = $1)"#,
        req.job_id
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

    let job = sqlx::query_as!(
        JobFullFromSql,
        // language=PostgreSQL
        r#"select
            job_id,
            job_name,
            job.project_id,
            job.generator_id,
            target_count,
            job_status "job_status: JobStatus",
            generator.model_name,
            generator.prompt_chain,
            generator.temperature,
            generator.word_count,
            generator.generator_name,
            (select count(*) from datadrop where job_id = $1 and datadrop_content is not null) as finished_count,
            job.created_at "created_at: Timestamptz",
            job.updated_at "updated_at: Timestamptz"
        from job
        left join generator on job.generator_id = generator.generator_id
        where job_id = $1"#,
        req.job_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "job": job,
        }),
    }))
}
