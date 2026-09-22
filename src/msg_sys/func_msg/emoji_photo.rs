use crate::PATH;
use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{FnHandler, Msg};
use crate::qq_link::{HTTP_CLIENT, http_get};
use anyhow::{Error, anyhow};
use anyhow_trace::anyhow_trace;
use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::ImageFormat::{Gif, Png};
use image::codecs::gif::GifDecoder;
use image::imageops::overlay;
use image::{AnimationDecoder, DynamicImage, Frame, GenericImageView, ImageReader, imageops};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use rusty_gif::DisposalMethod::Background;
use rusty_gif::Encoder;
use rusty_gif::Frame as GifFrame;
use rusty_gif::Repeat::Infinite;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use tracing::log::debug;
#[derive(PartialEq)]
enum ProcessEnum {
    None(),
    Die(),
    Mirror(String),
    Invert(),
}
pub struct MemPhoto {
    pub(crate) status: AtomicBool,
}
#[async_trait]
impl FnHandler for MemPhoto {
    async fn matches(&self, msg: &Msg) -> bool {
        if msg.raw_message.starts_with("[bot_msg]"){
            return false;
        }
        let start = match msg.raw_message.rfind("]") {
            None => return false,
            Some(ok) => ok,
        };
        let splits = msg.raw_message[start + 1..].split(" ");
        let mut ok = false;
        for s in splits {
            if s == "怀念"
                || s == "镜像上"
                || s == "镜像下"
                || s == "镜像左"
                || s == "镜像右"
                || s == "反色"
            {
                ok = true;
            }
        }
        (msg.raw_message.contains("[CQ:image,") || msg.raw_message.contains("[CQ:reply,id=")) && ok
    }
    #[anyhow_trace]
    async fn process(&self, msg: &Msg) -> Result<(), Error> {
        let start = msg.raw_message.rfind("]").unwrap();
        let splits = msg.raw_message[start + 1..].split(" ");
        let mut ways: Vec<ProcessEnum> = Vec::new();
        for s in splits {
            if s == "怀念" {
                ways.push(ProcessEnum::Die())
            } else if s == "镜像上" || s == "镜像下" || s == "镜像左" || s == "镜像右" {
                ways.push(ProcessEnum::Mirror(s.to_string()))
            } else if s == "反色" {
                ways.push(ProcessEnum::Invert())
            } else {
                ways.push(ProcessEnum::None())
            };
        }
        let start = std::time::Instant::now();
        let id = get_img_id(msg).await?;
        let bytes = get_img(id).await;
        let fmt = image::guess_format(&bytes)?;
        let b64: String;
        if fmt == Gif {
            let mut buf = Vec::new();
            let decoders = GifDecoder::new(Cursor::new(bytes))?;
            let mut frames = decoders.into_frames().collect_frames()?;
            let mut w = frames[0].buffer().width() as u16;
            let mut h = frames[0].buffer().height() as u16;
            if msg.raw_message.contains("怀念") {
                (w, h) = (778u16, 777u16);
            }
            {
                let mut encoder = Encoder::new(&mut buf, w, h, &[])?;
                encoder.set_repeat(Infinite)?;
                for way in ways {
                    frames = frames
                        .into_par_iter()
                        .map(|frame| {
                            let delay = frame.delay();
                            let img = frame.into_buffer();
                            let new_img =
                                photo_main_process(DynamicImage::from(img), &way).unwrap();
                            let new_frame = Frame::from_parts(new_img.into(), 0, 0, delay);
                            new_frame
                        })
                        .collect();
                }

                let new_frames: Vec<rusty_gif::Frame> = frames
                    .into_par_iter()
                    .map(|frame| {
                        let dispose = frame.buffer().pixels().any(|p| p.0[3] < 255);
                        let (num, den) = frame.delay().numer_denom_ms();
                        let delay = num as f64 / den as f64;
                        let delay = (delay / 10.0).round() as u16;
                        let mut img = frame.into_buffer();
                        let mut gif_img = GifFrame::from_rgba(w, h, &mut img);
                        gif_img.delay = delay;
                        if dispose {
                            gif_img.dispose = Background
                        }
                        gif_img
                    })
                    .collect();

                for new_frame in new_frames {
                    encoder.write_frame(&new_frame)?;
                }
            }
            let str = STANDARD.encode(&buf);
            b64 = str;
        } else {
            let img = ImageReader::new(Cursor::new(&bytes))
                .with_guessed_format()?
                .decode();
            let mut img = img?;
            for way in ways {
                if way == ProcessEnum::None() {
                    continue;
                }
                img = photo_main_process(img, &way)?;
            }
            b64 = img_to_base64(img)?;
        }
        let mut rep = SendMsg::new().await;
        rep.join_reply(msg.message_id).await;
        rep.join_image(format!("base64://{}", b64)).await;
        rep.join_text(format!("耗时:{:?}", start.elapsed())).await;
        rep.send_msg(msg).await;
        Ok(())
    }

    async fn init(&self) {
        self.status.store(true, Relaxed);
    }

    async fn status(&self) -> bool {
        self.status.load(Relaxed)
    }

    async fn help(&self, _: &str) -> String {
        "生成表情包,支持gif,支持遗照,镜像上下左右,反色,可一次性使用多个\neg:\n[回复图片]镜像右 反色 遗照\n根据指令顺序进行修改，比如eg中，先镜像右边，然后去图片的反色，最后加上遗照的边框并黑白".to_string()
    }

