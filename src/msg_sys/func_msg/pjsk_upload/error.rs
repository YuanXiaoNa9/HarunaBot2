use super::*;

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub(super) struct Fault {
    pub(super) stage: String,
    pub(super) code: String,
    pub(super) reason: String,
    #[serde(default)]
    pub(super) http: Option<u16>,
    #[serde(default)]
    pub(super) upstream: Option<String>,
    #[serde(default)]
    pub(super) reply_policy: Option<String>,
    #[serde(default)]
    pub(super) during: Option<String>,
    #[serde(default)]
    pub(super) cause: Option<Box<Fault>>,
    #[serde(skip)]
    pub(super) report: Option<String>,
}
pub(super) fn stage_name(stage: &str) -> &str {
    match stage {
        "game_auth" => "游戏身份认证",
        "game_login" => "游戏登录",
        "game_system" => "读取游戏系统信息",
        "suite_fetch" | "suite" | "fetch" => "获取 Suite 数据",
        "version_refresh" | "version_cache" => "同步并保存游戏版本",
        "suite_retry" => "版本同步后的 Suite 再次请求",
        "upload" => "向 Sakura 上传数据",
        "queue" => "任务排队",
        "consent" => "确认使用条款",
        "verify" => "验证短信验证码",
        "send_sms" => "发送短信验证码",
        "taptap" => "TapTap 网页授权",
        "gsdk" => "换取游戏登录凭证",
        "profile" => "核对游戏昵称与 UID",
        "bind" => "保存账号绑定",
        "storage" => "保存本地数据",
        "service" | "wait" => "等待服务处理结果",
        "access" => "校验使用资格",
        "input" | "validation" => "检查指令参数",
        "restart" => "恢复中断任务",
        p if p.starts_with("/api/suite/") => "获取 Suite 数据",
        p if p == "/api/user/auth" => "游戏身份认证",
        p if p.starts_with("/api/user/") && p.ends_with("/login") => "游戏登录",
        "/api/system" => "读取游戏系统信息",
        "/passport/mobile/send_code/v1/" => "发送短信验证码",
        "/passport/mobile/sms_login/" => "验证短信验证码",
        "/passport/auth/login/" => "交换 TapTap 授权",
        "/sdk/account/v3/login/login" => "平台登录换票",
        "/sdk/account/v3/login/auto_login" => "续用平台登录凭证",
        "/gsdk/account/login" => "换取游戏登录凭证",
        _ => "处理当前操作",
    }
}
impl Fault {
    pub(super) fn invalid(reason: &str) -> Self {
        Self {
            stage: "validation".into(),
            code: "INVALID_COMMAND".into(),
            reason: reason.into(),
            http: None,
            upstream: None,
            reply_policy: None,
            during: None,
            cause: None,
            report: None,
        }
    }
    pub(super) fn unavailable() -> Self {
        Self {
            stage: "service".into(),
            code: "SERVICE_UNAVAILABLE".into(),
            reason: "服务暂时不可用，当前操作结果尚未确认".into(),
            ..Self::invalid("")
        }
    }
    pub(super) fn display(&self) -> String {
        self.display_for_account(None)
    }
    pub(super) fn needs_reauthorization(&self) -> bool {
        matches!(self.code.as_str(), "REAUTH_REQUIRED" | "SESSION_EXPIRED")
            || self
                .cause
                .as_ref()
                .is_some_and(|cause| cause.needs_reauthorization())
    }
    pub(super) fn display_for_account(&self, account: Option<&str>) -> String {
        if let Some(report) = &self.report {
            return report.clone();
        }
        let mut blocks = vec![format!(
            "失败步骤：{}\n原因：{}",
            stage_name(&self.stage),
            self.reason
        )];
        if let Some(during) = &self.during {
            blocks.push(format!("恢复环节：{}", stage_name(during)));
        }
        if let Some(cause) = &self.cause {
            blocks.push(format!(
                "最初失败步骤：{}\n最初原因：{}{}",
                stage_name(&cause.stage),
                cause.reason,
                cause
                    .http
                    .map(|h| format!("（HTTP {h}）"))
                    .unwrap_or_default()
            ));
        }
        let account = account.filter(|s| number(s)).unwrap_or("编号");
        let advice = if self.needs_reauthorization() {
            format!(
                "登录凭证已失效，需要重新授权。请私聊 Bot 或使用群临时会话操作：\n\n手机号账号：\n/pjsk 重新授权 {account} 手机号\n收到短信后，直接回复六位验证码。\n\nTapTap 账号：\n/pjsk 重新授权 {account} TapTap\n按 Bot 提供的链接完成登录和授权。\n\n请使用原游戏账号；授权成功后再发送：\n/pjsk 更新数据 {account}\n\n查看编号和登录方式：/pjsk 账号列表"
            )
        } else {
            match self.code.as_str() {
            "PROFILE_OUTDATED" | "CLIENT_UPDATE_REQUIRED" | "VERSION_DISCOVERY_FAILED" => {
                "请维护者检查游戏版本和配套 app-hash 配置，无需反复绑定或申请验证码。"
            }
            "CONSENT_REQUIRED" | "TERMS_CHANGED" => {
                "请私聊重新发送绑定或更新指令，阅读并同意当前条款。"
            }
            "NO_ACCOUNT" | "NO_TARGET" => {
                "查看账号：/pjsk 账号列表\n绑定说明：/help PJSK数据上传-绑定账号"
            }
            "WRONG_CODE" => "请核对短信验证码后重新发送；不要连续申请短信。",
            "SERVICE_UNAVAILABLE" | "RESULT_UNKNOWN" | "UPLOAD_OUTCOME_UNKNOWN" => {
                "结果可能尚未确认，请稍后发送 /pjsk 更新状态，勿连续重试。"
            }
            "AUTH_EXPIRED" | "AUTH_REJECTED" if self.stage == "taptap" =>
                "本次网页授权已过期或未完成。请重新发送原来的 /pjsk 绑定账号 TapTap 或 /pjsk 重新授权 编号 TapTap，获取新链接；不要继续点击旧链接。",
            "INVALID_COMMAND" => "详细帮助：/help PJSK数据上传",
            _ if self.stage == "upload" => {
                "游戏数据已获取，但上传未完成。\n先查看结果：/pjsk 更新状态\n确认上传失败且缓存仍有效后：/pjsk 重传数据\n缓存失效时：/pjsk 更新数据"
            }
            _ => "需要帮助：/help PJSK数据上传；请将以下诊断信息提供给维护者。",
        }.into()
        };
        blocks.push(advice);
        blocks.push(format!(
            "诊断代码：{}{}{}",
            self.code,
            self.http
                .map(|h| format!("\nHTTP：{h}"))
                .unwrap_or_default(),
            self.upstream
                .as_ref()
                .map(|s| format!("\n上游码：{s}"))
                .unwrap_or_default()
        ));
        blocks.join("\n\n")
    }
}
impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display())
    }
}
impl std::error::Error for Fault {}
