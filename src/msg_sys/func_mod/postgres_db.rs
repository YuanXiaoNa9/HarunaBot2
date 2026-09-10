use crate::msg_sys::func_config::FUNC_CONFIG;
use crate::msg_sys::msg_sys::ModHandler;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};
use std::sync::LazyLock;
use tokio::sync::{OnceCell, watch};
use tokio::time::sleep;
use tracing::error;

pub static DBLINK: LazyLock<DbLink> = LazyLock::new(db_init);
fn db_init() -> DbLink {
    let (tx, rx) = watch::channel(false);
    DbLink {
        db_link: OnceCell::new(),
        tx,
        rx,
    }
}
#[derive(Debug)]
pub struct DbLink {
    pub db_link: OnceCell<Pool<Postgres>>,
    pub tx: watch::Sender<bool>,
    pub rx: watch::Receiver<bool>,
}
#[async_trait]
impl ModHandler for DbLink {
    async fn init(&self) {
        let config = FUNC_CONFIG.get().unwrap();
        let db_url = format!(
            "postgres://{}:{}@{}/{}",
            config.postgres.pg_username,
            config.postgres.pg_password,
            config.postgres.pg_ip_port,
            config.postgres.db_name
        );
        loop {
            let res_pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(10)
                .connect(db_url.as_str())
                .await;
            match res_pool {
                Ok(pool) => {
                    DBLINK.db_link.set(pool).unwrap();
                    DBLINK.tx.send(true).unwrap();
                    break;
                }
                Err(e) => {
                    error!("{}", e);
                    sleep(std::time::Duration::from_secs(3)).await;
                }
            };
        }
    }

    async fn name(&self) -> String {
        "pg数据库".to_string()
    }

    async fn init_status(&self) -> bool {
        *DBLINK.rx.clone().borrow()
    }
}
