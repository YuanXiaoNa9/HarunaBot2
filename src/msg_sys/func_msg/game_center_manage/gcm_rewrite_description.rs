use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tracing::debug;

pub struct GCMReDescription {
    pub status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMReDescription {
    async fn matches(&self, msg: &Msg) -> bool {
        let mut split_msg = msg.raw_message.split(" ");
        split_msg.next();
        let split2 = split_msg.next();
        if split2.is_none() {
            debug!("false1");
            return false;
        }
        if split2.unwrap() != "修改备注" {
            debug!("false2");
            return false;
        }
        true
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let split_msg = msg.raw_message.split(" ");
        if split_msg.count() != 4 {
            return Err(anyhow!(
                "参数错误\n使用方法：\n/机厅管理 修改备注 <机厅id/name> <备注内容>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let gc_id: i64 = if vec_msg[2].parse::<i64>().is_err() {
            let gc_id = match GCNAME
                .map
                .read()
                .await
                .get(format!("{}|{}", vec_msg[2], msg.group_id).as_str())
            {
                None => return Err(anyhow!("未找到该机厅")),
                Some(id) => id.gc_id,
            };
            gc_id
        } else {
            vec_msg[2].parse::<i64>()?
        };
        struct Data {
            admin_gid: i64,
            description: String,
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let res = sqlx::query_as!(
            Data,
            "select admin_gid,description from gamecenterdata where gc_id = $1",
            gc_id
        )
        .fetch_all(&mut *tx)
        .await?;
        if res.len() < 1 {
            return Err(anyhow!("未找到该机厅"));
        }
        if res[0].admin_gid != msg.group_id {
            return Err(anyhow!("没有权限修改绑定的机厅"));
        }
        sqlx::query!(
            "update gamecenterdata set description = $1 where gc_id = $2",
            vec_msg[3],
            gc_id
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(format!(
            "修改成功\nid: {}\n原备注: {}\n备注: {}",
            gc_id, res[0].description, vec_msg[3]
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
        "修改机厅备注(建议备注地名)\n使用方法：\n/机厅管理 修改备注 <机厅id/name> <备注内容>"
            .to_string()
    }

    async fn name(&self) -> String {
        "修改备注".to_string()
    }
}
