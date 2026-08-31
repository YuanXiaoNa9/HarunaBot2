use crate::MAIN_CONFIG;
use crate::msg_sys::msg_func::emoji_mujika::EmoMjk;
use crate::msg_sys::msg_func::test::Test;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, OnceLock};
use tokio::spawn;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, info, warn};
use crate::msg_sys::msg_func::play::Play;

//注册功能函数
#[async_trait]
pub trait MsgHandler {
    async fn matches(&self, _: Arc<Msg>) -> bool;
    async fn process(&self, _: Arc<Msg>);
    async fn init(&mut self) -> (String,bool);
    async fn status(&self) -> bool;
}

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

pub async fn msg_sys(mut msg_chan: Receiver<String>) {
    //模块初始化
    let msg_handlers = handler_init().await;
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
        let handlers = msg_handlers.clone();
        //后台执行
        spawn(async move {
            //使用MsgGet结构体进行解析
            let msg: Msg = match serde_json::from_str(msg.as_str()) {
                Ok(msg_struct) => {
                    debug!("{:?}", msg_struct);
                    msg_struct
                }
                Err(e) => {
                    error!("解析消息出现错误：{}", e);
                    return;
                }
            };
            //判断是否为文字消息
            if msg.post_type != "message" {
                return;
            }
            //判断是否为黑白名单用户
            if bw_right(&msg).await{return;};
            //打印消息日志
            log_msg(&msg);
            //进行解析
            dispatch(handlers, msg).await;
        });
    }
}

async fn dispatch(msg_handlers: Arc<Vec<Box<dyn MsgHandler + Send + Sync>>>, msg: Msg) {
    debug!("finding handler");
    let msg = Arc::new(msg);
    //取出handler
    for handler in msg_handlers.iter() {
        //使用handler的match方法进行判断消息是否符合
        if handler.matches(msg.clone()).await && handler.status().await {
            debug!("find handler");
            handler.process(msg.clone()).await;
            return;
        }
    }
    debug!("not find handler");
}

async fn handler_init() -> Arc<Vec<Box<dyn MsgHandler + Send + Sync>>> {
    //注册功能模块
    let handlers = handler_regin();
    //创建新的vec存储handler
    let mut init_handlers = Vec::new();
    //取出handler并执行初始化方法
    for mut handler in handlers {
        let (name,ok)=handler.init().await;
        if ok {
            info!("<{}>模块初始化成功",name)
        }else{
            warn!("<{}>模块未初始化",name)
        }
        init_handlers.push(handler);
    }
    Arc::new(init_handlers)
}

async fn bw_right(msg: &Msg) -> bool {
    if MAIN_CONFIG.bw_status == "black" {
        for black_id in MAIN_CONFIG.black_list.iter() {
            if msg.sender.user_id == *black_id {
                info!(
                    "非允许用户：{}({})",
                    msg.sender.nickname, msg.sender.user_id
                );
                return true;
            }
        }
    } else if MAIN_CONFIG.bw_status == "white" {
        for white_id in MAIN_CONFIG.white_list.iter() {
            if msg.sender.user_id != *white_id {
                info!(
                    "非允许用户：{}({})",
                    msg.sender.nickname, msg.sender.user_id
                );
                return true;
            }
        }
    }
    false
}
fn log_msg(msg:&Msg){
    if msg.message_type == "group" {
        info!(
                    "[{}]({}):[{}]({}) => <{}>",
                    msg.group_name,
                    msg.group_id,
                    msg.sender.nickname,
                    msg.sender.user_id,
                    msg.raw_message
                );
    } else if msg.message_type == "private" {
        info!(
                    "{}({}): <{}>",
                    msg.sender.nickname, msg.sender.user_id, msg.raw_message
                );
    }

}
fn handler_regin() -> Vec<Box<dyn MsgHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn MsgHandler + Send + Sync>> = vec![
        Box::new(Test { status: true }),
        Box::new(EmoMjk {
            status: true,
            ttf: OnceLock::new(),
        }),
        Box::new(Play{status:true})
    ];
    handlers
}
