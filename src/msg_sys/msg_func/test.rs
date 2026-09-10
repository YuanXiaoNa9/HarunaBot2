use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{Error, anyhow};
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use anyhow_trace::anyhow_trace;

pub struct Test {
    pub status: AtomicBool,
}

#[async_trait]
impl FnHandler for Test {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message == "test" {
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, _msg: &Msg) -> Result<(), Error> {
        Err(anyhow!("test err"))
    }

    async fn init(&self) {}
    async fn status(&self) -> bool {
        true
    }

    async fn help(&self) -> String {
        "测试使用".to_string()
    }

    async fn name(&self) -> String {
        "test".to_string()
    }
}
