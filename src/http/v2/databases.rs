use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use itertools::Itertools;
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route(
            "/database",
            post(handle_new_database).get(handle_database_info),
        )
        .route("/database/list", get(handle_list_database))
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseBody<T> {
    database: T,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseFromSql {
    database_id: Uuid,
    database_name: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DataFromSql {
    data_id: Uuid,
    datastore_id: Option<Uuid>,
    module_id: Option<Uuid>,
    data_module_type: Option<String>,
    tags: Option<String>,
    data_content: String,
    extra_data: Option<serde_json::Value>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseNewRequest {
    workspace_id: Uuid,
    database_name: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseInfoRequest {
    database_id: Uuid,
    is_raw: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseListRequest {
    workspace_id: Uuid,
    is_raw: bool,
}

async fn handle_new_database(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<DatabaseBody<DatabaseNewRequest>>,
) -> Result<Json<CommonResponse>> {
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        req.database.workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    if _member_record.user_level > 0 {
        return Err(Error::Forbidden);
    }

    let database = sqlx::query!(
        // language=PostgreSQL
        r#"insert into datastore_v2 (workspace_id, datastore_name) values ($1, $2)
        returning datastore_id, datastore_name"#,
        req.database.workspace_id,
        req.database.database_name,
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "databaseId": database.datastore_id,
            "databaseName": database.datastore_name,
        }),
    }))
}

async fn handle_database_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatabaseInfoRequest>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let workspace_id;
    if req.is_raw {
        workspace_id = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id from module_v2 where module_id = $1"#,
            req.database_id
        )
        .fetch_one(&ctx.db)
        .await?
        .workspace_id;
    } else {
        workspace_id = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id from datastore_v2 where datastore_id = $1"#,
            req.database_id
        )
        .fetch_one(&ctx.db)
        .await?
        .workspace_id;
    }
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let data;
    if req.is_raw {
        data = sqlx::query_as!(
            DataFromSql,
            // language=PostgreSQL
            r#"select
                data_id,
                datastore_id,
                module_id,
                data_module_type,
                tags,
                data_content,
                extra_data,
                created_at "created_at: Timestamptz",
                updated_at "updated_at: Timestamptz"
            from data_v2 where module_id = $1 and is_raw = true"#,
            req.database_id
        )
        .fetch_all(&ctx.db)
        .await?;
    } else {
        data = sqlx::query_as!(
            DataFromSql,
            // language=PostgreSQL
            r#"select
                data_id,
                datastore_id,
                module_id,
                data_module_type,
                tags,
                data_content,
                extra_data,
                created_at "created_at: Timestamptz",
                updated_at "updated_at: Timestamptz"
            from data_v2 where datastore_id = $1 and is_raw = false"#,
            req.database_id
        )
        .fetch_all(&ctx.db)
        .await?;
    }

    let tags = data
        .iter()
        .map(|r| {
            r.tags
                .clone()
                .unwrap_or_default()
                .split(',')
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        })
        .flatten()
        .collect::<Vec<String>>();
    let tags = tags.into_iter().unique().collect::<Vec<String>>();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "data": data,
            "tags": tags,
        }),
    }))
}

async fn handle_list_database(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<DatabaseBody<DatabaseListRequest>>,
) -> Result<Json<CommonResponse>> {
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        req.database.workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let databases;
    if req.database.is_raw {
        let records = sqlx::query!(
            r#"select
                distinct data_v2.module_id,
                m.module_name,
                m.module_category
            from data_v2
            left join (
                select
                    module_id, module_name, module_category
                from module_v2 where workspace_id = $1
            ) as m on m.module_id = data_v2.module_id
            where is_raw = true"#,
            req.database.workspace_id
        )
        .fetch_all(&ctx.db)
        .await?;
        databases = records
            .iter()
            .map(|r| {
                let name = if r.module_id.is_none() {
                    "Unknown".to_string()
                } else {
                    r.module_name.clone()
                };
                json!({
                    "category": r.module_category,
                    "databaseId": r.module_id,
                    "databaseName": name,
                    "isRaw": true,
                })
            })
            .collect::<Vec<serde_json::Value>>();
    } else {
        let records = sqlx::query!(
            r#"select
                datastore_id,
                datastore_name
            from datastore_v2 where workspace_id = $1"#,
            req.database.workspace_id
        )
        .fetch_all(&ctx.db)
        .await?;
        databases = records
            .iter()
            .map(|r| {
                json!({
                    "databaseId": r.datastore_id,
                    "databaseName": r.datastore_name,
                    "isRaw": false,
                })
            })
            .collect::<Vec<serde_json::Value>>();
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "databases": databases,
        }),
    }))
}
