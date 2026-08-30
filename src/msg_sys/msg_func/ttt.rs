use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;
pub struct TTT {
    status: bool,
}
#[async_trait]
impl MsgHandler for TTT {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        let mut split = msg.raw_message.split(" ");
        if split.next() == Some("[CQ:at,qq=1246137523]")
            && split.next() == Some("/ttt")
            && split.next().is_some()
        {
            debug!("TTT is OK");
            return true;
        }

        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut split = msg.raw_message.split(" ");
        split.next();
        split.next();
        let mut rep = SendMsg::new_msg().await;
        rep.join_text(split.next().unwrap().to_string()).await;
        rep.send_msg(msg.clone()).await;
    }

    async fn init(&mut self) {}

    async fn status(&self) -> bool {
        self.status
    }
}
