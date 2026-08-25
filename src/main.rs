pub mod main_config;
pub mod msg_sys;
pub mod qq_link;

use crate::main_config::{MainConfig, get_config};
use crate::msg_sys::func_config::FuncConfig;
use crate::msg_sys::msg_sys::msg_sys;
use crate::qq_link::qq_link;
use tracing_subscriber::fmt::SubscriberBuilder;

struct AppState {
    main_config: MainConfig,
    func_config: FuncConfig,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();
    //读取配置文件
    let main_config = get_config("main_config.yaml");
    //连接qq
    let msg_chan = qq_link(&main_config).await;

    //进入消息系统
    msg_sys(main_config, msg_chan).await;

    Ok(())
}
