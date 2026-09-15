use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::func_msg::game_center::{sub_matches, useable_judgment};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct GCMDeleteName {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMDeleteName {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let count = msg.raw_message.split(" ").count();
        if count != 3 && count != 4 {
            return Err(anyhow!(
                "参数错误\n使用方法：\n/机厅管理 删除名字 <id(可选)> <name>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let id: Option<i64>;
        let name: &str;
        if count == 3 {
            name = vec_msg.get(2).unwrap();
            id = None
        } else {
            name = vec_msg.get(3).unwrap();
            id = Some(vec_msg.get(2).unwrap().parse::<i64>()?)
        }
        struct Data {
            gc_id: i64,
            name: String,
            description: String,
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let res = sqlx::query_as!(Data,"select gamecenterdata.gc_id,gamecenterdata.description,gc_name.name from gc_name inner join gamecenterdata on gc_name.gc_id = gamecenterdata.gc_id where (gc_name.name = $2 and ($1::bigint is null or gc_name.gc_id = $1::bigint)) and gc_name.gid = $3",id,name,msg.group_id).fetch_all(&mut *tx).await?;
        if res.len() == 0 {
            return Err(anyhow!("未找到符合条件的机厅"));
        } else if res.len() > 1 {
            let mut rep = String::new();
            rep.push_str("找到多个符合条件的机厅");
            for i in res {
                rep.push_str(
                    format!("\nid:{} name:{}\n备注:{}", i.gc_id, i.name, i.description).as_str(),
                );
            }
            rep.push_str("\n\n请使用:\n/机厅管理 删除名字 <id> <name>\n来指定删除的名字");
        } else {
            let data = res.get(0).unwrap();
            let res1: (i64,) = sqlx::query_as("select count(*) from gc_name where gc_id = $1")
                .bind(data.gc_id)
                .fetch_one(&mut *tx)
                .await?;
            if res1.0 == 1 {
                return Err(anyhow!(
                    "当前机厅名字数量为1，无法进行删除名字操作，至少为机厅保持一个名字，如需删除，请使用删除机厅"
                ));
            }
            sqlx::query!(
                "delete from gc_name where gc_id = $1 and name = $2 and gid = $3",
                data.gc_id,
                data.name,
                msg.group_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!("已删除机厅id：{}的名字：{}", data.gc_id, data.name))
                .await;
            rep.send_msg(msg).await;
        }
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
        "删除名字\n注：\n当仅剩一个名字时无法使用\n使用方法：\n/机厅管理 删除名字 <id(可选)> <name>"
            .to_string()
    }

    async fn name(&self) -> String {
        "删除名字".to_string()
    }
}
