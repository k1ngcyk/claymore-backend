use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use crate::openai::ChatRequest;
use crate::{openai, queue};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use std::path::Path;
use tiktoken_rs::cl100k_base;
use uuid::Uuid;

use crate::http::CommonResponse;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct ModuleBody<T> {
    module: T,
}

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route(
            "/v2/module",
            post(handle_new_module).get(handle_module_info),
        )
        .route("/v2/module/list", get(handle_list_module))
        .route("/v2/module/try", post(handle_try_module))
        .route("/v2/module/save", post(handle_save_module))
        .route("/v2/module/reset", post(handle_reset_module))
        .route("/v2/module/run", post(handle_run_module))
        .route("/v2/module/clearFiles", post(handle_clear_files))
        .route("/v2/module/saveData", post(handle_save_data))
        .route("/v2/module/assignData", post(handle_assign_data))
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleFromSql {
    module_id: Uuid,
    module_name: String,
    template_id: Option<Uuid>,
    workspace_id: Uuid,
    module_category: String,
    config_data: serde_json::Value,
    created_at: Timestamptz,
    updated_at: Option<Timestamptz>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModuleWithStatusFromSql {
    module_id: Uuid,
    module_name: String,
    template_id: Option<Uuid>,
    workspace_id: Uuid,
    module_category: String,
    config_data: serde_json::Value,
    created_at: Timestamptz,
    updated_at: Option<Timestamptz>,
    status: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleNewRequest {
    module_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<Uuid>,
    workspace_id: Uuid,
    module_category: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleInfoRequest {
    module_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleListRequest {
    workspace_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleTryRequest {
    module_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleSaveRequest {
    module_id: Uuid,
    data: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleResetRequest {
    module_id: Uuid,
    template_id: Option<Uuid>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleRunRequest {
    module_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleClearFilesRequest {
    module_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleSaveDataRequest {
    module_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    tags: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ModuleAssignDataRequest {
    module_id: Uuid,
    database_id: Uuid,
    is_raw: bool,
    tags: Vec<String>,
}

async fn handle_new_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleNewRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_name = req.module.module_name;
    let template_id = req.module.template_id;
    let workspace_id = req.module.workspace_id;
    let module_category = req.module.module_category;
    let available_category = vec!["generator", "evaluator"];
    if !available_category.contains(&module_category.as_str()) {
        return Err(Error::unprocessable_entity([(
            "moduleCategory".to_string(),
            "invalid category".to_string(),
        )]));
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

    let module_config;
    if let Some(template_id) = req.module.template_id {
        let template = sqlx::query!(
            r#"select
                template_data,
                template_category
            from template_v2 where template_id = $1"#,
            template_id
        )
        .fetch_one(&ctx.db)
        .await?;
        if template.template_category != module_category {
            return Err(Error::unprocessable_entity([(
                "templateId".to_string(),
                "invalid template".to_string(),
            )]));
        }
        let template_data = template.template_data;
        let keys = template_data["keys"].as_array().unwrap();
        let prompt = template_data["prompt"].as_str().unwrap();
        let separator = template_data["separator"].as_str().unwrap_or_default();
        let key_configs = &template_data["keyConfigs"];
        let preprocess = &template_data["preprocess"];
        let mut map = serde_json::Map::new();
        for key in keys {
            let key = key.as_str().unwrap();
            let key_config = key_configs[key].as_object().unwrap();
            let key_display_name = key_config["displayName"].as_str().unwrap();
            let key_hint = key_config["hint"].as_str().unwrap();
            map.insert(
                key.to_string(),
                json!({
                    "displayName": key_display_name,
                    "hint": key_hint,
                    "value": "",
                }),
            );
        }
        module_config = json!({
            "prompt": prompt,
            "input": "",
            "keys": keys,
            "keyConfigs": serde_json::Value::Object(map),
            "separator": separator,
            "preprocess": preprocess,
            "assignData": {},
        });
    } else {
        module_config = json!({
            "prompt": "",
            "input": "",
            "keys": [],
            "keyConfigs": {},
            "separator": "",
            "preprocess": [],
            "assignData": {},
        });
    }

    let module = sqlx::query!(
        // language=PostgreSQL
        r#"insert into module_v2 (module_name, template_id, workspace_id, module_category, config_data)
        values ($1, $2, $3, $4, $5)
        returning module_id"#,
        module_name,
        template_id,
        workspace_id,
        module_category,
        module_config
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "module": {
                "moduleId": module.module_id,
            }
        }),
    }))
}

async fn handle_module_info(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<ModuleInfoRequest>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module_id;
    let module = sqlx::query_as!(
        ModuleFromSql,
        r#"select
            module_id,
            module_name,
            template_id,
            workspace_id,
            module_category,
            config_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;

    let workspace_id = module.workspace_id;
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let mut module_status = "Ready";

    let jobs = sqlx::query!(
        r#"select
            job_id,
            target_count
        from job_v2 where module_id = $1 and job_status = 0"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    if jobs.len() > 0 {
        let mut all_zero = true;
        for job in jobs {
            let counts = sqlx::query!(
                r#"select
                    count(*)
                from candidate_v2 where job_id = $1 group by job_status_group_id"#,
                job.job_id
            )
            .fetch_all(&ctx.db)
            .await?;
            all_zero = all_zero && counts.len() == 0;
            if counts.len() > 0 && counts.len() < job.target_count as usize {
                module_status = "Running";
                break;
            }
        }
        if module_status != "Running" {
            if all_zero {
                module_status = "Pending";
            } else {
                module_status = "Ready";
            }
        }
    }

    let files = sqlx::query!(
        r#"select
            files.file_id,
            finish_process,
            files.file_path,
            files.file_name
        from file_module
        left join files on files.file_id = file_module.file_id
        where module_id = $1"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let files = files
        .iter()
        .map(|f| {
            json!({
                "fileId": f.file_id,
                "fileName": f.file_name,
            })
        })
        .collect::<Vec<serde_json::Value>>();

    let candidates = sqlx::query!(
        r#"select
            content,
            extra_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from candidate_v2 where module_id = $1"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let candidates = candidates
        .iter()
        .map(|c| {
            json!({
                "content": c.content,
                "extraData": c.extra_data,
                "createdAt": c.created_at,
                "updatedAt": c.updated_at,
            })
        })
        .collect::<Vec<serde_json::Value>>();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "module": module,
            "files": files,
            "candidates": candidates,
            "status": module_status
        }),
    }))
}

async fn handle_try_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleTryRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let workspace_id = sqlx::query!(
        r#"select
            workspace_id
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?
    .workspace_id;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let mut module_config = sqlx::query!(
        r#"select
            config_data
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?
    .config_data;

    let module_config = module_config.as_object_mut().unwrap();

    let preprocess = module_config["preprocess"].as_array().unwrap().clone();
    for process in preprocess {
        let input_keys = process["inputKeys"].as_array().unwrap();
        let mut prompt = process["prompt"].as_str().unwrap().to_string();
        let model = process["model"].as_str().unwrap().to_string();
        let output_key = process["outputKey"].as_str().unwrap();
        for key in input_keys {
            let key = key.as_str().unwrap();
            let key_config = module_config["keyConfigs"][key].as_object().unwrap();
            prompt = prompt.replace(
                &format!("@key/{}", key),
                key_config["value"].as_str().unwrap(),
            );
        }
        let api_key = openai::get_available_key(&ctx.db).await?;
        let output = openai::chat(
            ChatRequest {
                model: model,
                input: prompt,
                max_tokens: Some(2048),
                temperature: Some(0.1),
                history: None,
            },
            &api_key.openai_key,
        )
        .await?;
        openai::release_key(&ctx.db, api_key).await?;
        let output_key_config = module_config["keyConfigs"][output_key]
            .as_object_mut()
            .unwrap();
        output_key_config["value"] = serde_json::Value::String(output);
    }

    let mut prompt = module_config["prompt"].as_str().unwrap().to_string();
    let input = req
        .module
        .input
        .unwrap_or_else(|| module_config["input"].as_str().unwrap().to_string());
    let keys = module_config["keys"].as_array().unwrap();
    let key_configs = &module_config["keyConfigs"];
    for key in keys {
        let key = key.as_str().unwrap();
        let key_config = key_configs[key].as_object().unwrap();
        prompt = prompt.replace(
            &format!("@key/{}", key),
            key_config["value"].as_str().unwrap(),
        );
    }
    prompt = prompt.replace("@key/input", &input);
    let bpe = cl100k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(&prompt);
    sqlx::query!(
        r#"insert into metric_v2 (workspace_id, user_id, module_id, token_count, word_count) values ($1, $2, $3, $4, $5)"#,
        workspace_id,
        auth_user.user_id,
        module_id,
        tokens.len() as i32,
        prompt.chars().count() as i32
    )
    .execute(&ctx.db)
    .await?;

    let api_key = openai::get_available_key(&ctx.db).await?;
    let output = openai::chat(
        ChatRequest {
            model: "gpt-3.5-turbo".to_string(),
            input: prompt,
            max_tokens: Some(2048),
            temperature: Some(0.1),
            history: None,
        },
        &api_key.openai_key,
    )
    .await?;
    openai::release_key(&ctx.db, api_key).await?;

    let tokens = bpe.encode_with_special_tokens(&output);
    sqlx::query!(
        r#"insert into metric_v2 (workspace_id, user_id, module_id, token_count, word_count) values ($1, $2, $3, $4, $5)"#,
        workspace_id,
        auth_user.user_id,
        module_id,
        tokens.len() as i32,
        output.chars().count() as i32
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "response": output,
        }),
    }))
}

async fn handle_save_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleSaveRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let data = req.module.data;
    let workspace_id = sqlx::query!(
        r#"select
            workspace_id
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?
    .workspace_id;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let module = sqlx::query_as!(
        ModuleFromSql,
        r#"update module_v2 set config_data = $1 where module_id = $2
        returning
            module_id,
            module_name,
            template_id,
            workspace_id,
            module_category,
            config_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        data,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "module": module }),
    }))
}

