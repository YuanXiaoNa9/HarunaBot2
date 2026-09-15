use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_func::game_center::{sub_matches, useable_judgment};
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, ModHandler, Msg};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use futures_util::TryFutureExt;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use crate::msg_sys::func_mod::gc_name::GCNAME;

pub struct GCMBind {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMBind {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let splits = msg.raw_message.split(" ");
        if splits.count() != 4 {
            return Err(anyhow!(
                "参数错误\n使用方法：\n/机厅管理 绑定机厅 <机厅id(无法使用名字绑定)> <自定义name>"
            ));
        }
        let vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
        let new_name = vec_msg[3];
        let bind_id = vec_msg[2].parse::<i64>()?;
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await?;
        #[derive(Debug)]
        struct Data {
            description: String,
            admin_gid: i64,
        }
        let res = sqlx::query_as!(
            Data,
            "select description,admin_gid from gamecenterdata where gc_id = $1",
            bind_id
        )
        .fetch_all(&mut *tx)
        .await?;
        if res.len() == 0 {
            return Err(anyhow!("未找到符合条件的机厅，请检查机厅id是否正确"));
        } else if res.len() > 1 {
            return Err(anyhow!(format!("寻找到多个数据，神秘未知错误{:?}", res)));
        } else {
            if res[0].admin_gid == msg.group_id {
                return Err(anyhow!("不能绑定该群聊创建的机厅"));
            }
            struct NameData {
                name: String,
            }
            let res1 = sqlx::query_as!(
                NameData,
                "select name from gc_name where name = $1",
                vec_msg[3]
            )
            .fetch_all(&mut *tx)
            .await?;
            if res1.len() != 0 {
                return Err(anyhow!("与现有机厅重名，请更换名字"));
            }
            sqlx::query!(
                "insert into gc_name (gc_id,name,gid) values ($1,$2,$3)",
                bind_id,
                new_name,
                msg.group_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!(
                "绑定机厅成功\nid: {}\nname: {}\n机厅备注: {}",
                bind_id, new_name, res[0].description
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
        "绑定机厅，可通过<搜索>功能获取需要的机厅id，然后将机厅绑定到群聊内，对该机厅除删除外功能可正常使用\n使用方法：\n/机厅管理 绑定机厅 <机厅id(无法使用名字绑定)> <自定义name>".to_string()
    }

    async fn name(&self) -> String {
        "绑定机厅".to_string()
    }
}
