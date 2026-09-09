use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tracing::debug;

pub struct TTT {
    pub enable: bool,
    pub status: AtomicBool,
}
#[async_trait]
impl FnHandler for TTT {
    async fn matches(&self, msg: &Msg) -> bool {
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

    async fn process(&self, msg: &Msg) {
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        splits.next();
        let mut rep = SendMsg::new().await;
        rep.join_text(splits.next().unwrap().to_string()).await;
        rep.send_msg(msg).await;
    }

    async fn init(&self) {
        let mut rx = DBLINK.rx.clone();
        if *rx.borrow_and_update() {
            self.status.store(true, Relaxed);
        } else {
            loop {
                let _ =rx.changed().await;
                if *rx.borrow_and_update() {
                    self.status.store(true, Relaxed);
                    break;
                } else {
                    continue;
                }
            }
        }
    }

    async fn status(&self) -> bool {
        if self.enable && self.status.load(Relaxed) {
            return true;
        }
        false
    }

    async fn help(&self) -> String {
        "开发中".to_string()
    }

    async fn name(&self) -> String {
        "井字棋".to_string()
    }
}
