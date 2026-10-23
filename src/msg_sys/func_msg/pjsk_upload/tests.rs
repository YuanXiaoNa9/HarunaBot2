use super::*;
#[tokio::test]
async fn help_visibility_uses_bot_and_group_allowlist() {
    let h = PjskUpload::default();
    assert!(
        h.client
            .set(Client {
                config: Config {
                    enabled: true,
                    bot_id: "1".into(),
                    service_url: "http://127.0.0.1:8080".into(),
                    api_key_file: String::new(),
                    allowed_groups: vec!["10".into()]
                },
                http: reqwest::Client::new(),
                api_key: "test".into()
            })
            .is_ok()
    );
    let mut msg = Msg {
        self_id: 1,
        group_id: 99,
        ..Default::default()
    };
    assert!(!h.help_visible(&msg).await);
    msg.group_id = 10;
    assert!(h.help_visible(&msg).await);
    msg.group_id = 0;
    assert!(h.help_visible(&msg).await);
    msg.self_id = 2;
    assert!(!h.help_visible(&msg).await);
}
#[tokio::test]
async fn web_binding_same_message_private_temp_chat_and_no_task_footer() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut paths = vec![];
        for _ in 0..4 {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            loop {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if request.windows(4).any(|x| x == b"\r\n\r\n") {
                    break;
                }
                assert!(request.len() < 8192);
            }
            let headers = String::from_utf8_lossy(&request).to_lowercase();
            assert!(headers.contains("x-scene: private"));
            assert!(headers.contains("x-group-id: 99"));
            let line = headers.lines().next().unwrap().to_owned();
            paths.push(line.clone());
            let data = if line.starts_with("get /v1/access/status ") {
                json!({"eligible":true})
            } else if line.starts_with("get /v1/terms ") {
                json!({"accepted":true})
            } else if line.starts_with("post /v1/web-login-attempts ") {
                json!({"id":"attempt-test","stage":"awaiting_web","expires":now()+60,"interval":2,"login_url":"https://accounts.taptap.cn/login","authorize_url":"https://accounts.taptap.cn/authorize?user_code=demo"})
            } else {
                assert!(line.starts_with("get /v1/web-login-attempts/attempt-test "));
                json!({"job":{"id":"internal-job-id","status":"success","deadline":now()+60,"items":[{"account_id":"1","game_id":"9000000000000000012","status":"success"}]}})
            };
            let body = json!({"data":data,"error":null}).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream.write_all(response.as_bytes()).await.unwrap();
        }
        paths
    });
    let client = Client {
        config: Config {
            enabled: true,
            bot_id: "1".into(),
            service_url: format!("http://{address}"),
            api_key_file: String::new(),
            allowed_groups: vec!["10".into()],
        },
        http: reqwest::Client::builder().no_proxy().build().unwrap(),
        api_key: "test".into(),
    };
    let h = PjskUpload::default();
    let msg = Msg {
        self_id: 1,
        user_id: 2,
        group_id: 99,
        message_type: "private".into(),
        message_id: 987655,
        raw_message: "绑定账号 TapTap".into(),
        ..Default::default()
    };
    assert!(command("账号绑定TapTap"));
    assert!(command("绑定账号 TapTap"));
    tokio::time::timeout(Duration::from_secs(10), h.run(&client, &msg))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(server.await.unwrap().len(), 4);
    let replies = TEST_REPLIES.lock().unwrap();
    let texts: Vec<_> = replies
        .iter()
        .filter(|(id, _)| *id == 987655)
        .map(|(_, t)| t.as_str())
        .collect();
    assert_eq!(texts.len(), 2);
    assert!(texts[0].contains("① 先登录"));
    assert!(texts[0].contains("网页可能持续加载"));
    assert!(texts[1].contains("绑定成功"));
    assert!(texts[1].contains("9000000000000000012"));
    assert!(!texts[1].contains("internal-job-id"));
    assert!(!h.pending.contains_key(&(1, 2)));
}

