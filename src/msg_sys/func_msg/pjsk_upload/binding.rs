use super::*;

impl PjskState {
    pub(super) async fn bind(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        let k = key(msg);
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
                reply(
                    msg,
                    "请私聊或使用群临时会话发送：/pjsk 绑定账号 TapTap".into(),
                )
                .await;
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
                    "格式：/pjsk 重新授权 编号 TapTap\n例如：/pjsk 重新授权 2 TapTap".into(),
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
                    "格式：/pjsk 绑定账号 手机号；或 /pjsk 重新授权 账号编号 手机号".into(),
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
                    "已有绑定进度，请输入验证码或发送 /pjsk 终止当前绑定。".into(),
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
                            "短信验证码已发送\n\n请在 5 分钟内直接回复六位验证码。\n请勿在群内发送手机号或验证码。\n\n放弃本次操作：/pjsk 终止当前绑定"
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
                                reply(msg,"已有有效的短信验证流程\n\n未重新发送短信，请直接回复六位验证码。\n放弃本次操作：/pjsk 终止当前绑定".into()).await;
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

        Ok(())
    }
    pub(super) async fn terminate(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        let k = key(msg);
        if first == "终止当前绑定" {
            if !private(msg) {
                reply(msg, "请在私聊或群临时会话中发送：/pjsk 终止当前绑定".into()).await;
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
                        "没有正在进行的绑定。\n\n移除已绑定账号：/pjsk 解除绑定 编号".into()
                    },
                )
                .await;
            }
            return Ok(());
        }

        Ok(())
    }
    pub(super) async fn verify(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let k = key(msg);
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
            reply(msg, Self::job_result(&result?, true)?).await;
            return Ok(());
        }

        Ok(())
    }
}
