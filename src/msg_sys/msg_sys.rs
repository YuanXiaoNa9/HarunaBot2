use crate::main_config::MainConfig;
use crate::msg_sys::msg_func::test::Test;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, info};
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MsgSender {
    pub user_id: i64,
    pub nickname: String,
    pub card: String,
    pub role: String,
    pub sex: String,
    pub age: i64,
}
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct Msg {
    pub time: i64,
    pub self_id: i64,
    pub post_type: String,
    pub message_type: String,
    pub sub_type: String,
    pub message_id: i64,
    pub message_seq: i64,
    pub group_id: i64,
    pub group_name: String,
    pub user_id: i64,
    pub message: String,
    pub raw_message: String,
    pub font: i16,
    pub sender: MsgSender,
}

pub trait MsgHandler {
    fn matches(&self, _: &Msg) -> bool;
    fn process(&self, _: &Msg);
}
impl Msg {
    pub(crate) fn send(&self) {
        println!("send ok");
    }
}

pub async fn msg_sys(main_config: MainConfig, mut msg_chan: Receiver<String>) {
    let msg_handlers: Arc<Vec<Box<dyn MsgHandler>>> = Arc::new(vec![Box::new(Test)]);
    loop {
        let msg_handlers = msg_handlers.clone();
        let msg: String = match msg_chan.recv().await {
            None => {
                error!("消息接收出现错误");
                continue;
            }
            Some(i) => i,
        };
        if msg == "" {
            continue;
        }

        let msg: Msg = match serde_json::from_str(msg.as_str()) {
            Ok(msg_struct) => {
                debug!("{:?}", msg_struct);
                msg_struct
            }
            Err(e) => {
                error!("{}", e);
                continue;
            }
        };
        if msg.post_type != "message" {
            continue;
        }
        info!("{}:{}", msg.sender.user_id, msg.raw_message);
        dispatch(&msg, msg_handlers);
    }
}

fn dispatch(msg: &Msg, handlers: Arc<Vec<Box<dyn MsgHandler>>>) {
    for handler in handlers.iter() {
        if handler.matches(&msg) {
            handler.process(&msg);
        }
    }
}
