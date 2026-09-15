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

pub struct GcqrPlus {
    pub(crate) status:AtomicBool
}
#[async_trait]
impl FnHandler for GcqrPlus {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.raw_message.contains(&['+','加'])
            && msg.raw_message.ends_with(&['0','1','2','3','4','5','6','7','8','9'])
            // && GCNAME.map.read().await.contains_key(format!("{}{}",msg.raw_message.split(&['+','加']).next().unwrap(), msg.group_id).as_str())
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let plus_headcount:i32 = msg.raw_message.split_once(&['+','加']).unwrap().1.parse()?;
        debug!("plus_headcount: {}", plus_headcount);
        if plus_headcount < 1 {
            return Err(Error::msg("plus headcount must be greater than 0"));
        }else if plus_headcount > 20 {
            return Err(Error::msg("plus headcount must be less than 20"));
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let now_time = Local::now().timestamp();
        let gc_name = msg.raw_message.split(&['+','加']).next().unwrap();
        let id = GCNAME.map.read().await.get(format!("{}{}",gc_name,msg.group_id).as_str()).unwrap().gc_id;
        let old_headcount = sqlx::query!("select headcount from gamecenterdata where gc_id = $1",id).fetch_one(&mut *tx).await?;
        let new_headcount = old_headcount.headcount+plus_headcount;
        sqlx::query!("update gamecenterdata set (headcount,report_id,report_time) = ($1,$2,$3) where gc_id = $4",new_headcount,msg.sender.user_id,now_time,id).execute(&mut *tx).await?;
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
        "机厅增加人数\n使用方法:\n<机厅name><+/加><人数>\neg:\n大玩家+1".to_string()
    }

    async fn name(&self) -> String {
        "增加人数".to_string()
    }
}
