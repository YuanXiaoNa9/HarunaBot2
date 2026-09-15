use crate::msg_sys::msg_analysis::Msg;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::FnHandler;
use anyhow::Error;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct Increase {
    pub enable: bool,
}
#[async_trait]
impl FnHandler for Increase {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.notice_type == "group_increase"
    }

    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let user_id = msg.user_id;
        #[derive(Serialize)]
        struct Body {
            user_id: i64,
            no_cache: bool,
        }
        #[derive(Deserialize)]
        struct Rep {
            status: String,
            retcode: i32,
            data: Data,
        }
        #[derive(Deserialize)]
        struct Data {
            nickname: String,
        }
        let mut rep = SendMsg::new().await;
        rep.join_at(user_id).await;
        rep.join_text("你好呀，欢迎加入，可以使用/help获取功能帮助哦\n新功能正在绝赞开发中\n当然你有好的想法不妨告诉我哦\n".to_string()).await;
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        self.enable
    }

    async fn help(&self, _: &str) -> String {
        "检测到退群自动提醒".to_string()
    }

    async fn name(&self) -> String {
        "退群提醒".to_string()
    }
}
