use crate::MAIN_CONFIG;
use crate::msg_sys::msg_analysis::{Msg, MsgSender};
use crate::msg_sys::msg_reply::Data::{Face, Image, Node, Record, Reply, Text, Video};
use crate::msg_sys::msg_reply::PostType::Poke;
use crate::qq_link::{SEND_CHAN, http_get};
use serde::{Deserialize, Serialize};
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
    message: String,
}
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Data {
    Text(DataText),
    At(DataAt),
    Image(DataImage),
    Face(DataFace),
    Reply(DataReply),
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
            message: String::new(),
        }
    }
    //添加文字消息
    pub async fn join_text(&mut self, s: String) {
        //接收字符串作为文字消息存入待发送数据中
        self.message.push_str(&s);
    }
    pub async fn join_at(&mut self, at_id: i64) {
        self.message
            .push_str(format!("[CQ:at,qq={}] ", at_id).as_str());
    }
    //添加图片消息
    pub async fn join_image(&mut self, pic_path: String) {
        //接收图片路径作为图片消息存入待发送数据中
        self.message
            .push_str(format!("[CQ:image,file={}]", pic_path).as_str());
    }
    pub async fn join_face(&mut self, face_id: i32) {
        self.message
            .push_str(format!("[CQ:face,id={}]", face_id).as_str());
    }
    pub async fn join_reply(&mut self, message_id: i64) {
        self.message
            .push_str(format!("[CQ:reply,id={}]", message_id).as_str());
    }
    pub async fn join_record(&mut self, file_path: String) {
        self.message
            .push_str(format!("[CQ:record,file={}]", file_path).as_str());
    }
    pub async fn join_video(&mut self, file_path: String) {
        self.message
            .push_str(format!("[CQ:video,file={}]", file_path).as_str());
    }
    pub async fn join_file(&mut self, file_path: String) {
        self.message
            .push_str(format!("[CQ:file,file={}]", file_path).as_str());
    }
    pub async fn join_node(&mut self, node: DataNode) {
        let node = serde_json::to_string(&node).unwrap();
        self.message.push_str(node.as_str());
    }
    //调用该方法时，传入原始消息作为基本数据，并将自身存储的消息数据发送，
    pub async fn send_msg(mut self, msg: &Msg) {
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
                raw_message: format!("[bot_msg]{}", self.message),
                font: 0,
                sender: MsgSender {
                    user_id: msg.self_id,
                    nickname: "bot".to_string(),
                    card: "".to_string(),
                    role: "".to_string(),
                    sex: "".to_string(),
                    age: 0,
                },
                notice_type: "".to_string(),
            };
            SEND_CHAN
                .get()
                .unwrap()
                .send(serde_json::to_string(&msg).unwrap())
                .await
                .unwrap();
        }
        debug!("try send msg");
        send(&PostType::Message(self), "/send_msg".to_string()).await;
    }
    pub async fn send_forward_msg(mut self, msg: &Msg) {
        if self.user_id == 0 && self.group_id == 0 {
            self.user_id = msg.sender.user_id;
            self.group_id = msg.group_id;
        }
        if self.group_id == 0 {
            self.message_type = "private".to_string();
        } else {
            self.message_type = "group".to_string();
        }

        debug!("try send msg");
        send(&PostType::Message(self), "/send_forward_msg".to_string()).await;
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
            r#type: "reply".to_string(),
            data: Reply(DataReply { id: message_id }),
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
struct DataAt {
    qq: i64,
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
        send(&Poke(data), "/friend_poke".to_string()).await;
    }
    pub async fn group(gid: i64, uid: i64) {
        let data = SendPoke {
            user_id: uid,
            group_id: Some(gid),
        };
        send(&Poke(data), "/group_poke".to_string()).await;
    }
}

async fn send(data: &PostType, post_type: String) {
    let ip_port = http_ip_process();
    match http_get(&*post_type).json(&data).send().await {
        Ok(i) => {
            info!("消息发送成功 server return:{:?}", i);
            debug!("send msg:{}", serde_json::to_string(&data).unwrap());
        }
        Err(e) => {
            error!("消息发送失败{:?}\norigin url:{}", e, ip_port);
        }
    };
}

pub fn http_ip_process() -> String {
    let mut ip_port = MAIN_CONFIG.nc_setting.http_ip_port.clone();
    if !ip_port.starts_with("http://") {
        ip_port = ip_port
            .strip_prefix("https://")
            .unwrap_or(ip_port.as_str())
            .to_string();
        ip_port = format!("http://{}", ip_port);
    }
    ip_port = ip_port
        .strip_suffix("/")
        .unwrap_or(ip_port.as_str())
        .to_string();
    ip_port
}
