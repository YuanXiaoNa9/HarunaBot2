use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tracing::{error, info};

pub static FUNC_CONFIG: OnceLock<FuncConfig> = OnceLock::new();
#[derive(Serialize, Deserialize, Debug)]
pub struct FuncConfig {
    pub(crate) postgres: PGConfig,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct PGConfig {
    pub pg_ip_port: String,
    pub pg_username: String,
    pub db_name: String,
    pub pg_password: String,
}
pub fn func_config_get() {
    //读取文件并存入string
    let res_config_str = std::fs::read_to_string("func_config.yaml");
    //出现err则创建文件并写入默认内容
    if res_config_str.is_err() {
        error!("Failed to read func_config file");
        create_config();
        std::process::exit(1);
    }
    //解析配置文件并存入结构体
    let config_str = res_config_str.unwrap();
    let res_config = serde_yaml::from_str(&config_str);
    //出现问题则写入默认内容
    if res_config.is_err() {
        error!("{}", res_config.err().unwrap());
        match std::fs::rename("func_config.yaml", "func_config.backup") {
            Ok(_) => {
                info!(
                    "原配置文件已备份成功,文件名: 'func_config.backup' 请检查原配置文件格式是否正确"
                )
            }
            Err(e) => {
                error!("尝试备份文件失败: {}", e);
            }
        }
        create_config();
        std::process::exit(1);
    }
    //合并默认以及现有内容

    let config: FuncConfig = res_config.unwrap();
    let new_config_str: String = serde_yaml::to_string(&config).unwrap();
    if new_config_str != config_str {
        std::fs::write("func_config.yaml", new_config_str).expect("TODO: panic message");
    };
    FUNC_CONFIG.set(config).unwrap();
}

fn create_config() {
    info!("正在写入新配置文件");
    let file_data = FuncConfig {
        postgres: PGConfig {
            pg_ip_port: "".to_string(),
            pg_username: "".to_string(),
            db_name: "".to_string(),
            pg_password: "".to_string(),
        },
    };
    let file_data = serde_yaml::to_string(&file_data).unwrap();
    std::fs::write("func_config.yaml", file_data).unwrap();
}
