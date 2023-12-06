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

pub async fn get_available_key(db: &PgPool) -> Result<String, sqlx::Error> {
    let key = nanoid!(16);
    let result = sqlx::query!(
        r#"select count(*) as count from openai_key where key = $1"#,
        key
    )
    .fetch_one(db)
    .await?;
    if result.count == 0 {
        Ok(key)
    } else {
        get_available_key(db).await
    }
}
