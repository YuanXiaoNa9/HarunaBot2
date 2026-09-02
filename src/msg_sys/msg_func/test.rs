use crate::msg_sys::msg_sys::{Handler, Msg};
use async_trait::async_trait;
use std::sync::Arc;

pub struct Test {
    pub(crate) status: bool,
}

#[async_trait]
impl Handler for Test {
    async fn matches(&self, _msg: Arc<Msg>) -> bool {
        false
    }

    async fn process(&self, _msg: Arc<Msg>) {
        todo!()
    }

    async fn init(&mut self) -> bool {
        self.status = true;
        true
    }
    async fn status(&self) -> bool {
        self.status
    }

    async fn help(&self) -> String {
        "测试使用".to_string()
    }

    async fn name(&self) -> String {
        "test".to_string()
    }
}
