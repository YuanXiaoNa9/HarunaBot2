use crate::MAIN_CONFIG;
use crate::msg_sys::msg_func::test::Test;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{ LazyLock};
use tokio::spawn;
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

static MSG_HANDLERS: LazyLock<Vec<Box<dyn MsgHandler + Send + Sync>>> =
    LazyLock::new(|| vec![Box::new(Test)]);

#[async_trait]
pub trait MsgHandler {
    fn matches(&self, _: &Msg) -> bool;
    async fn process(&self, _: Msg);
}

pub async fn msg_sys(mut msg_chan: Receiver<String>) {
    loop {
        //从通道中取出json消息
        let msg: String = match msg_chan.recv().await {
            None => {
                error!("消息接收出现错误");
                continue;
            }
            Some(i) => i,
        };
        //判断是否为空字符串（心跳）
        if msg == "" {
            continue;
        }
        //后台执行
        spawn(async move {
            //使用MsgGet结构体进行解析
            let msg: Msg = match serde_json::from_str(msg.as_str()) {
                Ok(msg_struct) => {
                    debug!("{:?}", msg_struct);
                    msg_struct
                }
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            //判断是否为文字消息
            if msg.post_type != "message" {
                return;
            }
            for black_id in MAIN_CONFIG.black_list.iter() {
                if msg.sender.user_id == *black_id {
                    info!(
                        "黑名单用户：{}({})",
                        msg.sender.nickname, msg.sender.user_id
                    );
                    return;
                }
            }
            //打印消息日志
            info!("{}:{}", msg.sender.user_id, msg.raw_message);
            //进行解析
            dispatch(msg).await;
        });
    }
}

async fn dispatch(msg: Msg) {
    debug!("finding handler");
    //取出handler
    for handler in MSG_HANDLERS.iter() {
        //使用handler的match方法进行判断消息是否符合
        if handler.matches(&msg) {
            debug!("find handler ok");
            //如果符合则调用该handler的process方法
            handler.process(msg).await;
            return;
        }
    }
    debug!("not find handler");
}
