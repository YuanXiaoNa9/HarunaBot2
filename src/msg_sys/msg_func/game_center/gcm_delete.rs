use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_func::game_center::{sub_matches, useable_judgment};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{anyhow, Error};
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use anyhow_trace::anyhow_trace;

pub struct GCMDelete {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMDelete {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg,self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let splits = msg.raw_message.split(" ").collect::<Vec<&str>>();
        if splits.len() != 3 {
            return Err(anyhow!("参数错误\n使用方法：\n/机厅管理 删除机厅 <name/id>"));
        }
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        // struct Data {
        //     gc_id: i64,
        //     description: String,
        // }
        let vec_splits: Vec<&str> = msg.raw_message.split(" ").collect();
        let name = vec_splits[2];
        let ok = vec_splits[2].parse::<i64>();
        let id = if ok.is_ok() { ok? } else { 0 };
        struct Data{
            gc_id:i64,
            name:Option<String>,
            description:String,
            admin_gid:i64,
        }
        let res = sqlx::query_as!(Data,"with matches as ( select gc_id from gc_name where (name = $1 or gc_id = $2)) select gamecenterdata.gc_id,string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description,gamecenterdata.admin_gid from gc_name inner join matches on matches.gc_id = gc_name.gc_id inner join gamecenterdata on gamecenterdata.gc_id = gc_name.gc_id where gamecenterdata.gc_id = matches.gc_id group by gamecenterdata.gc_id,gamecenterdata.description order by gamecenterdata.gc_id",name,id).fetch_all(&mut *tx).await?;
        let mut rep = SendMsg::new().await;
        if res.len() > 1 {
            rep.join_text("找到多个可删除项:".to_string()).await;
            for result in res {
                rep.join_text(format!("\nID:{}\nname:{}\n备注:\n{}", result.gc_id,result.name.unwrap(), result.description))
                    .await;
            }
            rep.send_forward_msg(msg).await;
        } else if res.len() == 1 {
            let result = res.get(0).unwrap();
            if result.admin_gid != msg.group_id {
                return Err(anyhow!("非可管理群聊，无法进行删除操作"))
            }
            sqlx::query!("delete from gamecenterdata where gc_id = $1", result.gc_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query!("delete from gc_name where gc_id = $1", result.gc_id)
                .execute(&mut *tx)
                .await?;
            if id == 0 {
                rep.join_text(format!("已删除名字为: <{}> 的机厅", splits[2]))
                    .await;
            } else {
                rep.join_text(format!("已删除id为: <{}> 的机厅", splits[2]))
                    .await;
            }
            rep.send_forward_msg(msg).await;
        } else if res.is_empty() {
  return Err(anyhow!("未找到可以删除的机厅"))
        }
        tx.commit().await?;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed)
    }

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        "删除已有的机厅，可以尝试使用名字或者id进行删除\n使用方法：\n/机厅管理 删除机厅 <name/id>".to_string()
    }

    async fn name(&self) -> String {
        "删除机厅".to_string()
    }
}
