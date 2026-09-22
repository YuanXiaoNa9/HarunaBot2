use crate::msg_sys::msg_sys::ModHandler;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::LazyLock;
use tokio::sync::watch;
pub static PLUSONE_DATA: LazyLock<PlusOneMap> = LazyLock::new(plusone_data_init);

fn plusone_data_init() -> PlusOneMap {
    let (tx, rx) = watch::channel(false);
    let a = PlusOneMap {
        map: DashMap::new(),
        rx,
        tx,
    };
    a
}

pub struct PlusOneData {
    pub last_message: String,
    pub user_id: i64,
    pub i: i16,
}
pub struct PlusOneMap {
    pub map: DashMap<i64, PlusOneData>,
    pub rx: watch::Receiver<bool>,
    pub tx: watch::Sender<bool>,
}
#[async_trait]
impl ModHandler for PlusOneMap {
    async fn init(&self) {
        PLUSONE_DATA.tx.send(true).expect("TODO: panic message");
    }

    async fn name(&self) -> String {
        "加一".to_string()
    }

    async fn init_status(&self) -> bool {
        *PLUSONE_DATA.rx.borrow()
    }
}
