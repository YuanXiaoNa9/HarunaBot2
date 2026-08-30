use crate::msg_sys::msg_reply::SendMsg;
use crate::msg_sys::msg_sys::{Msg, MsgHandler};
use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use async_trait::async_trait;
use image::{ImageReader, Rgba};
use imageproc::drawing::draw_text_mut;
use std::ops::Deref;
use std::sync::{Arc, LazyLock};

static TTF: LazyLock<FontVec> = LazyLock::new(ttf);

pub struct Test{
    pub(crate) status:bool
}
fn ttf() -> FontVec {
    FontVec::try_from_vec(include_bytes!("/home/q/rust/HarunaBot2/ttf/siyuan.ttf").to_vec())
        .unwrap()
}
#[async_trait]
impl MsgHandler for Test {
    async fn matches(&self, msg: Arc<Msg>) -> bool {
        false
    }

    async fn process(&self, msg: Arc<Msg>) {
        todo!()
    }

    async fn init(&mut self) {
    }
    async fn status(&self) -> bool {
        self.status
    }

}
