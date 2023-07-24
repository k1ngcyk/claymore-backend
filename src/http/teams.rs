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
        .route("/team", get(handle_get_team_info).post(handle_new_team))
        .route("/team/invite", post(handle_team_invite))
        .route("/team/list", get(handle_get_team_list))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TeamBody<T> {
    team: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewTeamRequest {
    team_name: String,
    team_level: i32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamInfoRequest {
    team_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamInviteRequest {
    team_id: Uuid,
    user_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamFromSql {
    team_id: Uuid,
    team_name: String,
    owner_id: Uuid,
    team_level: i32,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_team(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TeamBody<NewTeamRequest>>,
) -> Result<Json<CommonResponse>> {
    let team = sqlx::query!(
        r#"insert into team (team_name, owner_id, team_level) values ($1, $2, $3) returning team_id"#,
        req.team.team_name,
        auth_user.user_id,
        req.team.team_level
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "teamId": team.team_id,
        }),
    }))
}

async fn handle_get_team_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<TeamInfoRequest>,
) -> Result<Json<CommonResponse>> {
    let team = sqlx::query_as!(
        TeamFromSql,
        // language=PostgreSQL
        r#"select
            team_id,
            team_name,
            owner_id,
            team_level,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from team where team_id = $1"#,
        req.team_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if team.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from team_member where team_id = $1 and user_id = $2"#,
        req.team_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?;

    let user_level = member_record.map(|r| r.user_level).unwrap_or(-1);

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "teamId": team.team_id,
            "teamName": team.team_name,
            "ownerId": team.owner_id,
            "teamLevel": team.team_level,
            "userLevel": user_level,
        }),
    }))
}

async fn handle_team_invite(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TeamBody<TeamInviteRequest>>,
) -> Result<Json<CommonResponse>> {
    let team = sqlx::query!(
        // language=PostgreSQL
        r#"select owner_id from team where team_id = $1"#,
        req.team.team_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if team.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    sqlx::query!(
        // language=PostgreSQL
        r#"insert into team_member (team_id, user_id, user_level) values ($1, $2, 0)"#,
        req.team.team_id,
        req.team.user_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_get_team_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let teams = sqlx::query_as!(
        TeamFromSql,
        // language=PostgreSQL
        r#"select
            team_id,
            team_name,
            owner_id,
            team_level,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from team where owner_id = $1"#,
        auth_user.user_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "teams": teams,
        }),
    }))
}
