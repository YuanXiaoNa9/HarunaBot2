use crate::msg_sys::msg_reply::SendPoke;
use crate::msg_sys::msg_sys::{Handler, Msg};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::log::debug;

pub struct Poke {
    pub status: bool,
}
#[async_trait]
impl Handler for Poke {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        debug!("matches poke mod");
        debug!("{}", msg.sub_type);
        if msg.sub_type == "poke" && msg.target_id == msg.self_id {
            return true;
        }
        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        if msg.group_id == 0 {
            SendPoke::private(msg.user_id).await;
        } else {
            SendPoke::group(msg.group_id, msg.user_id).await;
        }
    }

    async fn init(&mut self) -> bool {
        self.status = true;
        self.status
    }

    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        "自动回戳".to_string()
    }

    async fn name(&self) -> String {
        "回戳".to_string()
    }
}
