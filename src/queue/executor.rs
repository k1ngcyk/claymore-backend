use async_openai::{
    types::{ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs, Role},
    Client,
};
use lapin::message::Delivery;
use log::info;
use serde_json::Value;
use sqlx::PgPool;
use std::str;
use uuid::Uuid;

#[derive(sqlx::Type, PartialEq, Debug)]
#[repr(i32)]
pub enum JobStatus {
    Pending,
    Running,
    Finished,
    Paused,
}

pub async fn execute_job(db: PgPool, delivery: &Delivery) {
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
        return;
    }

    let model_name = message["model_name"].as_str().unwrap();
    let word_count = message["word_count"].as_i64().unwrap() as u16;
    let temperature = message["temperature"].as_f64().unwrap() as f32;
    let prompt_chain = message["prompt_chain"].as_object().unwrap();
    let prompts = prompt_chain["prompts"].as_array().unwrap();
    let project_id = sqlx::query!(r#"select project_id from job where job_id = $1"#, job_id)
        .fetch_one(&db)
        .await
        .unwrap()
        .project_id;
    for _ in finished_count..target_count {
        let job_status = sqlx::query!(
            r#"select job_status "job_status: JobStatus" from job where job_id = $1"#,
            job_id
        )
        .fetch_one(&db)
        .await
        .unwrap()
        .job_status;
        if job_status == JobStatus::Paused {
            return;
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
        for prompt in prompts {
            info!(
                "Job {}: prompt: {} model_name: {}",
                job_id, prompt, model_name
            );
            let prompt = prompt.as_str().unwrap();
            let prompt = prompt.replace("^^", &response);
            let chat_request = CreateChatCompletionRequestArgs::default()
                .max_tokens(word_count)
                .model(model_name)
                .temperature(temperature)
                .messages([ChatCompletionRequestMessageArgs::default()
                    .role(Role::User)
                    .content(format!(r#"{}"#, prompt))
                    .build()
                    .unwrap()])
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
                .content;
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
    }
    sqlx::query!(
        r#"update job set job_status = $1 where job_id = $2"#,
        JobStatus::Finished as i32,
        job_id
    )
    .execute(&db)
    .await
    .unwrap();
}
