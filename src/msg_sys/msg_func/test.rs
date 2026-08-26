use crate::msg_sys::msg_reply::{DataNode, SendMsg};
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use async_trait::async_trait;
use core::str::Split;
use std::sync::Arc;

pub struct Test;
#[async_trait]
impl MsgHandler for Test {
    async fn matches(&self, msg: Arc<Msg>, _msg_split:&Split<&str>) -> bool {
        if msg.raw_message == "test" {
            true
        } else {
            false
        }
    }


    async fn process(&self, msg: Arc<Msg>, _msg_split:&Split<&str>) {
        let mut reply: SendMsg = SendMsg::new_msg().await;
        let mut node = DataNode::new(3474585798, "admin".to_string()).await;
        node.join_text("sss".to_string()).await;
        reply.join_node(node).await;
        reply.send_forward_msg(msg.clone()).await;
    }
}