    async fn name(&self) -> String {
        "生成表情包".to_string()
    }
}
pub async fn get_img(file_id: String) -> tungstenite::Bytes {
    let resp = HTTP_CLIENT.get(&file_id).send().await.unwrap();
    let bytes = resp.bytes().await.unwrap();
    bytes
}
#[anyhow_trace]
pub async fn get_img_id(msg: &Msg) -> Result<String, Error> {
    #[derive(Serialize, Deserialize, Debug)]
    struct Body {
        file: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct BodyGetMsg {
        message_id: String,
    }
    #[derive(Deserialize, Debug)]
    struct ImageData {
        data: Data,
        stream: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct Data {
        file: String,
        url: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct MessageData {
        status: String,
        retcode: i16,
        data: MsgData,
        message: String,
        wording: String,
        stream: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct MsgData {
        self_id: i64,
        user_id: i64,
        time: i64,
        message_id: i64,
        message_seq: i64,
        real_id: i64,
        real_seq: String,
        message_type: String,
        sender: Sender,
        raw_message: String,
        font: i16,
        sub_type: String,
        post_type: String,
    }
    #[derive(Serialize, Deserialize, Debug)]
    struct Sender {
        user_id: i64,
        nickname: String,
        card: String,
    }
    let start: usize;
    let end: usize;
    let id: String;
    if msg.raw_message.starts_with("[CQ:reply,id=") {
        let start1 = msg.raw_message.find("[CQ:reply,id=").unwrap() + 13;
        let end1 = msg.raw_message.find("]").unwrap();
        let message_id = msg.raw_message[start1..end1].to_string();
        let req = BodyGetMsg { message_id };
        let rsp = http_get("/get_msg").json(&req).send().await?;
        let str = rsp.text().await?;
        debug!("{}", str);
        let res: MessageData = serde_json::from_str(&str)?;
        let msg = res.data;
        start = msg.raw_message.find(",file=").unwrap() + 6;
        end = msg.raw_message.find(",sub_type=").unwrap();
        id = msg.raw_message[start..end].to_string();
    } else if msg.raw_message.contains("[CQ:image,file=") {
        start = msg.raw_message.find(",file=").unwrap() + 6;
        end = msg.raw_message.find(",sub_type=").unwrap();
        id = msg.raw_message[start..end].to_string();
    } else if msg.raw_message.contains("[CQ:image,summary=") {
        start = msg.raw_message.find(",file=").unwrap() + 6;
        end = msg.raw_message.find(",sub_type=").unwrap();
        id = msg.raw_message[start..end].to_string();
    } else {
        return Err(anyhow!("未找到图片id"));
    }
    let req = Body { file: id };
    let rsp = http_get("/get_image").json(&req).json(&req).send().await?;
    let str = rsp.text().await?;
    debug!("{}", &str);
    let resp: ImageData = serde_json::from_str(&str)?;
    Ok(resp.data.url)
}

pub fn img_to_base64(pic: DynamicImage) -> Result<String, Error> {
    let mut buf = Cursor::new(Vec::new());
    pic.write_to(&mut buf, Png)?;
    let b64 = STANDARD.encode(buf.into_inner());
    Ok(b64)
}
fn photo_main_process(img: DynamicImage, way: &ProcessEnum) -> Result<DynamicImage, Error> {
    let res: Result<DynamicImage, Error> = match way {
        ProcessEnum::None() => Err(anyhow!("未知错误")),
        ProcessEnum::Die() => photo_process_die(img),
        ProcessEnum::Mirror(s) => Ok(photo_process_mirror(s, img)),
        ProcessEnum::Invert() => Ok(photo_process_invert(img)),
    };
    res
}
fn photo_process_die(img: DynamicImage) -> Result<DynamicImage, Error> {
    let proportion: f32;
    let (ori_x, ori_y) = GenericImageView::dimensions(&img);
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
        imageops::FilterType::CatmullRom,
    );
    let end_img: DynamicImage;
    if ori_x > ori_y {
        end_img = img.crop(((ori_x as f32 * proportion) as u32 - 515) / 2, 0, 515, 515);
    } else if ori_y > ori_x {
        end_img = img.crop(0, ((ori_y as f32 * proportion) as u32 - 515) / 2, 515, 515);
    } else {
        end_img = img.crop(0, 0, 515, 515);
    }
    let mut ground_img = ImageReader::open(format!("{}/pic/die.jpg", PATH.as_str()))?.decode()?;
    let end_img = end_img.grayscale();
    overlay(&mut ground_img, &end_img, 126, 107);
    Ok(ground_img)
}
fn photo_process_mirror(s: &String, mut img: DynamicImage) -> DynamicImage {
    let (width, height) = GenericImageView::dimensions(&img);
    let half_w = width / 2;
    let half_h = height / 2;
    if s.contains("左") {
        let left = imageops::crop_imm(&img, 0, 0, half_w, height).to_image();
        let left_flipped = imageops::flip_horizontal(&left);
        imageops::replace(&mut img, &left_flipped, half_w as i64, 0);
    } else if s.contains("右") {
        let right = imageops::crop_imm(&img, half_w, 0, width, height).to_image();
        let right_flipped = imageops::flip_horizontal(&right);
        imageops::replace(&mut img, &right_flipped, 0, 0);
    } else if s.contains("上") {
        let up = imageops::crop_imm(&img, 0, 0, width, half_h).to_image();
        let up_flipped = imageops::flip_vertical(&up);
        imageops::replace(&mut img, &up_flipped, 0, half_h as i64);
    } else if s.contains("下") {
        let up = imageops::crop_imm(&img, 0, half_h, width, height).to_image();
        let up_flipped = imageops::flip_vertical(&up);
        imageops::replace(&mut img, &up_flipped, 0, 0);
    }
    img
}

fn photo_process_invert(img: DynamicImage) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    for pixel in &mut rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        *pixel = image::Rgba([255 - r, 255 - g, 255 - b, a]);
    }
    DynamicImage::ImageRgba8(rgba)
}
