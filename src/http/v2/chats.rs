use crate::http::extractor::AuthUser;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use async_openai::types::ChatCompletionRequestMessage;
use async_openai::{
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, Role,
    },
    Client,
};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use elasticsearch::http::transport::Transport;
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

#[derive(serde::Serialize, serde::Deserialize)]
struct ChatBody<T> {
    chat: T,
}

pub(crate) fn router() -> Router<ApiContext> {
    Router::new().route("/v2/chat", post(handle_chat))
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    module_id: Uuid,
    user_input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_history: Option<Vec<ChatHistory>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatHistory {
    user_input: String,
    ai_output: String,
}

async fn handle_chat(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<ChatBody<ChatRequest>>,
) -> Result<Json<CommonResponse>> {
    let module_id = req.chat.module_id;
    let workspace_id = sqlx::query!(
        // language=PostgreSQL
        r#"select workspace_id from module_v2 where module_id = $1"#,
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

    let user_input = req.chat.user_input;
    let chat_history = req.chat.chat_history;
    let new_history = chat_history.clone();
    let datadrops = sqlx::query!(
        // language=PostgreSQL
        r#"select
            candidate_id,
            content
        from candidate_v2
        where module_id = $1"#,
        module_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let datadrops = datadrops
        .iter()
        .map(|d| d.content.as_str())
        .collect::<Vec<&str>>();

    let transport = Transport::single_node(&ctx.config.es_url).unwrap();
    let es_client = elasticsearch::Elasticsearch::new(transport);
    let response = es_client
        .indices()
        .exists(elasticsearch::indices::IndicesExistsParts::Index(&[
            module_id.to_string().as_str(),
        ]))
        .send()
        .await?;
    if !response.status_code().is_success() {
        let mut count = 1;
        for datadrop in datadrops {
            let _resp = es_client
                .index(elasticsearch::IndexParts::IndexId(
                    module_id.to_string().as_str(),
                    count.to_string().as_str(),
                ))
                .body(json!({
                    "content": datadrop,
                }))
                .send()
                .await?;
            count += 1;
        }
    }

    let search_resp = es_client
        .search(elasticsearch::SearchParts::Index(&[module_id
            .to_string()
            .as_str()]))
        .size(5)
        .body(json!({
            "query": {
                "match": {
                    "content": &user_input,
                }
            }
        }))
        .send()
        .await?;
    let search_resp = search_resp.json::<serde_json::Value>().await?;
    log::info!("search_resp: {:?}", search_resp);
    let refs = search_resp["hits"]["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["_source"]["content"].as_str().unwrap().to_string())
        .collect::<Vec<String>>()
        .join("\n\n");

    let mut messages;
    if let Some(chat_history) = chat_history {
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
            .content(format!("你是一个 AI 聊天助手，你的目标是根据我提供的知识库 {} 回答我的问题。我会检验你对知识库中内容的掌握程度，是否正确地回答了我的问题。在回答时，你需要遵循以下规则：\n1. 你必须使用知识库中相关的文本，来回答我的问题，你的回答必须是完整，专业，严谨的。\n2. 如果在知识库中没有找到符合我提问的答案，请直接说不知道，不要编造虚假的内容，或者使用其他不相关的内容来回答。\n{}", &refs, &user_input))
            .build()
            .unwrap()
            .into(),
    );

    log::info!("messages: {:?}", messages);
    let client = Client::new();
    let chat_request = CreateChatCompletionRequestArgs::default()
        .max_tokens(2048u16)
        .model("gpt-4-1106-preview")
        .temperature(0.1)
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

    let mut history = new_history.unwrap_or_default();
    history.push(ChatHistory {
        user_input: user_input.clone(),
        ai_output: output.clone(),
    });

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "history": history,
        }),
    }))
}
