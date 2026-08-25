use crate::msg_sys::msg_reply::Data::Image;
use crate::msg_sys::msg_sys::Msg;
use crate::{HTTP_CLIENT, MAIN_CONFIG};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

#[derive(Serialize, Deserialize, Debug)]
pub struct SendMsg {
    message: Vec<Message>,
    pub group_id: i64,
    pub user_id: i64,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Data {
    Text(DataText),
    Image(DataImage),
}

impl SendMsg {
    pub async fn new() -> SendMsg {
        debug!("create new SendMsg");
        //创建一个新的sendmsg结构体用于存储待发送消息
        SendMsg {
            message: Vec::new(),
            group_id: 0,
            user_id: 0,
        }
    }
    //添加文字消息
    pub async fn join_text(&mut self, s: String) {
        //接收字符串作为文字消息存入待发送数据中
        self.message.append(&mut vec![Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText { text: s }),
        }]);
        debug!("join_text ok");
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.message.append(&mut vec![Message {
            r#type: "image".to_string(),
            data: Image(DataImage { file: pic_path }),
        }]);
    }
    //调用该方法时，传入原始消息作为基本数据，并将自身存储的消息数据发送，
    pub async fn send(&mut self, msg: Msg) {
        let mut post_type = "send_private_msg";
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }
        if self.group_id != 0 {
            post_type = "send_group_msg";
        }
        debug!("try send msg");
        match HTTP_CLIENT
            .post(format!("{}/{}", MAIN_CONFIG.http_ip_port, post_type))
            .json(&self)
            .header(
                "Authorization",
                format!("Bearer {}", MAIN_CONFIG.http_token),
            )
            .send()
            .await
        {
            Ok(i) => {
                info!("消息{}条发送成功", self.message.len());
                debug!("server return:{}", i.status());
            }
            Err(e) => {
                error!("消息发送失败{}", e)
            }
        };
        let send_msg = serde_json::to_string(self).unwrap();
        println!("{}", send_msg);
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct DataText {
    text: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct Message {
    r#type: String,
    data: Data,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataImage {
    file: String,
}