async fn handle_reset_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleResetRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let module = sqlx::query!(
        r#"select
            workspace_id,
            module_category
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let workspace_id = module.workspace_id;
    let module_category = module.module_category;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let _clean_candidate = sqlx::query!(
        r#"delete from candidate_v2 where module_id = $1"#,
        module_id
    )
    .execute(&ctx.db)
    .await?;

    let _clean_files = sqlx::query!(r#"delete from file_module where module_id = $1"#, module_id)
        .execute(&ctx.db)
        .await?;

    let _update_jobs = sqlx::query!(
        r#"update job_v2 set job_status = 1 where module_id = $1 and job_status = 0"#,
        module_id
    )
    .execute(&ctx.db)
    .await?;

    let module_config;
    if let Some(template_id) = req.module.template_id {
        let template = sqlx::query!(
            r#"select
                template_data,
                template_category
            from template_v2 where template_id = $1"#,
            template_id
        )
        .fetch_one(&ctx.db)
        .await?;
        if template.template_category != module_category {
            return Err(Error::unprocessable_entity([(
                "templateId".to_string(),
                "invalid template".to_string(),
            )]));
        }
        let template_data = template.template_data;
        let keys = template_data["keys"].as_array().unwrap();
        let prompt = template_data["prompt"].as_str().unwrap();
        let separator = template_data["separator"].as_str().unwrap_or_default();
        let key_configs = &template_data["keyConfigs"];
        let preprocess = &template_data["preprocess"];
        let mut map = serde_json::Map::new();
        for key in keys {
            let key = key.as_str().unwrap();
            let key_config = key_configs[key].as_object().unwrap();
            let key_display_name = key_config["displayName"].as_str().unwrap();
            let key_hint = key_config["hint"].as_str().unwrap();
            map.insert(
                key.to_string(),
                json!({
                    "displayName": key_display_name,
                    "hint": key_hint,
                    "value": "",
                }),
            );
        }
        module_config = json!({
            "prompt": prompt,
            "input": "",
            "keys": keys,
            "keyConfigs": serde_json::Value::Object(map),
            "separator": separator,
            "preprocess": preprocess,
            "assignData": {},
        });
    } else {
        module_config = json!({
            "prompt": "",
            "input": "",
            "keys": [],
            "keyConfigs": {},
            "separator": "",
            "preprocess": [],
            "assignData": {},
        });
    }

    let module = sqlx::query_as!(
        ModuleFromSql,
        // language=PostgreSQL
        r#"update module_v2 set config_data = $1, template_id = $2 where module_id = $3
        returning
            module_id,
            module_name,
            template_id,
            workspace_id,
            module_category,
            config_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        module_config,
        req.module.template_id,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "module": module }),
    }))
}

