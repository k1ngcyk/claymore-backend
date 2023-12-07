use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::CommonResponse;
use crate::http::Error;
use crate::http::Result;
use axum::extract::{DefaultBodyLimit, Multipart, State};
use axum::routing::post;
use axum::{Json, Router};
use md5;
use nanoid::nanoid;
use serde_json::json;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use uuid::Uuid;

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileFromSql {
    file_id: Uuid,
    file_name: String,
    file_path: String,
    file_type: String,
    md5: String,
    extra_data: Option<serde_json::Value>,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/v2/file/upload", post(handle_file_upload))
        .layer(DefaultBodyLimit::max(1024 * 1024 * 1024))
}

async fn handle_file_upload(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    mut multipart: Multipart,
) -> Result<Json<CommonResponse>> {
    let mut module_id: Option<Uuid> = None;
    while let Some(field) = multipart.next_field().await.unwrap() {
        let filename = field.name().unwrap().to_string();
        if filename == "moduleId" {
            let id = field.text().await.unwrap();
            if let Ok(uuid) = Uuid::parse_str(&id) {
                module_id = Some(uuid);
            }
            continue;
        }
        log::info!("filename: {}", filename);
        let file_path_store = nanoid!(16);
        if !Path::new(&ctx.config.upload_dir).exists() {
            fs::create_dir(&ctx.config.upload_dir).await?;
        }
        let file_path = Path::new(&ctx.config.upload_dir).join(&file_path_store);
        let field_data = field.bytes().await.unwrap();
        let file_md5 = format!("{:x}", md5::compute(&field_data));

        let file_from_query = sqlx::query_as!(
            FileFromSql,
            r#"select
                file_id,
                file_name,
                file_path,
                file_type,
                md5,
                extra_data,
                created_at "created_at: Timestamptz",
                updated_at "updated_at: Timestamptz"
            from files where md5 = $1"#,
            file_md5
        )
        .fetch_optional(&ctx.db)
        .await?;
        if let Some(file) = file_from_query {
            if let Some(module_id) = module_id {
                sqlx::query!(
                    r#"insert into file_module (module_id, file_id) values ($1, $2)"#,
                    module_id,
                    file.file_id
                )
                .execute(&ctx.db)
                .await?;
                return Ok(Json(CommonResponse {
                    code: 200,
                    message: "success".to_string(),
                    data: json!({
                        "file": {
                            "fileId": file.file_id,
                            "fileName": file.file_name,
                        }
                    }),
                }));
            } else {
                return Err(Error::unprocessable_entity([(
                    "moduleId",
                    "moduleId is required as first field",
                )]));
            }
        }
        let file_from_query = sqlx::query_as!(
            FileFromSql,
            r#"insert into files (file_name, file_path, file_type, md5) values ($1, $2, $3, $4) returning
                file_id,
                file_name,
                file_path,
                file_type,
                md5,
                extra_data,
                created_at "created_at: Timestamptz",
                updated_at "updated_at: Timestamptz"
            "#,
            filename,
            &file_path_store,
            "",
            file_md5,
        )
        .fetch_one(&ctx.db)
        .await?;
        let mut file = fs::File::create(&file_path).await.unwrap();
        file.write_all(&field_data).await.unwrap();
        let file_id = file_from_query.file_id;
        if let Some(module_id) = module_id {
            sqlx::query!(
                r#"insert into file_module (module_id, file_id) values ($1, $2)"#,
                module_id,
                file_id
            )
            .execute(&ctx.db)
            .await?;

            return Ok(Json(CommonResponse {
                code: 200,
                message: "success".to_string(),
                data: json!({
                    "file": {
                        "fileId": file_id,
                        "fileName": filename,
                    }
                }),
            }));
        } else {
            return Err(Error::unprocessable_entity([(
                "moduleId",
                "moduleId is required as first field",
            )]));
        }
    }

    if let None = module_id {
        Err(Error::unprocessable_entity([(
            "moduleId",
            "moduleId is required as first field",
        )]))
    } else {
        Ok(Json(CommonResponse {
            code: 200,
            message: "success".to_string(),
            data: json!({}),
        }))
    }
}
