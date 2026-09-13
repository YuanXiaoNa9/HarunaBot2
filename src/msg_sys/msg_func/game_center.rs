pub mod gcm_add;
pub mod gcm_delete;
pub mod gcm_rename;
pub mod gcm_bind;
pub mod gcm_add_name;
pub mod gcm_delete_name;
pub mod gcm_unbind;
pub mod gcm_search;

use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_sys::{FnHandler, Msg, sub_init, sub_match_process};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct GameCenterManager {
    pub enable: bool,
    pub status: AtomicBool,
    pub(crate) subfunction: Vec<Box<dyn FnHandler + Send + Sync>>,
}
#[async_trait]
impl FnHandler for GameCenterManager {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("/机厅管理") {
            return true;
        }
        return false;
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        sub_match_process(msg, &self.subfunction).await?;
        Ok(())
    }

    async fn init(&self) {
        let mut rx = DBLINK.rx.clone();
        if *rx.borrow_and_update() {
            self.status.store(true, Relaxed);
        } else {
            loop {
                let _ = rx.changed().await;
                if *rx.borrow_and_update() {
                    self.status.store(true, Relaxed);
                    break;
                } else {
                    continue;
                }
            }
        }
        pg_init().await;
        sub_init(&self.subfunction).await;
    }

    async fn status(&self) -> bool {
        if *DBLINK.rx.clone().borrow() && self.enable {
            return true;
        }
        false
    }

    async fn help(&self, helps: &str) -> String {
        let mut helps_splits = helps.split("-");
        helps_splits.next();
        let mut help_data = String::new();
        let mut unfind_help_data = String::new();
        'a: for help in helps_splits {
            for subfunction in &self.subfunction {
                let name = subfunction.name().await;
                if (subfunction.status().await && name == help) || helps.contains("all") {
                    let data = subfunction.help(help).await;
                    help_data.push_str(format!("<{}>\n{}\n", name, data.as_str()).as_str());
                    if !helps.contains("all") {
                        continue 'a;
                    }else {
                        continue;
                    }
                }
            }
            if helps.contains("all") {
                continue 'a;
            }
            unfind_help_data.push_str(format!("<{}>", help).as_str());
        }

        if !help_data.is_empty() {
            let res = help_data.strip_suffix("\n").unwrap().to_string();
            return res;
        }
        help_data.push_str(">");
        let mut sub_help_data = String::new();
        for handler in &self.subfunction {
            let name = handler.name().await;
            if name != "help".to_string() && handler.status().await {
                sub_help_data.push_str(format!("\n{}", name).as_str());
            }
        }
        if sub_help_data.is_empty() {
            sub_help_data.push_str("\n...");
        }
        help_data.push_str(sub_help_data.as_str());
        help_data.push_str("\n>\n\n使用\n/help <主功能>-<子功能>...\n来查询子功能详细用法\neg:\n/help 机厅管理-添加-删除");
        help_data
    }

    async fn name(&self) -> String {
        "机厅管理".to_string()
    }
}

async fn pg_init() {
    let mut link = DBLINK.db_link.get().unwrap().clone().begin().await.unwrap();
    sqlx::query!(
        "create table IF not exists group_data (
    gid bigint primary key)"
    )
    .execute(&mut *link)
    .await
    .unwrap();
    sqlx::query!(
        "create table if not exists GameCenterData(
    GC_id bigint primary key generated always as identity,
    admin_gid bigint not null,
    headcount int not null default 0,
    last_time bigint not null default 0,
    refresh boolean default true,
    description text not null
)"
    )
    .execute(&mut *link)
    .await
    .unwrap();
    sqlx::query!(
        "create table if not exists GC_name(
    GC_id bigint not null,
    gid bigint not null,
    name text not null primary key
)",
    )
    .execute(&mut *link)
    .await
    .unwrap();
    link.commit().await.unwrap();
}
#[anyhow_trace]
pub async fn useable_judgment(msg: &Msg) -> Result<(),Error> {
    if msg.message_type != "group" {
        return Err(anyhow!("请在群聊内使用"));
    }
    if msg.sender.role == "member" {
        return Err(anyhow!("非管理员无法操作"));
    }
    Ok(())
}
pub (self) fn sub_matches(msg: &Msg,name: String) -> bool {
    let mut splits = msg.raw_message.split(" ");
    splits.next();
    let a = splits.next();
    if a.is_some() && a.unwrap() == name {
        return true;
    }
    false
}