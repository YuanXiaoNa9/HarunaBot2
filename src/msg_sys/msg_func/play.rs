use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;

pub struct Play {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for Play {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.contains(r#"title":"QQ经典农场"#) {
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let mut rep = SendMsg::new().await;
        rep.join_text("不许给我转QQ农场喵".to_string()).await;
        rep.send_msg(msg).await;
        let mut rep1 = SendMsg::new().await;
        rep1.join_text("本喵会不开心的喵".to_string()).await;
        rep1.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        "娱乐功能，或许会有一些小彩蛋".to_string()
    }

    async fn name(&self) -> String {
        "娱乐".to_string()
    }
}
