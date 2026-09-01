use crate::msg_sys::func_config::FUNC_CONFIG;
use crate::msg_sys::msg_sys::ModHandler;
use async_trait::async_trait;
use sqlx::{Pool, Postgres};
use std::sync::OnceLock;
use tracing::error;

pub static DBLINK: OnceLock<DbLink> = OnceLock::new();
#[derive(Debug)]
pub struct DbLink {
    pub status: bool,
    pub db_link: OnceLock<Pool<Postgres>>,
}
#[async_trait]
impl ModHandler for DbLink {
    async fn init(&self) -> bool {
        let config = FUNC_CONFIG.get().unwrap();
        let db_url = format!(
            "postgres://{}:{}@{}/{}",
            config.pg_username, config.pg_password, config.pg_ip_port, config.db_name
        );
        let res_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await;
        match res_pool {
            Ok(pool) => {
                DBLINK
                    .set(DbLink {
                        status: true,
                        db_link: OnceLock::from(pool),
                    })
                    .expect("TODO: panic message");
                true
            }
            Err(e) => {
                DBLINK
                    .set(DbLink {
                        status: false,
                        db_link: Default::default(),
                    })
                    .expect("TODO: panic message");
                error!("{}", e);
                false
            }
        }
    }

    async fn name(&self) -> String {
        "pg数据库".to_string()
    }
}
