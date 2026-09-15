use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use chrono::Local;
use futures_util::FutureExt;
use sqlx::Acquire;
use tracing::debug;
use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler, Msg};

pub struct GcqrMinus {
    pub(crate) status:AtomicBool
}
#[async_trait]
impl FnHandler for GcqrMinus {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.raw_message.contains(&['-','减'])
            && msg.raw_message.ends_with(&['0','1','2','3','4','5','6','7','8','9'])
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let minus_headcount:i32 = msg.raw_message.split_once(&['-','减']).unwrap().1.parse()?;
        debug!("minus_headcount: {}", minus_headcount);
        if minus_headcount < 1 {
            return Err(Error::msg("minus headcount must be greater than 0"));
        }else if minus_headcount > 20 {
            return Err(Error::msg("minus headcount must be less than 20"));
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let now_time = Local::now().timestamp();
        let gc_name = msg.raw_message.split(&['-','减']).next().unwrap();
        let id = GCNAME.map.read().await.get(format!("{}|{}",gc_name,msg.group_id).as_str()).unwrap().gc_id;
        let old_headcount = sqlx::query!("select headcount from gamecenterdata where gc_id = $1",id).fetch_one(&mut *tx).await?;
        let new_headcount = old_headcount.headcount- minus_headcount;
        if new_headcount < 0 {
            return Err(Error::msg("人数为负？zdjd"));
        }
        sqlx::query!("update gamecenterdata set (headcount,report_id,report_time) = ($1,$2,$3)where gc_id = $4",new_headcount,msg.sender.user_id,now_time,id).execute(&mut *tx).await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(format!(r#"上报成功，现在"{}"有{}人"#, gc_name, new_headcount)).await;
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
        "机厅减少人数\n使用方法:\n<机厅name><-/减><人数>\neg:\n大玩家-1".to_string()
    }

    async fn name(&self) -> String {
        "减少人数".to_string()
    }
}
