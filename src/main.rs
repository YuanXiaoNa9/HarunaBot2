use anyhow::anyhow;
use serde::{Serialize, Deserialize };
use log::trace;

#[derive(Serialize,Deserialize)]
struct MainConfig {
    ws_ip_port: String,
    http_ip_port: String,
    bw_status: String,
    black_list: String,
    white_list: String,
}
fn main() -> anyhow::Result<()> {
    //读取配置文件
    let main_config = get_config("main_config.yaml");
    Ok(())
}
fn get_config(file_name: &str) -> MainConfig {
    //读取文件并存入string
    let a=std::fs::read_to_string(file_name);
    //出现err则创建文件并写入默认内容
    if a.is_err() {
        tracing::error!("Could not read config file: {}", a.err().unwrap());
        create_config(file_name);
        std::process::exit(1);
    }
    //解析配置文件并存入结构体
    let a=a.unwrap();
    let b = serde_yaml::from_str(&a);
    //出现问题则
    if b.is_err() {
        create_config(file_name);
        std::process::exit(1);
    }
    let b=b.unwrap();
    return b

}

fn create_config(file_name: &str) {
    let file_data = MainConfig {
        ws_ip_port: "".to_string(),
        http_ip_port: "".to_string(),
        bw_status: "".to_string(),
        black_list: "".to_string(),
        white_list: "".to_string(),
    };
    let file_data = serde_yaml::to_string(&file_data).unwrap();
    std::fs::write(file_name, file_data).unwrap();
}
