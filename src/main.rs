pub mod init_log;
pub mod main_config;
pub mod msg_sys;
pub mod qq_link;

use crate::init_log::init_tracing;
use crate::main_config::{MainConfig, main_config_get};
use crate::msg_sys::msg_sys::msg_sys;
use crate::qq_link::qq_link;
use reqwest::Client;
use std::env;
use std::string::ToString;
use std::sync::LazyLock;
static PATH: LazyLock<String> = LazyLock::new(|| env::current_dir().unwrap().display().to_string());
static MAIN_CONFIG: LazyLock<MainConfig> = LazyLock::new(|| main_config_get());
static HTTP_CLIENT: LazyLock<Client> =
    LazyLock::new(|| Client::builder().no_proxy().build().unwrap());
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
