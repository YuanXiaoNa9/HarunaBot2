use crate::msg_sys::msg_reply::{SendMsg, http_ip_process};
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use crate::{HTTP_CLIENT, MAIN_CONFIG, PATH};
use anyhow::{Error, anyhow};
use async_trait::async_trait;
use base64::Engine;
use image::ImageFormat::Png;
use image::imageops::overlay;
use image::{DynamicImage, ImageReader};
use imageproc::drawing::Canvas;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::str::Bytes;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tracing::error;
use tracing::log::debug;

pub struct MemPhoto {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for MemPhoto {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("永远怀念[CQ:image,")
            || msg.raw_message.starts_with("永远怀念\n[CQ:image,")
        {
            return true;
        }
        false
    }

    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let start = std::time::Instant::now();
        let id = get_img_id(msg).await;
        let id = match id {
            None => {
                return Err(anyhow!("未找到图片id"));
            }
            Some(id) => id,
        };
        let bytes = get_img(id).await;
        let img = ImageReader::new(Cursor::new(&bytes))
            .with_guessed_format()?
            .decode();
        let mut img = img?;
        let proportion: f32;
        let (ori_x, ori_y) = img.dimensions();
        if ori_x > ori_y {
            proportion = 515.0 / ori_y as f32;
        } else if ori_y > ori_x {
            proportion = 515.0 / ori_x as f32;
        } else {
            proportion = 515.0 / ori_x as f32;
        }
        debug!("{}", ori_x as f32 * proportion);
        let mut img = img.resize(
            (ori_x as f32 * proportion) as u32,
            (ori_y as f32 * proportion) as u32,
            image::imageops::FilterType::CatmullRom,
        );
        let end_img: DynamicImage;
        if ori_x > ori_y {
            end_img = img.crop(((ori_x as f32 * proportion) as u32 - 515) / 2, 0, 515, 515);
        } else if ori_y > ori_x {
            end_img = img.crop(0, ((ori_y as f32 * proportion) as u32 - 515) / 2, 515, 515);
        } else {
            end_img = img.crop(0, 0, 515, 515);
        }
        let mut ground_img =
            ImageReader::open(format!("{}/pic/die.jpg", PATH.as_str()))?.decode()?;
        let end_img = end_img.grayscale();
        overlay(&mut ground_img, &end_img, 126, 107);
        let mut buf = Cursor::new(Vec::new());
        ground_img.write_to(&mut buf, Png)?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(buf.into_inner());
        let mut rep = SendMsg::new().await;
        rep.join_image(format!("base64://{}", b64)).await;
        rep.join_text(format!("耗时:{:?}", start.elapsed())).await;
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {}

    async fn status(&self) -> bool {
        true
    }

    async fn help(&self) -> String {
        "生成遗照表情包，使用方法：\n永远怀念<图片>".to_string()
    }

    async fn name(&self) -> String {
        "遗照".to_string()
    }
}
async fn get_img(file_id: String) -> tungstenite::Bytes {
    let resp = HTTP_CLIENT.get(&file_id).send().await.unwrap();
    let bytes = resp.bytes().await.unwrap();
    bytes
}
async fn get_img_id(msg: &Msg) -> Option<String> {
    #[derive(Serialize, Deserialize, Debug)]
    struct Body {
        file: String,
    }
    #[derive(Deserialize, Debug)]
    struct ImageData {
        status: String,
        retcode: i16,
        data: Data,
        message: String,
        wording: String,
        stream: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct Data {
        file: String,
        url: String,
    }

    let start = msg.raw_message.find(",file=").unwrap() + 6;
    let end = msg.raw_message.find(",sub_type=").unwrap();
    let id = msg.raw_message[start..end].to_string();
    let req = Body { file: id };
    let url = format!("{}{}", http_ip_process(), "/get_image");

    let rsp = HTTP_CLIENT
        .post(&url)
        .json(&req)
        .header(
            "Authorization",
            format!("Bearer {}", MAIN_CONFIG.nc_setting.http_token),
        )
        .send()
        .await
        .unwrap();
    let str = rsp.text().await.unwrap();
    debug!("{}", &str);
    let resp: ImageData = serde_json::from_str(&str).unwrap();
    Option::from(resp.data.url)
}
