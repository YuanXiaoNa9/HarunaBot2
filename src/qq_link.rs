use crate::MAIN_CONFIG;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::spawn;
use tokio::sync::mpsc::Sender;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tracing::{error, info};

pub async fn qq_link() -> tokio::sync::mpsc::Receiver<String> {
    //创建消息通道
    let (chan_sender, chan_receiver) = tokio::sync::mpsc::channel(200);
    //构建ws链接

    let request = format!(
        "{}/?access_token={}",
        MAIN_CONFIG.ws_ip_port, MAIN_CONFIG.ws_token
    )
    .into_client_request();
    let request = match request {
        Ok(req) => req,
        Err(e) => {
            error!("{}", e);
            std::process::exit(1);
        }
    };
    //尝试ws连接
    spawn(async move {
        msg_get(request, chan_sender).await;
    });
    chan_receiver
}

async fn msg_get(request: tungstenite::handshake::server::Request, chan_sender: Sender<String>) {
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
            let msg = get.next().await;
            let msg = match msg {
                None => {
                    error!("ws连接出现位置错误");
                    break;
                }
                Some(msg) => msg,
            };
            let msg = match msg {
                Ok(msg) => msg,
                Err(e) => {
                    error!("消息接收出现错误:{}", e);
                    break;
                }
            };
            chan_sender.send(msg.to_string()).await.unwrap();
        }
    }
}
