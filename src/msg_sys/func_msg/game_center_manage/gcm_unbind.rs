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

pub struct GCMUnbind {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMUnbind {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let count = msg.raw_message.split(" ").count();
        if count != 3 {
            return Err(anyhow!(
                "参数错误\n使用方法：\n/机厅管理 解除绑定 <机厅id/机厅名字(别名)>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let id: Option<i64>;
        let name: Option<String>;
        let ok = vec_msg.get(2).unwrap().parse::<i64>();
        if ok.is_ok() {
            name = None;
            id = Some(ok?);
        } else {
            name = Some(vec_msg.get(2).unwrap().to_string());
            id = None;
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        struct Data {
            gc_id: i64,
            name: Option<String>,
            description: String,
            admin_gid: i64,
        }
        let res = sqlx::query_as!(Data,"with matches as (select distinct gc_id from gc_name where (($2::bigint is null or gc_id = $2::bigint) and ($1::text is null or name = $1::text))and gid = $3) select gc_name.gc_id,string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description,gamecenterdata.admin_gid from gc_name inner join matches on gc_name.gc_id = matches.gc_id inner join gamecenterdata on gc_name.gc_id = gamecenterdata.gc_id where gc_name.gid = $3 group by gc_name.gc_id,gamecenterdata.description,gamecenterdata.admin_gid",name,id,msg.group_id).fetch_all(&mut *tx).await?;
        if res.len() == 0 {
            return Err(anyhow!("未找到符合条件的机厅"));
        } else if res.len() > 1 {
            let mut rep = String::new();
            rep.push_str("找到多个机厅，使用\n/机厅管理 解绑机厅 <机厅id>\n进行解绑操作");
            for i in res {
                rep.push_str(
                    format!(
                        "\nid:{} name:{} 备注:{}",
                        i.gc_id,
                        i.name.unwrap(),
                        i.description
                    )
                    .as_str(),
                );
            }
            return Err(anyhow!(rep));
        } else {
            if res[0].admin_gid == msg.group_id {
                return Err(anyhow!(
                    "该机厅为当前群聊创建，非绑定机厅，如需删除请使用“删除机厅”"
                ));
            }
            sqlx::query!(
                "delete from gc_name where gc_id = $1 and gid = $2",
                res[0].gc_id,
                msg.group_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!(
                "成功解除机厅绑定\nid:{}\nname:{}\n备注:{}",
                res[0].gc_id,
                res[0].name.clone().unwrap(),
                res[0].description
            ))
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
        "解除绑定的机厅\n使用方法：\n/机厅管理 解除绑定 <机厅id/机厅名字(别名)>".to_string()
    }

    async fn name(&self) -> String {
        "解绑机厅".to_string()
    }
}
