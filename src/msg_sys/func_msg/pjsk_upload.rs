mod help;
// Suite upload interaction only. Each process call owns its final reply.
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
    AwaitingConsent,
    Sending,
    AwaitingCode,
    AwaitingWeb,
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
    revision: String,
    display: String,
    expires: u64,
}
#[derive(Clone)]
struct ConsentIntent {
    command: String,
    version: String,
    expires: u64,
    generation: u64,
}
#[derive(Default)]
pub struct PjskUpload {
    ready: AtomicBool,
    client: OnceLock<Client>,
    pending: Arc<DashMap<(i64, i64), Pending>>,
    deletes: DashMap<(i64, i64), DeleteConfirmation>,
    generation: AtomicU64,
    consents: Arc<DashMap<(i64, i64), ConsentIntent>>,
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
            "操作未完成\n\n{}\n\n{}\n\n诊断信息\n阶段：{}\n代码：{}{}{}",
            self.reason,
            match self.code.as_str() {
                "CONSENT_REQUIRED" => "请私聊重新发送绑定或更新指令，阅读并同意当前条款。",
                "NO_ACCOUNT" | "NO_TARGET" =>
                    "查看账号：账号列表\n绑定说明：/help PJSK数据上传-绑定账号",
                "WRONG_CODE" => "请核对短信验证码后重新发送；不要连续申请短信。",
                "SERVICE_UNAVAILABLE" | "RESULT_UNKNOWN" =>
                    "结果可能尚未确认，请稍后发送 更新状态，勿连续重试。",
                _ => "请根据提示处理；需要帮助：/help PJSK数据上传",
            },
            self.stage,
            self.code,
            self.http
                .map(|n| format!("\nHTTP：{n}"))
                .unwrap_or_default(),
            self.upstream
                .as_ref()
                .map(|s| format!("\n上游码：{s}"))
                .unwrap_or_default()
        )
    }
}
fn number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
fn line(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}
fn account_block(a: &Value) -> String {
    let id = a["id"]
        .as_str()
        .or_else(|| a["account_id"].as_str())
        .unwrap_or("?");
    let name = a["nickname"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("暂未获取");
    let provider = match a["provider"].as_str() {
        Some("mkcn") => "手机号",
        Some("mkcn_taptap") => "TapTap",
        Some("mkcn_bilibili") => "B站",
        _ => "未知",
    };
    let mut text = format!(
        "编号：{}{}\n游戏昵称：{}\nUID：{}\n登录方式：{}",
        line(id),
        if a["default"] == true {
            "（默认）"
        } else {
            ""
        },
        line(name),
        line(a["game_id"].as_str().unwrap_or("暂未获取")),
        provider
    );
    if let Some(remark) = a["remark"].as_str().filter(|s| !s.is_empty()) {
        text.push_str(&format!("\n备注：{}", line(remark)));
    }
    text
}
fn format_error(words: &[&str]) -> Option<String> {
    let first = words.first().copied().unwrap_or("");
    let phone = |s: &str| s.len() == 11 && s.starts_with('1') && number(s);
    let (valid, usage) = match first {
        "绑定账号" => (
            words.len() == 2 && (phone(words[1]) || words[1].eq_ignore_ascii_case("taptap")),
            "绑定账号 手机号\n或：绑定账号 TapTap",
        ),
        "重新授权" => (
            words.len() == 3
                && number(words[1])
                && (phone(words[2]) || words[2].eq_ignore_ascii_case("taptap")),
            "重新授权 编号 手机号\n或：重新授权 编号 TapTap",
        ),
        "解除绑定" | "确认解除绑定" | "默认账号" => (
            words.len() == 2 && number(words[1]),
            "指令后填写一个账号编号；先发送 账号列表 查看",
        ),
        "账号备注" => (
            words.len() >= 2 && number(words[1]),
            "账号备注 编号 备注内容",
        ),
        "更新数据" => (
            words.len() == 1
                || (words.len() == 2 && (number(words[1]) || matches!(words[1], "全部" | "all"))),
            "更新数据\n或：更新数据 编号\n或：更新数据 全部",
        ),
        "申请绑定" | "账号列表" | "更新状态" | "重传数据" | "同意条款" | "终止当前绑定" => {
            (words.len() == 1, "本指令无需填写其他内容")
        }
        _ if first.eq_ignore_ascii_case("账号绑定taptap")
            || first.eq_ignore_ascii_case("绑定taptap") =>
        {
            (words.len() == 1, "绑定账号 TapTap")
        }
        _ => return None,
    };
    if valid {
        None
    } else {
        Some(format!(
            "指令格式不正确\n\n{usage}\n\n详细说明：/help PJSK数据上传"
        ))
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
fn private(msg: &Msg) -> bool {
    msg.message_type == "private" || (msg.message_type.is_empty() && msg.group_id == 0)
}
fn key(msg: &Msg) -> (i64, i64) {
    (msg.self_id, msg.user_id)
}
fn command(s: &str) -> bool {
    matches!(
        s.split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "账号绑定taptap"
            | "绑定taptap"
            | "申请绑定"
            | "绑定账号"
            | "重新授权"
            | "终止当前绑定"
            | "账号列表"
            | "更新数据"
            | "默认账号"
            | "账号备注"
            | "解除绑定"
            | "确认解除绑定"
            | "更新状态"
            | "重传数据"
            | "同意条款"
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
    if private(msg) {
        rep.send_private_msg(msg).await;
    } else {
        rep.send_msg(msg).await;
    }
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
impl PjskUpload {
    async fn ensure_consent(&self, c: &Client, msg: &Msg, command: &str) -> Result<bool, Fault> {
        let terms = c
            .call(msg, reqwest::Method::GET, "terms", Value::Null)
            .await?;
        if terms["accepted"] == true {
            return Ok(true);
        }
        if !private(msg) {
            reply(
                msg,
                "需要确认使用条款。\n\n请私聊 Bot 或使用群临时会话，重新发送本次指令。".into(),
            )
            .await;
            return Ok(false);
        }
        let version = terms["version"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(Fault::unavailable)?
            .to_owned();
        let body = terms["text"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(Fault::unavailable)?
            .to_owned();
        let k = key(msg);
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
                            provider: String::new(),
                            stage: Stage::AwaitingConsent,
                            expires: now() + 600,
                            generation,
                        });
                        false
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(Pending {
                        attempt: String::new(),
                        provider: String::new(),
                        stage: Stage::AwaitingConsent,
                        expires: now() + 600,
                        generation,
                    });
                    false
                }
            }
        };
        if busy {
            reply(
                msg,
                "已有待处理流程。\n\n请先完成；如需放弃，请发送：终止当前绑定".into(),
            )
            .await;
            return Ok(false);
        }
        self.consents.insert(
            k,
            ConsentIntent {
                command: command.into(),
                version: version.clone(),
                expires: now() + 600,
                generation,
            },
        );
        reply(msg,format!("{}\n\n条款版本：{}\n\n阅读并同意：同意条款\n不同意并终止：终止当前绑定\n\n请在 10 分钟内选择；确认前不会发送短信或申请授权链接。",body,version)).await;
        Ok(false)
    }
    async fn accept_consent(&self, c: &Client, msg: &Msg) -> Result<(), Fault> {
        if !private(msg) {
            reply(msg, "请在私聊或群临时会话中确认条款。".into()).await;
            return Ok(());
        }
        let k = key(msg);
        let intent = self
            .consents
            .get(&k)
            .filter(|v| v.expires > now())
            .map(|v| v.clone());
        let Some(intent) = intent else {
            reply(
                msg,
                "没有等待确认的操作，或确认已过期。\n\n请重新发送绑定或更新指令，阅读当前条款。"
                    .into(),
            )
            .await;
            return Ok(());
        };
        let claimed = self.pending.get_mut(&k).is_some_and(|mut p| {
            if p.generation == intent.generation
                && p.stage == Stage::AwaitingConsent
                && p.expires > now()
            {
                p.stage = Stage::Sending;
                true
            } else {
                false
            }
        });
        if !claimed {
            reply(msg, "本次确认已在处理或已终止，请勿重复发送。".into()).await;
            return Ok(());
        }
        if let Err(f) = c
            .call(
                msg,
                reqwest::Method::POST,
                "consents",
                json!({"version":intent.version}),
            )
            .await
        {
            if f.code == "TERMS_CHANGED" {
                self.remove_generation(k, intent.generation);
                self.consents
                    .remove_if(&k, |_, v| v.generation == intent.generation);
                self.ensure_consent(c, msg, &intent.command).await?;
                return Ok(());
            }
            if let Some(mut p) = self.pending.get_mut(&k) {
                if p.generation == intent.generation {
                    p.stage = Stage::AwaitingConsent;
                }
            }
            return Err(f);
        }
        self.consents
            .remove_if(&k, |_, v| v.generation == intent.generation);
        Box::pin(self.run_text(c, msg, &intent.command, Some(intent.generation))).await
    }
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
                    "仍在处理中\n\n已等待超过 60 秒，请勿重复发送指令。\n处理结束后会回复结果。"
                        .into(),
                )
                .await;
                warned = true;
            }
            if now() > deadline + 15 {
                return Err(Fault {
                    stage: "wait".into(),
                    code: "RESULT_UNKNOWN".into(),
                    reason: format!(
                        "等待结果超时，当前操作结果尚未确认。\n任务：{}\n\n请发送 更新状态 查询，勿重复操作。",
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
        let bind = bind || job["kind"] == "bind";
        let status = job["status"].as_str().unwrap_or("unknown");
        let label = match status {
            "success" => {
                if bind {
                    "绑定成功"
                } else {
                    "更新成功"
                }
            }
            "running" => "正在处理",
            "queued" => "等待处理",
            "cancelled" => "操作已终止",
            _ => {
                if bind {
                    "绑定失败"
                } else {
                    "更新未完成"
                }
            }
        };
        let mut blocks = vec![label.to_string()];
        for item in job["items"].as_array().into_iter().flatten() {
            let mut block = account_block(item);
            if status != "success" {
                block.push_str(&format!(
                    "\n结果：{}",
                    match item["status"].as_str() {
                        Some("success") => "成功",
                        Some("queued") => "等待处理",
                        Some("running") => "正在处理",
                        _ => "未完成",
                    }
                ));
            }
            if let Ok(f) = serde_json::from_value::<Fault>(item["error"].clone()) {
                block.push_str(&format!("\n{}", f.display()));
            }
            blocks.push(block);
        }
        if let Ok(f) = serde_json::from_value::<Fault>(job["error"].clone()) {
            blocks.push(f.display());
        }
        if status == "success" {
            blocks.push(if bind{format!("已核对游戏资料并保存授权，本次未上传。\n\n更新默认账号：更新数据\n更新本账号：更新数据 {}\n以后更新无需每次重新输入手机号或验证码。",job["items"][0]["account_id"].as_str().unwrap_or("1"))}else{"数据已提交到 Sakura 公共数据服务。\n再次更新：更新数据".into()});
        } else {
            blocks.push(if bind{"本次未保存新授权；如为重新授权，本次操作未覆盖旧凭证。\n查看账号：账号列表\n重新绑定：/help PJSK数据上传-绑定账号".into()}else{"请先发送 更新状态 查询结果，再决定是否重试。".into()});
            blocks.push(format!(
                "诊断任务：{}\n服务版本：{}",
                job["id"].as_str().unwrap_or("未知"),
                job["version"].as_str().unwrap_or("未知")
            ));
        }
        blocks.join("\n\n")
    }
    async fn web_bind(&self, c: &Client, msg: &Msg, target: &str) -> Result<(), Fault> {
        let k = key(msg);
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
                            provider: "mkcn_taptap".into(),
                            stage: Stage::Sending,
                            expires: now() + 360,
                            generation,
                        });
                        false
                    }
                }
                Entry::Vacant(e) => {
                    e.insert(Pending {
                        attempt: String::new(),
                        provider: "mkcn_taptap".into(),
                        stage: Stage::Sending,
                        expires: now() + 360,
                        generation,
                    });
                    false
                }
            }
        };
        if busy {
            reply(msg, "已有绑定进度，请先完成或发送：终止当前绑定。".into()).await;
            return Ok(());
        }
        let result = self.web_flow(c, msg, generation, target).await;
        // Errors/cancellation never leave orphaned authorization flows behind.
        let pending = self
            .pending
            .get(&k)
            .filter(|p| p.generation == generation)
            .map(|p| p.attempt.clone());
        self.remove_generation(k, generation);
        if result.is_err() {
            if let Some(id) = pending.filter(|s| !s.is_empty()) {
                let _ = c
                    .call(
                        msg,
                        reqwest::Method::DELETE,
                        &format!("web-login-attempts/{id}"),
                        Value::Null,
                    )
                    .await;
            }
        }
        result
    }
    async fn web_flow(
        &self,
        c: &Client,
        msg: &Msg,
        generation: u64,
        target: &str,
    ) -> Result<(), Fault> {
        let attempt = c
            .call(
                msg,
                reqwest::Method::POST,
                "web-login-attempts",
                json!({"provider":"mkcn_taptap","target":target}),
            )
            .await?;
        let id = attempt["id"]
            .as_str()
            .ok_or_else(Fault::unavailable)?
            .to_owned();
        let expires = attempt["expires"].as_u64().ok_or_else(Fault::unavailable)?;
        let login = attempt["login_url"]
            .as_str()
            .ok_or_else(Fault::unavailable)?;
        let authorize = attempt["authorize_url"]
            .as_str()
            .ok_or_else(Fault::unavailable)?;
        let active = {
            if let Some(mut p) = self.pending.get_mut(&key(msg)) {
                if p.generation == generation {
                    p.attempt = id.clone();
                    p.stage = Stage::AwaitingWeb;
                    p.expires = expires + 15;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        };
        if !active {
            let _ = c
                .call(
                    msg,
                    reqwest::Method::DELETE,
                    &format!("web-login-attempts/{id}"),
                    Value::Null,
                )
                .await;
            return Ok(());
        }
        reply(msg,format!("TapTap 账号绑定\n\n① 先登录\n{login}\n\n② 登录完成后，再打开授权链接并确认\n{authorize}\n\n请直接在手机 QQ 内打开，依次操作两条链接，并使用同一个内置网页窗口。无需安装 QQ浏览器软件。授权链接约5分钟有效。\n确认授权后网页可能持续加载；Bot 仍会继续核对账号资料。\n收到“绑定成功”后即可关闭网页，不必重复点击。\n超时后可重新发送 绑定账号 TapTap；取消请发送 终止当前绑定。\n绑定会读取游戏资料核对昵称和 UID，不会自动上传。")).await;
        let mut interval = attempt["interval"].as_u64().unwrap_or(2).clamp(2, 60);
        while now() < expires {
            if !self
                .pending
                .get(&key(msg))
                .is_some_and(|p| p.generation == generation)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(interval)).await;
            let value = c
                .call(
                    msg,
                    reqwest::Method::GET,
                    &format!("web-login-attempts/{id}"),
                    Value::Null,
                )
                .await?;
            if value.get("job").is_some() {
                let job = self.wait_job(c, msg, value["job"].clone()).await?;
                reply(msg, Self::job_text(&job, true)).await;
                return Ok(());
            }
            interval = value["interval"].as_u64().unwrap_or(interval).clamp(2, 60);
        }
        let _ = c
            .call(
                msg,
                reqwest::Method::DELETE,
                &format!("web-login-attempts/{id}"),
                Value::Null,
            )
            .await;
        reply(msg,"TapTap 授权等待已到期，本次未确认绑定成功。可查看 账号列表；尚未绑定请重新发送 绑定账号 TapTap 获取新链接。无需反复点击旧链接。".into()).await;
        Ok(())
    }

    async fn run(&self, c: &Client, msg: &Msg) -> Result<(), Fault> {
        self.run_text(c, msg, msg.raw_message.trim(), None).await
    }
    async fn run_text(
        &self,
        c: &Client,
        msg: &Msg,
        input: &str,
        resume: Option<u64>,
    ) -> Result<(), Fault> {
        // Local group check prevents any requests from unapproved groups.
        if !private(msg) && !c.config.allowed_groups.contains(&msg.group_id.to_string()) {
            return Ok(());
        }
        if msg.self_id.to_string() != c.config.bot_id {
            return Ok(());
        }
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let k = key(msg);
        if let Some(text) = format_error(&words) {
            reply(msg, text).await;
            return Ok(());
        }
        if words.first() == Some(&"申请绑定") {
            c.call(msg, reqwest::Method::POST, "access/apply", json!({}))
                .await?;
            reply(
                msg,
                "已取得绑定资格\n\n接下来请私聊 Bot 或使用群临时会话。\n手机号绑定：绑定账号 手机号\nTapTap 绑定：绑定账号 TapTap\n\n首次操作需同意条款；绑定成功后再手动更新。\n详细帮助：/help PJSK数据上传"
                    .into(),
            )
            .await;
            return Ok(());
        }
        c.call(msg, reqwest::Method::GET, "access/status", Value::Null)
            .await?;
        if let Some(g) = resume {
            let active = self
                .pending
                .get(&k)
                .is_some_and(|p| p.generation == g && p.expires > now());
            if !active {
                reply(msg, "操作已终止，未继续发起登录。".into()).await;
                return Ok(());
            }
            self.remove_generation(k, g);
        }
        let first = words.first().copied().unwrap_or("");
        if first == "同意条款" {
            return self.accept_consent(c, msg).await;
        }
        if matches!(first, "更新数据" | "重传数据") {
            let accounts = c
                .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                .await?;
            if accounts.as_array().is_none_or(|xs| xs.is_empty()) {
                reply(msg,"尚未绑定账号，暂时无法更新。\n\n请先私聊绑定账号：\n手机号：绑定账号 手机号\nTapTap：绑定账号 TapTap\n\n详细说明：/help PJSK数据上传-绑定账号".into()).await;
                return Ok(());
            }
        }
        let login = matches!(first, "绑定账号" | "重新授权")
            || first.eq_ignore_ascii_case("账号绑定taptap")
            || first.eq_ignore_ascii_case("绑定taptap");
        if (login || matches!(first, "更新数据" | "重传数据"))
            && resume.is_none()
            && !self.ensure_consent(c, msg, text).await?
        {
            return Ok(());
        }
        if first.eq_ignore_ascii_case("账号绑定taptap")
            || first.eq_ignore_ascii_case("绑定taptap")
            || (first == "重新授权"
                && words
                    .get(2)
                    .is_some_and(|v| v.eq_ignore_ascii_case("taptap")))
            || (first == "绑定账号"
                && words
                    .get(1)
                    .is_some_and(|v| v.eq_ignore_ascii_case("taptap")))
        {
            if !private(msg) {
                reply(msg, "请私聊或使用群临时会话发送：绑定账号 TapTap".into()).await;
                return Ok(());
            }
            let target = if first == "重新授权" {
                words.get(1).copied().unwrap_or("")
            } else {
                ""
            };
            if first == "重新授权" && !number(target) {
                reply(
                    msg,
                    "格式：重新授权 编号 TapTap\n例如：重新授权 2 TapTap".into(),
                )
                .await;
                return Ok(());
            }
            return self.web_bind(c, msg, target).await;
        }
        if matches!(first, "绑定账号" | "重新授权") {
            if !private(msg) {
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
                reply(
                    msg,
                    "已有绑定进度，请输入验证码或发送 终止当前绑定。".into(),
                )
                .await;
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
                            "短信验证码已发送\n\n请在 5 分钟内直接回复六位验证码。\n请勿在群内发送手机号或验证码。\n\n放弃本次操作：终止当前绑定"
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
                                reply(msg,"已有有效的短信验证流程\n\n未重新发送短信，请直接回复六位验证码。\n放弃本次操作：终止当前绑定".into()).await;
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
        if first == "终止当前绑定" {
            if !private(msg) {
                reply(msg, "请在私聊或群临时会话中发送：终止当前绑定".into()).await;
                return Ok(());
            }
            let consent = self.consents.remove(&k).is_some();
            let p = self.pending.remove(&k).map(|(_, p)| p);
            if let Some(p) = p {
                if !p.attempt.is_empty() {
                    c.call(
                        msg,
                        reqwest::Method::DELETE,
                        &format!(
                            "{}/{}",
                            if p.provider == "mkcn_taptap" {
                                "web-login-attempts"
                            } else {
                                "login-attempts"
                            },
                            p.attempt
                        ),
                        Value::Null,
                    )
                    .await?;
                }
                reply(msg, "当前绑定已终止。\n\n已经绑定的账号不受影响。".into()).await;
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
                let mut cancelled = consent;
                for item in active.as_array().into_iter().flatten() {
                    if let Some(id) = item["attempt"]["id"].as_str() {
                        c.call(
                            msg,
                            reqwest::Method::DELETE,
                            &format!(
                                "{}/{id}",
                                if item["attempt"]["provider"] == "mkcn_taptap" {
                                    "web-login-attempts"
                                } else {
                                    "login-attempts"
                                }
                            ),
                            Value::Null,
                        )
                        .await?;
                        cancelled = true;
                    }
                }
                reply(
                    msg,
                    if cancelled {
                        "当前绑定已终止。\n\n已经绑定的账号不受影响。".into()
                    } else {
                        "没有正在进行的绑定。\n\n移除已绑定账号：解除绑定 编号".into()
                    },
                )
                .await;
            }
            return Ok(());
        }
        if private(msg) && otp(text) {
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
                let accounts = xs.as_array().cloned().unwrap_or_default();
                let text = if accounts.is_empty() {
                    "尚未绑定账号。\n\n手机号绑定：绑定账号 手机号\nTapTap 绑定：绑定账号 TapTap\n详细说明：/help PJSK数据上传-绑定账号".into()
                } else {
                    format!(
                        "已绑定账号：{} 个\n\n{}\n\n更新默认账号：更新数据\n更新指定账号：更新数据 编号\n管理指令：/help PJSK数据上传",
                        accounts.len(),
                        accounts
                            .iter()
                            .map(account_block)
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    )
                };
                reply(msg, text).await;
            }
            "默认账号" | "账号备注" => {
                let Some(id) = words.get(1) else {
                    reply(msg, "请填写账号编号，可用 账号列表 查看。".into()).await;
                    return Ok(());
                };
                let accounts = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let acc = accounts
                    .as_array()
                    .and_then(|xs| xs.iter().find(|a| a["id"] == *id))
                    .cloned();
                let Some(mut acc) = acc else {
                    reply(msg, "没有找到该账号。\n\n查看编号：账号列表".into()).await;
                    return Ok(());
                };
                let body = if first == "默认账号" {
                    json!({"default":true})
                } else {
                    json!({"remark":words.get(2..).unwrap_or(&[]).join(" ")})
                };
                c.call(
                    msg,
                    reqwest::Method::PATCH,
                    &format!("accounts/{id}"),
                    body.clone(),
                )
                .await?;
                if first == "默认账号" {
                    acc["default"] = json!(true);
                } else {
                    acc["remark"] = body["remark"].clone();
                }
                reply(
                    msg,
                    format!(
                        "账号设置已保存\n\n{}\n\n查看所有账号：账号列表",
                        account_block(&acc)
                    ),
                )
                .await;
            }
            "解除绑定" => {
                if !private(msg) {
                    reply(msg, "请在私聊或群临时会话中解除绑定。".into()).await;
                    return Ok(());
                }
                let id = words.get(1).copied().unwrap_or("");
                if !number(id) || words.len() != 2 {
                    reply(
                        msg,
                        "格式：解除绑定 编号\n例如：解除绑定 2\n\n查看编号：账号列表".into(),
                    )
                    .await;
                    return Ok(());
                }
                let xs = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let acc = xs
                    .as_array()
                    .and_then(|xs| xs.iter().find(|a| a["id"] == id));
                let Some(acc) = acc else {
                    reply(msg, "没有找到该账号。\n\n请发送 账号列表 核对编号。".into()).await;
                    return Ok(());
                };
                let display = account_block(acc);
                let revision = acc["revision"]
                    .as_str()
                    .ok_or_else(Fault::unavailable)?
                    .to_owned();
                self.deletes.insert(
                    k,
                    DeleteConfirmation {
                        account: id.into(),
                        revision,
                        display: display.clone(),
                        expires: now() + 60,
                    },
                );
                reply(msg,format!("请确认解除绑定\n\n{display}\n\n仅移除本工具内的凭证和缓存，不删除游戏账号或已上传数据。\n移除默认账号后，剩余最早绑定的账号会成为默认账号。\n\n确认请在 60 秒内发送：确认解除绑定 {id}\n放弃操作请等待本次确认过期。")).await;
            }
            "确认解除绑定" => {
                if !private(msg) {
                    reply(msg, "请在私聊或群临时会话中确认。".into()).await;
                    return Ok(());
                }
                let confirmation = self.deletes.remove(&k).map(|(_, p)| p).filter(|p| {
                    p.expires > now()
                        && words.len() == 2
                        && words.get(1).is_some_and(|id| *id == p.account)
                });
                let Some(p) = confirmation else {
                    reply(
                        msg,
                        "没有匹配的解除绑定确认，或确认已过期。\n\n请重新发送：解除绑定 编号"
                            .into(),
                    )
                    .await;
                    return Ok(());
                };
                c.call(
                    msg,
                    reqwest::Method::DELETE,
                    &format!("accounts/{}", p.account),
                    json!({"revision":p.revision}),
                )
                .await?;
                reply(
                    msg,
                    format!(
                        "已解除绑定\n\n{}\n\n本工具内的凭证和缓存已移除。\n查看剩余账号：账号列表",
                        p.display
                    ),
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
        private(msg)
            && otp(msg.raw_message.trim())
            && self
                .pending
                .get(&key(msg))
                .is_some_and(|p| p.expires > now() && p.stage == Stage::AwaitingCode)
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
    async fn status(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
    async fn help_visible(&self, msg: &Msg) -> bool {
        self.client.get().is_some_and(|c| {
            c.config.enabled
                && msg.self_id.to_string() == c.config.bot_id
                && (private(msg) || c.config.allowed_groups.contains(&msg.group_id.to_string()))
        })
    }
    async fn help(&self, query: &str) -> String {
        help::render(query).await
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
    #[tokio::test]
    async fn web_binding_same_message_private_temp_chat_and_no_task_footer() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut paths = vec![];
            for _ in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                loop {
                    let mut chunk = [0; 1024];
                    let n = stream.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                    if request.windows(4).any(|x| x == b"\r\n\r\n") {
                        break;
                    }
                    assert!(request.len() < 8192);
                }
                let headers = String::from_utf8_lossy(&request).to_lowercase();
                assert!(headers.contains("x-scene: private"));
                assert!(headers.contains("x-group-id: 99"));
                let line = headers.lines().next().unwrap().to_owned();
                paths.push(line.clone());
                let data = if line.starts_with("get /v1/access/status ") {
                    json!({"eligible":true})
                } else if line.starts_with("get /v1/terms ") {
                    json!({"accepted":true})
                } else if line.starts_with("post /v1/web-login-attempts ") {
                    json!({"id":"attempt-test","stage":"awaiting_web","expires":now()+60,"interval":2,"login_url":"https://accounts.taptap.cn/login","authorize_url":"https://accounts.taptap.cn/authorize?user_code=demo"})
                } else {
                    assert!(line.starts_with("get /v1/web-login-attempts/attempt-test "));
                    json!({"job":{"id":"internal-job-id","status":"success","deadline":now()+60,"items":[{"account_id":"1","game_id":"9000000000000000012","status":"success"}]}})
                };
                let body = json!({"data":data,"error":null}).to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            paths
        });
        let client = Client {
            config: Config {
                enabled: true,
                bot_id: "1".into(),
                service_url: format!("http://{address}"),
                api_key_file: String::new(),
                allowed_groups: vec!["10".into()],
            },
            http: reqwest::Client::builder().no_proxy().build().unwrap(),
            api_key: "test".into(),
        };
        let h = PjskUpload::default();
        let msg = Msg {
            self_id: 1,
            user_id: 2,
            group_id: 99,
            message_type: "private".into(),
            message_id: 987655,
            raw_message: "绑定账号 TapTap".into(),
            ..Default::default()
        };
        assert!(command("账号绑定TapTap"));
        assert!(command("绑定账号 TapTap"));
        tokio::time::timeout(Duration::from_secs(10), h.run(&client, &msg))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(server.await.unwrap().len(), 4);
        let replies = TEST_REPLIES.lock().unwrap();
        let texts: Vec<_> = replies
            .iter()
            .filter(|(id, _)| *id == 987655)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("① 先登录"));
        assert!(texts[0].contains("网页可能持续加载"));
        assert!(texts[1].contains("绑定成功"));
        assert!(texts[1].contains("9000000000000000012"));
        assert!(!texts[1].contains("internal-job-id"));
        assert!(!h.pending.contains_key(&(1, 2)));
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
    async fn scripted_client(
        script: Vec<(&'static str, Value)>,
    ) -> (Client, tokio::task::JoinHandle<Vec<String>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut captured = Vec::new();
            for (expected, data) in script {
                let (mut stream, _) =
                    tokio::time::timeout(Duration::from_secs(5), listener.accept())
                        .await
                        .unwrap()
                        .unwrap();
                let mut request = Vec::new();
                let end;
                loop {
                    let mut chunk = [0; 1024];
                    let n = stream.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                    if let Some(i) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                        end = i + 4;
                        break;
                    }
                    assert!(request.len() < 16384);
                }
                let header = String::from_utf8_lossy(&request[..end]).to_lowercase();
                assert!(
                    header.starts_with(expected),
                    "unexpected request: {}",
                    header.lines().next().unwrap()
                );
                let len = header
                    .lines()
                    .find_map(|s| {
                        s.strip_prefix("content-length:")
                            .and_then(|s| s.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while request.len() < end + len {
                    let mut chunk = [0; 1024];
                    let n = stream.read(&mut chunk).await.unwrap();
                    assert!(n > 0);
                    request.extend_from_slice(&chunk[..n]);
                }
                captured.push(String::from_utf8_lossy(&request).into_owned());
                let body = json!({"data":data,"error":null}).to_string();
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
            }
            captured
        });
        (
            Client {
                config: Config {
                    enabled: true,
                    bot_id: "1".into(),
                    service_url: format!("http://{address}"),
                    api_key_file: String::new(),
                    allowed_groups: vec!["10".into()],
                },
                http: reqwest::Client::builder().no_proxy().build().unwrap(),
                api_key: "test".into(),
            },
            server,
        )
    }
    fn message(id: i64, text: &str) -> Msg {
        Msg {
            self_id: 1,
            user_id: 2,
            message_type: "private".into(),
            post_type: "message".into(),
            message_id: id,
            raw_message: text.into(),
            ..Default::default()
        }
    }
    #[tokio::test]
    async fn hierarchy_uses_sub_help_and_lists_each_instruction() {
        let h = PjskUpload::default();
        let root = h.help("PJSK数据上传").await;
        for name in ["绑定账号", "终止当前绑定", "解除绑定", "使用条款"] {
            assert!(root.contains(name));
        }
        assert!(!root.contains("上传帮助"));
        assert!(root.contains("/help <主功能>-<子功能>"));
        let detail = h.help("PJSK数据上传-绑定账号-解除绑定").await;
        assert!(detail.contains("<绑定账号>"));
        assert!(detail.contains("<解除绑定>"));
        assert!(detail.contains("确认解除绑定 2"));
        let all = h.help("PJSK数据上传-all").await;
        assert!(all.contains("<使用条款>"));
        assert!(h.help("PJSK数据上传-不存在").await.contains("未找到子功能"));
    }
    #[test]
    fn normalized_commands_and_clear_account_cards() {
        for text in [
            "绑定账号 taptap",
            "绑定账号 TAPTAP",
            "绑定账号 TaPtAp",
            "账号绑定TAPTAP",
            "绑定账号\u{3000}TaPtAp",
        ] {
            assert!(command(text));
            assert!(format_error(&text.split_whitespace().collect::<Vec<_>>()).is_none());
        }
        for text in [
            "绑定账号",
            "绑定账号 TapTap 多余参数",
            "解除绑定",
            "解除绑定 abc",
            "更新数据 1 多余参数",
        ] {
            assert!(format_error(&text.split_whitespace().collect::<Vec<_>>()).is_some());
        }
        for text in ["删除账号 1", "取消绑定", "上传帮助"] {
            assert!(!command(text));
        }
        let card = account_block(
            &json!({"id":"2","nickname":"小李\n管理员\u{0001}","game_id":"9000000000000000001","provider":"mkcn_taptap","default":true,"remark":"日常\r使用"}),
        );
        assert!(card.starts_with("编号：2（默认）\n游戏昵称：小李 管理员\nUID："));
        assert!(card.contains("登录方式：TapTap\n备注：日常 使用"));
        assert!(!card.contains('\u{0001}'));
        assert!(account_block(&json!({"id":"1","provider":"mkcn"})).contains("游戏昵称：暂未获取"));
        let latest = PjskUpload::job_text(
            &json!({"kind":"bind","status":"success","id":"internal","items":[{"account_id":"2","nickname":"小李","game_id":"123","provider":"mkcn_taptap"}]}),
            false,
        );
        assert!(latest.starts_with("绑定成功"));
        assert!(!latest.contains("internal"));
    }
    #[tokio::test]
    async fn consent_resumes_phone_binding_in_second_message_and_never_treats_digits_as_consent() {
        let (c, server) = scripted_client(vec![
            ("get /v1/access/status ", json!({"eligible":true})),
            (
                "get /v1/terms ",
                json!({"version":"v1","text":"测试条款","accepted":false}),
            ),
            ("get /v1/access/status ", json!({"eligible":true})),
            ("post /v1/consents ", json!({"version":"v1"})),
            ("get /v1/access/status ", json!({"eligible":true})),
            (
                "post /v1/login-attempts ",
                json!({"id":"sms-new","expires":now()+300}),
            ),
        ])
        .await;
        let h = PjskUpload::default();
        h.run(&c, &message(990001, "绑定账号 13000000000"))
            .await
            .unwrap();
        assert_eq!(
            h.pending.get(&(1, 2)).unwrap().stage,
            Stage::AwaitingConsent
        );
        assert!(!h.matches(&message(990002, "123456")).await);
        let mut other = message(990003, "同意条款");
        other.user_id = 3;
        h.accept_consent(&c, &other).await.unwrap();
        assert!(h.consents.contains_key(&(1, 2)));
        h.run(&c, &message(990004, "同意条款")).await.unwrap();
        assert_eq!(h.pending.get(&(1, 2)).unwrap().stage, Stage::AwaitingCode);
        assert!(!h.consents.contains_key(&(1, 2)));
        let requests = server.await.unwrap();
        assert!(requests[5].contains("13000000000"));
        assert!(
            !requests[..5]
                .iter()
                .any(|r| r.starts_with("POST /v1/login-attempts"))
        );
        let replies = TEST_REPLIES.lock().unwrap();
        assert!(
            replies
                .iter()
                .any(|(id, t)| *id == 990001 && t.contains("同意条款"))
        );
        assert!(
            replies
                .iter()
                .any(|(id, t)| *id == 990004 && t.contains("短信验证码已发送"))
        );
    }
    #[tokio::test]
    async fn terminating_or_expiring_consent_never_starts_login() {
        let (c, server) = scripted_client(vec![
            ("get /v1/access/status ", json!({"eligible":true})),
            (
                "get /v1/terms ",
                json!({"version":"v1","text":"测试条款","accepted":false}),
            ),
            ("get /v1/access/status ", json!({"eligible":true})),
        ])
        .await;
        let h = PjskUpload::default();
        h.run(&c, &message(990010, "绑定账号 taptap"))
            .await
            .unwrap();
        let mut group = message(990011, "终止当前绑定");
        group.message_type = "group".into();
        group.group_id = 99;
        h.run(&c, &group).await.unwrap();
        assert!(h.consents.contains_key(&(1, 2)));
        h.run(&c, &message(990012, "终止当前绑定")).await.unwrap();
        h.accept_consent(&c, &message(990013, "同意条款"))
            .await
            .unwrap();
        assert!(h.pending.is_empty());
        assert!(h.consents.is_empty());
        assert_eq!(server.await.unwrap().len(), 3);
        h.consents.insert(
            (1, 2),
            ConsentIntent {
                command: "绑定账号 taptap".into(),
                version: "v1".into(),
                expires: now() - 1,
                generation: 100,
            },
        );
        h.accept_consent(&c, &message(990014, "同意条款"))
            .await
            .unwrap();
        assert!(h.pending.is_empty());
    }
    #[tokio::test]
    async fn unbind_confirms_the_displayed_account_revision() {
        let account = json!({"id":"2","nickname":"小李","game_id":"123","provider":"mkcn_taptap","revision":"confirmed-revision"});
        let (c, server) = scripted_client(vec![
            ("get /v1/access/status ", json!({"eligible":true})),
            ("get /v1/accounts ", json!([account])),
            ("get /v1/access/status ", json!({"eligible":true})),
            ("delete /v1/accounts/2 ", json!({"deleted":true})),
        ])
        .await;
        let h = PjskUpload::default();
        h.run(&c, &message(990020, "解除绑定 2")).await.unwrap();
        h.run(&c, &message(990021, "确认解除绑定 2")).await.unwrap();
        let requests = server.await.unwrap();
        assert!(
            requests[3]
                .to_lowercase()
                .contains("if-match: confirmed-revision")
        );
        let replies = TEST_REPLIES.lock().unwrap();
        let text = &replies.iter().find(|(id, _)| *id == 990020).unwrap().1;
        assert!(text.contains("游戏昵称：小李\nUID：123"));
        assert!(text.contains("确认解除绑定 2"));
    }
}
