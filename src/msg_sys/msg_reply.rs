use crate::msg_sys::msg_reply::Data::{Face, File, Image, Node, Record, Text, Video};
use crate::msg_sys::msg_reply::PostType::Poke;
use crate::msg_sys::msg_sys::{Msg, MsgSender};
use crate::qq_link::SEND_CHAN;
use crate::{HTTP_CLIENT, MAIN_CONFIG};
use serde::{Deserialize, Serialize};
use std::cmp::PartialEq;
use std::ops::Deref;
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum PostType {
    Message(SendMsg),
    Poke(SendPoke),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SendMsg {
    pub message_type: String,
    pub group_id: i64,
    pub user_id: i64,
    message: Vec<Message>,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Data {
    Text(DataText),
    Image(DataImage),
    Face(DataFace),
    Record(DataRecord),
    Video(DataVideo),
    File(DataFile),
    Node(DataNode),
}

impl SendMsg {
    pub async fn new() -> SendMsg {
        debug!("create new SendMsg");
        //创建一个新的sendmsg结构体用于存储待发送消息
        SendMsg {
            message_type: "".to_string(),
            group_id: 0,
            user_id: 0,
            message: Vec::new(),
        }
    }
    //添加文字消息
    pub async fn join_text(&mut self, s: String) {
        //接收字符串作为文字消息存入待发送数据中
        self.message.push(Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText { text: s }),
        });
        debug!("join_text ok");
    }
    pub async fn join_at(&mut self, at_id: i64) {
        self.message.push(Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText {
                text: format!("[CQ:at,qq={}", at_id),
            }),
        })
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.message.push(Message {
            r#type: "image".to_string(),
            data: Image(DataImage { file: pic_path }),
        });
    }
    pub async fn join_face(&mut self, face_id: i32) {
        self.message.push(Message {
            r#type: "face".to_string(),
            data: Face(DataFace { id: face_id }),
        });
    }
    pub async fn join_reply(&mut self, message_id: i64) {
        self.message.push(Message {
            r#type: "text".to_string(),
            data: Text(DataText {
                text: format!("[CQ:reply,id={}]", message_id),
            }),
        });
    }
    pub async fn join_record(&mut self, file_path: String) {
        self.message.push(Message {
            r#type: "record".to_string(),
            data: Record(DataRecord { file: file_path }),
        })
    }
    pub async fn join_video(&mut self, file_path: String) {
        self.message.push(Message {
            r#type: "video".to_string(),
            data: Video(DataVideo { file: file_path }),
        })
    }
    pub async fn join_file(&mut self, file_path: String, file_name: String) {
        self.message.push(Message {
            r#type: "".to_string(),
            data: File(DataFile {
                file: file_path.to_string(),
                name: file_name.to_string(),
            }),
        })
    }
    pub async fn join_node(&mut self, node: DataNode) {
        self.message.push(Message {
            r#type: "node".to_string(),
            data: Node(node),
        })
    }
    //调用该方法时，传入原始消息作为基本数据，并将自身存储的消息数据发送，
    pub async fn send_msg(mut self, msg: Arc<Msg>) {
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }
        if self.group_id == 0 {
            self.message_type = "private".to_string();
        } else {
            self.message_type = "group".to_string();
        }
        if self.message_type == "group" {
            let mut bot_msg = String::new();
            bot_msg.push_str("[bot_msg]");
            for i in &self.message {
                match &i.data {
                    Text(data) => bot_msg.push_str(data.text.as_str()),
                    _ => {}
                }
            }
            let msg = Msg {
                time: 0,
                self_id: msg.self_id,
                post_type: "message".to_string(),
                message_type: "group".to_string(),
                sub_type: "".to_string(),
                target_id: 0,
                message_id: 0,
                message_seq: 0,
                group_id: msg.group_id,
                group_name: "bot".to_string(),
                user_id: msg.self_id,
                message: "".to_string(),
                raw_message: bot_msg,
                font: 0,
                sender: MsgSender {
                    user_id: msg.self_id,
                    nickname: "bot".to_string(),
                    card: "".to_string(),
                    role: "".to_string(),
                    sex: "".to_string(),
                    age: 0,
                },
            };
            SEND_CHAN
                .get()
                .unwrap()
                .send(serde_json::to_string(&msg).unwrap())
                .await
                .unwrap();
        }
        debug!("try send msg");
        send(&PostType::Message(self), "send_msg".to_string()).await;
    }
    pub async fn send_forward_msg(mut self, msg: Arc<Msg>) {
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }

        debug!("try send msg");
        send(&PostType::Message(self), "send_forward_msg".to_string()).await;
    }
}
impl DataNode {
    pub async fn new(user_id: i64, nickname: String) -> DataNode {
        debug!("create new DataNode");
        DataNode {
            user_id,
            nickname,
            content: vec![],
        }
    }
    pub async fn join_text(&mut self, s: String) {
        //接收字符串作为文字消息存入待发送数据中
        self.content.push(Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText { text: s }),
        });
        debug!("join_text ok");
    }
    pub async fn join_at(&mut self, at_id: i64) {
        self.content.push(Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText {
                text: format!("[CQ:at,qq={}", at_id),
            }),
        })
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.content.push(Message {
            r#type: "image".to_string(),
            data: Image(DataImage { file: pic_path }),
        });
    }
    pub async fn join_face(&mut self, face_id: i32) {
        self.content.push(Message {
            r#type: "face".to_string(),
            data: Face(DataFace { id: face_id }),
        });
    }
    pub async fn join_reply(&mut self, message_id: i64) {
        self.content.push(Message {
            r#type: "text".to_string(),
            data: Text(DataText {
                text: format!("[CQ:reply,id={}]", message_id),
            }),
        });
    }
    pub async fn join_record(&mut self, file_path: String) {
        self.content.push(Message {
            r#type: "record".to_string(),
            data: Record(DataRecord { file: file_path }),
        })
    }
    pub async fn join_video(&mut self, file_path: String) {
        self.content.push(Message {
            r#type: "video".to_string(),
            data: Video(DataVideo { file: file_path }),
        })
    }
    pub async fn join_node(&mut self, user_id: i64, nickname: String, content: Vec<Message>) {
        self.content.push(Message {
            r#type: "node".to_string(),
            data: Node(DataNode {
                user_id,
                nickname,
                content,
            }),
        })
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct Message {
    r#type: String,
    data: Data,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataText {
    text: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataImage {
    file: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataFace {
    id: i32,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataRecord {
    file: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataVideo {
    file: String,
}
#[derive(Serialize, Deserialize, Debug)]
struct DataFile {
    file: String,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DataNode {
    user_id: i64,
    nickname: String,
    content: Vec<Message>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SendPoke {
    user_id: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    group_id: Option<i64>,
}

impl SendPoke {
    pub async fn private(uid: i64) {
        let data = SendPoke {
            user_id: uid,
            group_id: None,
        };
        send(&Poke(data), "friend_poke".to_string()).await;
    }
    pub async fn group(gid: i64, uid: i64) {
        let data = SendPoke {
            user_id: uid,
            group_id: Some(gid),
        };
        send(&Poke(data), "group_poke".to_string()).await;
    }
}

async fn send(data: &PostType, post_type: String) {
    match HTTP_CLIENT
        .post(format!("{}/{}", MAIN_CONFIG.http_ip_port, post_type))
        .json(&data)
        .header(
            "Authorization",
            format!("Bearer {}", MAIN_CONFIG.http_token),
        )
        .send()
        .await
    {
        Ok(i) => {
            info!("消息发送成功");
            debug!("send msg:{}", serde_json::to_string(&data).unwrap());
            debug!("server return:{:?}", i);
        }
        Err(e) => {
            error!("消息发送失败{:?}", e);
        }
    };
}
