use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn invoke(command: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Debug, Deserialize)]
pub struct Snapshot {
    pub state: String,
    pub milestones: Vec<String>,
    pub escrow: String,
    pub expires_at_hour: Option<u64>,
    pub hour: u64,
    pub error: Option<String>,
}

#[derive(Serialize)]
struct CommandArgs {
    action: Option<Value>,
}

pub async fn request(action: Option<Value>) -> Result<Snapshot, ()> {
    if js_sys::Reflect::has(&js_sys::global(), &JsValue::from_str("__TAURI__")).unwrap_or(false) {
        let args = CommandArgs { action }
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .map_err(|_| ())?;
        let response = invoke("booking_request", args).await.map_err(|_| ())?;
        return serde_wasm_bindgen::from_value(response).map_err(|_| ());
    }

    let response = match action {
        Some(action) => {
            Request::post("/booking/action")
                .json(&action)
                .map_err(|_| ())?
                .send()
                .await
        }
        None => Request::get("/booking").send().await,
    }
    .map_err(|_| ())?;

    // The API includes the current booking and the rejection reason on conflict.
    response.json().await.map_err(|_| ())
}
