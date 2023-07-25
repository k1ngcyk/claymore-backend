use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::Result;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new().route("/profile", get(handle_get_profile))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CommentBody<T> {
    comment: T,
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
}

async fn handle_get_profile(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let user_id = auth_user.user_id;
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ProjectInfo {
        project_id: Uuid,
        project_name: String,
    }
    #[derive(serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct TeamInfo {
        team_id: Uuid,
        team_name: String,
        projects: Vec<ProjectInfo>,
    }
    let all_teams = sqlx::query!(
        r#"
        select
            team.team_id,
            team_name
        from team_member
        left join team on team_member.team_id = team.team_id
        where user_id = $1
        "#,
        user_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let mut result = Vec::new();
    for team in all_teams {
        let projects = sqlx::query_as!(
            ProjectInfo,
            r#"select project_id, project_name from project where team_id = $1"#,
            team.team_id
        )
        .fetch_all(&ctx.db)
        .await?;
        result.push(TeamInfo {
            team_id: team.team_id,
            team_name: team.team_name,
            projects,
        });
    }
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "teams": result }),
    }))
}