async fn handle_list_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<ModuleListRequest>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let workspace_id = req.workspace_id;
    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let modules = sqlx::query_as!(
        ModuleWithStatusFromSql,
        // language=PostgreSQL
        r#"select
            module_v2.module_id,
            module_name,
            template_id,
            workspace_id,
            module_category,
            config_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz",
            case
                when job_count > 0 then
                    case
                        when some_running then 'Running'
                        when all_zero then 'Pending'
                        else 'Ready'
                    end
                else 'Ready'
            end as status
        from module_v2
        left join (
            select
                module_id,
                count(*) as job_count,
                bool_or(counts < target_count AND counts > 0) as some_running,
                bool_and(counts = 0) as all_zero
            from job_v2 j
            left join (
                select 
                    job_id, 
                    count(distinct job_status_group_id) as counts
                from
                    candidate_v2
                group by
                    job_id
            ) c on j.job_id = c.job_id
            where
                j.job_status = 0
            group by
                j.module_id
        ) job_status on module_v2.module_id = job_status.module_id
        where workspace_id = $1"#,
        workspace_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "modules": modules }),
    }))
}

async fn handle_run_module(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleRunRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let module = sqlx::query!(
        r#"select
            config_data,
            workspace_id,
            template_id,
            module_category
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        module.workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    if _member_record.user_level > 1 {
        return Err(Error::Forbidden);
    }

    let _clean_candidate = sqlx::query!(
        r#"delete from candidate_v2 where module_id = $1"#,
        module_id
    )
    .execute(&ctx.db)
    .await?;

    let _reset_files = sqlx::query!(
        r#"update file_module set finish_process = false where module_id = $1"#,
        module_id
    )
    .execute(&ctx.db)
    .await?;

    let _update_jobs = sqlx::query!(
        r#"update job_v2 set job_status = 1 where module_id = $1 and job_status = 0"#,
        module_id
    )
    .execute(&ctx.db)
    .await?;

    let mut module_config = module.config_data;

    let module_config = module_config.as_object_mut().unwrap();

    let preprocess = module_config["preprocess"].as_array().unwrap().clone();
    for process in preprocess {
        let input_keys = process["inputKeys"].as_array().unwrap();
        let mut prompt = process["prompt"].as_str().unwrap().to_string();
        let model = process["model"].as_str().unwrap().to_string();
        let output_key = process["outputKey"].as_str().unwrap();
        for key in input_keys {
            let key = key.as_str().unwrap();
            let key_config = module_config["keyConfigs"][key].as_object().unwrap();
            prompt = prompt.replace(
                &format!("@key/{}", key),
                key_config["value"].as_str().unwrap(),
            );
        }
        log::info!("preprocessing: key: {}", &output_key);
        let api_key = openai::get_available_key(&ctx.db).await?;
        let output = openai::chat(
            ChatRequest {
                model: model,
                input: prompt,
                max_tokens: Some(2048),
                temperature: Some(0.1),
                history: None,
            },
            &api_key.openai_key,
        )
        .await?;
        openai::release_key(&ctx.db, api_key).await?;
        log::info!("preprocessing: key: {}, finish", &output_key);
        let output_key_config = module_config["keyConfigs"][output_key]
            .as_object_mut()
            .unwrap();
        output_key_config["value"] = serde_json::Value::String(output);
    }

    log::info!("new module_config: {:?}", &module_config);
    let mut prompt = module_config["prompt"].as_str().unwrap().to_string();
    let keys = module_config["keys"].as_array().unwrap();
    let key_configs = &module_config["keyConfigs"];
    for key in keys {
        let key = key.as_str().unwrap();
        let key_config = key_configs[key].as_object().unwrap();
        prompt = prompt.replace(
            &format!("@key/{}", key),
            key_config["value"].as_str().unwrap(),
        );
    }
    let separtor = module_config["separator"].as_str().unwrap_or_default();

    let assign_data = &module_config["assignData"];
    let datastore_id = assign_data["datastoreId"].as_str();
    let is_raw = assign_data["isRaw"].as_bool();
    let tags = assign_data["tags"].as_str();
    if let (Some(datastore_id), Some(is_raw), Some(tags)) = (datastore_id, is_raw, tags) {
        let datastore_id = Uuid::parse_str(datastore_id).unwrap();
        let tags = tags
            .split(',')
            .map(|s| s.to_string())
            .collect::<Vec<String>>();
        let like_conditions = tags
            .iter()
            .map(|tag| {
                format!(
                    "(tags LIKE '%{}%' OR tags LIKE '{},%' OR tags LIKE '%,{}' OR tags = '{}')",
                    tag, tag, tag, tag
                )
            })
            .collect::<Vec<_>>()
            .join(" OR ");

        #[derive(serde::Serialize, serde::Deserialize, sqlx::FromRow)]
        struct DataFromSql {
            data_content: String,
            extra_data: Option<serde_json::Value>,
        }
        let assigned_data: Vec<DataFromSql>;
        if is_raw {
            let query = format!(
                r#"select
                    data_content,
                    extra_data
                from data_v2 where module_id = '{}' and is_raw = true and ({})"#,
                datastore_id, like_conditions
            );
            assigned_data = sqlx::query_as(&query).fetch_all(&ctx.db).await?;
        } else {
            let query = format!(
                r#"select
                    data_content,
                    extra_data
                from data_v2 where datastore_id = '{}' and is_raw = false and ({})"#,
                datastore_id, like_conditions
            );
            assigned_data = sqlx::query_as(&query).fetch_all(&ctx.db).await?;
        }
        log::info!("assigned: count: {}", assigned_data.len());
        let job = sqlx::query!(
            r#"insert into job_v2 (module_id, config_data, workspace_id, target_count) values ($1, $2, $3, $4) returning job_id"#,
            module_id,
            json!({}),
            module.workspace_id,
            assigned_data.len() as i32
        )
        .fetch_one(&ctx.db)
        .await?;
        let model_name = if module.module_category == "generator" {
            "gpt-3.5-turbo"
        } else {
            "gpt-4-1106-preview"
        };

        for data in assigned_data {
            let input = data.data_content;
            let mut reference = "".to_string();
            if let Some(extra_data) = data.extra_data {
                if let Some(reference_data) = extra_data["text"].as_str() {
                    reference = reference_data.to_string();
                }
            }
            queue::publish_message_evo(
                &queue::make_channel(&ctx.config.rabbitmq_url).await,
                json!({
                    "module_id": module_id,
                    "job_id": job.job_id,
                    "workspace_id": module.workspace_id,
                    "file_id": "",
                    "input": input,
                    "prompt": prompt,
                    "user_id": auth_user.user_id,
                    "separator": separtor,
                    "reference": reference,
                    "model_name": model_name,
                }),
            )
            .await;
        }
    }

    let files = sqlx::query!(
        r#"select
            files.file_id,
            finish_process,
            files.file_path,
            files.file_name,
            files.file_type
        from file_module
        left join files on files.file_id = file_module.file_id
        where module_id = $1"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    for file in files {
        if file.finish_process {
            continue;
        }
        log::info!("extracting: file_name: {}", &file.file_name);
        let file_path = Path::new(&ctx.config.upload_dir).join(&file.file_path);
        if file.file_type == "csv" {
            #[derive(serde::Serialize, serde::Deserialize)]
            struct CsvRecord {
                input: String,
                reference: String,
            }
            let mut reader = csv::Reader::from_path(&file_path).unwrap();
            let csv_data = reader
                .deserialize::<CsvRecord>()
                .map(|r| r.unwrap())
                .collect::<Vec<CsvRecord>>();
            let job = sqlx::query!(
                r#"insert into job_v2 (module_id, config_data, workspace_id, target_count) values ($1, $2, $3, $4) returning job_id"#,
                module_id,
                json!({}),
                module.workspace_id,
                csv_data.len() as i32
            )
            .fetch_one(&ctx.db)
            .await?;
            let model_name = if module.module_category == "generator" {
                "gpt-3.5-turbo"
            } else {
                "gpt-4-1106-preview"
            };
            for data in csv_data {
                let input = data.input;
                let reference = data.reference;
                queue::publish_message_evo(
                    &queue::make_channel(&ctx.config.rabbitmq_url).await,
                    json!({
                        "module_id": module_id,
                        "job_id": job.job_id,
                        "workspace_id": module.workspace_id,
                        "file_id": "",
                        "input": input,
                        "prompt": prompt,
                        "user_id": auth_user.user_id,
                        "separator": separtor,
                        "reference": reference,
                        "model_name": model_name,
                    }),
                )
                .await;
            }
        } else {
            let client = reqwest::Client::builder().build().unwrap();

            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Accept", "application/json".parse().unwrap());

            let form = reqwest::multipart::Form::new()
                .part(
                    "files",
                    reqwest::multipart::Part::bytes(std::fs::read(file_path).unwrap())
                        .file_name(file.file_name),
                )
                .text("strategy", "auto")
                .text("chunking_strategy", "by_title")
                .text("new_after_n_chars", "500")
                .text("max_characters", "1000")
                .text("combine_under_n_chars", "500");

            let request = client
                .request(
                    reqwest::Method::POST,
                    format!("{}/general/v0/general", &ctx.config.unstructured_url),
                )
                .headers(headers)
                .multipart(form);

            let response = request.send().await.unwrap();
            let body = response.json::<serde_json::Value>().await.unwrap();
            let body = body.as_array().unwrap();
            log::info!("extracted: count: {}", body.len());
            let job = sqlx::query!(
                r#"insert into job_v2 (module_id, config_data, workspace_id, target_count) values ($1, $2, $3, $4) returning job_id"#,
                module_id,
                json!({}),
                module.workspace_id,
                body.len() as i32
            )
            .fetch_one(&ctx.db)
            .await?;

            let model_name = if module.module_category == "generator" {
                "gpt-3.5-turbo"
            } else {
                "gpt-4-1106-preview"
            };

            for item in body {
                let input = item["text"].as_str().unwrap().to_string();
                queue::publish_message_evo(
                    &queue::make_channel(&ctx.config.rabbitmq_url).await,
                    json!({
                        "module_id": module_id,
                        "job_id": job.job_id,
                        "workspace_id": module.workspace_id,
                        "file_id": file.file_id,
                        "input": input,
                        "prompt": prompt,
                        "user_id": auth_user.user_id,
                        "separator": separtor,
                        "reference": "",
                        "model_name": model_name,
                    }),
                )
                .await;
            }
        }
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_clear_files(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleClearFilesRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let module = sqlx::query!(
        r#"select
            workspace_id
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let workspace_id = module.workspace_id;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    sqlx::query!(r#"delete from file_module where module_id = $1"#, module_id)
        .execute(&ctx.db)
        .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_save_data(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleSaveDataRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let module = sqlx::query!(
        r#"select
            workspace_id,
            module_category,
            module_name
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let workspace_id = module.workspace_id;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let candidates = sqlx::query!(
        r#"select
            content,
            module_id,
            extra_data
        from candidate_v2 where module_id = $1"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let distinct_tags = sqlx::query!(
        r#"select distinct tags from data_v2 where module_id = $1 and is_raw = true"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let default_tags = vec![format!(
        "{}-{}",
        module.module_name,
        distinct_tags.len() + 1
    )];
    let tags = req.module.tags.clone().unwrap_or(default_tags);
    let tags = tags.join(",");

    for candidate in candidates {
        let content = candidate.content;
        let extra_data = candidate.extra_data;
        sqlx::query!(
            r#"insert into data_v2 (module_id, data_module_type, is_raw, tags, data_content, extra_data) values ($1, $2, $3, $4, $5, $6)"#,
            module_id,
            module.module_category,
            true,
            tags,
            content,
            extra_data
        )
        .execute(&ctx.db)
        .await?;
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_assign_data(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ModuleBody<ModuleAssignDataRequest>>,
) -> Result<Json<CommonResponse>> {
    log::info!("{:?}", req);
    let module_id = req.module.module_id;
    let module = sqlx::query!(
        r#"select
            workspace_id,
            module_category,
            module_name
        from module_v2 where module_id = $1"#,
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let workspace_id = module.workspace_id;

    let _member_record = sqlx::query!(
        // language=PostgreSQL
        r#"select user_level from workspace_member_v2 where workspace_id = $1 and user_id = $2"#,
        workspace_id,
        auth_user.user_id
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::Forbidden)?;

    let datastore_id = req.module.database_id;
    let is_raw = req.module.is_raw;
    let tags = req.module.tags.clone();
    let tags = tags.join(",");

    let module = sqlx::query_as!(
        ModuleFromSql,
        r#"update module_v2 set config_data['assignData'] = to_jsonb($1::jsonb) where module_id = $2
        returning
            module_id,
            module_name,
            template_id,
            workspace_id,
            module_category,
            config_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        json!({
            "datastoreId": datastore_id,
            "isRaw": is_raw,
            "tags": tags,
        }),
        module_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "module": module,
        }),
    }))
}
