use crate::msg_sys::func_mod::ttf::TTF;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg, mod_status_examine};
use crate::{MAIN_CONFIG, PATH};
use ab_glyph::{Font, PxScale, ScaleFont};
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose;
use image::ImageFormat::Png;
use image::{ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub enum EmjSay {
    Mutsumi(()),
    Saki(()),
    Uika(()),
    Nyamu(()),
    Umiru(()),
    DS(()),
}
pub struct EmoMjk {
    pub(crate) status: AtomicBool,
    pub(crate) enable: bool,
}
#[async_trait]
impl FnHandler for EmoMjk {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("睦说")
            || msg.raw_message.starts_with("祥子说")
            || msg.raw_message.starts_with("初华说")
            || msg.raw_message.starts_with("喵梦说")
            || msg.raw_message.starts_with("海铃说")
            || msg.raw_message.starts_with("鲸说")
        {
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let mut name = "";
        let mut file_name: &str = "";
        let mut text_location = "mid";
        if msg.raw_message.starts_with("睦说") {
            name = "睦说";
            file_name = "mutsumi";
        } else if msg.raw_message.starts_with("祥子说") {
            name = "祥子说";
            file_name = "saki";
        } else if msg.raw_message.starts_with("初华说") {
            name = "初华说";
            file_name = "uika";
        } else if msg.raw_message.starts_with("喵梦说") {
            name = "喵梦说";
            file_name = "nyamu";
        } else if msg.raw_message.starts_with("海铃说") {
            name = "海铃说";
            file_name = "umiru";
        } else if msg.raw_message.starts_with("鲸说") {
            name = "鲸说";
            file_name = "DS";
            text_location = "left";
        }
        let text: &str = msg
            .raw_message
            .strip_prefix(format!("{}", name).as_str())
            .unwrap();
        let mut text = text.strip_prefix(" ").unwrap_or(text);
        if text.is_empty() {
            text = "请输入文本"
        }
        let pic_start_time = std::time::Instant::now();
        let mut img =
            ImageReader::open(format!("{}/pic/{}.png", PATH.as_str(), file_name))?.decode()?;
        let ori_y = img.height() as f32;
        let y = (ori_y * 0.16133) as i32;
        let size = ori_y / 6.9;
        let i: f32 = text
            .chars()
            .map(|c| {
                let scaled_font = TTF.ttf.get().unwrap().as_scaled(PxScale::from(size));
                let id = scaled_font.glyph_id(c);
                scaled_font.h_advance(id)
            })
            .sum();
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
            &TTF.ttf.get().unwrap(),
            text,
        );
        let end_time = pic_start_time.elapsed();
        let file_name: i32 = rand::random();
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        if MAIN_CONFIG.nc_setting.img_send_way == "b64"
            || MAIN_CONFIG.nc_setting.img_send_way == "base64"
        {
            let mut buf = Cursor::new(Vec::new());
            img.write_to(&mut buf, Png).expect("Cannot write to png");
            let b64 = general_purpose::STANDARD.encode(buf.into_inner());
            rep.join_image(format!("base64://{}", b64)).await;
        } else if MAIN_CONFIG.nc_setting.img_send_way == "file" {
            img.save(format!("{}/temp/{}.png", PATH.as_str(), file_name))?;
            rep.join_image(
                format!("{}/{}.png", MAIN_CONFIG.docker_path.as_str(), file_name).to_string(),
            )
            .await;
        }
        rep.join_text(format!("耗时:{:?}", end_time)).await;
        rep.send_msg(&msg).await;
        if MAIN_CONFIG.nc_setting.img_send_way == "file" {
            std::fs::remove_file(format!("{}/temp/{}.png", PATH.as_str(), file_name).to_string())?;
        }
        Ok(())
    }

    async fn init(&self) {
        if !self.enable {
            return;
        }
        let mut rx = TTF.rx.clone();
        mod_status_examine(rx).await;
        self.status.store(true, Relaxed);
    }
    async fn status(&self) -> bool {
        if self.enable && self.status.load(Relaxed) {
            return true;
        }
        false
    }

    async fn help(&self, _: &str) -> String {
        "生成表情包，格式为<角色说><空格><文字内容>\n角色支持:\n睦/祥子/初华/喵梦/海铃/鲸(DS娘)\neg:\n睦说 好女孩".to_string()
    }

    async fn name(&self) -> String {
        "表情".to_string()
    }
}
