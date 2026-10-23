use super::*;

impl PjskState {
    pub(super) async fn web_bind(&self, c: &Client, msg: &Msg, target: &str) -> Result<(), Fault> {
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
    pub(super) async fn web_flow(
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
                reply(msg, Self::job_result(&job, true)?).await;
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
}
