use crate::PATH;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use async_trait::async_trait;
use image::{ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use std::sync::{Arc, OnceLock};
use tracing::error;

pub struct EmoMjk {
    pub(crate) status: bool,
    pub(crate) ttf: OnceLock<FontVec>,
}
#[async_trait]
impl MsgHandler for EmoMjk {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        let mut msg_split = msg.raw_message.split(' ');
        let start_str = msg_split.next().unwrap();
        if (start_str == "睦说"
            || start_str == "祥子说"
            || start_str == "初华说"
            || start_str == "喵梦说"
            || start_str == "海铃说"
            || start_str == "鲸说")
            && msg_split.next().is_some()
        {
            true
        } else {
            false
        }
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut msg_split = msg.raw_message.split(' ');
        let name = msg_split.next().unwrap();
        let mut file_name: &str = "";
        let mut text_location = "mid";
        if name == "睦说" {
            file_name = "mutsumi";
        } else if name == "祥子说" {
            file_name = "saki";
        } else if name == "初华说" {
            file_name = "uika";
        } else if name == "喵梦说" {
            file_name = "nyamu";
        } else if name == "海铃说" {
            file_name = "umiru";
        } else if name == "鲸说" {
            file_name = "DS";
            text_location = "left";
        }
        let text = msg
            .raw_message
            .strip_prefix(format!("{} ", name).as_str())
            .unwrap();
        let pic_start_time = std::time::Instant::now();
        let mut img = ImageReader::open(format!("{}/pic/{}.png", PATH.as_str(), file_name))
            .unwrap()
            .decode()
            .unwrap();
        let ori_y = img.height() as f32;
        let y = (ori_y * 0.15333) as i32;
        let size = ori_y / 6.2;
        let i: f32 = text
            .chars()
            .map(|c| {
                let scaled_font = self.ttf.get().unwrap().as_scaled(PxScale::from(size));
                let id = scaled_font.glyph_id(c);
                scaled_font.h_advance(id)
            })
            .sum();
        println!("{}", i);
        let x: f32 = if text_location == "mid" || (text_location == "left" && i / ori_y >= 0.115) {
            i / 2.0
        } else if text_location == "left" && i / ori_y <= 0.115 {
            ori_y * 0.16433
        } else {
            0.0
        };
        let x = (ori_y * 0.3083333 - x).round() as i32;
        draw_text_mut(
            &mut img,
            Rgba([0, 0, 0, 245]),
            x,
            y,
            PxScale::from(size),
            &self.ttf.get().unwrap(),
            text,
        );
        let end_time = pic_start_time.elapsed();
        let file_name: i32 = rand::random();
        img.save(format!("{}/{}.png", PATH.as_str(), file_name))
            .unwrap();
        let mut rep = SendMsg::new_msg().await;
        rep.join_image(format!("{}/{}.png", PATH.as_str(), file_name).to_string())
            .await;
        rep.join_text(format!("耗时:{:?}", end_time)).await;
        rep.send_msg(msg).await;
        std::fs::remove_file(format!("{}/{}.png", PATH.as_str(), file_name).to_string()).unwrap();
    }

    async fn init(&mut self) -> (String, bool) {
        let name = "母鸡卡表情包".to_string();
        self.ttf = match FontVec::try_from_vec(include_bytes!("ttf/siyuan.ttf").to_vec()) {
            Ok(ttf) => OnceLock::from(ttf),
            Err(e) => {
                error!("{:?}", e);
                self.status = false;
                return (name, false);
            }
        };
        (name, true)
    }
    async fn status(&self) -> bool {
        self.status
    }
}
