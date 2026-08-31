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
            || start_str == "海铃说")
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
        if name == "睦说" {
            file_name = "mutsumi";
        }
        if name == "祥子说" {
            file_name = "saki";
        }
        if name == "初华说" {
            file_name = "uika";
        }
        if name == "喵梦说" {
            file_name = "nyamu";
        }
        if name == "海铃说" {
            file_name = "umiru";
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
        let x: f32 = text
            .chars()
            .map(|c| {
                let scaled_font = self.ttf.get().unwrap().as_scaled(PxScale::from(100.0));
                let id = scaled_font.glyph_id(c);
                scaled_font.h_advance(id)
            })
            .sum();
        let x = (185.0 - x / 2.0).round() as i32;
        draw_text_mut(
            &mut img,
            Rgba([0, 0, 0, 255]),
            x,
            92,
            PxScale::from(100.0),
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

    async fn init(&mut self) -> (String,bool) {
        let name = "母鸡卡表情包".to_string();
        self.ttf = match FontVec::try_from_vec(
            include_bytes!("/home/q/rust/HarunaBot2/ttf/siyuan.ttf").to_vec(),
        ) {
            Ok(ttf) => {OnceLock::from(ttf)},
            Err(e) => {
                error!("{:?}",e);
                self.status = false;
                return (name,false);
            }
        };
        (name,true)
    }
    async fn status(&self) -> bool {
        self.status
    }
}
