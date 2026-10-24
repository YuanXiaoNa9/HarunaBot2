use anyhow::{Result, anyhow};
use image::{DynamicImage, RgbaImage, imageops};
use std::sync::OnceLock;

const STAMP_BYTES: &[u8] = include_bytes!("../../../../pic/sold_out.png");
static STAMP: OnceLock<Result<RgbaImage, image::ImageError>> = OnceLock::new();

pub(super) const HELP: &str = "卖掉了\n\n用法：回复一张图片，发送 卖掉了；也可以图片与指令一起发送。\n效果：底图转灰度，居中叠加卖掉了图章；图章等比例缩放至底图约50%，透明度90%。\n支持静态图和GIF，可与其他处理指令按顺序组合。\n例如：[回复图片]卖掉了\n例如：[回复图片]镜像右 卖掉了";

// Preserve the original order of text commands around CQ image/reply segments.
pub(super) fn message_commands(raw: &str) -> Option<String> {
    if !raw.contains("[CQ:image,") && !raw.contains("[CQ:reply,id=") {
        return None;
    }
    let mut remaining = raw;
    let mut text = String::new();
    while let Some(start) = remaining.find("[CQ:") {
        text.push_str(&remaining[..start]);
        text.push(' ');
        let end = remaining[start..].find(']')?;
        remaining = &remaining[start + end + 1..];
    }
    text.push_str(remaining);
    let commands: Vec<_> = text.split_whitespace().collect();
    commands.contains(&"卖掉了").then(|| commands.join(" "))
}

pub(super) fn apply(img: DynamicImage) -> Result<DynamicImage> {
    let stamp = STAMP
        .get_or_init(|| image::load_from_memory(STAMP_BYTES).map(|img| img.to_rgba8()))
        .as_ref()
        .map_err(|_| anyhow!("卖掉了图章素材读取失败，请联系维护者"))?;
    composite(img, stamp)
}

fn composite(img: DynamicImage, stamp: &RgbaImage) -> Result<DynamicImage> {
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return Err(anyhow!("图片尺寸无效，无法生成卖掉了表情"));
    }
    let mut base = img.grayscale().to_rgba8();
    // Fit within half of both dimensions, preserving the stamp's aspect ratio.
    let scale = ((width / 2).max(1) as f64 / stamp.width() as f64)
        .min((height / 2).max(1) as f64 / stamp.height() as f64);
    let stamp_width = (stamp.width() as f64 * scale).round().max(1.0) as u32;
    let stamp_height = (stamp.height() as f64 * scale).round().max(1.0) as u32;
    let mut stamp = imageops::resize(
        stamp,
        stamp_width,
        stamp_height,
        imageops::FilterType::Lanczos3,
    );
    for pixel in stamp.pixels_mut() {
        // Multiply existing alpha rather than making transparent holes opaque.
        pixel[3] = ((u16::from(pixel[3]) * 9 + 5) / 10) as u8;
    }
    imageops::overlay(
        &mut base,
        &stamp,
        i64::from((width - stamp_width) / 2),
        i64::from((height - stamp_height) / 2),
    );
    Ok(DynamicImage::ImageRgba8(base))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    #[test]
    fn centers_stamp_preserving_holes_and_ninety_percent_opacity() {
        let base = DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255])));
        let mut stamp = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        stamp.put_pixel(0, 0, Rgba([71, 112, 76, 0]));
        let output = composite(base, &stamp).unwrap().to_rgba8();
        assert_eq!(output.dimensions(), (4, 4));
        assert_eq!(output.get_pixel(0, 0).0, [0, 0, 0, 255]);
        assert_eq!(output.get_pixel(1, 1).0, [0, 0, 0, 255]);
        let pixel = output.get_pixel(2, 2);
        assert!((228..=230).contains(&pixel[0]));
        assert_eq!(pixel[0], pixel[1]);
        assert_eq!(pixel[1], pixel[2]);
        assert_eq!(pixel[3], 255);
    }

    #[test]
    fn real_asset_keeps_dimensions_and_grayscale_background() {
        let base = RgbaImage::from_pixel(32, 48, Rgba([220, 100, 30, 190]));
        let output = apply(DynamicImage::ImageRgba8(base)).unwrap().to_rgba8();
        assert_eq!(output.dimensions(), (32, 48));
        let corner = output.get_pixel(0, 0);
        assert_eq!(corner[0], corner[1]);
        assert_eq!(corner[1], corner[2]);
        assert_eq!(corner[3], 190);
        for (width, height) in [(1, 1), (1000, 2), (2, 1000)] {
            assert_eq!(
                apply(DynamicImage::new_rgba8(width, height))
                    .unwrap()
                    .into_rgba8()
                    .dimensions(),
                (width, height)
            );
        }
    }

    #[tokio::test]
    async fn uses_existing_image_and_reply_command_routing() {
        use crate::msg_sys::func_msg::emoji_photo::{MemPhoto, ProcessEnum, photo_main_process};
        use crate::msg_sys::msg_sys::{FnHandler, Msg};
        use std::sync::atomic::AtomicBool;
        let handler = MemPhoto {
            status: AtomicBool::new(true),
        };
        for text in [
            "[CQ:reply,id=1]卖掉了",
            "[CQ:image,file=test]卖掉了",
            "[CQ:reply,id=1]镜像右 卖掉了",
            "卖掉了[CQ:image,file=test]",
            "镜像右 卖掉了 反色[CQ:image,file=test]",
            "镜像右 [CQ:image,file=test] 卖掉了 反色",
        ] {
            let message = Msg {
                raw_message: text.into(),
                ..Default::default()
            };
            assert!(handler.matches(&message).await);
        }
        assert!(
            !handler
                .matches(&Msg {
                    raw_message: "卖掉了".into(),
                    ..Default::default()
                })
                .await
        );
        let output =
            photo_main_process(DynamicImage::new_rgb8(32, 48), &ProcessEnum::SoldOut()).unwrap();
        assert_eq!((output.width(), output.height()), (32, 48));
        assert!(
            handler
                .help("生成表情包-卖掉了")
                .await
                .contains("透明度90%")
        );
    }
}

#[cfg(test)]
mod order_tests {
    use super::*;
    use crate::msg_sys::func_msg::emoji_photo::{ProcessEnum, photo_main_process};

    #[test]
    fn sold_out_composes_in_order_with_existing_effects() {
        let original = DynamicImage::new_rgb8(96, 128);
        let stamp_then_invert = photo_main_process(
            photo_main_process(original.clone(), &ProcessEnum::SoldOut()).unwrap(),
            &ProcessEnum::Invert(),
        )
        .unwrap()
        .to_rgba8();
        let invert_then_stamp = photo_main_process(
            photo_main_process(original, &ProcessEnum::Invert()).unwrap(),
            &ProcessEnum::SoldOut(),
        )
        .unwrap()
        .to_rgba8();
        assert_eq!(stamp_then_invert.dimensions(), (96, 128));
        assert_eq!(invert_then_stamp.dimensions(), (96, 128));
        assert_eq!(
            stamp_then_invert.get_pixel(0, 0),
            invert_then_stamp.get_pixel(0, 0)
        );
        assert_ne!(
            stamp_then_invert, invert_then_stamp,
            "later effects must also process the stamp"
        );
    }
}