#[test]
fn only_exact_ascii_otp() {
    assert!(otp("123456"));
    for s in ["12345", "１２３４５６", "a123456", "1234567"] {
        assert!(!otp(s));
    }
}
#[test]
fn generation_cannot_remove_new_flow() {
    let h = PjskUpload::default();
    let k = (1, 2);
    h.pending.insert(
        k,
        Pending {
            attempt: "new".into(),
            provider: "mkcn".into(),
            stage: Stage::AwaitingCode,
            expires: now() + 300,
            generation: 2,
        },
    );
    h.remove_generation(k, 1);
    assert_eq!(h.pending.get(&k).unwrap().attempt, "new");
    h.remove_generation(k, 2);
    assert!(!h.pending.contains_key(&k));
}
#[tokio::test]
async fn numeric_messages_need_private_pending_flow() {
    let h = PjskUpload::default();
    let mut msg = Msg {
        post_type: "message".into(),
        self_id: 1,
        user_id: 2,
        raw_message: "123456".into(),
        ..Default::default()
    };
    assert!(!h.matches(&msg).await);
    h.pending.insert(
        (1, 2),
        Pending {
            attempt: "a".into(),
            provider: "mkcn".into(),
            stage: Stage::AwaitingCode,
            expires: now() + 300,
            generation: 1,
        },
    );
    assert!(h.matches(&msg).await);
    msg.group_id = 3;
    assert!(!h.matches(&msg).await);
}
#[tokio::test(start_paused = true)]
async fn slow_notice_once_then_same_future_returns_final_result() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let completed = Arc::new(AtomicBool::new(false));
    let server_complete = completed.clone();
    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let done = server_complete.load(Ordering::Relaxed);
            tokio::spawn(async move {
                let mut buf = [0; 4096];
                let _ = stream.read(&mut buf).await;
                let body=json!({"data":{"id":"slow-job","status":if done{"success"}else{"running"},"deadline":now()+300},"error":null}).to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            });
        }
    });
    let client = Client {
        config: Config {
            enabled: true,
            bot_id: "1".into(),
            service_url: format!("http://{address}"),
            api_key_file: String::new(),
            allowed_groups: vec![],
        },
        http: reqwest::Client::builder().no_proxy().build().unwrap(),
        api_key: "test".into(),
    };
    let h = PjskUpload::default();
    let msg = Msg {
        self_id: 1,
        user_id: 2,
        message_id: 987654,
        ..Default::default()
    };
    let deadline = now() + 300;
    let operation = tokio::spawn(async move {
        h.wait_job(
            &client,
            &msg,
            json!({"id":"slow-job","status":"running","deadline":deadline}),
        )
        .await
    });
    for _ in 0..200 {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_secs(61)).await;
    for _ in 0..2000 {
        tokio::task::yield_now().await;
        if TEST_REPLIES
            .lock()
            .unwrap()
            .iter()
            .any(|(id, _)| *id == 987654)
        {
            break;
        }
    }
    assert!(
        !operation.is_finished(),
        "handler returned after its slow notice"
    );
    assert_eq!(
        TEST_REPLIES
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == 987654)
            .count(),
        1
    );
    completed.store(true, Ordering::Relaxed);
    tokio::time::advance(Duration::from_secs(10)).await;
    let result = operation.await.unwrap().unwrap();
    assert_eq!(result["status"], "success");
    assert_eq!(
        TEST_REPLIES
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, _)| *id == 987654)
            .count(),
        1
    );
    server.abort();
}
async fn scripted_client(
    script: Vec<(&'static str, Value)>,
) -> (Client, tokio::task::JoinHandle<Vec<String>>) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut captured = Vec::new();
        for (expected, data) in script {
            let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut request = Vec::new();
            let end;
            loop {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
                if let Some(i) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                    end = i + 4;
                    break;
                }
                assert!(request.len() < 16384);
            }
            let header = String::from_utf8_lossy(&request[..end]).to_lowercase();
            assert!(
                header.starts_with(expected),
                "unexpected request: {}",
                header.lines().next().unwrap()
            );
            let len = header
                .lines()
                .find_map(|s| {
                    s.strip_prefix("content-length:")
                        .and_then(|s| s.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while request.len() < end + len {
                let mut chunk = [0; 1024];
                let n = stream.read(&mut chunk).await.unwrap();
                assert!(n > 0);
                request.extend_from_slice(&chunk[..n]);
            }
            captured.push(String::from_utf8_lossy(&request).into_owned());
            let body = json!({"data":data,"error":null}).to_string();
            stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).as_bytes()).await.unwrap();
        }
        captured
    });
    (
        Client {
            config: Config {
                enabled: true,
                bot_id: "1".into(),
                service_url: format!("http://{address}"),
                api_key_file: String::new(),
                allowed_groups: vec!["10".into()],
            },
            http: reqwest::Client::builder().no_proxy().build().unwrap(),
            api_key: "test".into(),
        },
        server,
    )
}
fn message(id: i64, text: &str) -> Msg {
    Msg {
        self_id: 1,
        user_id: 2,
        message_type: "private".into(),
        post_type: "message".into(),
        message_id: id,
        raw_message: text.into(),
        ..Default::default()
    }
}
#[tokio::test]
async fn hierarchy_uses_sub_help_and_lists_each_instruction() {
    let h = PjskUpload::default();
    let root = h.help("PJSK数据上传").await;
    for name in ["绑定账号", "终止当前绑定", "解除绑定", "使用条款"] {
        assert!(root.contains(name));
    }
    assert!(!root.contains("上传帮助"));
    assert!(root.contains("/help <主功能>-<子功能>"));
    let detail = h.help("PJSK数据上传-绑定账号-解除绑定").await;
    assert!(detail.contains("<绑定账号>"));
    assert!(detail.contains("<解除绑定>"));
    assert!(detail.contains("确认解除绑定 2"));
    let all = h.help("PJSK数据上传-all").await;
    assert!(all.contains("<使用条款>"));
    assert!(h.help("PJSK数据上传-不存在").await.contains("未找到子功能"));
}
#[test]
fn normalized_commands_and_clear_account_cards() {
    for text in [
        "绑定账号 taptap",
        "绑定账号 TAPTAP",
        "绑定账号 TaPtAp",
        "账号绑定TAPTAP",
        "绑定账号\u{3000}TaPtAp",
    ] {
        assert!(command(text));
        assert!(format_error(&text.split_whitespace().collect::<Vec<_>>()).is_none());
    }
    for text in [
        "绑定账号",
        "绑定账号 TapTap 多余参数",
        "解除绑定",
        "解除绑定 abc",
        "更新数据 1 多余参数",
    ] {
        assert!(format_error(&text.split_whitespace().collect::<Vec<_>>()).is_some());
    }
    for text in ["删除账号 1", "取消绑定", "上传帮助"] {
        assert!(!command(text));
    }
    let card = account_block(
        &json!({"id":"2","nickname":"小李\n管理员\u{0001}","game_id":"9000000000000000001","provider":"mkcn_taptap","default":true,"remark":"日常\r使用"}),
    );
    assert!(card.starts_with("编号：2（默认）\n游戏昵称：小李 管理员\nUID："));
    assert!(card.contains("登录方式：TapTap\n备注：日常 使用"));
    assert!(!card.contains('\u{0001}'));
    assert!(account_block(&json!({"id":"1","provider":"mkcn"})).contains("游戏昵称：暂未获取"));
    let latest = PjskState::job_text(
        &json!({"kind":"bind","status":"success","id":"internal","items":[{"account_id":"2","nickname":"小李","game_id":"123","provider":"mkcn_taptap"}]}),
        false,
    );
    assert!(latest.starts_with("绑定成功"));
    assert!(!latest.contains("internal"));
}
#[tokio::test]
async fn consent_resumes_phone_binding_in_second_message_and_never_treats_digits_as_consent() {
    let (c, server) = scripted_client(vec![
        ("get /v1/access/status ", json!({"eligible":true})),
        (
            "get /v1/terms ",
            json!({"version":"v1","text":"测试条款","accepted":false}),
        ),
        ("get /v1/access/status ", json!({"eligible":true})),
        ("post /v1/consents ", json!({"version":"v1"})),
        ("get /v1/access/status ", json!({"eligible":true})),
        (
            "post /v1/login-attempts ",
            json!({"id":"sms-new","expires":now()+300}),
        ),
    ])
    .await;
    let h = PjskUpload::default();
    h.run(&c, &message(990001, "绑定账号 13000000000"))
        .await
        .unwrap();
    assert_eq!(
        h.pending.get(&(1, 2)).unwrap().stage,
        Stage::AwaitingConsent
    );
    assert!(!h.matches(&message(990002, "123456")).await);
    let mut other = message(990003, "同意条款");
    other.user_id = 3;
    h.accept_consent(&c, &other).await.unwrap();
    assert!(h.consents.contains_key(&(1, 2)));
    h.run(&c, &message(990004, "同意条款")).await.unwrap();
    assert_eq!(h.pending.get(&(1, 2)).unwrap().stage, Stage::AwaitingCode);
    assert!(!h.consents.contains_key(&(1, 2)));
    let requests = server.await.unwrap();
    assert!(requests[5].contains("13000000000"));
    assert!(
        !requests[..5]
            .iter()
            .any(|r| r.starts_with("POST /v1/login-attempts"))
    );
    let replies = TEST_REPLIES.lock().unwrap();
    assert!(
        replies
            .iter()
            .any(|(id, t)| *id == 990001 && t.contains("同意条款"))
    );
    assert!(
        replies
            .iter()
            .any(|(id, t)| *id == 990004 && t.contains("短信验证码已发送"))
    );
}
#[tokio::test]
async fn terminating_or_expiring_consent_never_starts_login() {
    let (c, server) = scripted_client(vec![
        ("get /v1/access/status ", json!({"eligible":true})),
        (
            "get /v1/terms ",
            json!({"version":"v1","text":"测试条款","accepted":false}),
        ),
        ("get /v1/access/status ", json!({"eligible":true})),
    ])
    .await;
    let h = PjskUpload::default();
    h.run(&c, &message(990010, "绑定账号 taptap"))
        .await
        .unwrap();
    let mut group = message(990011, "终止当前绑定");
    group.message_type = "group".into();
    group.group_id = 99;
    h.run(&c, &group).await.unwrap();
    assert!(h.consents.contains_key(&(1, 2)));
    h.run(&c, &message(990012, "终止当前绑定")).await.unwrap();
    h.accept_consent(&c, &message(990013, "同意条款"))
        .await
        .unwrap();
    assert!(h.pending.is_empty());
    assert!(h.consents.is_empty());
    assert_eq!(server.await.unwrap().len(), 3);
    h.consents.insert(
        (1, 2),
        ConsentIntent {
            command: "绑定账号 taptap".into(),
            version: "v1".into(),
            expires: now() - 1,
            generation: 100,
        },
    );
    h.accept_consent(&c, &message(990014, "同意条款"))
        .await
        .unwrap();
    assert!(h.pending.is_empty());
}
#[tokio::test]
async fn unbind_confirms_the_displayed_account_revision() {
    let account = json!({"id":"2","nickname":"小李","game_id":"123","provider":"mkcn_taptap","revision":"confirmed-revision"});
    let (c, server) = scripted_client(vec![
        ("get /v1/access/status ", json!({"eligible":true})),
        ("get /v1/accounts ", json!([account])),
        ("get /v1/access/status ", json!({"eligible":true})),
        ("delete /v1/accounts/2 ", json!({"deleted":true})),
    ])
    .await;
    let h = PjskUpload::default();
    h.run(&c, &message(990020, "解除绑定 2")).await.unwrap();
    h.run(&c, &message(990021, "确认解除绑定 2")).await.unwrap();
    let requests = server.await.unwrap();
    assert!(
        requests[3]
            .to_lowercase()
            .contains("if-match: confirmed-revision")
    );
    let replies = TEST_REPLIES.lock().unwrap();
    let text = &replies.iter().find(|(id, _)| *id == 990020).unwrap().1;
    assert!(text.contains("游戏昵称：小李\nUID：123"));
    assert!(text.contains("确认解除绑定 2"));
}

