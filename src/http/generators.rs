use crate::http::extractor::AuthUser;
use crate::http::types::Timestamptz;
use crate::http::ApiContext;
use crate::http::{Error, Result};
use axum::extract::{Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use uuid::Uuid;

use crate::http::CommonResponse;

// The `profiles` routes are very similar to the `users` routes, except they allow looking up
// other users' data.

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/api/ticket/list", get(handle_list_ticket))
        .route(
            "/api/ticket/link/file",
            post(handle_ticket_link_file).get(handle_get_linked_file),
        )
        .route("/api/ticket/link/chat", post(handle_ticket_link_chat))
        .route("/api/ticket/custom/new", post(handle_new_ticket))
        .route("/api/ticket/custom", post(handle_update_ticket))
        .route("/api/ticket/info", get(handle_get_ticket_detail))
        .route("/api/ticket/unlink/file", post(handle_ticket_unlink_file))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct TicketBody<T> {
    ticket: T,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTicketRequest {
    ticket_id: Uuid,
    title: String,
    ai_type: String,
    detail: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkFileRequest {
    ticket_id: Uuid,
    file_ids: Vec<Uuid>,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnlinkFileRequest {
    ticket_id: Uuid,
    file_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LinkChatRequest {
    ticket_id: Uuid,
    chat_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TicketRequest {
    ticket_id: Uuid,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct TicketFromQuery {
    ticket_id: Uuid,
    owner_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_session_id: Option<Uuid>,
    title: String,
    ai_type: String,
    detail: String,
    created_at: Timestamptz,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<Timestamptz>,
}

async fn handle_new_ticket(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"insert into ticket (owner_id) values ($1) returning ticket_id"#,
        auth_user.user_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket_id": ticket.ticket_id,
        }),
    }))
}

async fn handle_update_ticket(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TicketBody<UpdateTicketRequest>>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"select owner_id from ticket where ticket_id = $1"#,
        req.ticket.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let _ticket = sqlx::query!(
        r#"update ticket set title = $1, ai_type = $2, detail = $3 where ticket_id = $4"#,
        req.ticket.title,
        req.ticket.ai_type,
        req.ticket.detail,
        req.ticket.ticket_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket_id": req.ticket.ticket_id,
        }),
    }))
}

async fn handle_ticket_link_file(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TicketBody<LinkFileRequest>>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"select owner_id from ticket where ticket_id = $1"#,
        req.ticket.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    for file_id in req.ticket.file_ids {
        let _result = sqlx::query!(
            r#"insert into ticket_file (ticket_id, file_id) values ($1, $2)"#,
            req.ticket.ticket_id,
            file_id
        )
        .execute(&ctx.db)
        .await?;
    }

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket_id": req.ticket.ticket_id,
        }),
    }))
}

async fn handle_ticket_link_chat(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TicketBody<LinkChatRequest>>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"select owner_id from ticket where ticket_id = $1"#,
        req.ticket.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let chat = sqlx::query!(
        r#"select user_id from chat where session_id = $1"#,
        req.ticket.chat_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if chat.user_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let _result = sqlx::query!(
        r#"update ticket set chat_session_id = $1 where ticket_id = $2"#,
        req.ticket.chat_id,
        req.ticket.ticket_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket_id": req.ticket.ticket_id,
            "chat_id": req.ticket.chat_id,
        }),
    }))
}

async fn handle_list_ticket(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<CommonResponse>> {
    let tickets = sqlx::query_as!(
        TicketFromQuery,
        r#"select ticket_id, 
            owner_id,
            chat_session_id,
            title,
            ai_type,
            detail,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from ticket where owner_id = $1"#,
        auth_user.user_id
    )
    .fetch_all(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "tickets": tickets,
        }),
    }))
}

async fn handle_get_linked_file(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<TicketRequest>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"select owner_id from ticket where ticket_id = $1"#,
        req.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let files = sqlx::query!(
        r#"select file_id from ticket_file where ticket_id = $1"#,
        req.ticket_id
    )
    .fetch_all(&ctx.db)
    .await?;

    let files = files
        .iter()
        .map(|f| {
            json!({
                "file_id": f.file_id,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "files": files,
        }),
    }))
}

async fn handle_get_ticket_detail(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Query(req): Query<TicketRequest>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query_as!(
        TicketFromQuery,
        r#"select ticket_id, 
            owner_id,
            chat_session_id,
            title,
            ai_type,
            detail,
            created_at "created_at: Timestamptz",
            updated_at "updated_at: Timestamptz"
        from ticket where ticket_id = $1"#,
        req.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let files = sqlx::query!(
        // get filename from upload where file_id = ticket_file.file_id
        r#"select file_id, filename from upload where file_id in (select file_id from ticket_file where ticket_id = $1)"#,
        req.ticket_id
    )
    .fetch_all(&ctx.db)
    .await?
    .iter()
    .map(|f| {
        json!({
            "file_id": f.file_id,
            "filename": f.filename,
        })
    })
    .collect::<Vec<_>>();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket": ticket,
            "files": files,
        }),
    }))
}

async fn handle_ticket_unlink_file(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<TicketBody<UnlinkFileRequest>>,
) -> Result<Json<CommonResponse>> {
    let ticket = sqlx::query!(
        r#"select owner_id from ticket where ticket_id = $1"#,
        req.ticket.ticket_id
    )
    .fetch_one(&ctx.db)
    .await?;

    if ticket.owner_id != auth_user.user_id {
        return Err(Error::Unauthorized);
    }

    let _result = sqlx::query!(
        r#"delete from ticket_file where ticket_id = $1 and file_id = $2"#,
        req.ticket.ticket_id,
        req.ticket.file_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "ticket_id": req.ticket.ticket_id,
            "file_id": req.ticket.file_id,
        }),
    }))
}
