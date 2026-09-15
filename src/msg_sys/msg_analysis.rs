use crate::msg_sys::handler_init::{MSG_HANDLERS, NOTICE_HANDLERS};
use crate::msg_sys::msg_reply::SendMsg;
use anyhow_trace::anyhow_trace;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};
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
    pub(crate) notice_type: String,
}

pub async fn msg_analysis(msg: String) {
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
    if crate::msg_sys::msg_sys::bw_right(&msg).await {
        return;
    };
    //打印消息日志
    crate::msg_sys::msg_sys::log_msg(&msg);
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
