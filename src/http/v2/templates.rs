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
    Router::new().route("/v2/template/list", get(handle_get_template_list))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TemplateBody<T> {
    template: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TemplateFromSql {
    template_id: Uuid,
    template_name: String,
    template_icon: String,
    template_description: String,
    template_data: serde_json::Value,
    template_category: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_get_template_list(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let templates = sqlx::query_as!(
        TemplateFromSql,
        // language=PostgreSQL
        r#"select
            template_id,
            template_name,
            template_icon,
            template_description,
            template_data,
            template_category,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from template_v2
        order by created_at desc"#,
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "templates": templates }),
    }))
}
