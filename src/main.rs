pub mod main_config;
pub mod msg_sys;
pub mod qq_link;

use crate::main_config::{MainConfig, get_config};
use crate::msg_sys::msg_sys::msg_sys;
use crate::qq_link::qq_link;
use reqwest::Client;
use std::sync::LazyLock;

static MAIN_CONFIG: LazyLock<MainConfig> = LazyLock::new(|| get_config());
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| Client::new());
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(false)
        .init();
    //读取配置文件
    //连接qq
    let msg_chan = qq_link().await;

    //进入消息系统
    msg_sys(msg_chan).await;

    Ok(())
}
