use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MainConfig {
    pub docker_path: String,
    pub ws_ip_port: String,
    pub ws_token: String,
    pub http_ip_port: String,
    pub http_token: String,
    #[serde(default = "default_bw_status")]
    pub bw_status: String,
    pub black_list: Vec<i64>,
    pub white_list: Vec<i64>,
    pub log_level: String,
}

pub(crate) fn main_config_get() -> MainConfig {
    //读取文件并存入string
    let res_config_str = std::fs::read_to_string("main_config.yaml");
    //出现err则创建文件并写入默认内容
    if res_config_str.is_err() {
        error!("Failed to read main_config file");
        create_config();
        std::process::exit(1);
    }
    //解析配置文件并存入结构体
    let config_str = res_config_str.unwrap();
    let res_config = serde_yaml::from_str(&config_str);
    //出现问题则写入默认内容
    if res_config.is_err() {
        error!("{}", res_config.err().unwrap());
        match std::fs::rename("main_config.yaml", "main_config.backup") {
            Ok(_) => {
                info!(
                    "原配置文件已备份成功,文件名: 'main_config.backup' 请检查原配置文件格式是否正确"
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

    let config: MainConfig = res_config.unwrap();
    let new_config_str: String = serde_yaml::to_string(&config).unwrap();
    if new_config_str != config_str {
        std::fs::write("main_config.yaml", new_config_str).expect("TODO: panic message");
    }
    config
}

fn create_config() {
    info!("正在写入新配置文件");
    let file_data = MainConfig {
        docker_path:"/app/temp".to_string(),
        ws_ip_port: "".to_string(),
        ws_token: "".to_string(),
        http_ip_port: "".to_string(),
        http_token: "".to_string(),
        bw_status: "black".to_string(),
        black_list: Vec::new(),
        white_list: Vec::new(),
        log_level: "info".to_string(),
    };
    let file_data = serde_yaml::to_string(&file_data).unwrap();
    std::fs::write("main_config.yaml", file_data).unwrap();
}

fn default_bw_status() -> String {
    "black".to_string()
}
