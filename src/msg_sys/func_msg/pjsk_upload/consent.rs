use super::*;

impl PjskState {
    pub(super) async fn ensure_consent(
        &self,
        c: &Client,
        msg: &Msg,
        command: &str,
    ) -> Result<bool, Fault> {
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
    pub(super) async fn accept_consent(&self, c: &Client, msg: &Msg) -> Result<(), Fault> {
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
}
