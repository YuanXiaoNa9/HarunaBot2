use super::*;

impl PjskState {
    pub(super) async fn run(&self, c: &Client, msg: &Msg) -> Result<(), Fault> {
        self.run_text(c, msg, msg.raw_message.trim(), None).await
    }
    pub(super) async fn run_text(
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
            return Err(Fault::invalid(&text));
        }
        if words.first() == Some(&"申请绑定") {
            return self.apply(c, msg, input).await;
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
        if login {
            return self.bind(c, msg, input).await;
        }
        if first == "终止当前绑定" {
            return self.terminate(c, msg, input).await;
        }
        if private(msg) && otp(text) {
            return self.verify(c, msg, input).await;
        }
        match first {
            "账号列表" | "默认账号" | "账号备注" | "解除绑定" | "确认解除绑定" => {
                self.accounts(c, msg, input).await
            }
            "更新数据" | "更新状态" | "重传数据" => self.update(c, msg, input).await,
            _ => Err(Fault::invalid(
                "未识别的操作，请使用 /help PJSK数据上传 查看指令",
            )),
        }
    }
}
