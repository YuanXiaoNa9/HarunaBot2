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

pub struct GCMAddName {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMAddName {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let splits = msg.raw_message.split(" ");
        if splits.count() != 4 {
            return Err(anyhow!(
                "参数有误\n使用方法：\n/机厅管理 添加名字 <机厅id/机厅name> <new_name>"
            ));
        }
        let vec_msg: Vec<&str> = msg.raw_message.split(" ").collect();
        if vec_msg[2].contains(&['0', '1', '2', '3', '4', '5', '6', '7', '8', '9']) {
            return Err(anyhow!("机厅名字不能包含数字"));
        }
        let name = vec_msg[2];
        let ok = vec_msg[2].parse::<i64>();
        let id = if ok.is_ok() { ok? } else { 0 };
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        struct Data {
            gc_id: i64,
            name: Option<String>,
            description: String,
        }
        let res = sqlx::query_as!(Data,"with matches1 as ( select distinct gc_id from gc_name where (name = $1 or gc_id = $2) and gid = $3)select gamecenterdata.gc_id,string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description from gc_name inner join gamecenterdata on gc_name.gc_id = gamecenterdata.gc_id inner join matches1 on matches1.gc_id = gc_name.gc_id where gc_name.gid = $3 group by gamecenterdata.gc_id, gamecenterdata.description order by gamecenterdata.gc_id",name,id,msg.group_id).fetch_all(&mut *tx).await?;
        if res.len() == 0 {
            return Err(anyhow!("未找到符合条件的机厅"));
        } else if res.len() > 1 {
            let mut rep = String::new();
            rep.push_str("找到多个同名机厅：");
            for res in res {
                rep.push_str(
                    format!(
                        "\nid: {}\nname: {}\n备注: {}",
                        res.gc_id,
                        res.name.unwrap(),
                        res.description
                    )
                    .as_str(),
                );
            }
            return Err(anyhow!(rep));
        }
        let old_names = res[0].name.clone().unwrap();
        let res1 = sqlx::query!(
            "select name from gc_name where name = $1 and gid = $2",
            vec_msg[3],
            msg.group_id
        )
        .fetch_all(&mut *tx)
        .await?;
        if res1.len() != 0 {
            return Err(anyhow!("与现有机厅重名，请更换名字"));
        }

        sqlx::query!(
            "insert into gc_name (gc_id,gid,name)values ($1,$2,$3)",
            res[0].gc_id,
            msg.group_id,
            vec_msg[3]
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text(format!(
            "添加名字成功\nid: {}\nname: {} {}\n备注: {}",
            res[0].gc_id, old_names, vec_msg[3], res[0].description
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
        "为你的机厅添加别名\n注：\n一个机厅不能有重复名字\n使用方法：\n/机厅管理 添加名字 <机厅id/机厅name> <new_name>".to_string()
    }
    async fn name(&self) -> String {
        "添加名字".to_string()
    }
}
