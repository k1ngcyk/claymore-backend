use std::collections::HashMap;

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

pub(crate) fn router() -> Router<ApiContext> {
    Router::new()
        .route("/v2/invoice/pay", post(handle_pay_invoice))
        .route("/v2/invoice/test", post(handle_test))
        .route("/v2/invoice/notify", get(handle_invoice_notify))
        .route("/v2/invoice/return", get(handle_invoice_return))
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct InvoiceBody<T> {
    invoice: T,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct InvoicePayRequest {
    invoice_id: Uuid,
    device: String,
}

async fn handle_pay_invoice(
    auth_user: AuthUser,
    ctx: State<ApiContext>,
    Json(body): Json<InvoiceBody<InvoicePayRequest>>,
) -> Result<Json<CommonResponse>> {
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_test() -> Result<Json<CommonResponse>> {
    let mut pramas = HashMap::new();
    pramas.insert("pid".to_string(), "1059".to_string());
    pramas.insert("type".to_string(), "alipay".to_string());
    pramas.insert("out_trade_no".to_string(), "20210708123456789".to_string());
    pramas.insert("name".to_string(), "测试商品".to_string());
    pramas.insert("money".to_string(), "1.00".to_string());
    pramas.insert(
        "notify_url".to_string(),
        "https://claymore-dev.fluxusapis.com/v2/invoice/notify".to_string(),
    );
    pramas.insert(
        "return_url".to_string(),
        "https://claymore-dev.fluxusapis.com/v2/invoice/return".to_string(),
    );
    let sign = get_sign(pramas.clone());
    pramas.insert("sign".to_string(), sign);
    pramas.insert("sign_type".to_string(), "MD5".to_string());

    let client = reqwest::Client::builder().build().unwrap();

    let request = client
        .request(
            reqwest::Method::POST,
            format!("https://pay.ucany.net/pay/apisubmit"),
        )
        .form(&pramas);

    let response = request.send().await.unwrap();
    let body = response.json::<serde_json::Value>().await.unwrap();

    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({
            "body": body,
        }),
    }))
}

async fn handle_invoice_notify(
    Query(req): Query<HashMap<String, String>>,
) -> Result<Json<CommonResponse>> {
    log::info!("handle_invoice_notify: {:?}", req);
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

async fn handle_invoice_return(
    Query(req): Query<HashMap<String, String>>,
) -> Result<Json<CommonResponse>> {
    log::info!("handle_invoice_return: {:?}", req);
    Ok(Json(CommonResponse {
        code: 200,
        message: "success".to_string(),
        data: json!({}),
    }))
}

fn get_sign(params: HashMap<String, String>) -> String {
    let mut keys: Vec<_> = params.keys().collect();
    keys.sort();
    let param_string = keys
        .into_iter()
        .map(|key| format!("{}={}", key, params.get(key).unwrap()))
        .collect::<Vec<String>>()
        .join("&");
    let combined_string = format!("{}{}", param_string, "DnpewBiCFrJq15Vp0kizttbzS4GbKnEv");
    format!("{:x}", md5::compute(combined_string))
}