#[test]
fn failed_job_is_explained_once_with_semantic_steps() {
    let fault = json!({"stage":"suite_fetch","code":"PROFILE_OUTDATED","reason":"同步版本后仍被拒绝","http":426,"during":"suite_retry"});
    let job = json!({"id":"diagnosis-task","version":"0.5.0","status":"failed","error":fault,"items":[{"account_id":"1","game_id":"123","nickname":"小李","provider":"mkcn","status":"failed","error":fault}]});
    let err = PjskState::job_result(&job, false).unwrap_err();
    let text = err.to_string();
    assert_eq!(text.matches("诊断代码：PROFILE_OUTDATED").count(), 1);
    assert!(text.contains("失败步骤：获取 Suite 数据"));
    assert!(text.contains("版本同步后的 Suite 再次请求"));
    assert!(text.contains("诊断任务：diagnosis-task"));
    let cause:Fault=serde_json::from_value(json!({"stage":"game_login","code":"PROFILE_OUTDATED","reason":"登录接口拒绝旧版本","http":426,"during":"version_refresh","cause":{"stage":"suite_fetch","code":"PROFILE_OUTDATED","reason":"数据版本过期","http":426}})).unwrap();
    let text = cause.to_string();
    assert!(text.contains("失败步骤：游戏登录"));
    assert!(text.contains("最初失败步骤：获取 Suite 数据"));
}
#[tokio::test]
async fn registered_subfunction_bubbles_safe_error_to_existing_framework() {
    let fault = json!({"stage":"suite_fetch","code":"PROFILE_OUTDATED","reason":"同步后仍被拒绝","http":426});
    let(c,server)=scripted_client(vec![
        ("get /v1/access/status ",json!({"eligible":true})),
        ("get /v1/accounts ",json!([{"id":"1","nickname":"小李","game_id":"123"}])),
        ("get /v1/terms ",json!({"accepted":true})),
        ("post /v1/sync-jobs ",json!({"id":"failed-task","status":"failed","error":fault,"deadline":now()+60,"events":[{"sequence":1,"account_id":"1","code":"VERSION_SYNCED","message":"已同步服务器版本和本地缓存，正在获取游戏数据。"}],"items":[{"account_id":"1","status":"failed","error":fault}]})),
    ]).await;
    let h = PjskUpload::default();
    assert!(h.client.set(c).is_ok());
    let msg = message(990031, "更新数据");
    let err = h.process(&msg).await.unwrap_err();
    assert!(
        format!("{err:#}").contains("失败步骤：获取 Suite 数据"),
        "safe error did not reach framework: {err:#}"
    );
    {
        let texts = TEST_REPLIES.lock().unwrap();
        let texts: Vec<_> = texts.iter().filter(|(id, _)| *id == 990031).collect();
        assert_eq!(
            texts.len(),
            1,
            "module sent a duplicate final error instead of returning it"
        );
        assert!(texts[0].1.contains("已同步服务器版本"));
    }
    assert_eq!(server.await.unwrap().len(), 4);
}
#[tokio::test]
async fn progress_sequence_is_not_resent_on_each_job_poll() {
    let event = json!({"sequence":1,"account_id":"1","message":"检测到版本更新，已同步本地。"});
    let (c, server) = scripted_client(vec![
        (
            "get /v1/jobs/progress-job ",
            json!({"id":"progress-job","status":"running","deadline":now()+60,"events":[event]}),
        ),
        (
            "get /v1/jobs/progress-job ",
            json!({"id":"progress-job","status":"success","deadline":now()+60,"events":[event]}),
        ),
    ])
    .await;
    let h = PjskUpload::default();
    h.wait_job(
        &c,
        &message(990032, "更新数据"),
        json!({"id":"progress-job","status":"running","deadline":now()+60,"events":[event]}),
    )
    .await
    .unwrap();
    assert_eq!(server.await.unwrap().len(), 2);
    let replies = TEST_REPLIES.lock().unwrap();
    assert_eq!(replies.iter().filter(|(id, _)| *id == 990032).count(), 1);
}
