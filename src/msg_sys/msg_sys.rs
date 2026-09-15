use crate::MAIN_CONFIG;
use crate::msg_sys::func_config::func_config_get;
use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::func_mod::ttf::TTF;
use crate::msg_sys::msg_func::gc_query_report::GCQR;
use crate::msg_sys::msg_func::gc_query_report::gcqr_query::GcqrQuery;
use crate::msg_sys::msg_func::emoji_photo::MemPhoto;
use crate::msg_sys::msg_func::emoji_say::{EmjSay, EmoMjk};
use crate::msg_sys::msg_func::game_center::GameCenterManager;
use crate::msg_sys::msg_func::game_center::gcm_add::GCMAdd;
use crate::msg_sys::msg_func::game_center::gcm_add_name::GCMAddName;
use crate::msg_sys::msg_func::game_center::gcm_delete::GCMDelete;
use crate::msg_sys::msg_func::game_center::gcm_rename::GCMRename;
use crate::msg_sys::msg_func::help::Help;
use crate::msg_sys::msg_func::play::Play;
use crate::msg_sys::msg_func::plusone::PlusOne;
use crate::msg_sys::msg_func::test::Test;
use crate::msg_sys::msg_func::ttt::TTT;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::notice_func::poke::Poke;
use ab_glyph::FontVec;
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tokio::spawn;
use tokio::sync::mpsc::Receiver;
use tracing::log::warn;
use tracing::{debug, error, info};
use crate::msg_sys::msg_func::game_center::gcm_bind::GCMBind;
use crate::msg_sys::msg_func::game_center::gcm_delete_name::GCMDeleteName;
use crate::msg_sys::msg_func::game_center::gcm_search::GCMSearch;
use crate::msg_sys::msg_func::game_center::gcm_unbind::GCMUnbind;
use crate::msg_sys::msg_func::gc_query_report::gcqr_repo_plus::GcqrPlus;
use crate::msg_sys::msg_func::gc_query_report::gcqr_report_minus::GcqrMinus;
use crate::msg_sys::msg_func::gc_query_report::gcqr_report_set::GcqrSet;

