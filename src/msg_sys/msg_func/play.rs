use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Handler, Msg};
use async_trait::async_trait;
use std::sync::Arc;

pub struct Play {
    pub(crate) status: bool,
}
#[async_trait]
impl Handler for Play {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        msg.raw_message.contains(r#"title":"QQ经典农场"#)
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut rep = SendMsg::new().await;
        rep.join_text("不许给我转QQ农场喵".to_string()).await;
        rep.send_msg(msg.clone()).await;
        let mut rep1 = SendMsg::new().await;
        rep1.join_text("本喵会不开心的喵".to_string()).await;
        rep1.send_msg(msg.clone()).await;
    }

    async fn init(&mut self) -> bool {
        self.status = true;
        true
    }

    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        "娱乐功能，或许会有一些小彩蛋".to_string()
    }

    async fn name(&self) -> String {
        "娱乐".to_string()
    }
}
