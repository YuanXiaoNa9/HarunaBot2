use crate::msg_sys::msg_sys::{FnHandler, Msg};
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;

pub struct Test {
    pub status: AtomicBool,
}

#[async_trait]
impl FnHandler for Test {
    async fn matches(&self, _msg: &Msg) -> bool {
        false
    }

    async fn process(&self, _msg: &Msg) {}

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
