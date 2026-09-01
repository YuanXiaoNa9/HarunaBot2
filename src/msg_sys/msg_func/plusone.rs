use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
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
impl MsgHandler for PlusOne {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        if msg.message_type == "group" {
            return true;
        }
        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        let raw_message = msg.raw_message.clone();
        let data = match self.map.get().unwrap().get(&msg.group_id) {
            None => {
                self.map.get().unwrap().insert(
                    msg.group_id,
                    PlusOneData {
                        last_message: raw_message,
                        user_id: msg.sender.user_id,
                        i: 1,
                    },
                );
                debug!("no find plusone map");
                return;
            }
            Some(data) => data,
        };
        let i = data.i;
        if data.last_message == raw_message {
            drop(data);
            self.map.get().unwrap().insert(
                msg.group_id,
                PlusOneData {
                    last_message: raw_message,
                    user_id: msg.sender.user_id,
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
                    user_id: msg.sender.user_id,
                    i: 1,
                },
            );
            debug!("different msg");
            return;
        }
        if i == 2 {
            let mut rep = SendMsg::new_msg().await;
            rep.join_text(msg.raw_message.clone()).await;
            rep.send_msg(msg).await;
            debug!("plusone ok");
            return;
        }
    }

    async fn init(&mut self) -> bool {
        self.map = OnceLock::from(DashMap::new());
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
