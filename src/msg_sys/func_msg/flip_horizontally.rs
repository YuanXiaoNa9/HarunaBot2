use crate::msg_sys::func_msg::emoji_photo::{get_img, get_img_id, img_to_base64};
use crate::msg_sys::msg_analysis::Msg;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::FnHandler;
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use image::{imageops, GenericImageView};
use image::{DynamicImage, ImageReader};
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct FlipHorizontally {
    pub enabled: bool,
    pub status:AtomicBool
}
#[async_trait]
impl FnHandler for FlipHorizontally{
    async fn matches(&self, msg: &Msg) -> bool {
        if (msg.raw_message.contains("图片镜像")&&msg.raw_message.contains("[CQ:image,"))
            ||( msg.raw_message.starts_with("[CQ:reply,id=") && msg.raw_message.contains("]图片镜像"))
        {
            return true;
        }
        false
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let time = std::time::Instant::now();
        let id = get_img_id(msg).await?;
        let bytes = get_img(id).await;
        let mut img = ImageReader::new(Cursor::new(&bytes)).with_guessed_format()?.decode()?;
        let (width, height) = img.dimensions();
        let half_w = width / 2;
        let half_h = height / 2;
        if msg.raw_message.contains("左"){
            let left = imageops::crop_imm(&img, 0, 0, half_w, height).to_image();
            let left_flipped = imageops::flip_horizontal(&left);
            imageops::replace(&mut img, &left_flipped, half_w as i64, 0);
        } else if msg.raw_message.contains("右"){
            let right = imageops::crop_imm(&img, half_w, 0, width, height).to_image();
            let right_flipped = imageops::flip_horizontal(&right);
            imageops::replace(&mut img, &right_flipped,0,0);
        } else if msg.raw_message.contains("上"){
            let up = imageops::crop_imm(&img, 0, 0, width, half_h).to_image();
            let up_flipped = imageops::flip_vertical(&up);
            imageops::replace(&mut img, &up_flipped, 0, half_h as i64);
        }else if msg.raw_message.contains("下"){
            let up = imageops::crop_imm(&img, 0, half_h, width, height).to_image();
            let up_flipped = imageops::flip_vertical(&up);
            imageops::replace(&mut img, &up_flipped, 0, 0);
        }
        let bs64 = img_to_base64(DynamicImage::from(img))?;
        let mut rep = SendMsg::new().await;
        let time = time.elapsed();
        rep.join_reply(msg.message_id).await;
        rep.join_image(format!("base64://{}", bs64)).await;
        rep.join_text(format!("耗时:{:?}",time)).await;
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {
        if self.enabled {
            self.status.store(true, Relaxed);
        }
    }

    async fn status(&self) -> bool {
        self.status.load(Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "图片中间裁剪并镜像\n使用方法：\n图片镜像<上/下/左/右><图片>\n[回复图片]图片镜像<上/下/左/右>\n左右分别为保留图片的左/右半部分\neg：\n图片镜像左<图片>".to_string()
    }

    async fn name(&self) -> String {
        "图片镜像".to_string()
    }
}