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
        let advice = match self.code.as_str() {
            "PROFILE_OUTDATED" | "CLIENT_UPDATE_REQUIRED" | "VERSION_DISCOVERY_FAILED" => {
                "请维护者检查游戏版本和配套 app-hash 配置，无需反复绑定或申请验证码。"
            }
            "CONSENT_REQUIRED" | "TERMS_CHANGED" => {
                "请私聊重新发送绑定或更新指令，阅读并同意当前条款。"
            }
            "NO_ACCOUNT" | "NO_TARGET" => {
                "查看账号：账号列表\n绑定说明：/help PJSK数据上传-绑定账号"
            }
            "WRONG_CODE" => "请核对短信验证码后重新发送；不要连续申请短信。",
            "SERVICE_UNAVAILABLE" | "RESULT_UNKNOWN" | "UPLOAD_OUTCOME_UNKNOWN" => {
                "结果可能尚未确认，请稍后发送 更新状态，勿连续重试。"
            }
            "REAUTH_REQUIRED" | "SESSION_EXPIRED" => {
                "请使用 重新授权 编号 手机号，TapTap 账号使用 重新授权 编号 TapTap。"
            }
            "INVALID_COMMAND" => "详细帮助：/help PJSK数据上传",
            _ if self.stage == "upload" => {
                "游戏数据已获取，但上传未完成；请先查询 更新状态，再决定是否重传数据。"
            }
            _ => "需要帮助：/help PJSK数据上传；请将以下诊断信息提供给维护者。",
        };
        blocks.push(advice.into());
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
