use std::fmt::format;
use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{Error, anyhow};
use async_trait::async_trait;
use chrono::{Local, TimeZone, Utc};
use std::sync::atomic::AtomicBool;
use anyhow_trace::anyhow_trace;
use tracing::debug;

pub struct GcqrQuery {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GcqrQuery {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.contains(&[
            '+', '加', '-', '减', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
        ]) {
            return false;
        }
        let mut gc_name = String::new();
        debug!("ok11");
        if msg.raw_message.ends_with(&['j', '几']) {
            gc_name.push_str(msg.raw_message.strip_suffix(&['j', '几']).unwrap());
        } else if msg.raw_message.ends_with("几人") {
            gc_name.push_str(msg.raw_message.strip_suffix("几人").unwrap());
        } else if msg.raw_message.ends_with("几个人") {
            gc_name.push_str(msg.raw_message.strip_suffix("几个人").unwrap());
        } else {
            return false;
        }
        let name_map = GCNAME.map.read().await;
        debug!("ok22{}",!name_map.contains_key(&format!("{}{}", &gc_name, msg.group_id)));
        if !name_map.contains_key(&format!("{}{}", &gc_name, msg.group_id)) {
            debug!("not found");
            return false;
        }
        true
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let mut gc_name = String::new();
        if msg.raw_message.ends_with(&['j', '几']) {
            gc_name.push_str(msg.raw_message.strip_suffix(&['j', '几']).unwrap());
        } else if msg.raw_message.ends_with("几人") {
            gc_name.push_str(msg.raw_message.strip_suffix("几人").unwrap());
        } else if msg.raw_message.ends_with("几个人") {
            gc_name.push_str(msg.raw_message.strip_suffix("几个人").unwrap());
        }
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        let name_map = GCNAME.map.read().await;
        let gc_id = name_map
            .get(&format!("{}|{}", &gc_name, msg.group_id))
            .unwrap().gc_id;
        struct Data {
            report_time: i64,
            refresh: Option<bool>,
            headcount: i64,
            report_id: i64,
        }
        let res = sqlx::query_as!(
            Data,
            "select headcount,refresh,report_time,report_id from gamecenterdata where gc_id = $1",
            gc_id
        )
        .fetch_all(&mut *tx)
        .await?;
        if res.len() == 0 {
            Err(anyhow!("未找到机厅，未知错误"))
        } else if res.len() > 1 {
            Err(anyhow!("找到多个机厅，未知错误"))
        } else {
            let now_time = Local::now();
            let now_secs = now_time.timestamp();
            let now_day = now_time.date_naive().and_hms_opt(0, 0, 0).unwrap();
            let data_time = Local.timestamp_opt(res[0].report_time, 0).unwrap();
            let data_day = data_time.date_naive().and_hms_opt(0, 0, 0).unwrap();
            debug!("{} {} {} {} {} {}",res[0].report_time,now_secs,now_time,data_time,data_day,now_day);
            if res[0].refresh.unwrap_or(true) && data_day != now_day {
                let mut rep = SendMsg::new().await;
                rep.join_reply(msg.message_id).await;
                rep.join_text("今天还没有上报人数".to_string()).await;
                rep.send_msg(msg).await;
                return Ok(());
            }
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!(
                r#"机厅"{}"现在有 {} 人"#,
                gc_name,
                res[0].headcount,
            ))
                .await;
            rep.join_text(format!("\n上报时间: {}", data_time.format("%H:%M:%S"))).await;
            rep.join_text(format!("\n上报人: {}", res[0].report_id)).await;
            rep.send_msg(msg).await;
            Ok(())
        }
    }

    async fn init(&self) {
        self.status
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    async fn status(&self) -> bool {
        self.status.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "查询机厅人数使用\n使用方法：\n<机厅name><j/几/几人/几个人>\neg:\n大玩家几人".to_string()
    }

    async fn name(&self) -> String {
        "人数查询".to_string()
    }
}
