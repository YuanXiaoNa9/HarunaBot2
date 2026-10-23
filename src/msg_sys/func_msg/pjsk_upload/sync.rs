use super::*;

impl PjskState {
    pub(super) async fn wait_job(
        &self,
        c: &Client,
        msg: &Msg,
        mut job: Value,
    ) -> Result<Value, Fault> {
        // This future stays in process(), including after the 60-second notice.
        let start = Instant::now();
        let mut warned = false;
        let mut seen = 0u64;
        let deadline = job["deadline"].as_u64().unwrap_or(now() + 300);
        loop {
            for event in job["events"].as_array().into_iter().flatten() {
                let seq = event["sequence"].as_u64().unwrap_or(0);
                if seq <= seen {
                    continue;
                }
                seen = seq;
                let text = event["message"].as_str().unwrap_or("");
                if !text.is_empty() {
                    reply(
                        msg,
                        format!(
                            "{}{}",
                            event["account_id"]
                                .as_str()
                                .filter(|s| !s.is_empty())
                                .map(|id| format!("账号 {}\n\n", line(id)))
                                .unwrap_or_default(),
                            text
                        ),
                    )
                    .await;
                }
            }
            if matches!(
                job["status"].as_str(),
                Some("success" | "failed" | "cancelled")
            ) {
                return Ok(job);
            }
            if !warned && start.elapsed() >= Duration::from_secs(60) {
                reply(
                    msg,
                    "仍在处理中\n\n已等待超过 60 秒，请勿重复发送指令。\n处理结束后会回复结果。"
                        .into(),
                )
                .await;
                warned = true;
            }
            if now() > deadline + 15 {
                return Err(Fault {
                    stage: "wait".into(),
                    code: "RESULT_UNKNOWN".into(),
                    reason: format!(
                        "等待结果超时，当前操作结果尚未确认。\n任务：{}\n\n请发送 /pjsk 更新状态 查询，勿重复操作。",
                        job["id"].as_str().unwrap_or("未知")
                    ),
                    http: None,
                    upstream: None,
                    reply_policy: None,
                    during: None,
                    cause: None,
                    report: None,
                });
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
            let id = job["id"].as_str().ok_or_else(Fault::unavailable)?;
            match c
                .call(
                    msg,
                    reqwest::Method::GET,
                    &format!("jobs/{id}"),
                    Value::Null,
                )
                .await
            {
                Ok(v) => job = v,
                Err(f) => {
                    if f.code != "SERVICE_UNAVAILABLE" {
                        return Err(f);
                    }
                }
            }
        }
    }
    pub(super) async fn update(&self, c: &Client, msg: &Msg, input: &str) -> Result<(), Fault> {
        let text = input.trim();
        let words: Vec<&str> = text.split_whitespace().collect();
        let first = words.first().copied().unwrap_or("");
        match first {
            "更新数据" => {
                let selector = words.get(1).copied().unwrap_or("");
                let j = c
                    .call(
                        msg,
                        reqwest::Method::POST,
                        "sync-jobs",
                        json!({"selector":selector}),
                    )
                    .await?;
                let done = self.wait_job(c, msg, j).await?;
                reply(msg, Self::job_result(&done, false)?).await;
            }
            "更新状态" => {
                let j = c
                    .call(msg, reqwest::Method::GET, "jobs/latest", Value::Null)
                    .await?;
                reply(msg, Self::job_result(&j, false)?).await;
            }
            "重传数据" => {
                let j = c
                    .call(msg, reqwest::Method::GET, "jobs/latest", Value::Null)
                    .await?;
                let id = j["id"].as_str().ok_or_else(Fault::unavailable)?;
                let j = c
                    .call(
                        msg,
                        reqwest::Method::POST,
                        &format!("jobs/{id}/retry-upload"),
                        json!({}),
                    )
                    .await?;
                let done = self.wait_job(c, msg, j).await?;
                reply(msg, Self::job_result(&done, false)?).await;
            }
            _ => {}
        }
        Ok(())
    }
}
