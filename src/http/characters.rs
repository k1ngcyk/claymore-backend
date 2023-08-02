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
        .route(
            "/character",
            post(handle_new_character).get(handle_get_character_info),
        )
        .route("/character/list", get(handle_get_character_list))
        .route("/character/settings", put(handle_modify_character_settings))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CharacterBody<T> {
    character: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewCharacterRequest {
    character_name: String,
    project_id: Uuid,
    settings: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterInfoRequest {
    character_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterListRequest {
    project_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterModifyRequest {
    character_id: Uuid,
    settings: Option<serde_json::Value>,
    character_name: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CharacterFromSql {
    character_id: Uuid,
    character_name: String,
    project_id: Uuid,
    settings: serde_json::Value,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_character(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<CharacterBody<NewCharacterRequest>>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = $1"#,
        req.character.project_id
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

    if member_record.user_level > 1 {
        return Err(Error::Unauthorized);
    }

    let character = sqlx::query!(
        // language=PostgreSQL
        r#"insert into character (character_name, project_id, settings) values ($1, $2, $3) returning character_id"#,
        req.character.character_name,
        req.character.project_id,
        req.character.settings
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "characterId": character.character_id,
        }),
    }))
}

async fn handle_get_character_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<CharacterInfoRequest>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from character where character_id = $1)"#,
        req.character_id
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

    let character = sqlx::query_as!(
        CharacterFromSql,
        // language=PostgreSQL
        r#"select
            character_id,
            character_name,
            settings,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from character where character_id = $1"#,
        req.character_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "character": character,
        }),
    }))
}

async fn handle_get_character_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<CharacterListRequest>,
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

    let characters = sqlx::query_as!(
        CharacterFromSql,
        // language=PostgreSQL
        r#"select
            character_id,
            character_name,
            settings,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from character where project_id = $1"#,
        req.project_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "characters": characters }),
    }))
}

async fn handle_modify_character_settings(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<CharacterBody<CharacterModifyRequest>>,
) -> Result<Json<CommonResponse>> {
    let team_id = sqlx::query!(
        r#"select team_id from project where project_id = (select project_id from character where character_id = $1)"#,
        req.character.character_id
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

    if req.character.character_name.is_none() && req.character.settings.is_none() {
        return Err(Error::unprocessable_entity([(
            "key",
            "either character_name or settings must be provided",
        )]));
    }

    if req.character.character_name.is_some() {
        sqlx::query!(
            // language=PostgreSQL
            r#"update character set character_name = $1 where character_id = $2"#,
            req.character.character_name,
            req.character.character_id
        )
        .execute(&ctx.db)
        .await?;
    }

    if req.character.settings.is_some() {
        sqlx::query!(
            // language=PostgreSQL
            r#"update character set settings = $1 where character_id = $2"#,
            req.character.settings,
            req.character.character_id
        )
        .execute(&ctx.db)
        .await?;
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "characterId": req.character.character_id,
        }),
    }))
}
