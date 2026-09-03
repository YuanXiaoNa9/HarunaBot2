use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Handler, Msg};
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::{Arc, OnceLock};
use tracing::debug;

pub struct PlusOneData {
    last_message: String,
    user_id: i64,
    i: i16,
}

pub struct PlusOne {
    pub(crate) status: bool,
    pub(crate) map: OnceLock<DashMap<i64, PlusOneData>>,
}
#[async_trait]
impl Handler for PlusOne {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        if msg.message_type == "group" {
            return true;
        }
        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        let raw_message = msg
            .raw_message
            .clone()
            .strip_prefix("[bot_msg]")
            .unwrap_or(msg.raw_message.clone().as_str())
            .to_string();
        let user_id = msg.user_id.clone();
        let self_id = msg.self_id.clone();
        let data = match self.map.get().unwrap().get(&msg.group_id) {
            None => {
                self.map.get().unwrap().insert(
                    msg.group_id,
                    PlusOneData {
                        last_message: raw_message,
                        user_id,
                        i: 1,
                    },
                );
                debug!("no find plusone map");
                return;
            }
            Some(data) => data,
        };
        debug!(
            "\nlast msg:\n{}\nnow msg:\n{}",
            data.last_message, raw_message
        );
        let i = data.i;
        if data.last_message == raw_message {
            if data.user_id == self_id && i == 1 && user_id != self_id {
                let mut rep = SendMsg::new().await;
                rep.join_text("不要复读人家喵".to_string()).await;
                rep.send_msg(msg.clone()).await;
                let mut rep = SendMsg::new().await;
                rep.join_text("打断复读喵".to_string()).await;
                rep.send_msg(msg.clone()).await;
                return;
            };
            drop(data);
            self.map.get().unwrap().insert(
                msg.group_id,
                PlusOneData {
                    last_message: raw_message,
                    user_id,
                    i: i + 1,
                },
            );
            debug!("same msg");
        } else {
            drop(data);
            self.map.get().unwrap().insert(
                msg.group_id,
                PlusOneData {
                    last_message: raw_message,
                    user_id,
                    i: 1,
                },
            );
            debug!("different msg");
            return;
        }
        if i == 2 {
            let mut rep = SendMsg::new().await;
            rep.join_text(msg.raw_message.clone()).await;
            rep.send_msg(msg).await;
            debug!("plusone ok");
            return;
        }
    }

    async fn init(&mut self) -> bool {
        self.map = OnceLock::from(DashMap::new());
        self.status = true;
        self.status
    }

    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        "群聊自动加一，仅在群聊开启".to_string()
    }

    async fn name(&self) -> String {
        "加一".to_string()
    }
}
