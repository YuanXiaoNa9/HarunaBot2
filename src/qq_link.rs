use crate::main_config::MainConfig;
use futures_util::StreamExt;
use std::sync::mpsc::Receiver;
use std::time::Duration;
use tokio::spawn;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{error, info};

pub async fn qq_link(main_config: &MainConfig) -> Receiver<String> {
    //创建消息通道
    let (chan_sender, chan_receiver) = std::sync::mpsc::channel();
    //构建ws链接
    let request = format!(
        "{}/?access_token={}",
        main_config.ws_ip_port, main_config.ws_token
    )
    .into_client_request()
    .unwrap();
    //尝试ws连接
    spawn(async move {
        data_get(request, chan_sender).await;
    });
    chan_receiver
}

async fn data_get(
    request: tungstenite::handshake::server::Request,
    chan_sender: std::sync::mpsc::Sender<String>,
) {
    loop {
        let request = request.clone();
        let res = tokio_tungstenite::connect_async(request).await;
        if res.is_err() {
            error!("连接出现错误:{}", res.err().unwrap());
            sleep(Duration::from_secs(5)).await;
            continue;
        }
        //抛掉服务器返回消息
        let (ws_stream, _) = res.unwrap();
        //分割发送，接收消息
        let (_, mut get) = ws_stream.split();
        info!("ws连接成功,开始接收消息");
        //循环接收消息
        loop {
            let msg = get.next().await.unwrap().unwrap().to_string();
            chan_sender.send(msg).unwrap();
        }
    }
}
