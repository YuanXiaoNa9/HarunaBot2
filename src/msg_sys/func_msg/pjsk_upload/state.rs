use super::*;
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Stage {
    AwaitingConsent,
    Sending,
    AwaitingCode,
    AwaitingWeb,
    Verifying,
}
#[derive(Clone, Debug)]
pub(super) struct Pending {
    pub(super) attempt: String,
    pub(super) provider: String,
    pub(super) stage: Stage,
    pub(super) expires: u64,
    pub(super) generation: u64,
}
#[derive(Clone)]
pub(super) struct DeleteConfirmation {
    pub(super) account: String,
    pub(super) revision: String,
    pub(super) display: String,
    pub(super) expires: u64,
}
#[derive(Clone)]
pub(super) struct ConsentIntent {
    pub(super) command: String,
    pub(super) version: String,
    pub(super) expires: u64,
    pub(super) generation: u64,
}
#[derive(Default)]
pub struct PjskState {
    pub(super) ready: AtomicBool,
    pub(super) client: OnceLock<Client>,
    pub(super) pending: Arc<DashMap<(i64, i64), Pending>>,
    pub(super) deletes: DashMap<(i64, i64), DeleteConfirmation>,
    pub(super) generation: AtomicU64,
    pub(super) consents: Arc<DashMap<(i64, i64), ConsentIntent>>,
}

impl PjskState {
    pub(super) fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }
    pub(super) fn remove_generation(&self, k: (i64, i64), generation: u64) {
        self.pending
            .remove_if(&k, |_, v| v.generation == generation);
    }
    pub(super) async fn restore(&self, c: &Client) {
        let Ok(bot) = c.config.bot_id.parse::<i64>() else {
            return;
        };
        let msg = Msg {
            self_id: bot,
            user_id: bot,
            message_id: 1,
            ..Default::default()
        };
        if let Ok(Value::Array(items)) = c
            .call(
                &msg,
                reqwest::Method::GET,
                "login-attempts/active",
                Value::Null,
            )
            .await
        {
            for item in items {
                let Some(user) = item["user_id"].as_str().and_then(|s| s.parse::<i64>().ok())
                else {
                    continue;
                };
                let v = &item["attempt"];
                if v["stage"] != "awaiting_code" {
                    continue;
                }
                let expires = v["expires"].as_u64().unwrap_or(0);
                if expires <= now() {
                    continue;
                }
                self.pending.insert(
                    (bot, user),
                    Pending {
                        attempt: v["id"].as_str().unwrap_or("").to_owned(),
                        provider: v["provider"].as_str().unwrap_or("mkcn").to_owned(),
                        stage: Stage::AwaitingCode,
                        expires,
                        generation: self.next_generation(),
                    },
                );
            }
        }
    }
    pub(super) async fn initialize(&self) {
        let path = std::env::var("PJSK_BOT_CONFIG").unwrap_or_else(|_| "pjsk_upload.json".into());
        let Some(config) = std::fs::read(path)
            .ok()
            .and_then(|raw| serde_json::from_slice::<Config>(&raw).ok())
        else {
            return;
        };
        if !config.enabled {
            return;
        }
        let Ok(url) = reqwest::Url::parse(&config.service_url) else {
            return;
        };
        if url.scheme() != "http"
            || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "::1"))
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return;
        }
        let Some(api_key) = std::fs::read_to_string(&config.api_key_file)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| s.len() >= 32)
        else {
            return;
        };
        let Ok(http) = reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(55))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        else {
            return;
        };
        let c = Client {
            config,
            http,
            api_key,
        };
        self.restore(&c).await;
        if self.client.set(c).is_err() {
            return;
        }
        self.ready.store(true, Ordering::Relaxed);
        let map = self.pending.clone();
        let intents = self.consents.clone();
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(30));
            loop {
                timer.tick().await;
                map.retain(|_, p| p.expires > now());
                intents.retain(|_, p| p.expires > now());
            }
        });
    }
}
