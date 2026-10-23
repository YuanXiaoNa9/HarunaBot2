use crate::msg_sys::handler_init::MSG_HANDLERS;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use tracing::debug;

pub struct Help {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for Help {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("/help") {
            debug!("find help mod");
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let text = self.render(msg, MSG_HANDLERS.get().unwrap()).await;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(text).await;
        if msg.message_type == "private" {
            rep.send_private_msg(msg).await;
        } else {
            rep.send_msg(msg).await;
        }
        Ok(())
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        // Legacy callers have no message context; restricted modules stay hidden.
        help_list(MSG_HANDLERS.get().unwrap(), &Msg::default()).await
    }

    async fn name(&self) -> String {
        "help".to_string()
    }
}

impl Help {
    async fn render(&self, msg: &Msg, handlers: &[Box<dyn FnHandler + Send + Sync>]) -> String {
        let mut splits = msg.raw_message.split_whitespace();
        splits.next();
        let mut define_help = String::new();
        let mut help_data = String::new();
        'query: for split in splits {
            for handler in handlers {
                let name = handler.name().await;
                if split.starts_with("help") || !handler.status().await || !split.starts_with(&name)
                {
                    continue;
                }
                // A hidden item contributes neither title, description nor an echoed name.
                if !handler.help_visible(msg).await {
                    continue 'query;
                }
                help_data.push_str(&format!("\n\n[{}]\n{}", name, handler.help(split).await));
                continue 'query;
            }
            define_help.push_str(&format!(" {}", split));
        }
        if help_data.is_empty() {
            help_data.push_str(&format!("\n\n{}", help_list(handlers, msg).await));
        }
        let mut text = format!("[ help list ]{}", help_data);
        if !define_help.is_empty() {
            text.push_str(&format!("\n未找到帮助项:{}", define_help));
        }
        text
    }
}
async fn help_list(handlers: &[Box<dyn FnHandler + Send + Sync>], msg: &Msg) -> String {
    let mut text = String::from(">");
    for handler in handlers {
        let name = handler.name().await;
        if name != "help" && handler.status().await && handler.help_visible(msg).await {
            text.push_str(&format!("\n{}", name));
        }
    }
    text.push_str("\n>\n\n使用\n/help name1 name2...\n查询对应功能帮助列表\neg:\n/help test 表情");
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Item {
        restricted: bool,
    }
    #[async_trait]
    impl FnHandler for Item {
        async fn matches(&self, _: &Msg) -> bool {
            false
        }
        async fn process(&self, _: &Msg) -> Result<(), Error> {
            Ok(())
        }
        async fn init(&self) {}
        async fn status(&self) -> bool {
            true
        }
        async fn name(&self) -> String {
            if self.restricted {
                "PJSK数据上传"
            } else {
                "普通功能"
            }
            .into()
        }
        async fn help(&self, _: &str) -> String {
            if self.restricted {
                "私聊绑定账号，发送手机号与验证码"
            } else {
                "普通说明"
            }
            .into()
        }
        async fn help_visible(&self, msg: &Msg) -> bool {
            !self.restricted || msg.group_id == 10
        }
    }
    fn items() -> Vec<Box<dyn FnHandler + Send + Sync>> {
        vec![
            Box::new(Item { restricted: true }),
            Box::new(Item { restricted: false }),
        ]
    }
    #[tokio::test]
    async fn outside_group_hides_list_and_explicit_details() {
        let h = Help {
            status: AtomicBool::new(true),
        };
        let handlers = items();
        for command in ["/help", "/help PJSK数据上传", "/help PJSK数据上传 普通功能"] {
            let msg = Msg {
                group_id: 99,
                raw_message: command.into(),
                ..Default::default()
            };
            let text = h.render(&msg, &handlers).await;
            for secret in ["PJSK", "绑定账号", "手机号", "验证码"] {
                assert!(!text.contains(secret), "hidden module leaked into help");
            }
            assert!(text.contains("普通功能"));
        }
    }
    #[tokio::test]
    async fn allowed_group_keeps_list_and_details() {
        let h = Help {
            status: AtomicBool::new(true),
        };
        let handlers = items();
        let mut msg = Msg {
            group_id: 10,
            raw_message: "/help".into(),
            ..Default::default()
        };
        assert!(h.render(&msg, &handlers).await.contains("PJSK数据上传"));
        msg.raw_message = "/help PJSK数据上传".into();
        assert!(h.render(&msg, &handlers).await.contains("手机号与验证码"));
    }
}