enum Handler {
    MsgFn(Vec<Box<dyn FnHandler + Send + Sync>>),
    NtcFn(Vec<Box<dyn FnHandler + Send + Sync>>),
    Mods(Vec<&'static (dyn ModHandler + Send + Sync)>),
}
pub enum Subroutine {
    Main(()),
    EmjSay(EmjSay),
}
pub static MSG_HANDLERS: OnceLock<Vec<Box<dyn FnHandler + Send + Sync>>> = OnceLock::new();
pub static NOTICE_HANDLERS: OnceLock<Vec<Box<dyn FnHandler + Send + Sync>>> = OnceLock::new();
//注册功能函数
#[async_trait]
pub trait FnHandler {
    async fn matches(&self, _: &Msg) -> bool;
    #[anyhow_trace]
    async fn process(&self, _: &Msg) -> Result<(), Error>;
    async fn init(&self);
    async fn status(&self) -> bool;
    async fn help(&self, _: &str) -> String;
    async fn name(&self) -> String;
}

#[async_trait]
pub trait ModHandler {
    async fn init(&self);
    async fn name(&self) -> String;
    async fn init_status(&self) -> bool;
}
pub enum ModData {
    Ttf(&'static FontVec),
    Pgpool(&'static Pool<Postgres>),

    Err(bool),
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
    pub target_id: i64,
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
    spawn(async move {
        mod_handlers_init().await;
    });

    notice_handlers_init().await;
    msg_handlers_init().await;
    loop {
        //从通道中取出json消息
        let msg: String = match msg_chan.recv().await {
            None => {
                error!("消息接收出现错误");
                std::process::exit(1);
            }
            Some(i) => {
                debug!("{}", i);
                i
            }
        };
        //判断是否为空字符串（心跳）
        if msg == "" {
            continue;
        }
        //后台执行
        spawn(async move {
            msg_analysis(msg).await;
        });
    }
}
//消息路由
async fn msg_analysis(msg: String) {
    //使用MsgGet结构体进行解析
    let mut msg: Msg = match serde_json::from_str(msg.as_str()) {
        Ok(msg_struct) => msg_struct,
        Err(e) => {
            error!("解析消息出现错误：{}", e);
            return;
        }
    };
    if msg.raw_message == "" {
        msg.raw_message = " ".to_string();
    }
    //判断是否为黑白名单用户
    if bw_right(&msg).await {
        return;
    };
    //打印消息日志
    log_msg(&msg);
    if msg.post_type == "notice" {
        //判断是否为提醒类消息
        notice_dispatch(msg).await;
        return;
    } else if msg.post_type == "message" {
        //判断是否为文字类消息
        msg_dispatch(msg).await;
        return;
    }
}
//msg（普通）消息dispatch，由msg_analysis路由
async fn msg_dispatch(msg: Msg) {
    debug!("finding msg handler");
    //取出handler
    for handler in MSG_HANDLERS.get().unwrap().iter() {
        debug!(
            "match {} handler status {}",
            handler.name().await,
            handler.status().await
        );
        //使用handler的match方法进行判断消息是否符合
        if !handler.status().await || !handler.matches(&msg).await {
            continue;
        }
        let res = handler.process(&msg).await;
        if res.is_err() {
            let err = res.unwrap_err();
            error!("{}", err);
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!("{:#}", err).to_string()).await;
            rep.send_msg(&msg).await;
        }
        return;
    }
    debug!("not find handler");
}

//notice消息dispatch，由msg_analysis路由
#[anyhow_trace]
async fn notice_dispatch(msg: Msg) {
    debug!("finding notice handler");
    for handler in NOTICE_HANDLERS.get().unwrap().iter() {
        debug!("match {} handler", handler.name().await);
        if !handler.status().await || !handler.matches(&msg).await {
            continue;
        }
        let res = handler.process(&msg).await;
        if res.is_err() {
            let err = res.unwrap_err();
            error!("{}", err);
            let mut rep = SendMsg::new().await;
            rep.join_text(format!("{:#}", err).to_string()).await;
            rep.send_forward_msg(&msg).await;
        }
        return;
    }
    debug!("not find handler");
}

//模块注册函数
fn mod_handler_regin() -> Vec<&'static (dyn ModHandler + Send + Sync)> {
    let handlers: Vec<&'static (dyn ModHandler + Send + Sync)> = vec![&*TTF, &*DBLINK, &*GCNAME];
    handlers
}

//msg功能注册函数
fn msg_handler_regin() -> Vec<Box<dyn FnHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn FnHandler + Send + Sync>> = vec![
        Box::new(Test {
            status: AtomicBool::from(false),
        }),
        Box::new(EmoMjk {
            enable: true,
            status: AtomicBool::from(false),
        }),
        Box::new(Play {
            status: AtomicBool::from(false),
        }),
        Box::new(Help {
            status: AtomicBool::from(false),
        }),
        Box::new(TTT {
            enable: true,
            status: AtomicBool::from(false),
        }),
        Box::new(MemPhoto {
            status: AtomicBool::from(false),
        }),
        Box::new(GameCenterManager {
            enable: true,
            status: Default::default(),
            subfunction: vec![
                Box::new(GCMAdd {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMDelete {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMRename {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMAddName {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMBind {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMUnbind {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMDeleteName {
                    status: AtomicBool::from(false),
                }),
                Box::new(GCMSearch {
                    status: AtomicBool::from(false),
                }),
            ],
        }),
        Box::new(GCQR {
            status: AtomicBool::from(false),
            sub_function: vec![Box::new(GcqrQuery {
                status: AtomicBool::from(false),
            }),Box::new(GcqrSet {
                status: AtomicBool::from(false),
            }),Box::new(GcqrPlus {
                status: AtomicBool::from(false),
            }),Box::new(GcqrMinus {
                status: AtomicBool::from(false),
            })
            ],
        }),
        Box::new(PlusOne {
            status: AtomicBool::from(false),
            map: OnceLock::new(),
        }),
    ];
    handlers
}
//notice功能注册函数
fn notice_handler_regin() -> Vec<Box<dyn FnHandler + Send + Sync>> {
    let handlers: Vec<Box<dyn FnHandler + Send + Sync>> = vec![Box::new(Poke {
        status: AtomicBool::from(false),
    })];
    handlers
}
//模块功能初始化函数
async fn mod_handlers_init() {
    let handlers = mod_handler_regin();
    mian_init(Handler::Mods(handlers)).await;
}
//模块功能初始化主函数
async fn mian_init(handlers: Handler) {
    match handlers {
        Handler::MsgFn(handlers) => {
            let _ = MSG_HANDLERS.set(handlers);
            for handler in MSG_HANDLERS.get().unwrap().iter() {
                spawn(async move {
                    let _ = &handler.init().await;
                    log_init(handler.name().await, handler.status().await).await;
                });
            }
        }
        Handler::NtcFn(handlers) => {
            let _ = NOTICE_HANDLERS.set(handlers);
            for handler in NOTICE_HANDLERS.get().unwrap().iter() {
                spawn(async move {
                    let _ = &handler.init().await;
                    log_init(handler.name().await, handler.status().await).await;
                });
            }
        }
        Handler::Mods(handlers) => {
            for handler in handlers {
                spawn(async move {
                    handler.init().await;
                    log_init(handler.name().await, handler.init_status().await).await;
                });
            }
        }
    }
}

//msg功能函数初始化
async fn msg_handlers_init() {
    //注册功能模块
    let handlers = msg_handler_regin();
    //取出handler并执行初始化方法
    mian_init(Handler::MsgFn(handlers)).await;
}

//notice功能函数初始化
async fn notice_handlers_init() {
    let handlers = notice_handler_regin();
    mian_init(Handler::NtcFn(handlers)).await;
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
    if msg.message_type == "group" && msg.sender.user_id != msg.self_id {
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
async fn log_init(name: String, ok: bool) {
    if ok {
        info!("<{}>初始化成功", name);
    } else {
        warn!("<{}>初始化失败", name);
    }
}
#[anyhow_trace]
pub async fn sub_match_process(
    msg: &Msg,
    handlers: &Vec<Box<dyn FnHandler + Send + Sync>>,
) -> Result<(), Error> {
    for handler in handlers {
        if !handler.status().await || !handler.matches(msg).await {
            continue;
        }
        handler.process(msg).await?;
        return Ok(());
    }
    Err(anyhow!("参数有误，请检查参数".to_string()))
}
pub async fn sub_init(handlers: &Vec<Box<dyn FnHandler + Send + Sync>>) {
    for handler in handlers {
        handler.init().await;
        log_init(handler.name().await, handler.status().await).await;
    }
}

pub async fn mod_status_examine(mut rx: tokio::sync::watch::Receiver<bool>) -> bool {
    if *rx.borrow_and_update() {
        true
    } else {
        loop {
            let _ = rx.changed().await;
            if *rx.borrow_and_update() {
                return true;
            } else {
                continue;
            }
        }
    }
}

pub async fn sub_help(helps:&str, handlers: &Vec<Box<dyn FnHandler + Send + Sync>>) -> String {
    let mut helps_splits = helps.split("-");
    helps_splits.next();
    let mut help_data = String::new();
    let mut unfind_help_data = String::new();
    'a: for help in helps_splits {
        for subfunction in handlers {
            let name = subfunction.name().await;
            if (subfunction.status().await && name == help) || helps.contains("all") {
                let data = subfunction.help(help).await;
                help_data.push_str(format!("<{}>\n{}\n", name, data.as_str()).as_str());
                if !helps.contains("all") {
                    continue 'a;
                } else {
                    continue;
                }
            }
        }
        if helps.contains("all") {
            continue 'a;
        }
        unfind_help_data.push_str(format!("<{}>", help).as_str());
    }

    if !help_data.is_empty() {
        let res = help_data.strip_suffix("\n").unwrap().to_string();
        return res;
    }
    help_data.push_str(">");
    let mut sub_help_data = String::new();
    for handler in handlers {
        let name = handler.name().await;
        if name != "help".to_string() && handler.status().await {
            sub_help_data.push_str(format!("\n{}", name).as_str());
        }
    }
    if sub_help_data.is_empty() {
        sub_help_data.push_str("\n...");
    }
    help_data.push_str(sub_help_data.as_str());
    help_data.push_str("\n>\n\n使用\n/help <主功能>-<子功能>...\n来查询子功能详细用法\neg:\n/help 机厅管理-添加-删除");
    help_data
}