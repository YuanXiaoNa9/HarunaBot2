use super::*;
#[derive(Deserialize)]
pub(super) struct Config {
    pub(super) enabled: bool,
    pub(super) bot_id: String,
    pub(super) service_url: String,
    pub(super) api_key_file: String,
    pub(super) allowed_groups: Vec<String>,
}
pub(super) struct Client {
    pub(super) config: Config,
    pub(super) http: reqwest::Client,
    pub(super) api_key: String,
}
#[derive(Deserialize)]
struct Envelope {
    data: Value,
    error: Option<Fault>,
}
impl Client {
    pub(super) async fn call(
        &self,
        msg: &Msg,
        method: reqwest::Method,
        path: &str,
        body: Value,
    ) -> Result<Value, Fault> {
        let mut request = self
            .http
            .request(
                method.clone(),
                format!(
                    "{}/v1/{}",
                    self.config.service_url.trim_end_matches('/'),
                    path
                ),
            )
            .bearer_auth(&self.api_key)
            .header("X-Bot-ID", msg.self_id.to_string())
            .header("X-User-ID", msg.user_id.to_string())
            .header("X-Group-ID", msg.group_id.to_string())
            .header("X-Message-ID", msg.message_id.to_string())
            .header("X-Scene", if private(msg) { "private" } else { "group" });
        if method == reqwest::Method::POST || method == reqwest::Method::PATCH {
            request = request.json(&body);
        }
        if method == reqwest::Method::DELETE && path.starts_with("accounts/") {
            request = request.header("If-Match", body["revision"].as_str().unwrap_or(""));
        }
        let response = request.send().await.map_err(|_| Fault::unavailable())?;
        let success = response.status().is_success();
        let envelope = response
            .json::<Envelope>()
            .await
            .map_err(|_| Fault::unavailable())?;
        if let Some(f) = envelope.error {
            return Err(f);
        }
        if !success {
            return Err(Fault::unavailable());
        }
        Ok(envelope.data)
    }
}
