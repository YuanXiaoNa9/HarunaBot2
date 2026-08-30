use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use async_trait::async_trait;
use std::sync::Arc;

pub struct Play {
    pub(crate) status:bool,
}
#[async_trait]
impl MsgHandler for Play {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        msg.raw_message.contains(r#"title":"QQ经典农场"#)
    }

    async fn process(&self,msg: Arc<Msg>) {
        let mut rep = SendMsg::new_msg().await;
        rep.join_text("不许给我转QQ农场喵".to_string()).await;
        rep.send_msg(msg.clone()).await;
        let mut rep1 = SendMsg::new_msg().await;
        rep1.join_text("本喵会不开心的喵".to_string()).await;
        rep1.send_msg(msg.clone()).await;
    }

    async fn init(&mut self) {
    }

    async fn status(&self) -> bool {
        self.status
    }
}