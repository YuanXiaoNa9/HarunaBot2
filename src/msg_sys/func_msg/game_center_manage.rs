pub mod gcm_add;
pub mod gcm_add_name;
pub mod gcm_bind;
pub mod gcm_delete;
pub mod gcm_delete_name;
pub mod gcm_rename;
pub mod gcm_rewrite_description;
pub mod gcm_search;
pub mod gcm_unbind;
pub mod gcm_query_gc;

use crate::msg_sys::func_config::FUNC_CONFIG;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_sys::{
    FnHandler, Msg, mod_status_examine, sub_help, sub_init, sub_match_process,
};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;

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
        let rx = DBLINK.rx.clone();
        mod_status_examine(rx).await;
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
        sub_help(helps, &self.subfunction).await
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
    report_time bigint not null default 0,
    refresh boolean default true,
    description text not null default '该机厅未备注',
    report_id bigint not null default 0
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
pub async fn useable_judgment(msg: &Msg) -> Result<(), Error> {
    if msg.message_type != "group" {
        return Err(anyhow!("请在群聊内使用"));
    }
    if msg.sender.role == "member" && !FUNC_CONFIG.get().unwrap().super_admin.contains(&msg.sender.user_id){
        return Err(anyhow!("非管理员无法操作"));
    }
    Ok(())
}
pub(self) fn sub_matches(msg: &Msg, name: String) -> bool {
    let mut splits = msg.raw_message.split(" ");
    splits.next();
    let a = splits.next();
    if a.is_some() && a.unwrap() == name {
        return true;
    }
    false
}
