use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Handler, Msg};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::debug;

pub struct TTT {
    pub status: bool,
}
#[async_trait]
impl Handler for TTT {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        let mut splits = msg.raw_message.split(" ");
        if splits.next() == Some("[CQ:at,qq=1246137523]")
            && splits.next() == Some("/ttt")
            && splits.next().is_some()
        {
            debug!("TTT is OK");
            return true;
        }

        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        splits.next();
        let mut rep = SendMsg::new().await;
        rep.join_text(splits.next().unwrap().to_string()).await;
        rep.send_msg(msg.clone()).await;
    }

    async fn init(&mut self) -> bool {
        self.status = DBLINK.get().unwrap().status;
        self.status
    }

    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        "开发中".to_string()
    }

    async fn name(&self) -> String {
        "井字棋".to_string()
    }
}
