use crate::msg_sys::msg_sys::{Msg, MsgHandler};

pub struct Test;
impl MsgHandler for Test {
    fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message == "test" {
            true
        } else {
            false
        }
    }
    fn process(&self, msg: &Msg) {
        msg.send();
    }
}
