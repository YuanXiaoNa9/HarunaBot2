use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;

pub struct Play {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for Play {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.raw_message.contains(r#"title":"QQ经典农场"#)
    }

    async fn process(&self, msg: &Msg) {
        let mut rep = SendMsg::new().await;
        rep.join_text("不许给我转QQ农场喵".to_string()).await;
        rep.send_msg(msg).await;
        let mut rep1 = SendMsg::new().await;
        rep1.join_text("本喵会不开心的喵".to_string()).await;
        rep1.send_msg(msg).await;
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self) -> String {
        "娱乐功能，或许会有一些小彩蛋".to_string()
    }

    async fn name(&self) -> String {
        "娱乐".to_string()
    }
}
