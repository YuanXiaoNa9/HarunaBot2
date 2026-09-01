use crate::MAIN_CONFIG;
use crate::msg_sys::func_config::func_config_get;
use crate::msg_sys::func_mod::postgres_db::DbLink;
use crate::msg_sys::func_mod::ttf::TtfData;
use crate::msg_sys::msg_func::emojimujika::EmoMjk;
use crate::msg_sys::msg_func::help::Help;
use crate::msg_sys::msg_func::play::Play;
use crate::msg_sys::msg_func::plusone::PlusOne;
use crate::msg_sys::msg_func::test::Test;
use crate::msg_sys::msg_func::ttt::TTT;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::spawn;
use tokio::sync::mpsc::Receiver;
use tracing::{debug, error, info, warn};

pub static MSGHANDLERS: OnceLock<Vec<Box<dyn MsgHandler + Send + Sync>>> = OnceLock::new();

//注册功能函数
#[async_trait]
pub trait MsgHandler {
    async fn matches(&self, _: Arc<Msg>) -> bool;
    async fn process(&self, _: Arc<Msg>);
    async fn init(&mut self) -> bool;
    async fn status(&self) -> bool;
    async fn help(&self) -> String;
    async fn name(&self) -> String;
}
#[async_trait]
pub trait ModHandler {
    async fn init(&self) -> bool;
    async fn name(&self) -> String;
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
//消息系统主逻辑
pub async fn msg_sys(mut msg_chan: Receiver<String>) {
    //获取模块配置文件
    func_config_get();
    //模块初始化
    mod_handlers_init().await;
    let msg_handlers = msg_handlers_init().await;
    let _ = MSGHANDLERS.set(msg_handlers);
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
                    error!("解析消息出现错误：{}", e);
                    return;
                }
            };
            //判断是否为文字消息
            if msg.post_type != "message" {
                return;
            }
            //判断是否为黑白名单用户
            if bw_right(&msg).await {
                return;
            };
            //打印消息日志
            log_msg(&msg);
            //进行解析
            dispatch(msg).await;
        });
    }
}

async fn dispatch(msg: Msg) {
    debug!("finding handler");
    let msg = Arc::new(msg);
    //取出handler
    for handler in MSGHANDLERS.get().unwrap().iter() {
        //使用handler的match方法进行判断消息是否符合
        if handler.matches(msg.clone()).await && handler.status().await {
            debug!("find handler");
            handler.process(msg.clone()).await;
            return;
        }
    }
    debug!("not find handler");
}
//模块函数初始化
async fn mod_handlers_init() {
    let handlers = mod_handler_regin();
    for handler in handlers {
        let ok = handler.init().await;
        if ok {
            info!("<{}>模块初始化成功", handler.name().await);
        } else {
            warn!("<{}>模块未初始化", handler.name().await);
        }
    }
}
//功能函数初始化
async fn msg_handlers_init() -> Vec<Box<dyn MsgHandler + Send + Sync>> {
    //注册功能模块
    let handlers = msg_handler_regin();
    //创建新的vec存储handler
    let mut init_handlers = Vec::new();
    //取出handler并执行初始化方法
    for mut handler in handlers {
        let ok = handler.init().await;
        if ok {
            info!("<{}>模块初始化成功", handler.name().await);
        } else {
            warn!("<{}>模块未初始化", handler.name().await);
        }
        init_handlers.push(handler);
    }
    init_handlers
}

//判断黑白名单
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
//打印接收消息
fn log_msg(msg: &Msg) {
    if msg.message_type == "group" {
        info!(
            "[{}]({}):[{}]({}) => <{}>",
            msg.group_name, msg.group_id, msg.sender.nickname, msg.sender.user_id, msg.raw_message
        );
    } else if msg.message_type == "private" {
        info!(
            "{}({}): <{}>",
            msg.sender.nickname, msg.sender.user_id, msg.raw_message
        );
    }
}
fn mod_handler_regin() -> Vec<Box<dyn ModHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn ModHandler + Send + Sync>> = vec![
        Box::new(TtfData {
            status: false,
            ttf: Default::default(),
        }),
        Box::new(DbLink {
            status: false,
            db_link: Default::default(),
        }),
    ];
    handlers
}
//注册功能函数
fn msg_handler_regin() -> Vec<Box<dyn MsgHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn MsgHandler + Send + Sync>> = vec![
        Box::new(Test { status: true }),
        Box::new(EmoMjk { status: true }),
        Box::new(Play { status: true }),
        Box::new(Help { status: true }),
        Box::new(TTT { status: true }),
        Box::new(PlusOne {
            status: true,
            map: OnceLock::new(),
        }),
    ];
    handlers
}
