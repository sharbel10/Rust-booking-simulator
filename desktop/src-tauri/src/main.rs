use std::time::Duration;

use serde_json::Value;
use tauri::State;

#[tauri::command]
async fn booking_request(
    client: State<'_, reqwest::Client>,
    action: Option<Value>,
) -> Result<Value, String> {
    let request = match action {
        Some(action) => client
            .post("http://127.0.0.1:3000/booking/action")
            .json(&action),
        None => client.get("http://127.0.0.1:3000/booking"),
    };

    request
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

fn main() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .expect("could not create HTTP client");
    tauri::Builder::default()
        .manage(client)
        .invoke_handler(tauri::generate_handler![booking_request])
        .run(tauri::generate_context!())
        .expect("could not start the booking simulator");
}
