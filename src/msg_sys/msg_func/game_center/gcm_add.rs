use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_func::game_center::{sub_matches, useable_judgment};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use crate::msg_sys::func_mod::gc_name::GCNAME;

pub struct GCMAdd {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMAdd {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let i = msg.raw_message.split(" ").count() as i16;
        if i != 4 {
            return Err(anyhow!(
                "参数有误\n使用方式:\n/机厅管理 添加机厅 <name> <备注>"
            ));
        }
        let splits = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let description: &str;
        if !splits[3].is_empty() {
            description = splits[3]
        } else {
            description = "该机厅未备注"
        }
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        struct NameData {
            name: String,
        }
        let res = sqlx::query_as!(
            NameData,
            "select name from gc_name where name = $1",
            splits[2]
        )
        .fetch_all(&mut *tx)
        .await?;
        if res.len() != 0 {
            return Err(anyhow!("与现有机厅重名，请更换名字"));
        }
        struct Data {
            gc_id: i64,
        }
        let res = sqlx::query_as!(
            Data,
            "insert into gamecenterdata (admin_gid,description) values ($1,$2) returning gc_id",
            msg.group_id,
            description
        )
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query!(
            "insert into gc_name (gc_id,gid,name)values($1,$2,$3)",
            res.gc_id,
            msg.group_id,
            splits[2]
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_text(format!(
            "成功添加机厅: {}\n机厅备注为: {}\n机厅id为: {}",
            splits[2], splits[3], res.gc_id
        ))
        .await;
        rep.send_forward_msg(msg).await;
        GCNAME.init().await;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed)
    }

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        "添加一个新的机厅\n使用方式:\n/机厅管理 添加机厅 <name> <备注>".to_string()
    }

    async fn name(&self) -> String {
        "添加机厅".to_string()
    }
}
