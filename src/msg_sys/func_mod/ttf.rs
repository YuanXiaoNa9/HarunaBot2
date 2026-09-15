use crate::msg_sys::msg_sys::ModHandler;
use ab_glyph::FontVec;
use async_trait::async_trait;
use std::sync::LazyLock;
use tokio::sync::{OnceCell, watch};
use tokio::time::sleep;
use tracing::error;

pub static TTF: LazyLock<TtfData> = LazyLock::new(ttf_init);
fn ttf_init() -> TtfData {
    let (tx, rx) = watch::channel(false);
    TtfData {
        ttf: OnceCell::new(),
        tx,
        rx,
    }
}
#[derive(Debug)]
pub struct TtfData {
    pub ttf: OnceCell<FontVec>,
    pub tx: watch::Sender<bool>,
    pub rx: watch::Receiver<bool>,
}
#[async_trait]
impl ModHandler for TtfData {
    async fn init(&self) {
        loop {
            match FontVec::try_from_vec(include_bytes!("ttf/siyuan.ttf").to_vec()) {
                Ok(ttf) => {
                    let err = TTF.ttf.set(ttf);
                    if err.is_err() {
                        error!("TTF error: {:?}", err.unwrap_err());
                    }
                    TTF.tx.send(true).unwrap();
                    break;
                }
                Err(e) => {
                    error!("{:?}", e);
                    sleep(std::time::Duration::from_secs(3)).await;
                }
            }
        }
    }

    async fn name(&self) -> String {
        "字体".to_string()
    }

    async fn init_status(&self) -> bool {
        *TTF.tx.clone().borrow()
    }
}
