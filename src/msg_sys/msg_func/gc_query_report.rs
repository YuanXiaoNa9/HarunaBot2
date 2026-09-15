use crate::msg_sys::func_mod::gc_name::GCNAME;
use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_sys::{mod_status_examine, sub_help, sub_init, sub_match_process, FnHandler, Msg};
use anyhow::Error;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use anyhow_trace::anyhow_trace;
use tracing::debug;

pub mod gcqr_query;
pub mod gcqr_repo_plus;
pub mod gcqr_report_minus;
pub mod gcqr_report_set;

pub struct GCQR {
    pub(crate) status: AtomicBool,
    pub(crate) sub_function: Vec<Box<dyn FnHandler + Send + Sync>>,
}
#[async_trait]
impl FnHandler for GCQR {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.len() > 30{
            return false;
        }
        let gc_name =msg.raw_message.clone();
        let gc_name = gc_name.strip_suffix(&['j','几']).unwrap_or(&*gc_name);
        let gc_name = gc_name.strip_suffix("几人").unwrap_or(gc_name);
        let gc_name = gc_name.strip_suffix("几个人").unwrap_or(gc_name);
        let gc_name = gc_name.trim_end_matches(|c: char| { c.is_ascii_digit() || ['+', '-'].contains(&c) });
        let gc_name = gc_name.strip_suffix(&['加','减']).unwrap_or(gc_name);
        GCNAME.map.read().await.contains_key(format!("{}{}", gc_name, msg.group_id).as_str())
    }
#[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        sub_match_process(msg, &self.sub_function).await?;
        Ok(())
    }

    async fn init(&self) {
        let rx = DBLINK.rx.clone();
        mod_status_examine(rx).await;
        debug!("pg_done");
        let rx = GCNAME.rx.clone();
        mod_status_examine(rx).await;
        debug!("gc_done");
        sub_init(&self.sub_function).await;
        self.status
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    async fn status(&self) -> bool {
        self.status.load(std::sync::atomic::Ordering::Relaxed)
    }

    async fn help(&self, helps: &str) -> String {
        sub_help(helps,&self.sub_function).await
    }

    async fn name(&self) -> String {
        "机厅查询上报".to_string()
    }
}
