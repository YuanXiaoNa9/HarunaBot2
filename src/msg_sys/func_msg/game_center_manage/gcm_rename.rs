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

pub struct GCMRename {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for GCMRename {
    async fn matches(&self, msg: &Msg) -> bool {
        sub_matches(msg, self.name().await)
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        useable_judgment(msg).await?;
        let vec_msg: Vec<&str>;
        let gc_name: &str;
        let mut id: i64 = 0;
        let new_name: &str;
        if msg.raw_message.split(" ").count() == 4 {
            vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
            gc_name = vec_msg[2];
            new_name = vec_msg[3];
            if gc_name.contains(&['0','1','2','3','4','5','6','7','8','9']) {
                return Err(anyhow!("机厅名字不能包含数字"))
            }
        } else if msg.raw_message.split(" ").count() == 5 {
            vec_msg = msg.raw_message.split(" ").collect::<Vec<&str>>();
            gc_name = vec_msg[3];
            new_name = vec_msg[4];
            if gc_name.contains(&['0','1','2','3','4','5','6','7','8','9']) {
                return Err(anyhow!("机厅名字不能包含数字"))
            }
            let ok = vec_msg[2];
            if ok.parse::<i64>().is_ok() {
                id = ok.parse::<i64>()?;
            }
        } else {
            return Err(anyhow!(
                "参数错误\n使用方法:\n/机厅管理 重命名 <机厅id(可选)> <origin_name> <new_name>"
            ));
        }
        struct DataNames {
            name: Option<String>,
            description: String,
            gc_id: i64,
        }
        let mut tx = DBLINK.db_link.get().unwrap().begin().await?;
        let res = sqlx::query_as!(DataNames,"with id as(select gc_id from gc_name where (name = $1 or gc_id = $2) and gid = $3) select string_agg(gc_name.name,' 'order by gc_name.name)as name,gamecenterdata.description,gamecenterdata.gc_id from gc_name inner join id on gc_name.gc_id = id.gc_id inner join gamecenterdata on id.gc_id = gamecenterdata.gc_id group by gamecenterdata.gc_id, gamecenterdata.description order by gamecenterdata.gc_id",gc_name,id,msg.group_id).fetch_all(&mut *tx).await?;
        let data_counts = res.len();
        if data_counts == 0 {
            return Err(anyhow!("未找到符合条件的机厅"));
        } else if data_counts > 1 {
            let mut query_list: String = String::new();
            query_list.push_str("查找到多个相同名称：\n");
            for data in res {
                query_list.push_str(
                    format!(
                        "\nid: {}\nname: {}\n备注: {}",
                        &data.gc_id,
                        &data.name.unwrap(),
                        &data.description
                    )
                    .as_str(),
                );
            }
            return Err(anyhow!("{}\n\n请使用机厅id进行", query_list));
        } else {
            let res1 = sqlx::query!("select name from gc_name where name = $1", new_name)
                .fetch_all(&mut *tx)
                .await?;
            if res1.len() != 0 {
                return Err(anyhow!("与现有机厅重名，请更换名字"));
            }
            let _ = sqlx::query!(
                "update gc_name set name = $1 where name = $2 and gid = $3 and gc_id = $4",
                new_name,
                gc_name,
                msg.group_id,
                res[0].gc_id
            )
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            let mut rep = SendMsg::new().await;
            rep.join_reply(msg.message_id).await;
            rep.join_text(format!(
                "成功修改机厅名字\nid:{}\nold_name:{}\nnew_name:{}",
                res[0].gc_id, gc_name, new_name
            ))
            .await;
            rep.send_forward_msg(msg).await;
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
        "重命名机厅（可重命名别名，修改一个名字不影响别名使用）\n例如，机厅a有名字a(本名也算),b,c三个名字,机厅id为3,修改名字a为d之后，id为3的机厅拥有的名字就为b,c,d\n使用方法:\n/机厅管理 重命名  <origin_name> <new_name> ".to_string()
    }

    async fn name(&self) -> String {
        "重命名".to_string()
    }
}
