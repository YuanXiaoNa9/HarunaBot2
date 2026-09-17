use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use anyhow::{anyhow, Error};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use sqlx::Acquire;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::func_msg::game_center_manage::sub_matches;
use crate::msg_sys::msg_analysis::Msg;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::FnHandler;

pub struct GCMQueryGc {
    pub(crate) status:AtomicBool
}
#[async_trait]
impl FnHandler for GCMQueryGc {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg,self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let split_msg = msg.raw_message.split(" ").count();
        if split_msg != 2 {
            return Err(anyhow!("参数错误\n使用方法：\n/机厅管理 本群机厅"));
        }
        if msg.message_type != "group" {
            return Err(anyhow!("请在群聊中使用"))
        }
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        struct Data{
            gc_id:i64,
            name:Option<String>,
            description:String,
        }
        let res = sqlx::query_as!(Data,"select gamecenterdata.gc_id,string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description from gc_name inner join gamecenterdata on gc_name.gc_id=gamecenterdata.gc_id where gc_name.gid = $1 group by gamecenterdata.gc_id,gamecenterdata.description order by gamecenterdata.gc_id",msg.group_id).fetch_all(&mut *tx).await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        if res.len()==0{
            return Err(anyhow!("本群没有找到机厅"))
        }
        rep.join_text(format!("找到{}个机厅:",res.len())).await;
        for data in res {
            rep.join_text(format!("\n\nid: {}\nname: {}\n备注: {}",data.gc_id,data.name.unwrap(),data.description)).await;
        }
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
        "一键获取本群所有的机厅".to_string()
    }

    async fn name(&self) -> String {
        "本群机厅".to_string()
    }
}