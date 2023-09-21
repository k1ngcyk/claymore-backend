use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result, ResultExt};
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
use uuid::Uuid;

use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route(
            "/generator",
            post(handle_new_generator)
                .get(handle_get_generator_info)
                .delete(handle_delete_generator),
        )
        .route("/generator/list", get(handle_get_generator_list))
        .route("/generator/try", post(handle_try_generator))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct GeneratorBody<T> {
    generator: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewGeneratorRequest {
    generator_name: String,
    prompt_chain: serde_json::Value,
    model_name: String,
    temperature: f64,
    word_count: i32,
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
struct GeneratorDeleteRequest {
    generator_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorTryRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    generator_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_chain: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    word_count: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_id: Option<Uuid>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeneratorFromSql {
    generator_id: Uuid,
    project_id: Uuid,
    generator_name: String,
    prompt_chain: serde_json::Value,
    model_name: String,
    temperature: f64,
    word_count: i32,
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

    let available_models = vec!["gpt-4", "gpt-3.5-turbo-16k", "gpt-3.5-turbo"];

    if !available_models.contains(&req.generator.model_name.as_str()) {
        return Err(Error::unprocessable_entity([(
            "modelName",
            "unavailable model name",
        )]));
    }

    if req.generator.generator_name == "" {
        return Err(Error::unprocessable_entity([(
            "generatorName",
            "generator name is required",
        )]));
    }

    let generator = sqlx::query!(
        // language=PostgreSQL
        r#"insert into generator (generator_name, prompt_chain, model_name, temperature, word_count, project_id)
        values ($1, $2, $3, $4, $5, $6) returning generator_id"#,
        req.generator.generator_name,
        req.generator.prompt_chain,
        req.generator.model_name,
        req.generator.temperature,
        req.generator.word_count,
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
        r#"select team_id from project where project_id = (select project_id from generator where generator_id = $1)"#,
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

    let generator = sqlx::query!(
        r#"select
            generator_id,
            generator_name,
            prompt_chain,
            model_name,
            temperature,
            word_count,
            project_id,
            created_at,
            updated_at
        from generator where generator_id = $1"#,
        req.generator_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "generator_id": generator.generator_id,
            "generator_name": generator.generator_name,
            "prompt_chain": generator.prompt_chain,
            "model_name": generator.model_name,
            "temperature": generator.temperature,
            "word_count": generator.word_count,
            "project_id": generator.project_id,
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
            prompt_chain,
            model_name,
            temperature,
            word_count,
            project_id,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from generator where project_id = $1"#,
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
}

async fn handle_delete_generator(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<GeneratorBody<GeneratorDeleteRequest>>,
) -> Result<Json<CommonResponse>> {
    let generator_id = req.generator.generator_id;
    let generator = sqlx::query!(
        r#"select
            project_id
        from generator where generator_id = $1"#,
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

    sqlx::query!(
        // language=PostgreSQL
        r#"delete from generator where generator_id = $1"#,
        generator_id
    )
    .execute(&ctx.db)
    .await
    .on_constraint("job_generator_id_fkey", |_| {
        Error::unprocessable_entity([("generatorId", "generatorId is in use")])
    })?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}
