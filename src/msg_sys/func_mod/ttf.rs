use crate::msg_sys::msg_sys::ModHandler;
use ab_glyph::FontVec;
use async_trait::async_trait;
use std::sync::OnceLock;
use tracing::error;

pub static TTF: OnceLock<TtfData> = OnceLock::new();

#[derive(Debug)]
pub struct TtfData {
    pub status: bool,
    pub ttf: OnceLock<FontVec>,
}
#[async_trait]
impl ModHandler for TtfData {
    async fn init(&self) -> bool {
        match FontVec::try_from_vec(include_bytes!("ttf/siyuan.ttf").to_vec()) {
            Ok(ttf) => {
                TTF.set(TtfData {
                    status: true,
                    ttf: OnceLock::from(ttf),
                })
                .unwrap();
                true
            }
            Err(e) => {
                TTF.set(TtfData {
                    status: false,
                    ttf: OnceLock::new(),
                })
                .expect("TODO: panic message");
                error!("{:?}", e);
                false
            }
        }
    }

    async fn name(&self) -> String {
        "字体".to_string()
    }
}
