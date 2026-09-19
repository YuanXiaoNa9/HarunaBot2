use crate::msg_sys::func_msg::emoji_photo::{get_img, get_img_id, img_to_base64};
use crate::msg_sys::msg_analysis::Msg;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::FnHandler;
use anyhow::Error;
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::ImageFormat::Gif;
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, Frame, ImageReader};
use image::{GenericImageView, imageops};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use rusty_gif::Encoder;
use rusty_gif::Frame as GifFrame;
use rusty_gif::Repeat::Infinite;
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

pub struct FlipHorizontally {
    pub enabled: bool,
    pub status: AtomicBool,
}
#[async_trait]
impl FnHandler for FlipHorizontally {
    async fn matches(&self, msg: &Msg) -> bool {
        if (msg.raw_message.contains("镜像") && msg.raw_message.contains("[CQ:image,"))
            || (msg.raw_message.starts_with("[CQ:reply,id=") && msg.raw_message.contains("]镜像"))
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
        let fmt = image::guess_format(&bytes)?;
        let bs64: String;
        if fmt == Gif {
            let decoders = GifDecoder::new(Cursor::new(bytes))?;
            let frames = decoders.into_frames().collect_frames()?;
            let new_frames: Vec<Frame> = frames
                .into_par_iter()
                .map(|frame| {
                    let delay = frame.delay();
                    let img = frame.into_buffer();
                    let new_img = photo_process(msg, DynamicImage::from(img));
                    Frame::from_parts(new_img.into(), 0, 0, delay)
                })
                .collect();
            let mut buf = Vec::new();
            let w = new_frames[0].buffer().width() as u16;
            let h = new_frames[0].buffer().height() as u16;

            {
                let mut encoder = Encoder::new(&mut buf, w, h, &[])?;
                encoder.set_repeat(Infinite)?;
                for new_frame in new_frames {
                    let (num, den) = new_frame.delay().numer_denom_ms();
                    let delay = num as f64 / den as f64;
                    let delay = (delay / 10.0).round() as u16;
                    let mut img = new_frame.into_buffer();
                    let mut gif_img = GifFrame::from_rgba(w, h, &mut img);
                    gif_img.delay = delay;
                    encoder.write_frame(&rusty_gif::Frame::from(gif_img))?;
                }
            }
            let str = STANDARD.encode(&buf);
            bs64 = str;
        } else {
            let img = ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()?
                .decode()?;
            let img = photo_process(msg, img);
            bs64 = img_to_base64(DynamicImage::from(img))?;
        }

        let mut rep = SendMsg::new().await;
        let time = time.elapsed();
        rep.join_reply(msg.message_id).await;
        rep.join_image(format!("base64://{}", bs64)).await;
        rep.join_text(format!("耗时:{:?}", time)).await;
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
        "图片中间裁剪并镜像\n使用方法：\n镜像<上/下/左/右><图片>\n[回复图片]镜像<上/下/左/右>\n左右分别为保留图片的左/右半部分\neg：\n镜像左<图片>".to_string()
    }

    async fn name(&self) -> String {
        "图片镜像".to_string()
    }
}
fn photo_process(msg: &Msg, mut img: DynamicImage) -> DynamicImage {
    let (width, height) = GenericImageView::dimensions(&img);
    let half_w = width / 2;
    let half_h = height / 2;
    if msg.raw_message.contains("左") {
        let left = imageops::crop_imm(&img, 0, 0, half_w, height).to_image();
        let left_flipped = imageops::flip_horizontal(&left);
        imageops::replace(&mut img, &left_flipped, half_w as i64, 0);
    } else if msg.raw_message.contains("右") {
        let right = imageops::crop_imm(&img, half_w, 0, width, height).to_image();
        let right_flipped = imageops::flip_horizontal(&right);
        imageops::replace(&mut img, &right_flipped, 0, 0);
    } else if msg.raw_message.contains("上") {
        let up = imageops::crop_imm(&img, 0, 0, width, half_h).to_image();
        let up_flipped = imageops::flip_vertical(&up);
        imageops::replace(&mut img, &up_flipped, 0, half_h as i64);
    } else if msg.raw_message.contains("下") {
        let up = imageops::crop_imm(&img, 0, half_h, width, height).to_image();
        let up_flipped = imageops::flip_vertical(&up);
        imageops::replace(&mut img, &up_flipped, 0, 0);
    }
    img
}
