use super::*;

struct SubCommand {
    name: &'static str,
    state: Arc<PjskState>,
}
pub(super) fn handlers(state: Arc<PjskState>) -> Vec<Box<dyn FnHandler + Send + Sync>> {
    help::ITEMS
        .iter()
        .map(|(name, _)| {
            Box::new(SubCommand {
                name,
                state: state.clone(),
            }) as Box<dyn FnHandler + Send + Sync>
        })
        .collect()
}
#[async_trait]
impl FnHandler for SubCommand {
    async fn matches(&self, msg: &Msg) -> bool {
        let text = command_input(&msg.raw_message).unwrap_or(msg.raw_message.trim());
        let first = text.split_whitespace().next().unwrap_or("");
        match self.name {
            "绑定账号" => {
                first == "绑定账号"
                    || first.eq_ignore_ascii_case("账号绑定taptap")
                    || first.eq_ignore_ascii_case("绑定taptap")
                    || (private(msg)
                        && otp(first)
                        && self
                            .state
                            .pending
                            .get(&key(msg))
                            .is_some_and(|p| p.expires > now() && p.stage == Stage::AwaitingCode))
            }
            "解除绑定" => matches!(first, "解除绑定" | "确认解除绑定"),
            "使用条款" => first == "同意条款",
            name => first == name,
        }
    }
    async fn process(&self, msg: &Msg) -> Result<(), anyhow::Error> {
        let Some(c) = self.state.client.get() else {
            return Ok(());
        };
        match self.state.run(c, msg).await {
            Err(f) if f.reply_policy.as_deref() == Some("silent") => Ok(()),
            result => result.map_err(anyhow::Error::new),
        }
    }
    async fn init(&self) {}
    async fn status(&self) -> bool {
        true
    }
    async fn help(&self, _: &str) -> String {
        help::ITEMS
            .iter()
            .find(|(name, _)| *name == self.name)
            .map(|(_, s)| s.to_string())
            .unwrap_or_default()
    }
    async fn name(&self) -> String {
        self.name.into()
    }
}
