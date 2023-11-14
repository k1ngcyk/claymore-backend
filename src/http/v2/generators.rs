use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result, ResultExt};
use crate::queue;
use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use log::info;
use regex::Regex;
use serde_json::json;
use std::path::Path;
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route(
            "/v2/generator",
            post(handle_new_generator).get(handle_get_generator_info),
        )
        .route("/v2/generator/list", get(handle_get_generator_list))
        .route("/v2/generator/try", post(handle_try_generator))
        .route("/v2/generator/save", post(handle_save_generator))
        .route("/v2/generator/reset", post(handle_reset_generator))
        .route("/v2/generator/run", post(handle_run_generator))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GeneratorBody<T> {
    generator: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGeneratorRequest {
    generator_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<Uuid>,
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
struct GeneratorSaveRequest {
    generator_id: Uuid,
    data: serde_json::Value,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorTryRequest {
    generator_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    input: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorResetRequest {
    generator_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<Uuid>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorRunRequest {
    generator_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorFromSql {
    generator_id: Uuid,
    generator_name: String,
    template_id: Option<Uuid>,
    config_data: serde_json::Value,
    project_id: Uuid,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DatadropFromSql {
    datadrop_id: Uuid,
    datadrop_name: String,
    datadrop_content: String,
    generator_id: Option<Uuid>,
    project_id: Uuid,
    extra_data: Option<serde_json::Value>,
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

    if member_record.user_level > 1 {
        return Err(Error::Unauthorized);
    }

    if req.generator.generator_name == "" {
        return Err(Error::unprocessable_entity([(
            "generatorName",
            "generator name is required",
        )]));
    }

    let generator_config;
    if let Some(generator_id) = req.generator.template_id {
        let template = sqlx::query!(
            r#"select
                template_data
            from template_v2 where template_id = $1"#,
            generator_id
        )
        .fetch_one(&ctx.db)
        .await?;
        let template_data = template.template_data;
        let keys = template_data["keys"].as_array().unwrap();
        let prompt = template_data["prompt"].as_str().unwrap();
        let key_configs = &template_data["keyConfigs"];
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
        generator_config = json!({
            "prompt": prompt,
            "input": "",
            "keys": keys,
            "keyConfigs": serde_json::Value::Object(map),
        });
    } else {
        generator_config = json!({
            "prompt": "",
            "input": "",
            "keys": [],
            "keyConfigs": {},
        });
    }

    let generator = sqlx::query!(
        // language=PostgreSQL
        r#"insert into generator_v2 (generator_name, template_id, config_data, project_id)
        values ($1, $2, $3, $4) returning generator_id"#,
        req.generator.generator_name,
        req.generator.template_id,
        generator_config,
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
        r#"select team_id from project where project_id = (select project_id from generator_v2 where generator_id = $1)"#,
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

    let generator = sqlx::query_as!(
        GeneratorFromSql,
        r#"select
            generator_id,
            generator_name,
            template_id,
            config_data,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from generator_v2 where generator_id = $1"#,
        req.generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    let datadrops = sqlx::query_as!(
        DatadropFromSql,
        r#"select
            datadrop_id,
            datadrop_name,
            datadrop_content,
            generator_id,
            project_id,
            extra_data,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from datadrop_v2 where generator_id = $1"#,
        req.generator_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let files = sqlx::query!(
        r#"select
            file_v2.file_id,
            file_v2.file_name,
            file_generator_v2.finish_process
        from file_generator_v2
        left join file_v2 using (file_id)
        where generator_id = $1"#,
        req.generator_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let files = files
        .iter()
        .map(|x| {
            json!({
                "fileId": x.file_id,
                "fileName": x.file_name,
                "finishProcess": x.finish_process,
            })
        })
        .collect::<Vec<serde_json::Value>>();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "generator": generator,
            "datadrops": datadrops,
            "files": files,
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
            template_id,
            config_data,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from generator_v2 where project_id = $1"#,
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

async fn handle_try_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<GeneratorTryRequest>>,
) -> Result<Json<CommonResponse>> {
    let generator_id = req.generator.generator_id;
    let generator = sqlx::query!(
        r#"select
            project_id
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    let team_id = sqlx::query!(
        // language=PostgreSQL
        r#"select team_id from project where project_id = $1"#,
        generator.project_id
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

    let generator_config = sqlx::query!(
        r#"select
            config_data
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?
    .config_data;

    let generator_config = generator_config.as_object().unwrap();
    let mut prompt = generator_config["prompt"].as_str().unwrap().to_string();
    let input = req
        .generator
        .input
        .unwrap_or_else(|| generator_config["input"].as_str().unwrap().to_string());
    let keys = generator_config["keys"].as_array().unwrap();
    let key_configs = &generator_config["keyConfigs"];
    for key in keys {
        let key = key.as_str().unwrap();
        let key_config = key_configs[key].as_object().unwrap();
        prompt = prompt.replace(
            &format!("@key/{}", key),
            key_config["value"].as_str().unwrap(),
        );
    }
    prompt = prompt.replace("@key/input", &input);
    let client = Client::new();
    let chat_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(2048 as u16)
        .model("gpt-3.5-turbo")
        .temperature(0.1)
        .messages([ChatCompletionRequestMessageArgs::default()
            .role(Role::User)
            .content(format!(r#"{}"#, prompt))
            .build()
            .unwrap()])
        .build()?;
    let gpt_response = client.chat().create(chat_request).await?;
    let output = &gpt_response
        .choices
        .iter()
        .filter(|x| x.message.role == Role::Assistant)
        .next()
        .unwrap()
        .message
        .content;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "response": output,
        }),
    }))
    /*
        let generator_id;
        let model_name;
        let prompt_chain;
        let temperature;
        let word_count;
        let project_id;
        if req.generator.generator_id.is_some() {
            generator_id = req.generator.generator_id.unwrap();
            let generator = sqlx::query!(
                r#"select
                    prompt_chain,
                    model_name,
                    temperature,
                    word_count,
                    project_id
                from generator where generator_id = $1"#,
                generator_id
            )
            .fetch_one(&ctx.db)
            .await?;
            project_id = generator.project_id;
            let team_id = sqlx::query!(
                // language=PostgreSQL
                r#"select team_id from project where project_id = $1"#,
                generator.project_id
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
            model_name = generator.model_name;
            prompt_chain = generator.prompt_chain;
            temperature = generator.temperature;
            word_count = generator.word_count;
        } else {
            if req.generator.model_name.is_none()
                || req.generator.prompt_chain.is_none()
                || req.generator.temperature.is_none()
                || req.generator.word_count.is_none()
            {
                return Err(Error::unprocessable_entity([(
                    "generatorId",
                    "generatorId or model_name, prompt_chain, temperature, word_count is required",
                )]));
            }
            model_name = req.generator.model_name.unwrap();
            prompt_chain = req.generator.prompt_chain.unwrap();
            temperature = req.generator.temperature.unwrap();
            word_count = req.generator.word_count.unwrap();
            project_id = req.generator.project_id.unwrap();
            let available_models = vec!["gpt-4", "gpt-3.5-turbo-16k", "gpt-3.5-turbo"];
            if !available_models.contains(&model_name.as_str()) {
                return Err(Error::unprocessable_entity([(
                    "modelName",
                    "unavailable model name",
                )]));
            }
        }
        let prompts = prompt_chain["prompts"].as_array().unwrap();
        let client = Client::new();
        let mut response = String::new();
        let mut prompt_responses: Vec<String> = Vec::new();
        for (current_idx, prompt) in prompts.iter().enumerate() {
            info!("prompt: {} model_name: {}", prompt, &model_name);
            let prompt = prompt.as_str().unwrap();
            let mut prompt = prompt.replace("^^", &response);
            let regex = Regex::new(r"@(ref|prompt)/([\w+/]+)").unwrap();
            let mut patterns = Vec::new();
            for cap in regex.captures_iter(&prompt) {
                let mut pattern = Vec::new();
                pattern.push(cap[1].to_string());
                pattern.extend(cap[2].split('/').map(|s| s.to_string()));
                patterns.push(pattern);
            }
            for pattern in patterns {
                let pattern_type = &pattern[0];
                if pattern_type == "ref" {
                    let name = &pattern[1];
                    let character = sqlx::query!(
                        r#"select settings from character where character_name = $1 and project_id = $2"#,
                        name,
                        project_id
                    )
                    .fetch_one(&ctx.db)
                    .await
                    .unwrap()
                    .settings;
                    let character = character["kv"].as_array().unwrap();
                    let result;
                    if pattern.len() == 2 {
                        // whole
                        result = character
                            .iter()
                            .map(|x| {
                                let key = x["key"].as_str().unwrap();
                                let value_type = x["type"].as_str().unwrap();
                                let value;
                                if value_type == "array" {
                                    let values = x["value"].as_array().unwrap();
                                    value = values
                                        .iter()
                                        .map(|x| x.as_str().unwrap())
                                        .collect::<Vec<&str>>()
                                        .join("; ");
                                } else {
                                    value = x["value"].as_str().unwrap().to_string();
                                }
                                format!("{}: {}", key, value)
                            })
                            .collect::<Vec<String>>()
                            .join(", ");
                    } else if pattern.len() == 3 {
                        if pattern[2] == "random" {
                            // random from `name`
                            let idx = rand::random::<usize>() % character.len();
                            let x = &character[idx];
                            let value_type = x["type"].as_str().unwrap();
                            let value;
                            if value_type == "array" {
                                let values = x["value"].as_array().unwrap();
                                value = values
                                    .iter()
                                    .map(|x| x.as_str().unwrap())
                                    .collect::<Vec<&str>>()
                                    .join("; ");
                            } else {
                                value = x["value"].as_str().unwrap().to_string();
                            }
                            result = format!("{}: {}", x["key"].as_str().unwrap(), value);
                        } else {
                            let keys = pattern[2].split('+').collect::<Vec<&str>>();
                            let mut temp = Vec::new();
                            for key in keys {
                                let x = character
                                    .iter()
                                    .find(|x| x["key"].as_str().unwrap() == key)
                                    .unwrap();
                                let value_type = x["type"].as_str().unwrap();
                                let value;
                                if value_type == "array" {
                                    let values = x["value"].as_array().unwrap();
                                    value = values
                                        .iter()
                                        .map(|x| x.as_str().unwrap())
                                        .collect::<Vec<&str>>()
                                        .join("; ");
                                } else {
                                    value = x["value"].as_str().unwrap().to_string();
                                }
                                temp.push(format!("{}: {}", key, value));
                            }
                            result = temp.join(", ");
                        }
                    } else if pattern.len() == 4 {
                        let key = &pattern[2];
                        if pattern[3] == "random" {
                            let value_type = character
                                .iter()
                                .find(|x| x["key"].as_str().unwrap() == key)
                                .unwrap()["type"]
                                .as_str()
                                .unwrap();
                            if value_type != "array" {
                                result = "".to_string();
                            } else {
                                // random from `name` with `key`
                                let value = character
                                    .iter()
                                    .find(|x| x["key"].as_str().unwrap() == key)
                                    .unwrap()["value"]
                                    .as_array();
                                if let Some(value) = value {
                                    let idx = rand::random::<usize>() % value.len();
                                    result = value[idx].as_str().unwrap().to_string();
                                } else {
                                    result = "".to_string();
                                }
                            }
                        } else {
                            result = "".to_string();
                        }
                    } else {
                        result = "".to_string();
                    }
                    prompt = prompt.replacen(&format!("@{}", pattern.join("/")), &result, 1);
                } else if pattern_type == "prompt" {
                    let prompt_idx = pattern[1].parse::<usize>().unwrap();
                    if prompt_idx > current_idx {
                        continue;
                    }
                    let prompt_response = prompt_responses[prompt_idx - 1].clone();
                    prompt = prompt.replace(&format!("@prompt/{}", prompt_idx), &prompt_response);
                }
            }
            let chat_request = CreateChatCompletionRequestArgs::default()
                .max_tokens(word_count as u16)
                .model(&model_name)
                .temperature(temperature as f32)
                .messages([ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(format!(r#"{}"#, prompt))
                    .build()
                    .unwrap()])
                .build()?;
            let gpt_response = client.chat().create(chat_request).await?;
            let output = &gpt_response
                .choices
                .iter()
                .filter(|x| x.message.role == Role::Assistant)
                .next()
                .unwrap()
                .message
                .content;
            prompt_responses.push(output.to_string());
            response = output.to_string();
        }

        Ok(Json(CommonResponse {
            code: 200,
            message: "success".to_string(),
            data: json!({
                "response": response,
            }),
        }))
    */
}

async fn handle_save_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<GeneratorSaveRequest>>,
) -> Result<Json<CommonResponse>> {
    let generator_id = req.generator.generator_id;
    let generator = sqlx::query!(
        r#"select
            project_id
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let team_id = sqlx::query!(
        // language=PostgreSQL
        r#"select team_id from project where project_id = $1"#,
        generator.project_id
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

    let generator_config = req.generator.data;

    let generator = sqlx::query_as!(
        GeneratorFromSql,
        // language=PostgreSQL
        r#"update generator_v2 set config_data = $1 where generator_id = $2
        returning
            generator_id,
            generator_name,
            template_id,
            config_data,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        generator_config,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "generator": generator }),
    }))
}

async fn handle_reset_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<GeneratorResetRequest>>,
) -> Result<Json<CommonResponse>> {
    let generator_id = req.generator.generator_id;
    let generator = sqlx::query!(
        r#"select
            project_id,
            template_id
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let team_id = sqlx::query!(
        // language=PostgreSQL
        r#"select team_id from project where project_id = $1"#,
        generator.project_id
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

    if _member_record.user_level > 1 {
        return Err(Error::Unauthorized);
    }

    let generator_config;
    if let Some(template_id) = req.generator.template_id {
        let template = sqlx::query!(
            r#"select
                template_data
            from template_v2 where template_id = $1"#,
            template_id
        )
        .fetch_one(&ctx.db)
        .await?;
        let template_data = template.template_data;
        let keys = template_data["keys"].as_array().unwrap();
        let prompt = template_data["prompt"].as_str().unwrap();
        let key_configs = &template_data["keyConfigs"];
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
        generator_config = json!({
            "prompt": prompt,
            "input": "",
            "keys": keys,
            "keyConfigs": serde_json::Value::Object(map),
        });
    } else {
        generator_config = json!({
            "prompt": "",
            "input": "",
            "keys": [],
            "keyConfigs": {},
        });
    }

    let generator = sqlx::query_as!(
        GeneratorFromSql,
        // language=PostgreSQL
        r#"update generator_v2 set config_data = $1, template_id = $2 where generator_id = $3
        returning
            generator_id,
            generator_name,
            template_id,
            config_data,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        "#,
        generator_config,
        req.generator.template_id,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({ "generator": generator }),
    }))
}

async fn handle_run_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<GeneratorRunRequest>>,
) -> Result<Json<CommonResponse>> {
    let generator_id = req.generator.generator_id;
    let generator = sqlx::query!(
        r#"select
            project_id,
            template_id
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?;
    let team_id = sqlx::query!(
        // language=PostgreSQL
        r#"select team_id from project where project_id = $1"#,
        generator.project_id
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

    if _member_record.user_level > 1 {
        return Err(Error::Unauthorized);
    }

    let generator_config = sqlx::query!(
        r#"select
            config_data
        from generator_v2 where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&ctx.db)
    .await?
    .config_data;

    let generator_config = generator_config.as_object().unwrap();
    let mut prompt = generator_config["prompt"].as_str().unwrap().to_string();
    let keys = generator_config["keys"].as_array().unwrap();
    let key_configs = &generator_config["keyConfigs"];
    for key in keys {
        let key = key.as_str().unwrap();
        let key_config = key_configs[key].as_object().unwrap();
        prompt = prompt.replace(
            &format!("@key/{}", key),
            key_config["value"].as_str().unwrap(),
        );
    }

    let files = sqlx::query!(
        r#"select
            file_v2.file_id,
            finish_process,
            file_v2.file_path,
            file_v2.file_name
        from file_generator_v2
        left join file_v2 on file_v2.file_id = file_generator_v2.file_id
        where generator_id = $1"#,
        generator_id
    )
    .fetch_all(&ctx.db)
    .await?;

    for file in files {
        if file.finish_process {
            continue;
        }
        let file_path = Path::new(&ctx.config.upload_dir).join(&file.file_path);
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
            .text("combine_under_n_chars", "750");

        let request = client
            .request(
                reqwest::Method::POST,
                format!("{}/general/v0/general", &ctx.config.unstructured_url),
            )
            .headers(headers)
            .multipart(form);

        let response = request.send().await.unwrap();
        let body = response.json::<serde_json::Value>().await.unwrap();
        // log::info!("{:?}", body);
        let body = body.as_array().unwrap();
        for item in body {
            let input = item["text"].as_str().unwrap().to_string();
            queue::publish_message_v2(
                &queue::make_channel(&ctx.config.rabbitmq_url).await,
                json!({
                    "generator_id": generator_id,
                    "file_id": file.file_id,
                    "project_id": generator.project_id,
                    "input": input,
                    "prompt": prompt,
                }),
            )
            .await;
        }
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}
