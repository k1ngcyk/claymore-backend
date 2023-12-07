use crate::http::{ApiContext, Result};
use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

use crate::http::error::{Error, ResultExt};
use crate::http::extractor::AuthUser;
use crate::http::CommonResponse;

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/user/signup", post(create_user))
        .route("/user/signin", post(login_user))
        .route("/user", get(get_current_user).put(update_user))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct UserBody<T> {
    user: T,
}

#[derive(serde::Deserialize)]
struct NewUser {
    username: String,
    email: String,
    password: String,
}

#[derive(serde::Deserialize)]
struct LoginUser {
    email: String,
    password: String,
}

#[derive(serde::Deserialize, Default, PartialEq, Eq)]
#[serde(default)]
struct UpdateUser {
    email: Option<String>,
    username: Option<String>,
    password: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct User {
    email: String,
    token: String,
    username: String,
}

async fn create_user(
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<NewUser>>,
) -> Result<Json<CommonResponse>> {
    let password_hash = hash_password(req.user.password).await?;
    let user_id = sqlx::query_scalar!(
        r#"insert into "user" (user_name, email, password_hash) values ($1, $2, $3) returning user_id"#,
        req.user.username,
        req.user.email,
        password_hash
    )
    .fetch_one(&ctx.db)
    .await
    .on_constraint("user_user_name_key", |_| {
        Error::unprocessable_entity([("username", "username taken")])
    })
    .on_constraint("user_email_key", |_| {
        Error::unprocessable_entity([("email", "email taken")])
    })?;

    // Create default Personal Team
    let team_id = sqlx::query_scalar!(
        r#"insert into team (team_name, owner_id) values ($1, $2) returning team_id"#,
        format!(r#"{}'s Personal"#, &req.user.username),
        user_id
    )
    .fetch_one(&ctx.db)
    .await?;

    sqlx::query!(
        r#"insert into team_member (team_id, user_id) values ($1, $2)"#,
        team_id,
        user_id
    )
    .execute(&ctx.db)
    .await?;

    let workspace = sqlx::query!(
        r#"insert into workspace_v2 (workspace_name, owner_id, config_data) values ($1, $2, $3) returning workspace_id"#,
        "Personal",
        user_id,
        json!({})
    )
    .fetch_one(&ctx.db)
    .await?;

    sqlx::query!(
        r#"insert into workspace_member_v2 (workspace_id, user_id) values ($1, $2)"#,
        workspace.workspace_id,
        user_id
    )
    .execute(&ctx.db)
    .await?;

    sqlx::query!(
        r#"insert into project (project_name, team_id) values ($1, $2)"#,
        format!(r#"{}'s Personal Project"#, &req.user.username),
        team_id
    )
    .execute(&ctx.db)
    .await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "user": {
                "email": req.user.email,
                "token": AuthUser { user_id }.to_jwt(&ctx),
                "username": req.user.username,
                "userId": user_id,
            }
        }),
    }))
}

async fn login_user(
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<LoginUser>>,
) -> Result<Json<CommonResponse>> {
    let user = sqlx::query!(
        r#"
            select user_id, email, user_name, password_hash
            from "user" where email = $1
        "#,
        req.user.email,
    )
    .fetch_optional(&ctx.db)
    .await?
    .ok_or_else(|| Error::unprocessable_entity([("email", "does not exist")]))?;

    verify_password(req.user.password, user.password_hash).await?;

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "user": {
                "email": user.email,
                "token": AuthUser { user_id: user.user_id }.to_jwt(&ctx),
                "username": user.user_name,
                "userId": user.user_id,
            }
        }),
    }))
}

async fn get_current_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
) -> Result<Json<UserBody<User>>> {
    let user = sqlx::query!(
        r#"select email, user_name from "user" where user_id = $1"#,
        auth_user.user_id
    )
    .fetch_one(&ctx.db)
    .await?;

    Ok(Json(UserBody {
        user: User {
            email: user.email,
            // The spec doesn't state whether we're supposed to return the same token we were passed,
            // or generate a new one. Generating a new one is easier the way the code is structured.
            //
            // This has the side-effect of automatically refreshing the session if the frontend
            // updates its token based on this response.
            token: auth_user.to_jwt(&ctx),
            username: user.user_name,
        },
    }))
}

// https://realworld-docs.netlify.app/docs/specs/backend-specs/endpoints#update-user
// Semantically, because this route allows a partial update it should be `PATCH`, not `PUT`.
// However, we have a spec to follow so `PUT` it is.
async fn update_user(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(req): Json<UserBody<UpdateUser>>,
) -> Result<Json<UserBody<User>>> {
    if req.user == UpdateUser::default() {
        // If there's no fields to update, these two routes are effectively identical.
        return get_current_user(auth_user, ctx).await;
    }

    // WTB `Option::map_async()`
    let password_hash = if let Some(password) = req.user.password {
        Some(hash_password(password).await?)
    } else {
        None
    };

    let user = sqlx::query!(
        // This is how we do optional updates of fields without needing a separate query for each.
        // language=PostgreSQL
        r#"
            update "user"
            set email = coalesce($1, "user".email),
                user_name = coalesce($2, "user".user_name),
                password_hash = coalesce($3, "user".password_hash)
            where user_id = $4
            returning email, user_name
        "#,
        req.user.email,
        req.user.username,
        password_hash,
        auth_user.user_id
    )
    .fetch_one(&ctx.db)
    .await
    .on_constraint("user_username_key", |_| {
        Error::unprocessable_entity([("username", "username taken")])
    })
    .on_constraint("user_email_key", |_| {
        Error::unprocessable_entity([("email", "email taken")])
    })?;

    Ok(Json(UserBody {
        user: User {
            email: user.email,
            token: auth_user.to_jwt(&ctx),
            username: user.user_name,
        },
    }))
}

async fn hash_password(password: String) -> Result<String> {
    // Argon2 hashing is designed to be computationally intensive,
    // so we need to do this on a blocking thread.
    tokio::task::spawn_blocking(move || -> Result<String> {
        let salt = SaltString::generate(rand::thread_rng());
        Ok(PasswordHash::generate(Argon2::default(), password, &salt)
            .map_err(|e| anyhow::anyhow!("failed to generate password hash: {}", e))?
            .to_string())
    })
    .await
    .context("panic in generating password hash")?
}

async fn verify_password(password: String, password_hash: String) -> Result<()> {
    tokio::task::spawn_blocking(move || -> Result<()> {
        let hash = PasswordHash::new(&password_hash)
            .map_err(|e| anyhow::anyhow!("invalid password hash: {}", e))?;

        hash.verify_password(&[&Argon2::default()], password)
            .map_err(|e| match e {
                argon2::password_hash::Error::Password => Error::Unauthorized,
                _ => anyhow::anyhow!("failed to verify password hash: {}", e).into(),
            })
    })
    .await
    .context("panic in verifying password hash")?
}
