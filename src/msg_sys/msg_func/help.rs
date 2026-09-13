use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, MSG_HANDLERS, Msg, Subroutine};
use anyhow::Error;
use async_trait::async_trait;
use std::sync::atomic::AtomicBool;
use anyhow_trace::anyhow_trace;
use tracing::debug;

pub struct Help {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for Help {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("/help") {
            debug!("find help mod");
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        //按空格切分消息
        let mut splits = msg.raw_message.split(" ");
        splits.next();
        //创建回复的空回复结构体
        let mut rep = SendMsg::new().await;
        //回复总help list
        //help带有参数时会开始的逻辑
        //创建未找到帮助名称的空string
        let mut unfind_help = String::new();
        //构建help list消息头
        rep.join_text("[ help list ]".to_string()).await;
        let mut help_data = String::new();
        //循环取出后续参数
        's: for split in splits {
            //遍历取出handler
            for handler in MSG_HANDLERS.get().unwrap().iter() {
                //除掉help查询,以及未开启的功能
                let handler_name = handler.name().await;
                if split.starts_with("help")
                    || !handler.status().await
                    || !split.starts_with(handler_name.as_str())
                {
                    continue;
                }
                debug!("find help handler");
                //调用handler的help_matcher方法
                help_data.push_str(
                    format!("\n\n[{}]\n{}", handler_name, handler.help(split).await).as_str(),
                );
                continue 's;
                //判断是否匹配成功
            }
            //如果上述条件均不符合就加入到未找到列表
            unfind_help.push_str(format!(" {}", split).as_str());
        }

        if help_data.is_empty() {
            help_data.push_str(format!("\n\n{}", self.help("").await).as_str());
        }
        rep.join_text(help_data.to_string()).await;
        //如果未找到的功能字符串不为为空，就在末尾加上未找到的提示
        if !unfind_help.is_empty() {
            rep.join_text(format!("\n未找到帮助项:{}", unfind_help))
                .await;
        }
        //发送消息
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self, _: &str) -> String {
        let handlers = MSG_HANDLERS.get().unwrap();
        let mut rep = String::new();
        rep.push_str(">");
        for handler in handlers {
            let name = handler.name().await;
            if name != "help".to_string() && handler.status().await {
                rep.push_str(format!("\n{}", name).as_str());
            }
        }
        rep.push_str(
            "\n>\n\n使用\n/help name1 name2...\n查询对应功能帮助列表\neg:\n/help test 表情",
        );
        rep
    }

    async fn name(&self) -> String {
        "help".to_string()
    }
}
