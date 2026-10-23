use super::*;

impl PjskState {
    pub(super) async fn apply(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.first() == Some(&"申请绑定") {
            c.call(msg, reqwest::Method::POST, "access/apply", json!({}))
                .await?;
            reply(
                msg,
                "已取得绑定资格\n\n接下来请私聊 Bot 或使用群临时会话。\n手机号绑定：/pjsk 绑定账号 手机号\nTapTap 绑定：/pjsk 绑定账号 TapTap\n\n首次操作需同意条款；绑定成功后再手动更新。\n详细帮助：/help PJSK数据上传"
                    .into(),
            )
            .await;
            return Ok(());
        }

        Ok(())
    }
}
