use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use itertools::Itertools;
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route(
            "/v2/database",
            post(handle_new_database).get(handle_database_info),
        )
        .route("/v2/database/list", get(handle_list_database))
        .route("/v2/database/moveData", post(handle_move_data))
        .route("/v2/database/download", post(handle_database_download))
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

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseMoveDataRequest {
    database_id: Uuid,
    data_id: Vec<Uuid>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct DatabaseDownloadRequest {
    database_id: Uuid,
    is_raw: bool,
    data_id: Vec<Uuid>,
    file_type: String,
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
    let database_name;
    if req.is_raw {
        let record = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id, module_name from module_v2 where module_id = $1"#,
            req.database_id
        )
        .fetch_one(&ctx.db)
        .await?;
        workspace_id = record.workspace_id;
        database_name = record.module_name;
    } else {
        let record = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id, datastore_name from datastore_v2 where datastore_id = $1"#,
            req.database_id
        )
        .fetch_one(&ctx.db)
        .await?;
        workspace_id = record.workspace_id;
        database_name = record.datastore_name;
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
            "databaseName": database_name,
            "data": data,
            "tags": tags,
        }),
    }))
}

async fn handle_list_database(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<DatabaseListRequest>,
) -> Result<Json<CommonResponse>> {
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        req.workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let databases;
    if req.is_raw {
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
            req.workspace_id
        )
        .fetch_all(&ctx.db)
        .await?;

        let mut result = Vec::new();
        for r in records {
            let name = if r.module_id.is_none() {
                "Unknown".to_string()
            } else {
                r.module_name.clone()
            };
            let records = sqlx::query!(
                r#"select
                        distinct tags
                        from data_v2
                        where is_raw = true and module_id = $1"#,
                r.module_id
            )
            .fetch_all(&ctx.db)
            .await?;
            let data_count = sqlx::query!(
                r#"select
                    count(data_id)
                    from data_v2
                    where is_raw = true and module_id = $1"#,
                r.module_id
            )
            .fetch_one(&ctx.db)
            .await?
            .count
            .unwrap_or(0);
            let tags = records
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
            result.push(json!({
                "category": r.module_category,
                "dataCount": data_count,
                "databaseId": r.module_id,
                "databaseName": name,
                "isRaw": true,
                "tags": tags,
            }));
        }
        databases = result;
    } else {
        let records = sqlx::query!(
            r#"select
                datastore_id,
                datastore_name
            from datastore_v2 where workspace_id = $1"#,
            req.workspace_id
        )
        .fetch_all(&ctx.db)
        .await?;

        let mut result = Vec::new();
        for r in records {
            let records = sqlx::query!(
                r#"select
                        distinct tags
                        from data_v2
                        where is_raw = false and datastore_id = $1"#,
                r.datastore_id
            )
            .fetch_all(&ctx.db)
            .await?;
            let data_count = sqlx::query!(
                r#"select
                    count(data_id)
                    from data_v2
                    where is_raw = false and datastore_id = $1"#,
                r.datastore_id
            )
            .fetch_one(&ctx.db)
            .await?
            .count
            .unwrap_or(0);
            let tags = records
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
            result.push(json!({
                "dataCount": data_count,
                "databaseId": r.datastore_id,
                "databaseName": r.datastore_name,
                "isRaw": false,
                "tags": tags,
            }));
        }
        databases = result;
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "databases": databases,
        }),
    }))
}

async fn handle_move_data(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<DatabaseBody<DatabaseMoveDataRequest>>,
) -> Result<Json<CommonResponse>> {
    let all_records = sqlx::query!(
        r#"select
            d.data_id,
            d.datastore_id,
            d.module_id,
            w.workspace_id,
            w.user_level
        from data_v2 d
        left join module_v2 m on d.module_id = m.module_id
        left join (
            select
                wmv2.workspace_id,
                wmv2.user_level
            from workspace_member_v2 wmv2
            where user_id = $1
        ) w on w.workspace_id = m.workspace_id
        where data_id = any($2)"#,
        auth_user.user_id,
        &req.database.data_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let mut count = 0;
    for record in all_records {
        if record.workspace_id.is_some() && record.user_level.is_some() {
            if record.user_level.unwrap() == 0 && record.datastore_id.is_none() {
                sqlx::query!(
                    r#"update data_v2 set datastore_id = $1, is_raw = false where data_id = $2"#,
                    req.database.database_id,
                    record.data_id
                )
                .execute(&ctx.db)
                .await?;
                count += 1;
            }
        }
    }

    if count == 0 {
        return Err(Error::Forbidden);
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_database_download(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<DatabaseBody<DatabaseDownloadRequest>>,
) -> Result<Response<Body>> {
    let workspace_id;
    if req.database.is_raw {
        let record = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id from module_v2 where module_id = $1"#,
            req.database.database_id
        )
        .fetch_one(&ctx.db)
        .await?;
        workspace_id = record.workspace_id;
    } else {
        let record = sqlx::query!(
            // language=PostgreSQL
            r#"select workspace_id from datastore_v2 where datastore_id = $1"#,
            req.database.database_id
        )
        .fetch_one(&ctx.db)
        .await?;
        workspace_id = record.workspace_id;
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
    if req.database.is_raw {
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
                from data_v2 where module_id = $1 and is_raw = true and data_id = any($2)"#,
            req.database.database_id,
            &req.database.data_id
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
                from data_v2 where datastore_id = $1 and is_raw = false and data_id = any($2)"#,
            req.database.database_id,
            &req.database.data_id
        )
        .fetch_all(&ctx.db)
        .await?;
    }

    if req.database.file_type == "csv" {
        let mut wtr = csv::Writer::from_writer(vec![]);
        #[derive(serde::Serialize)]
        struct Data {
            content: String,
            reference: String,
        }
        for r in data {
            let extra_data = r.extra_data.unwrap_or(json!({}));
            let d = Data {
                content: r.data_content,
                reference: serde_json::to_string(&extra_data).unwrap_or_default(),
            };
            wtr.serialize(d).unwrap();
        }
        let csv = wtr.into_inner().unwrap();
        let csv = String::from_utf8(csv).unwrap();
        let response = Response::builder()
            .header(CONTENT_DISPOSITION, "attachment; filename=\"data.csv\"")
            .header(CONTENT_TYPE, "text/csv; charset=utf-8")
            .body(Body::from(csv))
            .unwrap();
        return Ok(response);
    } else if req.database.file_type == "txt" {
        let mut txt = String::new();
        for r in data {
            txt.push_str(&r.data_content);
            txt.push('\n');
            txt.push('\n');
        }
        let response = Response::builder()
            .header(CONTENT_DISPOSITION, "attachment; filename=\"data.txt\"")
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from(txt))
            .unwrap();
        return Ok(response);
    } else {
        return Err(Error::unprocessable_entity([(
            "fileType",
            "fileType is not supported",
        )]));
    }
}
