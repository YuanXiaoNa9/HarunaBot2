use crate::msg_sys::msg_analysis::Msg;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::FnHandler;
use crate::qq_link::http_get;
use anyhow::Error;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub struct Decrease {
    pub enable: bool,
}
#[async_trait]
impl FnHandler for Decrease {
    async fn matches(&self, msg: &Msg) -> bool {
        msg.notice_type == "group_decrease"
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
        let body = Body {
            user_id,
            no_cache: false,
        };
        let res = http_get("/get_stranger_info").json(&body).send().await?;
        let str = res.text().await?;
        let res: Rep = serde_json::from_str(&str)?;
        let mut rep = SendMsg::new().await;
        rep.join_text(
            format!("いつか、みんなと一緒に出会うかもね ——{}", res.data.nickname).to_string(),
        )
        .await;
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
