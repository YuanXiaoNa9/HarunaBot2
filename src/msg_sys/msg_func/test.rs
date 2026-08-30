use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use async_trait::async_trait;
use image::{ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

static TTF: LazyLock<FontVec> = LazyLock::new(ttf);

pub struct Test;
fn ttf() -> FontVec {
    FontVec::try_from_vec(include_bytes!("/home/q/rust/HarunaBot2/ttf/siyuan.ttf").to_vec())
        .unwrap()
}
#[async_trait]
impl MsgHandler for Test {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        let mut msg_split = msg.raw_message.split(' ');
        let start_str = msg_split.next().unwrap();
        if (start_str == "睦说"|| start_str == "祥子说"||start_str == "初华说"||start_str == "喵梦说"||start_str == "海铃说")&&msg_split.next().is_some(){
            true
        } else {
            false
        }
    }

    async fn process(&self, msg: Arc<Msg>) {
        let mut msg_split = msg.raw_message.split(' ');
        let name = msg_split.next().unwrap();
        let mut file_name:&str = "";
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
        let mut img = ImageReader::open(format!("/home/q/rust/HarunaBot2/pic/{}.png", file_name))
            .unwrap()
            .decode()
            .unwrap();
        let text = msg.raw_message.strip_prefix(format!("{} ",name).as_str()).unwrap();
        let x: f32 = text
            .chars()
            .map(|c| {
                let scaled_font = TTF.as_scaled(PxScale::from(100.0));
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
            &TTF.deref(),
            text,
        );
        let file_name: i32 = rand::random();
        img.save(format!("/home/q/rust/HarunaBot2/{}.png", file_name))
            .unwrap();
        let mut rep = SendMsg::new_msg().await;
        rep.join_image(format!("/home/q/rust/HarunaBot2/{}.png", file_name).to_string())
            .await;
        rep.send_msg(msg).await;
        std::fs::remove_file(format!("/home/q/rust/HarunaBot2/{}.png", file_name).to_string())
            .unwrap();
    }
}
