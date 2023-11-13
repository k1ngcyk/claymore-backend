use sqlx::PgPool;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChatRequest {
    model: String,
    input: String,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    history: Option<Vec<String>>,
}

pub async fn chat() {}
