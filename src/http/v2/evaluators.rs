use crate::http::extractor::AuthUser;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

#[derive(serde::Serialize, serde::Deserialize)]
struct EvaluatorBody<T> {
    evaluator: T,
}

pub(crate) fn router() -> Router<ApiContext> {
    Router::new().route("/v2/evaluator/save", post(handle_save_evaluator))
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvaluatorSaveRequest {
    evaluator_id: Uuid,
    data: serde_json::Value,
}

async fn handle_save_evaluator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<EvaluatorBody<EvaluatorSaveRequest>>,
) -> Result<Json<CommonResponse>> {
    let evaluator_id = req.evaluator.evaluator_id;
    let evaluator = sqlx::query!(
        r#"select
            project_id
        from evaluator_v2 where evaluator_id = $1"#,
        evaluator_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let team_id = sqlx::query!(
        // language=PostgreSQL
        r#"select team_id from project where project_id = $1"#,
        evaluator.project_id
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

    let evaluator_config = req.evaluator.data;

    let evaluator = sqlx::query!(
        // language=PostgreSQL
        r#"update evaluator_v2 set config_data = $1 where evaluator_id = $2
        returning
            evaluator_id,
            template_id,
            config_data
        "#,
        evaluator_config,
        evaluator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "evaluator": {
                "evaluatorId": evaluator.evaluator_id,
                "templateId": evaluator.template_id,
                "configData": evaluator.config_data,
            }
        }),
    }))
}
