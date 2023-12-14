use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs, Role,
    },
    Client,
};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub model: String,
    pub input: String,
    pub max_tokens: Option<u16>,
    pub temperature: Option<f32>,
    pub history: Option<Vec<History>>,
}

impl Default for ChatRequest {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            input: "".to_string(),
            max_tokens: None,
            temperature: None,
            history: None,
        }
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct History {
    pub user_input: String,
    pub ai_output: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAIKey {
    pub openai_id: Uuid,
    pub openai_key: String,
}

pub async fn chat(
    request: ChatRequest,
    api_key: &String,
) -> Result<String, async_openai::error::OpenAIError> {
    // log::info!(
    //     "chat with: model: {}, input_chars: {}",
    //     &request.model,
    //     &request.input.chars().count(),
    // );
    let mut messages;
    if let Some(chat_history) = request.history {
        messages = chat_history
            .iter()
            .map(|h| {
                vec![
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(h.user_input.clone())
                        .build()
                        .unwrap()
                        .into(),
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(h.ai_output.clone())
                        .build()
                        .unwrap()
                        .into(),
                ]
            })
            .flatten()
            .collect::<Vec<ChatCompletionRequestMessage>>();
    } else {
        messages = Vec::<ChatCompletionRequestMessage>::new()
    };
    messages.push(
        ChatCompletionRequestUserMessageArgs::default()
            .content(request.input)
            .build()?
            .into(),
    );

    let config = OpenAIConfig::new().with_api_key(api_key);
    let client = Client::with_config(config);
    let chat_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(request.max_tokens.unwrap_or(2048))
        .model(request.model)
        .temperature(request.temperature.unwrap_or(0.1))
        .messages(messages)
        .build()?;
    let gpt_response = client.chat().create(chat_request).await?;
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

    Ok(output.to_string())
}

pub async fn get_available_key(db: &PgPool) -> Result<OpenAIKey, sqlx::Error> {
    let mut tx = db.begin().await?;
    let key = sqlx::query!(
        r#"select openai_id, openai_key from openai where openai_status = 0 limit 1 for update"#,
    )
    .fetch_one(tx.as_mut())
    .await?;

    sqlx::query!(
        r#"update openai set openai_status = 1 where openai_id = $1"#,
        key.openai_id
    )
    .execute(tx.as_mut())
    .await?;

    tx.commit().await?;

    Ok(OpenAIKey {
        openai_id: key.openai_id,
        openai_key: key.openai_key,
    })
}

pub async fn release_key(db: &PgPool, key: OpenAIKey) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"update openai set openai_status = 0 where openai_id = $1"#,
        key.openai_id
    )
    .execute(db)
    .await?;

    Ok(())
}
