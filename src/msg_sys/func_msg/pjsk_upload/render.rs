use super::*;
pub(super) fn line(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

pub(super) fn account_block(a: &Value) -> String {
    let id = a["id"]
        .as_str()
        .or_else(|| a["account_id"].as_str())
        .unwrap_or("?");
    let name = a["nickname"]
        .as_str()
        .filter(|s| !s.is_empty())
        .unwrap_or("暂未获取");
    let provider = match a["provider"].as_str() {
        Some("mkcn") => "手机号",
        Some("mkcn_taptap") => "TapTap",
        Some("mkcn_bilibili") => "B站",
        _ => "未知",
    };
    let mut text = format!(
        "编号：{}{}\n游戏昵称：{}\nUID：{}\n登录方式：{}",
        line(id),
        if a["default"] == true {
            "（默认）"
        } else {
            ""
        },
        line(name),
        line(a["game_id"].as_str().unwrap_or("暂未获取")),
        provider
    );
    if let Some(remark) = a["remark"].as_str().filter(|s| !s.is_empty()) {
        text.push_str(&format!("\n备注：{}", line(remark)));
    }
    text
}
#[cfg(not(test))]
pub(super) async fn reply(msg: &Msg, text: String) {
    let mut rep = SendMsg::new().await;
    rep.join_reply(msg.message_id).await;
    rep.join_text(
        text.replace('&', "&amp;")
            .replace('[', "&#91;")
            .replace(']', "&#93;"),
    )
    .await;
    if private(msg) {
        rep.send_private_msg(msg).await;
    } else {
        rep.send_msg(msg).await;
    }
}
#[cfg(test)]
pub(super) static TEST_REPLIES: std::sync::Mutex<Vec<(i64, String)>> =
    std::sync::Mutex::new(Vec::new());
#[cfg(test)]
pub(super) async fn reply(msg: &Msg, text: String) {
    TEST_REPLIES.lock().unwrap().push((msg.message_id, text));
}

impl PjskState {
    pub(super) fn job_result(job: &Value, bind: bool) -> Result<String, Fault> {
        let text = Self::job_text(job, bind);
        if matches!(job["status"].as_str(), Some("failed" | "cancelled")) {
            let mut fault = serde_json::from_value::<Fault>(job["error"].clone())
                .ok()
                .or_else(|| {
                    job["items"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find_map(|it| serde_json::from_value::<Fault>(it["error"].clone()).ok())
                })
                .unwrap_or_else(|| Fault::invalid("操作未完成，请检查任务状态"));
            fault.report = Some(text);
            return Err(fault);
        }
        Ok(text)
    }
    pub(super) fn job_text(job: &Value, bind: bool) -> String {
        let bind = bind || job["kind"] == "bind";
        let status = job["status"].as_str().unwrap_or("unknown");
        let label = match status {
            "success" => {
                if bind {
                    "绑定成功"
                } else {
                    "更新成功"
                }
            }
            "running" => "正在处理",
            "queued" => "等待处理",
            "cancelled" => "操作已终止",
            _ => {
                if bind {
                    "绑定失败"
                } else {
                    "更新未完成"
                }
            }
        };
        let mut blocks = vec![label.to_string()];
        for item in job["items"].as_array().into_iter().flatten() {
            let mut block = account_block(item);
            if status != "success" {
                block.push_str(&format!(
                    "\n结果：{}",
                    match item["status"].as_str() {
                        Some("success") => "成功",
                        Some("queued") => "等待处理",
                        Some("running") => "正在处理",
                        _ => "未完成",
                    }
                ));
            }
            if let Ok(f) = serde_json::from_value::<Fault>(item["error"].clone()) {
                block.push_str(&format!("\n{}", f.display()));
            }
            blocks.push(block);
        }
        if let Ok(f) = serde_json::from_value::<Fault>(job["error"].clone()) {
            let duplicated = job["items"].as_array().into_iter().flatten().any(|it| {
                serde_json::from_value::<Fault>(it["error"].clone()).is_ok_and(|item| item == f)
            });
            if !duplicated {
                blocks.push(f.display());
            }
        }
        if status == "success" {
            blocks.push(if bind{format!("已核对游戏资料并保存授权，本次未上传。\n\n更新默认账号：更新数据\n更新本账号：更新数据 {}\n以后更新无需每次重新输入手机号或验证码。",job["items"][0]["account_id"].as_str().unwrap_or("1"))}else{"数据已提交到 Sakura 公共数据服务。\n再次更新：更新数据".into()});
        } else {
            blocks.push(if bind{"本次未保存新授权；如为重新授权，本次操作未覆盖旧凭证。\n查看账号：账号列表\n重新绑定：/help PJSK数据上传-绑定账号".into()}else{"请先发送 更新状态 查询结果，再决定是否重试。".into()});
            blocks.push(format!(
                "诊断任务：{}\n服务版本：{}",
                job["id"].as_str().unwrap_or("未知"),
                job["version"].as_str().unwrap_or("未知")
            ));
        }
        blocks.join("\n\n")
    }
}
