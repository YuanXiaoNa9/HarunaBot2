use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use chrono::Local;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct GcqrSet {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GcqrSet {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.raw_message
            .ends_with(&['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'])
            && !msg.raw_message.contains(&['+', '加', '-', '减'])
            && GCNAME.map.read().await.contains_key(
                format!(
                    "{}|{}",
                    msg.raw_message
                        .trim_end_matches(|c: char| c.is_ascii_digit()),
                    msg.group_id
                )
                .as_str(),
            )
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let gc_name = msg
            .raw_message
            .trim_end_matches(|c: char| c.is_ascii_digit());
        let new_headcount = msg.raw_message[gc_name.len()..].parse::<i32>()?;
        let now_time = Local::now().timestamp();
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let gc_id = GCNAME
            .map
            .read()
            .await
            .get(format!("{}|{}", gc_name, msg.group_id).as_str())
            .ok_or_else(|| Error::msg("no gc name found"))?
            .gc_id;
        sqlx::query!("update gamecenterdata set (headcount,report_id,report_time) = ($1,$2,$3) where gc_id = $4", new_headcount,msg.sender.user_id,now_time,gc_id).execute(&mut *tx).await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(format!(
            r#"上报成功，现在"{}"有{}人"#,
            gc_name, new_headcount
        ))
        .await;
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed);
    }

    async fn status(&self) -> bool {
        self.status.load(Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "上报人数\n使用方法：\n<机厅name><人数>\neg：\n大玩家1".to_string()
    }

    async fn name(&self) -> String {
        "上报人数".to_string()
    }
}
