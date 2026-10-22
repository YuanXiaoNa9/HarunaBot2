//! Suite upload interaction only. Each process call owns its final reply.
#[cfg(not(test))]
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Instant;

#[derive(Deserialize)]
struct Config {
    enabled: bool,
    bot_id: String,
    service_url: String,
    api_key_file: String,
    allowed_groups: Vec<String>,
}
struct Client {
    config: Config,
    http: reqwest::Client,
    api_key: String,
}
#[derive(Clone, Debug, PartialEq)]
enum Stage {
    Sending,
    AwaitingCode,
    Verifying,
}
#[derive(Clone, Debug)]
struct Pending {
    attempt: String,
    provider: String,
    stage: Stage,
    expires: u64,
    generation: u64,
}
#[derive(Clone)]
struct DeleteConfirmation {
    account: String,
    expires: u64,
}
#[derive(Default)]
pub struct PjskUpload {
    ready: AtomicBool,
    client: OnceLock<Client>,
    pending: Arc<DashMap<(i64, i64), Pending>>,
    deletes: DashMap<(i64, i64), DeleteConfirmation>,
    generation: AtomicU64,
}
#[derive(Deserialize)]
struct Envelope {
    data: Value,
    error: Option<Fault>,
}
#[derive(Deserialize, Clone, Debug)]
struct Fault {
    stage: String,
    code: String,
    reason: String,
    #[serde(default)]
    http: Option<u16>,
    #[serde(default)]
    upstream: Option<String>,
    #[serde(default)]
    reply_policy: Option<String>,
}
impl Fault {
    fn unavailable() -> Self {
        Self {
            stage: "service".into(),
            code: "SERVICE_UNAVAILABLE".into(),
            reason: "服务暂时不可用；操作结果无法确认，请稍后查询任务".into(),
            http: None,
            upstream: None,
            reply_policy: None,
        }
    }
    fn display(&self) -> String {
        format!(
            "{}\n阶段：{}；代码：{}；HTTP：{}；上游码：{}",
            self.reason,
            self.stage,
            self.code,
            self.http
                .map(|n| n.to_string())
                .unwrap_or_else(|| "无".into()),
            self.upstream.as_deref().unwrap_or("无")
        )
    }
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn otp(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit())
}
fn key(msg: &Msg) -> (i64, i64) {
    (msg.self_id, msg.user_id)
}
fn command(s: &str) -> bool {
    matches!(
        s.split_whitespace().next().unwrap_or(""),
        "申请绑定"
            | "绑定账号"
            | "重新授权"
            | "取消绑定"
            | "账号列表"
            | "更新数据"
            | "默认账号"
            | "账号备注"
            | "删除账号"
            | "确认删除账号"
            | "更新状态"
            | "重传数据"
            | "上传帮助"
    )
}
#[cfg(not(test))]
async fn reply(msg: &Msg, text: String) {
    let mut rep = SendMsg::new().await;
    rep.join_reply(msg.message_id).await;
    rep.join_text(
        text.replace('&', "&amp;")
            .replace('[', "&#91;")
            .replace(']', "&#93;"),
    )
    .await;
    rep.send_msg(msg).await;
}
#[cfg(test)]
static TEST_REPLIES: std::sync::Mutex<Vec<(i64, String)>> = std::sync::Mutex::new(Vec::new());
#[cfg(test)]
async fn reply(msg: &Msg, text: String) {
    TEST_REPLIES.lock().unwrap().push((msg.message_id, text));
}
impl Client {
    async fn call(
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
            .header(
                "X-Scene",
                if msg.group_id == 0 {
                    "private"
                } else {
                    "group"
                },
            );
        if method == reqwest::Method::POST || method == reqwest::Method::PATCH {
            request = request.json(&body);
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
impl PjskUpload {
    fn next_generation(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::Relaxed) + 1
    }
    fn remove_generation(&self, k: (i64, i64), generation: u64) {
        self.pending
            .remove_if(&k, |_, v| v.generation == generation);
    }
    async fn restore(&self, c: &Client) {
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
    async fn wait_job(&self, c: &Client, msg: &Msg, mut job: Value) -> Result<Value, Fault> {
        // This future stays in process(), including after the 60-second notice.
        let start = Instant::now();
        let mut warned = false;
        let deadline = job["deadline"].as_u64().unwrap_or(now() + 300);
        loop {
            if matches!(
                job["status"].as_str(),
                Some("success" | "failed" | "cancelled")
            ) {
                return Ok(job);
            }
            if !warned && start.elapsed() >= Duration::from_secs(60) {
                reply(
                    msg,
                    "本次处理已超过60秒，仍在等待结果，请勿重复操作。".into(),
                )
                .await;
                warned = true;
            }
            if now() > deadline + 15 {
                return Err(Fault {
                    stage: "wait".into(),
                    code: "RESULT_UNKNOWN".into(),
                    reason: format!(
                        "等待结果超时，无法确认是否已上传。任务：{}，可发送 更新状态 查询。",
                        job["id"].as_str().unwrap_or("未知")
                    ),
                    http: None,
                    upstream: None,
                    reply_policy: None,
                });
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            let id = job["id"].as_str().ok_or_else(Fault::unavailable)?;
            match c
                .call(
                    msg,
                    reqwest::Method::GET,
                    &format!("jobs/{id}"),
                    Value::Null,
                )
                .await
            {
                Ok(v) => job = v,
                Err(f) => {
                    if f.code != "SERVICE_UNAVAILABLE" {
                        return Err(f);
                    }
                }
            }
        }
    }
    fn job_text(job: &Value, bind: bool) -> String {
        let mut lines = vec![];
        if bind && job["status"] == "success" {
            let id = job["items"][0]["account_id"].as_str().unwrap_or("默认");
            lines.push(format!("账号 {id} 绑定成功。绑定只保存授权。\n以后发送 更新数据 即可上传默认账号；更新数据 {id} 可指定此账号，无需每次输入手机号。"));
        } else {
            lines.push(if bind {
                "绑定未完成".into()
            } else {
                format!("更新任务：{}", job["status"].as_str().unwrap_or("未知"))
            });
            if let Some(items) = job["items"].as_array() {
                for it in items {
                    lines.push(format!(
                        "账号 {}：{}",
                        it["account_id"].as_str().unwrap_or("?"),
                        it["status"].as_str().unwrap_or("未知")
                    ));
                    if let Ok(f) = serde_json::from_value::<Fault>(it["error"].clone()) {
                        lines.push(f.display());
                    }
                }
            }
            if let Ok(f) = serde_json::from_value::<Fault>(job["error"].clone()) {
                lines.push(f.display());
            }
        }
        lines.push(format!(
            "任务：{}；耗时：{}ms；版本：{}",
            job["id"].as_str().unwrap_or("未知"),
            job["elapsed_ms"].as_i64().unwrap_or(0),
            job["version"].as_str().unwrap_or("未知")
        ));
        lines.join("\n")
    }
    async fn run(&self, c: &Client, msg: &Msg) -> Result<(), Fault> {
        // Local group check prevents any requests from unapproved groups.
        if msg.group_id != 0 && !c.config.allowed_groups.contains(&msg.group_id.to_string()) {
            return Ok(());
        }
        if msg.self_id.to_string() != c.config.bot_id {
            return Ok(());
        }
        let text = msg.raw_message.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let k = key(msg);
        if words.first() == Some(&"申请绑定") {
            c.call(msg, reqwest::Method::POST, "access/apply", json!({}))
                .await?;
            reply(
                msg,
                "已开通绑定资格。请私聊发送：绑定账号 手机号。绑定成功后，使用 更新数据 上传。"
                    .into(),
            )
            .await;
            return Ok(());
        }
        c.call(msg, reqwest::Method::GET, "access/status", Value::Null)
            .await?;
        let first = words.first().copied().unwrap_or("");
        if matches!(first, "绑定账号" | "重新授权") {
            if msg.group_id != 0 {
                reply(msg, "请私聊发送手机号和验证码。".into()).await;
                return Ok(());
            }
            let (target, phone) = if first == "重新授权" {
                (
                    words.get(1).copied().unwrap_or(""),
                    words.get(2).copied().unwrap_or(""),
                )
            } else {
                ("", words.get(1).copied().unwrap_or(""))
            };
            if phone.len() != 11
                || !phone.bytes().all(|b| b.is_ascii_digit())
                || !phone.starts_with('1')
                || (first == "重新授权" && target.is_empty())
            {
                reply(
                    msg,
                    "格式：绑定账号 手机号；或 重新授权 账号编号 手机号".into(),
                )
                .await;
                return Ok(());
            }
            let generation = self.next_generation();
            let busy = {
                use dashmap::mapref::entry::Entry;
                match self.pending.entry(k) {
                    Entry::Occupied(mut e) => {
                        if e.get().expires > now() {
                            true
                        } else {
                            e.insert(Pending {
                                attempt: String::new(),
                                provider: "mkcn".into(),
                                stage: Stage::Sending,
                                expires: now() + 300,
                                generation,
                            });
                            false
                        }
                    }
                    Entry::Vacant(e) => {
                        e.insert(Pending {
                            attempt: String::new(),
                            provider: "mkcn".into(),
                            stage: Stage::Sending,
                            expires: now() + 300,
                            generation,
                        });
                        false
                    }
                }
            };
            if busy {
                reply(msg, "已有绑定进度，请输入验证码或发送 取消绑定。".into()).await;
                return Ok(());
            }
            let result = c
                .call(
                    msg,
                    reqwest::Method::POST,
                    "login-attempts",
                    json!({"phone":phone,"provider":"mkcn","target":target}),
                )
                .await;
            match result {
                Ok(v) => {
                    let id = v["id"].as_str().unwrap_or("").to_owned();
                    let current = {
                        if let Some(mut p) = self.pending.get_mut(&k) {
                            if p.generation == generation {
                                p.attempt = id.clone();
                                p.expires = v["expires"].as_u64().unwrap_or(now() + 300);
                                p.stage = Stage::AwaitingCode;
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    };
                    if current {
                        reply(
                            msg,
                            "验证码已发送，请在5分钟内直接回复六位验证码。取消请发送 取消绑定。"
                                .into(),
                        )
                        .await;
                    } else {
                        let _ = c
                            .call(
                                msg,
                                reqwest::Method::DELETE,
                                &format!("login-attempts/{id}"),
                                Value::Null,
                            )
                            .await;
                    }
                }
                Err(f) => {
                    // Recover a successful send whose HTTP reply was lost, without resending SMS.
                    let active = c
                        .call(
                            msg,
                            reqwest::Method::GET,
                            "login-attempts/active",
                            Value::Null,
                        )
                        .await;
                    if let Ok(Value::Array(items)) = active {
                        for item in items {
                            let a = &item["attempt"];
                            if a["stage"] == "awaiting_code" {
                                if let Some(mut current) = self.pending.get_mut(&k) {
                                    if current.generation == generation {
                                        current.attempt = a["id"].as_str().unwrap_or("").into();
                                        current.expires = a["expires"].as_u64().unwrap_or(now());
                                        current.stage = Stage::AwaitingCode;
                                    }
                                }
                                reply(msg,"检测到有效的验证码进度，未重新发送短信。请继续输入验证码，或发送 取消绑定。".into()).await;
                                return Ok(());
                            }
                        }
                    }
                    self.remove_generation(k, generation);
                    return Err(f);
                }
            };
            return Ok(());
        }
        if first == "取消绑定" {
            if msg.group_id != 0 {
                reply(msg, "请在私聊取消绑定。".into()).await;
                return Ok(());
            }
            let p = self.pending.remove(&k).map(|(_, p)| p);
            if let Some(p) = p {
                if !p.attempt.is_empty() {
                    c.call(
                        msg,
                        reqwest::Method::DELETE,
                        &format!("login-attempts/{}", p.attempt),
                        Value::Null,
                    )
                    .await?;
                }
                reply(msg, "本次绑定已取消。".into()).await;
            } else {
                // A Bot restart or lost begin reply can leave only the backend state.
                let active = c
                    .call(
                        msg,
                        reqwest::Method::GET,
                        "login-attempts/active",
                        Value::Null,
                    )
                    .await?;
                let mut cancelled = false;
                for item in active.as_array().into_iter().flatten() {
                    if let Some(id) = item["attempt"]["id"].as_str() {
                        c.call(
                            msg,
                            reqwest::Method::DELETE,
                            &format!("login-attempts/{id}"),
                            Value::Null,
                        )
                        .await?;
                        cancelled = true;
                    }
                }
                reply(
                    msg,
                    if cancelled {
                        "本次绑定已取消。".into()
                    } else {
                        "没有进行中的绑定。".into()
                    },
                )
                .await;
            }
            return Ok(());
        }
        if msg.group_id == 0 && otp(text) {
            let pending = {
                self.pending.get_mut(&k).and_then(|mut p| {
                    if p.stage == Stage::AwaitingCode && p.expires > now() {
                        p.stage = Stage::Verifying;
                        Some(p.clone())
                    } else {
                        None
                    }
                })
            };
            let Some(p) = pending else {
                reply(msg, "验证码正在处理或绑定已过期，请勿重复发送。".into()).await;
                return Ok(());
            };
            let _provider = &p.provider;
            let result = async {
                let j = c
                    .call(
                        msg,
                        reqwest::Method::POST,
                        &format!("login-attempts/{}/verify", p.attempt),
                        json!({"code":text}),
                    )
                    .await?;
                self.wait_job(c, msg, j).await
            }
            .await;
            let success = result.as_ref().is_ok_and(|j| j["status"] == "success");
            if success {
                self.remove_generation(k, p.generation);
            } else {
                // Ask the service whether another code attempt remains. No OTP is retained.
                let active = c
                    .call(
                        msg,
                        reqwest::Method::GET,
                        "login-attempts/active",
                        Value::Null,
                    )
                    .await;
                let still = active
                    .ok()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .any(|v| {
                        v["attempt"]["id"] == p.attempt && v["attempt"]["stage"] == "awaiting_code"
                    });
                if still {
                    if let Some(mut current) = self.pending.get_mut(&k) {
                        if current.generation == p.generation {
                            current.stage = Stage::AwaitingCode;
                        }
                    }
                } else {
                    self.remove_generation(k, p.generation);
                }
            }
            reply(msg, Self::job_text(&result?, true)).await;
            return Ok(());
        }
        match first {
            "账号列表" => {
                let xs = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let mut lines = vec![];
                for a in xs.as_array().into_iter().flatten() {
                    lines.push(format!(
                        "{}{}：{} {}（游戏ID {}）",
                        a["id"].as_str().unwrap_or("?"),
                        if a["default"] == true {
                            " [默认]"
                        } else {
                            ""
                        },
                        a["phone"].as_str().unwrap_or(""),
                        a["remark"].as_str().unwrap_or(""),
                        a["game_id"].as_str().unwrap_or("")
                    ));
                }
                reply(
                    msg,
                    if lines.is_empty() {
                        "尚未绑定账号。请私聊：绑定账号 手机号".into()
                    } else {
                        lines.join("\n")
                    },
                )
                .await;
            }
            "默认账号" | "账号备注" => {
                let Some(id) = words.get(1) else {
                    reply(msg, "请填写账号编号，可用 账号列表 查看。".into()).await;
                    return Ok(());
                };
                let body = if first == "默认账号" {
                    json!({"default":true})
                } else {
                    json!({"remark":words.get(2..).unwrap_or(&[]).join(" ")})
                };
                c.call(msg, reqwest::Method::PATCH, &format!("accounts/{id}"), body)
                    .await?;
                reply(msg, "账号设置已保存。".into()).await;
            }
            "删除账号" => {
                let Some(id) = words.get(1) else {
                    reply(msg, "格式：删除账号 编号".into()).await;
                    return Ok(());
                };
                self.deletes.insert(
                    k,
                    DeleteConfirmation {
                        account: id.to_string(),
                        expires: now() + 60,
                    },
                );
                reply(msg,format!("这只会删除本地账号和授权，不会删除游戏账号或网站数据。60秒内发送 确认删除账号 {id} 确认。默认账号被删除后将选取剩余最早的账号。")).await;
            }
            "确认删除账号" => {
                let confirmed = self.deletes.remove(&k).is_some_and(|(_, p)| {
                    p.expires > now() && words.get(1).is_some_and(|id| *id == p.account)
                });
                if !confirmed {
                    reply(msg, "没有匹配的删除确认，或确认已过期。".into()).await;
                    return Ok(());
                }
                let id = words[1];
                c.call(
                    msg,
                    reqwest::Method::DELETE,
                    &format!("accounts/{id}"),
                    Value::Null,
                )
                .await?;
                reply(
                    msg,
                    "本地账号已删除。可发送 账号列表 查看当前默认账号。".into(),
                )
                .await;
            }
            "更新数据" => {
                let selector = words.get(1).copied().unwrap_or("");
                let j = c
                    .call(
                        msg,
                        reqwest::Method::POST,
                        "sync-jobs",
                        json!({"selector":selector}),
                    )
                    .await?;
                let done = self.wait_job(c, msg, j).await?;
                reply(msg, Self::job_text(&done, false)).await;
            }
            "更新状态" => {
                let j = c
                    .call(msg, reqwest::Method::GET, "jobs/latest", Value::Null)
                    .await?;
                reply(msg, Self::job_text(&j, false)).await;
            }
            "重传数据" => {
                let j = c
                    .call(msg, reqwest::Method::GET, "jobs/latest", Value::Null)
                    .await?;
                let id = j["id"].as_str().ok_or_else(Fault::unavailable)?;
                let j = c
                    .call(
                        msg,
                        reqwest::Method::POST,
                        &format!("jobs/{id}/retry-upload"),
                        json!({}),
                    )
                    .await?;
                let done = self.wait_job(c, msg, j).await?;
                reply(msg, Self::job_text(&done, false)).await;
            }
            "上传帮助" => {
                reply(msg,"先在开放群 申请绑定，再私聊 绑定账号 手机号，随后直接回复验证码。\n绑定后手动 更新数据；更新数据 编号；更新数据 全部。\n账号列表；默认账号 编号；账号备注 编号 备注；删除账号 编号；重新授权 编号 手机号；取消绑定；更新状态；重传数据。\n重传数据只重传24小时内的缓存，不重新获取游戏数据。".into()).await;
            }
            _ => {}
        }
        Ok(())
    }
}
#[async_trait]
impl FnHandler for PjskUpload {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.post_type != "message" {
            return false;
        }
        if command(msg.raw_message.trim()) {
            return true;
        }
        msg.group_id == 0
            && otp(msg.raw_message.trim())
            && self
                .pending
                .get(&key(msg))
                .is_some_and(|p| p.expires > now())
    }
    async fn process(&self, msg: &Msg) -> Result<(), anyhow::Error> {
        if let Some(c) = self.client.get() {
            if let Err(f) = self.run(c, msg).await {
                if f.reply_policy.as_deref() != Some("silent") {
                    reply(msg, f.display()).await;
                }
            }
        }
        Ok(()) // Expected failures are handled here, avoiding the framework's raw error reply.
    }
    async fn init(&self) {
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
        tokio::spawn(async move {
            let mut timer = tokio::time::interval(Duration::from_secs(30));
            loop {
                timer.tick().await;
                map.retain(|_, p| p.expires > now());
            }
        });
    }
    async fn status(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
    async fn help_visible(&self, msg: &Msg) -> bool {
        self.client.get().is_some_and(|c| {
            c.config.enabled
                && msg.self_id.to_string() == c.config.bot_id
                && (msg.group_id == 0
                    || c.config.allowed_groups.contains(&msg.group_id.to_string()))
        })
    }
    async fn help(&self, _: &str) -> String {
        "游戏数据上传：在开放群申请绑定，再私聊绑定账号。完整指令：上传帮助。".into()
    }
    async fn name(&self) -> String {
        "PJSK数据上传".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn help_visibility_uses_bot_and_group_allowlist() {
        let h = PjskUpload::default();
        assert!(
            h.client
                .set(Client {
                    config: Config {
                        enabled: true,
                        bot_id: "1".into(),
                        service_url: "http://127.0.0.1:8080".into(),
                        api_key_file: String::new(),
                        allowed_groups: vec!["10".into()]
                    },
                    http: reqwest::Client::new(),
                    api_key: "test".into()
                })
                .is_ok()
        );
        let mut msg = Msg {
            self_id: 1,
            group_id: 99,
            ..Default::default()
        };
        assert!(!h.help_visible(&msg).await);
        msg.group_id = 10;
        assert!(h.help_visible(&msg).await);
        msg.group_id = 0;
        assert!(h.help_visible(&msg).await);
        msg.self_id = 2;
        assert!(!h.help_visible(&msg).await);
    }
    #[test]
    fn only_exact_ascii_otp() {
        assert!(otp("123456"));
        for s in ["12345", "１２３４５６", "a123456", "1234567"] {
            assert!(!otp(s));
        }
    }
    #[test]
    fn generation_cannot_remove_new_flow() {
        let h = PjskUpload::default();
        let k = (1, 2);
        h.pending.insert(
            k,
            Pending {
                attempt: "new".into(),
                provider: "mkcn".into(),
                stage: Stage::AwaitingCode,
                expires: now() + 300,
                generation: 2,
            },
        );
        h.remove_generation(k, 1);
        assert_eq!(h.pending.get(&k).unwrap().attempt, "new");
        h.remove_generation(k, 2);
        assert!(!h.pending.contains_key(&k));
    }
    #[tokio::test]
    async fn numeric_messages_need_private_pending_flow() {
        let h = PjskUpload::default();
        let mut msg = Msg {
            post_type: "message".into(),
            self_id: 1,
            user_id: 2,
            raw_message: "123456".into(),
            ..Default::default()
        };
        assert!(!h.matches(&msg).await);
        h.pending.insert(
            (1, 2),
            Pending {
                attempt: "a".into(),
                provider: "mkcn".into(),
                stage: Stage::AwaitingCode,
                expires: now() + 300,
                generation: 1,
            },
        );
        assert!(h.matches(&msg).await);
        msg.group_id = 3;
        assert!(!h.matches(&msg).await);
    }
    #[tokio::test(start_paused = true)]
    async fn slow_notice_once_then_same_future_returns_final_result() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let completed = Arc::new(AtomicBool::new(false));
        let server_complete = completed.clone();
        let server = tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let done = server_complete.load(Ordering::Relaxed);
                tokio::spawn(async move {
                    let mut buf = [0; 4096];
                    let _ = stream.read(&mut buf).await;
                    let body=json!({"data":{"id":"slow-job","status":if done{"success"}else{"running"},"deadline":now()+300},"error":null}).to_string();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await.unwrap();
                });
            }
        });
        let client = Client {
            config: Config {
                enabled: true,
                bot_id: "1".into(),
                service_url: format!("http://{address}"),
                api_key_file: String::new(),
                allowed_groups: vec![],
            },
            http: reqwest::Client::builder().no_proxy().build().unwrap(),
            api_key: "test".into(),
        };
        let h = PjskUpload::default();
        let msg = Msg {
            self_id: 1,
            user_id: 2,
            message_id: 987654,
            ..Default::default()
        };
        let deadline = now() + 300;
        let operation = tokio::spawn(async move {
            h.wait_job(
                &client,
                &msg,
                json!({"id":"slow-job","status":"running","deadline":deadline}),
            )
            .await
        });
        for _ in 0..200 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(Duration::from_secs(61)).await;
        for _ in 0..2000 {
            tokio::task::yield_now().await;
            if TEST_REPLIES
                .lock()
                .unwrap()
                .iter()
                .any(|(id, _)| *id == 987654)
            {
                break;
            }
        }
        assert!(
            !operation.is_finished(),
            "handler returned after its slow notice"
        );
        assert_eq!(
            TEST_REPLIES
                .lock()
                .unwrap()
                .iter()
                .filter(|(id, _)| *id == 987654)
                .count(),
            1
        );
        completed.store(true, Ordering::Relaxed);
        tokio::time::advance(Duration::from_secs(10)).await;
        let result = operation.await.unwrap().unwrap();
        assert_eq!(result["status"], "success");
        assert_eq!(
            TEST_REPLIES
                .lock()
                .unwrap()
                .iter()
                .filter(|(id, _)| *id == 987654)
                .count(),
            1
        );
        server.abort();
    }
}
