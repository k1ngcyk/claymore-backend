use crate::config::Config;
use anyhow::Context;
use axum::Router;
use serde_json::Value;
use sqlx::PgPool;
use std::{
    marker::PhantomData,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
};

mod error;
mod extractor;
mod types;

mod characters;
mod comments;
mod datadrops;
mod generators;
mod jobs;
mod projects;
mod teams;
mod users;

pub use error::{Error, ResultExt};

pub type Result<T, E = Error> = std::result::Result<T, E>;

use tower_http::trace::TraceLayer;

/// The core type through which handler functions can access common API state.
///
/// This can be accessed by adding a parameter `State<ApiContext>` to a handler function's
/// parameters.
///
/// In other projects I've passed this stuff as separate objects, e.g.
/// using a separate actix-web `Data` extractor for each of `Config`, `PgPool`, etc.
/// It just ends up being kind of annoying that way, but does have the whole
/// "pass only what you need where you need it" angle.
///
/// It may not be a bad idea if you need your API to be more modular (turn routes
/// on and off, and disable any unused extension objects) but it's really up to a
/// judgement call.
#[derive(Clone)]
pub(crate) struct ApiContext {
    config: Arc<Config>,
    db: PgPool,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(crate) struct CommonResponseData<T = Value> {
    label: String,
    value: T,
}

trait ExcludeValue {}
impl ExcludeValue for PhantomData<Value> {}
impl ExcludeValue for String {}
impl ExcludeValue for bool {}

impl<T> From<CommonResponseData<T>> for CommonResponseData<Value>
where
    T: serde::Serialize + ExcludeValue,
{
    fn from(data: CommonResponseData<T>) -> Self {
        CommonResponseData {
            label: data.label,
            value: serde_json::to_value(data.value).unwrap(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommonResponse {
    code: u16,
    message: String,
    data: Value,
}

pub async fn serve(config: Config, db: PgPool) -> anyhow::Result<()> {
    let api_context = ApiContext {
        config: Arc::new(config),
        db,
    };

    let app = api_router(api_context);

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, 8080));
    log::info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .context("error running HTTP server")
}

fn api_router(api_context: ApiContext) -> Router {
    Router::new()
        .merge(users::router())
        .merge(teams::router())
        .merge(projects::router())
        .merge(generators::router())
        .merge(jobs::router())
        .merge(characters::router())
        .merge(datadrops::router())
        .merge(comments::router())
        // Enables logging. Use `RUST_LOG=tower_http=debug`
        .layer(TraceLayer::new_for_http())
        .with_state(api_context)
}
