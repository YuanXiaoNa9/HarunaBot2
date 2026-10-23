use super::*;

impl PjskState {
    pub(super) async fn accounts(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        let k = key(msg);
        match first {
            "账号列表" => {
                let xs = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let accounts = xs.as_array().cloned().unwrap_or_default();
                let text = if accounts.is_empty() {
                    "尚未绑定账号。\n\n手机号绑定：绑定账号 手机号\nTapTap 绑定：绑定账号 TapTap\n详细说明：/help PJSK数据上传-绑定账号".into()
                } else {
                    format!(
                        "已绑定账号：{} 个\n\n{}\n\n更新默认账号：更新数据\n更新指定账号：更新数据 编号\n管理指令：/help PJSK数据上传",
                        accounts.len(),
                        accounts
                            .iter()
                            .map(account_block)
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    )
                };
                reply(msg, text).await;
            }
            "默认账号" | "账号备注" => {
                let Some(id) = words.get(1) else {
                    reply(msg, "请填写账号编号，可用 账号列表 查看。".into()).await;
                    return Ok(());
                };
                let accounts = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let acc = accounts
                    .as_array()
                    .and_then(|xs| xs.iter().find(|a| a["id"] == *id))
                    .cloned();
                let Some(mut acc) = acc else {
                    reply(msg, "没有找到该账号。\n\n查看编号：账号列表".into()).await;
                    return Ok(());
                };
                let body = if first == "默认账号" {
                    json!({"default":true})
                } else {
                    json!({"remark":words.get(2..).unwrap_or(&[]).join(" ")})
                };
                c.call(
                    msg,
                    reqwest::Method::PATCH,
                    &format!("accounts/{id}"),
                    body.clone(),
                )
                .await?;
                if first == "默认账号" {
                    acc["default"] = json!(true);
                } else {
                    acc["remark"] = body["remark"].clone();
                }
                reply(
                    msg,
                    format!(
                        "账号设置已保存\n\n{}\n\n查看所有账号：账号列表",
                        account_block(&acc)
                    ),
                )
                .await;
            }
            "解除绑定" => {
                if !private(msg) {
                    reply(msg, "请在私聊或群临时会话中解除绑定。".into()).await;
                    return Ok(());
                }
                let id = words.get(1).copied().unwrap_or("");
                if !number(id) || words.len() != 2 {
                    reply(
                        msg,
                        "格式：解除绑定 编号\n例如：解除绑定 2\n\n查看编号：账号列表".into(),
                    )
                    .await;
                    return Ok(());
                }
                let xs = c
                    .call(msg, reqwest::Method::GET, "accounts", Value::Null)
                    .await?;
                let acc = xs
                    .as_array()
                    .and_then(|xs| xs.iter().find(|a| a["id"] == id));
                let Some(acc) = acc else {
                    reply(msg, "没有找到该账号。\n\n请发送 账号列表 核对编号。".into()).await;
                    return Ok(());
                };
                let display = account_block(acc);
                let revision = acc["revision"]
                    .as_str()
                    .ok_or_else(Fault::unavailable)?
                    .to_owned();
                self.deletes.insert(
                    k,
                    DeleteConfirmation {
                        account: id.into(),
                        revision,
                        display: display.clone(),
                        expires: now() + 60,
                    },
                );
                reply(msg,format!("请确认解除绑定\n\n{display}\n\n仅移除本工具内的凭证和缓存，不删除游戏账号或已上传数据。\n移除默认账号后，剩余最早绑定的账号会成为默认账号。\n\n确认请在 60 秒内发送：确认解除绑定 {id}\n放弃操作请等待本次确认过期。")).await;
            }
            "确认解除绑定" => {
                if !private(msg) {
                    reply(msg, "请在私聊或群临时会话中确认。".into()).await;
                    return Ok(());
                }
                let confirmation = self.deletes.remove(&k).map(|(_, p)| p).filter(|p| {
                    p.expires > now()
                        && words.len() == 2
                        && words.get(1).is_some_and(|id| *id == p.account)
                });
                let Some(p) = confirmation else {
                    reply(
                        msg,
                        "没有匹配的解除绑定确认，或确认已过期。\n\n请重新发送：解除绑定 编号"
                            .into(),
                    )
                    .await;
                    return Ok(());
                };
                c.call(
                    msg,
                    reqwest::Method::DELETE,
                    &format!("accounts/{}", p.account),
                    json!({"revision":p.revision}),
                )
                .await?;
                reply(
                    msg,
                    format!(
                        "已解除绑定\n\n{}\n\n本工具内的凭证和缓存已移除。\n查看剩余账号：账号列表",
                        p.display
                    ),
                )
                .await;
            }
            _ => {}
        }
        Ok(())
    }
}
