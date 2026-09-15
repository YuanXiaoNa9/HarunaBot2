use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_func::game_center::sub_matches;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct GCMSearch {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMSearch {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let count = msg.raw_message.split(" ").count();
        if count != 3 {
            return Err(anyhow!(
                "参数错误\n使用方法：\n/机厅管理 搜索机厅 <机厅名字(别名)>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let search_name = vec_msg.get(2).unwrap();
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        struct Data {
            gc_id: i64,
            name: Option<String>,
            description: String,
        }
        let res = sqlx::query_as!(Data,"select gamecenterdata.gc_id,string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description from gc_name inner join gamecenterdata on gc_name.gc_id = gamecenterdata.gc_id where gc_name.name = $1 group by gamecenterdata.gc_id,gamecenterdata.description",search_name).fetch_all(&mut *tx).await?;
        tx.commit().await?;
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_text("查询列表".to_string()).await;
        for i in res {
            rep.join_text(format!(
                "\n\nid：{}\nname：{}\n备注：{}",
                i.gc_id,
                i.name.unwrap(),
                i.description
            ))
            .await;
        }
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed)
    }

    async fn status(&self) -> bool {
        self.status.load(Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "通过搜索机厅名字获取机厅id\n使用方法：\n/机厅管理 搜索机厅 <机厅名字(别名)>".to_string()
    }

    async fn name(&self) -> String {
        "搜索机厅".to_string()
    }
}
