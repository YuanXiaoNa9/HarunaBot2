pub mod main_config;
pub mod msg_sys;
pub mod qq_link;

use crate::main_config::{MainConfig, get_config};
use crate::msg_sys::msg_sys::msg_sys;
use crate::qq_link::qq_link;
use reqwest::Client;
use std::sync::LazyLock;
use crate::msg_sys::init_tracing::init_tracing;

static MAIN_CONFIG: LazyLock<MainConfig> = LazyLock::new(|| get_config());
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| Client::new());
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //初始化日志
    init_tracing();
    //连接qq
    let msg_chan = qq_link().await;

    //进入消息系统
    msg_sys(msg_chan).await;

    Ok(())
}
