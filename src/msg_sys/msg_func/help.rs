use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Handler, MSG_HANDLERS, Msg};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

pub struct Help {
    pub status: bool,
}
#[async_trait]
impl Handler for Help {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        let mut splits = msg.raw_message.split(" ");
        if splits.next().unwrap() == "/help" {
            debug!("find help mod");
            return true;
        }
        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        let mut rep = SendMsg::new().await;
        if splits.next().is_none() {
            debug!("return help list");
            rep.join_text(self.help().await).await;
            rep.send_msg(msg.clone()).await;
            return;
        }
        let mut unfind_help = String::new();
        rep.join_text("help list".to_string()).await;
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        's: for split in splits {
            for handler in MSG_HANDLERS.get().unwrap().iter() {
                if split == "help" {
                    continue 's;
                }
                if split == handler.name().await.as_str() {
                    rep.join_text(format!("\n\n<{}>:\n{}", split, handler.help().await))
                        .await;
                    continue 's;
                }
            }
            unfind_help.push_str(format!(" {}", split).as_str());
        }
        if !unfind_help.is_empty() {
            rep.join_text(format!("\n\n未找到帮助项:{}", unfind_help))
                .await;
        }
        rep.send_msg(msg.clone()).await;
        return;
    }

    async fn init(&mut self) -> bool {
        self.status = true;
        true
    }

    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        let handlers = MSG_HANDLERS.get().unwrap();
        let mut rep = String::new();
        rep.push_str("help list\n\n>");
        for handler in handlers {
            let name = handler.name().await;
            if name != "help".to_string() && handler.status().await {
                rep.push_str(format!("\n{}", name).as_str());
            }
        }
        rep.push_str(
            "\n>\n\n使用\n/help name1 name2...\n查询对应功能帮助列表\neg:\n/help test 表情",
        );
        rep
    }

    async fn name(&self) -> String {
        "help".to_string()
    }
}
