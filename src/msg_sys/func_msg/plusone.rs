use crate::msg_sys::func_mod::plusone_data::{PLUSONE_DATA, PlusOneData};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg, mod_status_examine};
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tracing::debug;

pub struct PlusOne {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for PlusOne {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.message_type == "group" {
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let raw_message = msg.raw_message.clone().to_string();
        let user_id = msg.user_id.clone();
        let self_id = msg.self_id.clone();
        let data = match PLUSONE_DATA.map.get(&msg.group_id) {
            None => {
                PLUSONE_DATA.map.insert(
                    msg.group_id,
                    PlusOneData {
                        last_message: raw_message,
                        user_id,
                        i: 1,
                    },
                );
                debug!("no find plusone map");
                return Ok(());
            }
            Some(data) => data,
        };
        debug!(
            "\nlast msg:\n{}\n{}\nnow msg:\n{}",
            data.last_message, data.i, raw_message
        );
        let i = data.i;
        if data.last_message == raw_message {
            if data.user_id == self_id && i == 1 && user_id != self_id {
                drop(data);
                let mut rep = SendMsg::new().await;
                rep.join_text("不要复读人家喵".to_string()).await;
                rep.send_msg(msg).await;
                let mut rep = SendMsg::new().await;
                rep.join_text("打断复读喵".to_string()).await;
                rep.send_msg(msg).await;
                return Ok(());
            };
            drop(data);
            PLUSONE_DATA.map.insert(
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
            PLUSONE_DATA.map.insert(
                msg.group_id,
                PlusOneData {
                    last_message: raw_message,
                    user_id,
                    i: 1,
                },
            );
            debug!("different msg");
            return Ok(());
        }
        if i == 2 {
            let mut rep = SendMsg::new().await;
            rep.join_text(msg.raw_message.clone()).await;
            rep.send_msg(msg).await;
            debug!("plusone ok");
            return Ok(());
        }
        Ok(())
    }

    async fn init(&self) {
        let rx = PLUSONE_DATA.rx.clone();
        mod_status_examine(rx).await;
        self.status.store(true, Relaxed);
    }

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        "群聊自动加一，仅在群聊开启".to_string()
    }

    async fn name(&self) -> String {
        "加一".to_string()
    }
}
