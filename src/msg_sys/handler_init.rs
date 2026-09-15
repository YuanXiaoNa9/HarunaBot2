use crate::msg_sys::handler_regin::{mod_handler_regin, msg_handler_regin, notice_handler_regin};
use crate::msg_sys::msg_sys::FnHandler;
use std::sync::OnceLock;
use tokio::spawn;
pub static NOTICE_HANDLERS: OnceLock<Vec<Box<dyn FnHandler + Send + Sync>>> = OnceLock::new();
pub static MSG_HANDLERS: OnceLock<Vec<Box<dyn FnHandler + Send + Sync>>> = OnceLock::new();

pub async fn mod_handlers_init() {
    let handlers = mod_handler_regin();
    mian_init(crate::msg_sys::msg_sys::Handler::Mods(handlers)).await;
}
//模块功能初始化主函数
pub async fn mian_init(handlers: crate::msg_sys::msg_sys::Handler) {
    match handlers {
        crate::msg_sys::msg_sys::Handler::MsgFn(handlers) => {
            let _ = MSG_HANDLERS.set(handlers);
            for handler in MSG_HANDLERS.get().unwrap().iter() {
                spawn(async move {
                    let _ = &handler.init().await;
                    crate::msg_sys::msg_sys::log_init(handler.name().await, handler.status().await)
                        .await;
                });
            }
        }
        crate::msg_sys::msg_sys::Handler::NtcFn(handlers) => {
            let _ = NOTICE_HANDLERS.set(handlers);
            for handler in NOTICE_HANDLERS.get().unwrap().iter() {
                spawn(async move {
                    let _ = &handler.init().await;
                    crate::msg_sys::msg_sys::log_init(handler.name().await, handler.status().await)
                        .await;
                });
            }
        }
        crate::msg_sys::msg_sys::Handler::Mods(handlers) => {
            for handler in handlers {
                spawn(async move {
                    handler.init().await;
                    crate::msg_sys::msg_sys::log_init(
                        handler.name().await,
                        handler.init_status().await,
                    )
                    .await;
                });
            }
        }
    }
}

//msg功能函数初始化
pub async fn msg_handlers_init() {
    //注册功能模块
    let handlers = msg_handler_regin();
    //取出handler并执行初始化方法
    mian_init(crate::msg_sys::msg_sys::Handler::MsgFn(handlers)).await;
}

//notice功能函数初始化
pub async fn notice_handlers_init() {
    let handlers = notice_handler_regin();
    mian_init(crate::msg_sys::msg_sys::Handler::NtcFn(handlers)).await;
}
