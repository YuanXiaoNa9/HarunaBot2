use crate::msg_sys::func_mod::postgres_db::DBLINK;
use crate::msg_sys::msg_sys::{ModHandler, mod_status_examine};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::atomic::AtomicBool;
use tokio::sync::{RwLock, watch};
use tracing::debug;

pub static GCNAME: LazyLock<GcName> = LazyLock::new(gc_name_new);
fn gc_name_new() -> GcName {
    let (tx, rx) = watch::channel(false);
    GcName {
        map: RwLock::new(HashMap::new()),
        rx,
        tx,
    }
}

pub struct GcData {
    pub gc_id: i64,
}
pub struct GcName {
    pub map: RwLock<HashMap<String, GcData>>,
    pub rx: watch::Receiver<bool>,
    pub tx: watch::Sender<bool>,
}
#[async_trait]
impl ModHandler for GcName {
    async fn init(&self) {
        let rx = DBLINK.rx.clone();
        mod_status_examine(rx).await;
        let mut tx = DBLINK.db_link.get().unwrap().clone().begin().await.unwrap();
        struct Data {
            name: String,
            gc_id: i64,
            gid: i64,
        }
        let vec_name = sqlx::query_as!(Data, "select name,gc_id,gid from gc_name")
            .fetch_all(&mut *tx)
            .await
            .unwrap();
        let mut map = GCNAME.map.write().await;
        map.clear();
        for data in vec_name {
            debug!("found GC {} {}", format!("{}{}", data.name, data.gid), data.gc_id);
            map.insert(
                format!("{}{}", data.name, data.gid),
                GcData { gc_id: data.gc_id },
            );
        }
        let _ = self.tx.send(true);
    }

    async fn name(&self) -> String {
        "机厅名字".to_string()
    }

    async fn init_status(&self) -> bool {
        let rx = DBLINK.rx.clone();
        if mod_status_examine(rx).await && *GCNAME.rx.clone().borrow() {
            return true;
        }
        false
    }
}
