use crate::main_config::MainConfig;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct  MsgSender{
    pub user_id:i64,
    pub nickname:String,
    pub card:String,
    pub role:String,
    pub sex:String,
    pub age:i64
}
#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MsgGet {
    pub time:i64,
    pub self_id:i64,
    pub post_type: String,
    pub message_type: String,
    pub sub_type: String,
    pub message_id: i64,
    pub message_seq:i64,
    pub group_id:i64,
    pub group_name:String,
    pub user_id:i64,
    pub message:String,
    pub raw_message:String,
    pub font:i16,
    pub sender :MsgSender,

}
pub trait MsgHandler {
    fn matches(&self, _: &MsgGet) ->bool;
    fn process(&self,_: &MsgGet);

}
impl MsgGet{
    fn send(&self){
        println!("send ok");
    }
}
struct Test;
impl MsgHandler for Test {
    fn matches(&self, msg: &MsgGet) -> bool {
        if msg.raw_message == "test"{
            true
        }else { false }
    }
    fn process(&self,msg:&MsgGet) {
        msg.send();
    }
}
impl MsgGet {

}
pub async fn msg_sys(main_config: MainConfig, msg_chan: std::sync::mpsc::Receiver<String>) {
    let msg_handlers :Vec<Box<dyn MsgHandler>> = vec![Box::new(Test)];
    loop {
        let msg= match msg_chan.recv() {
            Ok(i) => i,
            Err(e) => {error!("消息接收出现错误：{}",e);continue}
        };
        if msg == ""{
            continue;
        }
        let msg: MsgGet = match serde_json::from_str(msg.as_str()){
            Ok(msg_struct) => {debug!("{:?}",msg_struct);msg_struct},
            Err(e) => {error!("{}",e);continue},
        };
        if msg.post_type != "message"{
            continue;
        }
        info!("{}:{}", msg.sender.user_id,msg.raw_message);
        dispatch(&msg,&msg_handlers);
    }

}

fn dispatch(msg: &MsgGet,handlers:&Vec<Box<dyn MsgHandler>>){
    for handler in handlers{
        if handler.matches(&msg) {
            handler.process(&msg);
        }
    }
}
