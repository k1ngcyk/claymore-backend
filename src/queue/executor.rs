use async_openai::{
    types::{
        ChatCompletionRequestMessageArgs, CreateChatCompletionRequestArgs,
        CreateEmbeddingRequestArgs, Role,
    },
    Client,
};
use lapin::message::Delivery;
use serde_json::Value;
use sqlx::PgPool;
use std::str;
use uuid::Uuid;

pub async fn execute_job(db: PgPool, delivery: &Delivery) {
    let message = str::from_utf8(&delivery.data).unwrap();
    let message: Value = serde_json::from_str(message).unwrap();
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

    if finished_count >= target_count {
        return;
    }

    let generator_id = message["generator_id"].as_str().unwrap();
    let generator_id = Uuid::parse_str(generator_id).unwrap();
    let prompt_chain = sqlx::query!(
        r#"select prompt_chain from generator where generator_id = $1"#,
        generator_id
    )
    .fetch_one(&db)
    .await
    .unwrap()
    .prompt_chain;
    let prompts = prompt_chain["prompts"].as_array().unwrap();
    for _ in finished_count..=target_count {
        let job_status = sqlx::query!(r#"select job_status from job where job_id = $1"#, job_id)
            .fetch_one(&db)
            .await
            .unwrap()
            .job_status;
        if job_status != 0 {
            return;
        }
        let mut response = String::new();
        for prompt in prompts {
            let prompt = prompt.as_str().unwrap();
            let prompt = prompt.replace("^^", &response);
            let chat_request = CreateChatCompletionRequestArgs::default()
                .max_tokens(1024u16)
                .model("gpt-4")
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
        let result = sqlx::query!(
            r#"insert into datadrop (job_id, datadrop_name, datadrop_content) values ($1, $2, $3)"#,
            job_id,
            format!("Data {}", job_id),
            response
        )
        .execute(&db)
        .await
        .unwrap();
    }
}
