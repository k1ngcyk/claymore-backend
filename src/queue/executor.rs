use crate::openai;
use crate::openai::ChatRequest;
use async_openai::{
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use lapin::message::Delivery;
use log::info;
use regex::Regex;
use serde_json::Value;
use sqlx::PgPool;
use std::str;
use tiktoken_rs::{cl100k_base, model};
use uuid::Uuid;

#[derive(sqlx::Type, PartialEq, Debug)]
#[repr(i32)]
pub enum JobStatus {
    Pending,
    Running,
    Finished,
    Paused,
}

pub enum ExecuteResult {
    Overflow,
    Success,
}

pub enum ExecuteResultV2 {
    Success,
    Failed(i32),
}

pub async fn execute_job(db: PgPool, delivery: &Delivery) -> Result<ExecuteResult, anyhow::Error> {
    let message = str::from_utf8(&delivery.data).unwrap();
    let message: Value = serde_json::from_str(message).unwrap();
    info!("Start execute job: {}", message);
    let job_id = message["job_id"].as_str().unwrap();
    let job_id = Uuid::parse_str(job_id).unwrap();
    let finished_count = sqlx::query!(
        r#"select count(*) as finished_count from datadrop where job_id = $1"#,
        job_id
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .finished_count
    .unwrap_or(0);

    let target_count = sqlx::query!(r#"select target_count from job where job_id = $1"#, job_id)
        .fetch_one(&db)
        .await
        .unwrap()
        .target_count;
    let target_count = target_count as i64;

    let client = Client::new();
    info!(
        "Job {}: finished count: {}, target_count: {}",
        job_id, finished_count, target_count
    );
    if finished_count >= target_count {
        sqlx::query!(
            r#"update job set job_status = $1 where job_id = $2"#,
            JobStatus::Finished as i32,
            job_id
        )
        .execute(&db)
        .await
        .unwrap();
        return Ok(ExecuteResult::Overflow);
    }

    let model_name;
    let word_count;
    let temperature;
    let prompt_chain;
    let prompts;

    let generator_id = message["generator_id"].as_str();
    if generator_id.is_some() {
        let generator_id = Uuid::parse_str(generator_id.unwrap()).unwrap();
        let generator = sqlx::query!(
            r#"select model_name, word_count, prompt_chain, temperature from generator where generator_id = $1"#,
            generator_id
        )
        .fetch_one(&db)
        .await
        .unwrap();
        model_name = generator.model_name;
        word_count = generator.word_count as u16;
        temperature = generator.temperature as f32;
        prompt_chain = generator.prompt_chain;
        prompts = prompt_chain["prompts"].as_array().unwrap();
    } else {
        model_name = message["model_name"].as_str().unwrap().to_string();
        word_count = message["word_count"].as_i64().unwrap() as u16;
        temperature = message["temperature"].as_f64().unwrap() as f32;
        let prompt_chain = message["prompt_chain"].as_object().unwrap();
        prompts = prompt_chain["prompts"].as_array().unwrap();
    }

    let project_id = sqlx::query!(r#"select project_id from job where job_id = $1"#, job_id)
        .fetch_one(&db)
        .await
        .unwrap()
        .project_id;
    // Remove loop here, but nack message outside
    // for _ in finished_count..target_count {
    let job_status = sqlx::query!(
        r#"select job_status "job_status: JobStatus" from job where job_id = $1"#,
        job_id
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .job_status;
    if job_status == JobStatus::Paused {
        return Ok(ExecuteResult::Overflow);
    }
    if job_status == JobStatus::Pending {
        sqlx::query!(
            r#"update job set job_status = $1 where job_id = $2"#,
            JobStatus::Running as i32,
            job_id
        )
        .execute(&db)
        .await
        .unwrap();
    }
    let mut response = String::new();
    let mut prompt_responses: Vec<String> = Vec::new();
    for (current_idx, prompt) in prompts.iter().enumerate() {
        info!(
            "Job {}: prompt: {} model_name: {}",
            job_id, prompt, &model_name
        );
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
                .fetch_one(&db)
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
        info!(
            "Job {}: processed prompt: {} model_name: {}",
            job_id, prompt, &model_name
        );
        let chat_request = CreateChatCompletionRequestArgs::default()
            .max_tokens(word_count)
            .model(&model_name)
            .temperature(temperature)
            .messages([ChatCompletionRequestUserMessageArgs::default()
                .content(format!(r#"{}"#, prompt))
                .build()
                .unwrap()
                .into()])
            .build()
            .unwrap();
        let gpt_response = client.chat().create(chat_request).await.unwrap();
        let output = &gpt_response
            .choices
            .iter()
            .filter(|x| x.message.role == Role::Assistant)
            .next()
            .unwrap()
            .message
            .content
            .clone()
            .unwrap_or_default();
        prompt_responses.push(output.to_string());
        response = output.to_string();
    }
    let _result = sqlx::query!(
            r#"insert into datadrop (job_id, datadrop_name, datadrop_content, project_id) values ($1, $2, $3, $4)"#,
            job_id,
            format!("Data {}", job_id),
            response,
            project_id
        )
        .execute(&db)
        .await
        .unwrap();
    Ok(ExecuteResult::Success)
    // }
}

pub async fn execute_job_v2(
    db: PgPool,
    delivery: &Delivery,
) -> Result<ExecuteResultV2, anyhow::Error> {
    let message = str::from_utf8(&delivery.data).unwrap();
    let message: Value = serde_json::from_str(message).unwrap();
    let mut attempts = 0;
    if let Some(headers) = delivery.properties.headers() {
        for (key, value) in headers.into_iter() {
            if key.as_str() == "x-attempts" {
                if let lapin::types::AMQPValue::LongLongInt(val) = value {
                    attempts = *val as i32;
                    break;
                }
            }
        }
    }
    let generator_id = message["generator_id"].as_str().unwrap();
    let generator_id = Uuid::parse_str(generator_id).unwrap();
    let project_id = message["project_id"].as_str().unwrap();
    let project_id = Uuid::parse_str(project_id).unwrap();
    let file_id = message["file_id"].as_str().unwrap();
    let file_id = Uuid::parse_str(file_id).unwrap();
    let input = message["input"].as_str().unwrap();
    let input = input.to_string();
    let prompt = message["prompt"].as_str().unwrap();
    let prompt = prompt.to_string();
    let prompt = prompt.replace("@key/input", &input);
    let team_id = message["team_id"].as_str().unwrap();
    let team_id = Uuid::parse_str(team_id).unwrap();
    let user_id = message["user_id"].as_str().unwrap();
    let user_id = Uuid::parse_str(user_id).unwrap();
    let separator = message["separator"].as_str().unwrap_or("\n\n");
    let separator = separator.to_string();
    let bpe = cl100k_base().unwrap();
    let client = Client::new();
    let chat_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(2048u16)
        .model("gpt-3.5-turbo")
        .temperature(0.1)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(format!(r#"{}"#, prompt))
            .build()
            .unwrap()
            .into()])
        .build()
        .unwrap();
    let tokens = bpe.encode_with_special_tokens(&prompt);
    sqlx::query!(
        r#"insert into usage_v2 (team_id, project_id, generator_id, user_id, token_count) values ($1, $2, $3, $4, $5)"#,
        team_id,
        project_id,
        generator_id,
        user_id,
        tokens.len() as i32
    )
    .execute(&db)
    .await
    .unwrap();

    let gpt_response = client.chat().create(chat_request).await;
    if gpt_response.is_err() {
        let error = gpt_response.unwrap_err();
        log::error!("attempt: {}, error: {}", attempts, error);
        return Ok(ExecuteResultV2::Failed(attempts + 1));
    }
    let gpt_response = gpt_response.unwrap();
    let output = &gpt_response
        .choices
        .iter()
        .filter(|x| x.message.role == Role::Assistant)
        .next()
        .unwrap()
        .message
        .content
        .clone()
        .unwrap_or_default();
    let tokens = bpe.encode_with_special_tokens(&output);
    sqlx::query!(
        r#"insert into usage_v2 (team_id, project_id, generator_id, user_id, token_count) values ($1, $2, $3, $4, $5)"#,
        team_id,
        project_id,
        generator_id,
        user_id,
        tokens.len() as i32
    )
    .execute(&db)
    .await
    .unwrap();

    let results = output.split(&separator).collect::<Vec<&str>>();
    for result in results {
        let _result = sqlx::query!(
            r#"insert into datadrop_v2 (datadrop_name, datadrop_content, generator_id, project_id, extra_data) values ($1, $2, $3, $4, $5)"#,
            format!("Data {}", generator_id),
            result,
            generator_id,
            project_id,
            serde_json::json!({
                "text": input.replace("\u{0000}", ""),
            })
        )
        .execute(&db)
        .await
        .unwrap();
    }

    sqlx::query!(
        r#"update file_generator_v2 set finish_process = $1 where file_id = $2 and generator_id = $3"#,
        true,
        file_id,
        generator_id
    )
    .execute(&db)
    .await
    .unwrap();

    Ok(ExecuteResultV2::Success)
}

pub async fn execute_job_v2_evaluate(
    db: PgPool,
    delivery: &Delivery,
) -> Result<ExecuteResultV2, anyhow::Error> {
    let message = str::from_utf8(&delivery.data).unwrap();
    let message: Value = serde_json::from_str(message).unwrap();
    let mut attempts = 0;
    if let Some(headers) = delivery.properties.headers() {
        for (key, value) in headers.into_iter() {
            if key.as_str() == "x-attempts" {
                if let lapin::types::AMQPValue::LongLongInt(val) = value {
                    attempts = *val as i32;
                    break;
                }
            }
        }
    }
    let generator_id = message["generator_id"].as_str().unwrap();
    let generator_id = Uuid::parse_str(generator_id).unwrap();
    let datadrop_id = message["datadrop_id"].as_str().unwrap();
    let datadrop_id = Uuid::parse_str(datadrop_id).unwrap();
    let project_id = message["project_id"].as_str().unwrap();
    let project_id = Uuid::parse_str(project_id).unwrap();
    let input = message["input"].as_str().unwrap();
    let input = input.to_string();
    let prompt = message["prompt"].as_str().unwrap();
    let prompt = prompt.to_string();
    let prompt = prompt.replace("@key/input", &input);
    let team_id = message["team_id"].as_str().unwrap();
    let team_id = Uuid::parse_str(team_id).unwrap();
    let user_id = message["user_id"].as_str().unwrap();
    let user_id = Uuid::parse_str(user_id).unwrap();
    let reference = message["reference"].as_str().unwrap();
    let reference = reference.to_string();
    let prompt = prompt.replace("@key/reference", &reference);
    let bpe = cl100k_base().unwrap();
    let client = Client::new();
    let chat_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(2048u16)
        .model("gpt-4")
        .temperature(0.1)
        .messages([ChatCompletionRequestUserMessageArgs::default()
            .content(format!(r#"{}"#, prompt))
            .build()
            .unwrap()
            .into()])
        .build()
        .unwrap();
    let tokens = bpe.encode_with_special_tokens(&prompt);
    sqlx::query!(
        r#"insert into usage_v2 (team_id, project_id, generator_id, user_id, token_count) values ($1, $2, $3, $4, $5)"#,
        team_id,
        project_id,
        generator_id,
        user_id,
        tokens.len() as i32
    )
    .execute(&db)
    .await
    .unwrap();

    let gpt_response = client.chat().create(chat_request).await;
    if gpt_response.is_err() {
        return Ok(ExecuteResultV2::Failed(attempts + 1));
    }
    let gpt_response = gpt_response.unwrap();
    let output = &gpt_response
        .choices
        .iter()
        .filter(|x| x.message.role == Role::Assistant)
        .next()
        .unwrap()
        .message
        .content
        .clone()
        .unwrap_or_default();
    let tokens = bpe.encode_with_special_tokens(&output);
    sqlx::query!(
        r#"insert into usage_v2 (team_id, project_id, generator_id, user_id, token_count) values ($1, $2, $3, $4, $5)"#,
        team_id,
        project_id,
        generator_id,
        user_id,
        tokens.len() as i32
    )
    .execute(&db)
    .await
    .unwrap();

    let _result = sqlx::query!(
        r#"update datadrop_v2 set extra_data['evaluate'] = to_jsonb($1::text) where datadrop_id = $2"#,
        output,
        datadrop_id
    )
    .execute(&db)
    .await
    .unwrap();

    Ok(ExecuteResultV2::Success)
}

pub async fn execute_job_evo(
    db: PgPool,
    delivery: &Delivery,
) -> Result<ExecuteResultV2, anyhow::Error> {
    let message = str::from_utf8(&delivery.data).unwrap();
    let message: Value = serde_json::from_str(message).unwrap();
    let mut attempts = 0;
    if let Some(headers) = delivery.properties.headers() {
        for (key, value) in headers.into_iter() {
            if key.as_str() == "x-attempts" {
                if let lapin::types::AMQPValue::LongLongInt(val) = value {
                    attempts = *val as i32;
                    break;
                }
            }
        }
    }
    let module_id = message["module_id"].as_str().unwrap();
    let module_id = Uuid::parse_str(module_id).unwrap();
    let job_id = message["job_id"].as_str().unwrap();
    let job_id = Uuid::parse_str(job_id).unwrap();
    let workspace_id = message["workspace_id"].as_str().unwrap();
    let workspace_id = Uuid::parse_str(workspace_id).unwrap();
    let file_id = message["file_id"].as_str().unwrap_or_default();
    let input = message["input"].as_str().unwrap();
    let input = input.to_string();
    let prompt = message["prompt"].as_str().unwrap();
    let prompt = prompt.to_string();
    let mut prompt = prompt.replace("@key/input", &input);
    let user_id = message["user_id"].as_str().unwrap();
    let user_id = Uuid::parse_str(user_id).unwrap();
    let separator = message["separator"].as_str().unwrap_or_default();
    let separator = separator.to_string();
    let reference = message["reference"].as_str().unwrap_or_default();
    let reference = reference.to_string();
    let model_name = message["model_name"].as_str().unwrap_or("gpt-3.5-turbo");
    if reference != "" {
        prompt = prompt.replace("@key/reference", &reference);
    }

    let bpe = cl100k_base().unwrap();
    let tokens = bpe.encode_with_special_tokens(&prompt);
    sqlx::query!(
        r#"insert into metric_v2 (workspace_id, user_id, module_id, token_count, word_count) values ($1, $2, $3, $4, $5)"#,
        workspace_id,
        user_id,
        module_id,
        tokens.len() as i32,
        prompt.chars().count() as i32
    )
    .execute(&db)
    .await.unwrap();

    let api_key = openai::get_available_key(&db).await.unwrap();
    let output = openai::chat(
        ChatRequest {
            max_tokens: Some(2048),
            input: prompt,
            model: model_name.to_string(),
            temperature: Some(0.1),
            history: None,
        },
        &api_key.openai_key,
    )
    .await;
    openai::release_key(&db, api_key).await.unwrap();

    if output.is_err() {
        let error = output.unwrap_err();
        log::error!("attempt: {}, error: {}", attempts, error);
        return Ok(ExecuteResultV2::Failed(attempts + 1));
    }
    let output = output.unwrap();

    let tokens = bpe.encode_with_special_tokens(&output);
    sqlx::query!(
        r#"insert into metric_v2 (workspace_id, user_id, module_id, token_count, word_count) values ($1, $2, $3, $4, $5)"#,
        workspace_id,
        user_id,
        module_id,
        tokens.len() as i32,
        output.chars().count() as i32
    )
    .execute(&db)
    .await.unwrap();

    struct Result {
        content: String,
        extra_data: serde_json::Value,
    }
    let results;
    if separator != "" {
        results = output
            .split(&separator)
            .map(|x| Result {
                content: x.to_string(),
                extra_data: serde_json::json!({
                    "text": input.replace("\u{0000}", ""),
                }),
            })
            .collect::<Vec<Result>>();
    } else {
        let mut content = output.to_string();
        let mut extra_data = serde_json::json!({
            "text": input.replace("\u{0000}", ""),
        });
        let try_json = serde_json::from_str::<Value>(&output);
        if let Ok(try_json) = try_json {
            if let Some(candidate) = try_json["candidate"].as_object() {
                if let Some(text) = candidate["data"].as_str() {
                    content = text.to_string();
                    extra_data = serde_json::json!({
                        "text": input.replace("\u{0000}", ""),
                        "reference": reference,
                        "rating": try_json["rating"].clone(),
                    });
                }
            }
        }
        results = vec![Result {
            content,
            extra_data,
        }];
    }
    let job_status_group_id = Uuid::new_v4();
    for result in results {
        let _result = sqlx::query!(
            r#"insert into candidate_v2 (content, module_id, job_id, job_status_group_id, extra_data) values ($1, $2, $3, $4, $5)"#,
            result.content,
            module_id,
            job_id,
            job_status_group_id,
            result.extra_data
        )
        .execute(&db)
        .await
        .unwrap();
    }

    if file_id != "" {
        let file_id = Uuid::parse_str(file_id).unwrap();
        sqlx::query!(
            r#"update file_module set finish_process = $1 where file_id = $2 and module_id = $3"#,
            true,
            file_id,
            module_id
        )
        .execute(&db)
        .await
        .unwrap();
    }

    Ok(ExecuteResultV2::Success)
}
