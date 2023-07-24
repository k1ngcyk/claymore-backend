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
        .route(
            "/generator",
            post(handle_new_generator).get(handle_get_generator_info),
        )
        .route("/generator/list", get(handle_get_generator_list))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GeneratorBody<T> {
    generator: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGeneratorRequest {
    generator_name: String,
    prompt_chain: serde_json::Value,
    model_name: String,
    temperature: f64,
    word_count: i32,
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorInfoRequest {
    generator_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorListRequest {
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorFromSql {
    generator_id: Uuid,
    project_id: Uuid,
    generator_name: String,
    prompt_chain: serde_json::Value,
    model_name: String,
    temperature: f64,
    word_count: i32,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<NewGeneratorRequest>>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        req.generator.project_id
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

    let available_models = vec!["gpt-4", "gpt-3.5-turbo-16k", "gpt-3.5-turbo"];

    if !available_models.contains(&req.generator.model_name.as_str()) {
        return Err(Error::unprocessable_entity([(
            "modelName",
            "unavailable model name",
        )]));
    }

    let generator = sqlx::query!(
        r#"insert into generator (generator_name, prompt_chain, model_name, temperature, word_count, project_id) values ($1, $2, $3, $4, $5, $6) returning generator_id"#,
        req.generator.generator_name,
        req.generator.prompt_chain,
        req.generator.model_name,
        req.generator.temperature,
        req.generator.word_count,
        req.generator.project_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "generatorId": generator.generator_id,
        }),
    }))
}

async fn handle_get_generator_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<GeneratorInfoRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from generator where generator_id = $1)"#,
        req.generator_id
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

    let generator = sqlx::query!(
        r#"select
            generator_id,
            generator_name,
            prompt_chain,
            model_name,
            temperature,
            word_count,
            project_id,
            created_at,
            updated_at
        from generator where generator_id = $1"#,
        req.generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "generator_id": generator.generator_id,
            "generator_name": generator.generator_name,
            "prompt_chain": generator.prompt_chain,
            "model_name": generator.model_name,
            "temperature": generator.temperature,
            "word_count": generator.word_count,
            "project_id": generator.project_id,
        }),
    }))
}

async fn handle_get_generator_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<GeneratorListRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        // language=PostgreSQL
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

    let generators = sqlx::query_as!(
        GeneratorFromSql,
        // language=PostgreSQL
        r#"select
            generator_id,
            generator_name,
            prompt_chain,
            model_name,
            temperature,
            word_count,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from generator where project_id = $1"#,
        req.project_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "generators": generators }),
    }))
}
