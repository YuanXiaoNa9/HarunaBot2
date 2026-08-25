use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use async_trait::async_trait;

pub struct Test;
#[async_trait]
impl MsgHandler for Test {
    fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message == "test" {
            true
        } else {
            false
        }
    }
    async fn process(&self, msg: Msg) {
        let mut reply: SendMsg = SendMsg::new().await;
        reply.join_text("ok".to_string()).await;
        reply.send(msg).await;
    }
}
