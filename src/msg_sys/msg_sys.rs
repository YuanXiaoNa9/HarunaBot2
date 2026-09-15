use crate::MAIN_CONFIG;
use crate::msg_sys::func_config::func_config_get;
use crate::msg_sys::handler_init::{mod_handlers_init, msg_handlers_init, notice_handlers_init};
pub(crate) use crate::msg_sys::msg_analysis::{Msg, msg_analysis};
use ab_glyph::FontVec;
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use tokio::spawn;
use tokio::sync::mpsc::Receiver;
use tracing::log::warn;
use tracing::{debug, error, info};

pub(crate) enum Handler {
    MsgFn(Vec<Box<dyn FnHandler + Send + Sync>>),
    NtcFn(Vec<Box<dyn FnHandler + Send + Sync>>),
    Mods(Vec<&'static (dyn ModHandler + Send + Sync)>),
}

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

//判断黑白名单
pub(crate) async fn bw_right(msg: &Msg) -> bool {
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
        return false;
    } else if MAIN_CONFIG.bw_status == "white" {
        for white_id in MAIN_CONFIG.white_list.iter() {
            if msg.sender.user_id == *white_id {
                return false;
            }
        }
    }
    info!(
        "非允许用户：{}({})",
        msg.sender.nickname, msg.sender.user_id
    );
    true
}
//打印接收消息
pub(crate) fn log_msg(msg: &Msg) {
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
pub(crate) async fn log_init(name: String, ok: bool) {
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

pub async fn sub_help(helps: &str, handlers: &Vec<Box<dyn FnHandler + Send + Sync>>) -> String {
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
    help_data.push_str("\n>\n\n使用\n/help <主功能>-<子功能>...\n来查询子功能详细用法\neg:\n/help 机厅管理-添加-删除\n或者使用-all获取全部子项详细信息");
    help_data
}
