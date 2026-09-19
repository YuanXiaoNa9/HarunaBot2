use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::func_msg::game_center_manage::{sub_matches, useable_judgment};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tracing::debug;

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
        if i < 3 {
            return Err(anyhow!(
                "参数有误\n使用方式:\n/机厅管理 添加机厅 <name> <备注>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let name = vec_msg[2];
        if name.contains(&['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
            return Err(anyhow!("机厅名字不能包含数字"));
        }
        let description: &str;
        if vec_msg.get(3).is_some() {
            description = vec_msg[3]
        } else {
            description = "该机厅未备注"
        }
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        let res = sqlx::query!(
            "select name from gc_name where name = $1 and gid = $2",
            name,
            msg.group_id
        )
        .fetch_all(&mut *tx)
        .await?;
        if res.len() != 0 {
            debug!("{:?}", res.first());
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
            name
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(format!(
            "成功添加机厅: {}\n机厅备注为: {}\n机厅id为: {}",
            name, description, res.gc_id
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
        self.status.load(Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "添加一个新的机厅\n使用方式:\n/机厅管理 添加机厅 <name> <备注>".to_string()
    }

    async fn name(&self) -> String {
        "添加机厅".to_string()
    }
}
