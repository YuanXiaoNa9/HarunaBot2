use crate::msg_sys::msg_reply::Data::{At, Face, File, Image, Node, Record, Reply, Video};
use crate::msg_sys::msg_sys::{Msg};
use crate::{HTTP_CLIENT, MAIN_CONFIG};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
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
    At(DataAt),
    Face(DataFace),
    Reply(DataReply),
    Record(DataRecord),
    Video(DataVideo),
    File(DataFile),
    Node(DataNode),
}

impl SendMsg {
    pub async fn new_msg() -> SendMsg {
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
    pub async fn join_at(&mut self, at_id: i64) {
        self.message.append(&mut vec![Message {
            r#type: "at".to_string(),
            data: At(DataAt { qq: at_id }),
        }])
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.message.append(&mut vec![Message {
            r#type: "image".to_string(),
            data: Image(DataImage { file: pic_path }),
        }]);
    }
    pub async fn join_face(&mut self, face_id: i32) {
        self.message.append(&mut vec![Message {
            r#type: "face".to_string(),
            data: Face(DataFace { id: face_id }),
        }]);
    }
    pub async fn join_reply(&mut self, message_id: i64) {
        self.message.append(&mut vec![Message {
            r#type: "reply".to_string(),
            data: Reply(DataReply { id: message_id }),
        }]);
    }
    pub async fn join_record(&mut self, file_path: String) {
        self.message.append(&mut vec![Message {
            r#type: "record".to_string(),
            data: Record(DataRecord { file: file_path }),
        }])
    }
    pub async fn join_video(&mut self, file_path: String) {
        self.message.append(&mut vec![Message {
            r#type: "video".to_string(),
            data: Video(DataVideo { file: file_path }),
        }])
    }
    pub async fn join_file(&mut self, file_path: String, file_name: String) {
        self.message.append(&mut vec![Message {
            r#type: "".to_string(),
            data: File(DataFile {
                file: file_path.to_string(),
                name: file_name.to_string(),
            }),
        }])
    }
    pub async fn join_node(&mut self, node: DataNode) {
        self.message.append(&mut vec![Message {
            r#type: "node".to_string(),
            data: Node(node),
        }])
    }
    //调用该方法时，传入原始消息作为基本数据，并将自身存储的消息数据发送，
    pub async fn send_msg(&mut self, msg: Arc<Msg>) {
        let mut post_type = "send_private_msg";
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }
        if self.group_id != 0 {
            post_type = "send_group_msg";
        }
        debug!("try send msg");
        self.send(post_type.to_string()).await;
    }
    pub async fn send_forward_msg(&mut self, msg: Arc<Msg>) {
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }

        debug!("try send msg");
        self.send("send_forward_msg".to_string()).await;
    }
    async fn send(&mut self, post_type: String) {
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
                info!("消息发送成功");
                debug!("send msg:{}", serde_json::to_string(self).unwrap());
                debug!("server return:{}", i.status());
            }
            Err(e) => {
                error!("消息发送失败{:?}", e);
            }
        };
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
        self.content.append(&mut vec![Message {
            r#type: "text".to_string(),
            data: Data::Text(DataText { text: s }),
        }]);
        debug!("join_text ok");
    }
    pub async fn join_at(&mut self, at_id: i64) {
        self.content.append(&mut vec![Message {
            r#type: "at".to_string(),
            data: At(DataAt { qq: at_id }),
        }])
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.content.append(&mut vec![Message {
            r#type: "image".to_string(),
            data: Image(DataImage { file: pic_path }),
        }]);
    }
    pub async fn join_face(&mut self, face_id: i32) {
        self.content.append(&mut vec![Message {
            r#type: "face".to_string(),
            data: Face(DataFace { id: face_id }),
        }]);
    }
    pub async fn join_reply(&mut self, message_id: i64) {
        self.content.append(&mut vec![Message {
            r#type: "reply".to_string(),
            data: Reply(DataReply { id: message_id }),
        }]);
    }
    pub async fn join_record(&mut self, file_path: String) {
        self.content.append(&mut vec![Message {
            r#type: "record".to_string(),
            data: Record(DataRecord { file: file_path }),
        }])
    }
    pub async fn join_video(&mut self, file_path: String) {
        self.content.append(&mut vec![Message {
            r#type: "video".to_string(),
            data: Video(DataVideo { file: file_path }),
        }])
    }
    pub async fn join_node(&mut self, user_id: i64, nickname: String, content: Vec<Message>) {
        self.content.append(&mut vec![Message {
            r#type: "node".to_string(),
            data: Node(DataNode {
                user_id,
                nickname,
                content,
            }),
        }])
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
struct DataAt {
    qq: i64,
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
struct DataReply {
    id: i64,
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
