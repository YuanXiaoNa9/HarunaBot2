//! PJSK message routing; channel protocol and persistent business state live in Go.
mod access;
mod accounts;
mod binding;
mod client;
mod commands;
mod consent;
mod dispatch;
mod error;
mod help;
mod render;
mod state;
mod subfunction;
mod sync;
mod taptap;
#[cfg(test)]
mod tests;

#[cfg(not(test))]
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg, sub_init, sub_match_process};
use async_trait::async_trait;
use client::{Client, Config};
use commands::*;
use dashmap::DashMap;
use error::Fault;
#[cfg(test)]
use render::TEST_REPLIES;
use render::*;
use serde::Deserialize;
use serde_json::{Value, json};
use state::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::Instant;

pub struct PjskUpload {
    state: Arc<PjskState>,
    subfunction: Vec<Box<dyn FnHandler + Send + Sync>>,
}
impl Default for PjskUpload {
    fn default() -> Self {
        let state = Arc::new(PjskState::default());
        let subfunction = subfunction::handlers(state.clone());
        Self { state, subfunction }
    }
}
impl std::ops::Deref for PjskUpload {
    type Target = PjskState;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}
#[async_trait]
impl FnHandler for PjskUpload {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.post_type == "message"
            && (command(msg.raw_message.trim())
                || (private(msg)
                    && otp(msg.raw_message.trim())
                    && self
                        .pending
                        .get(&key(msg))
                        .is_some_and(|p| p.expires > now() && p.stage == Stage::AwaitingCode)))
    }
    async fn process(&self, msg: &Msg) -> Result<(), anyhow::Error> {
        sub_match_process(msg, &self.subfunction).await
    }
    async fn init(&self) {
        self.state.initialize().await;
        if self.ready.load(Ordering::Relaxed) {
            sub_init(&self.subfunction).await;
        }
    }
    async fn status(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
    async fn help_visible(&self, msg: &Msg) -> bool {
        self.client.get().is_some_and(|c| {
            c.config.enabled
                && msg.self_id.to_string() == c.config.bot_id
                && (private(msg) || c.config.allowed_groups.contains(&msg.group_id.to_string()))
        })
    }
    async fn help(&self, query: &str) -> String {
        help::render(query, &self.subfunction).await
    }
    async fn name(&self) -> String {
        "PJSK数据上传".into()
    }
}
