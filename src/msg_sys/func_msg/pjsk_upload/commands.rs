use super::*;
pub(super) fn number(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

pub(super) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(super) fn otp(s: &str) -> bool {
    s.len() == 6 && s.bytes().all(|b| b.is_ascii_digit())
}

pub(super) fn private(msg: &Msg) -> bool {
    msg.message_type == "private" || (msg.message_type.is_empty() && msg.group_id == 0)
}

pub(super) fn key(msg: &Msg) -> (i64, i64) {
    (msg.self_id, msg.user_id)
}

pub(super) fn command(s: &str) -> bool {
    matches!(
        s.split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "账号绑定taptap"
            | "绑定taptap"
            | "申请绑定"
            | "绑定账号"
            | "重新授权"
            | "终止当前绑定"
            | "账号列表"
            | "更新数据"
            | "默认账号"
            | "账号备注"
            | "解除绑定"
            | "确认解除绑定"
            | "更新状态"
            | "重传数据"
            | "同意条款"
    )
}

pub(super) fn format_error(words: &[&str]) -> Option<String> {
    let first = words.first().copied().unwrap_or("");
    let phone = |s: &str| s.len() == 11 && s.starts_with('1') && number(s);
    let (valid, usage) = match first {
        "绑定账号" => (
            words.len() == 2 && (phone(words[1]) || words[1].eq_ignore_ascii_case("taptap")),
            "绑定账号 手机号\n或：绑定账号 TapTap",
        ),
        "重新授权" => (
            words.len() == 3
                && number(words[1])
                && (phone(words[2]) || words[2].eq_ignore_ascii_case("taptap")),
            "重新授权 编号 手机号\n或：重新授权 编号 TapTap",
        ),
        "解除绑定" | "确认解除绑定" | "默认账号" => (
            words.len() == 2 && number(words[1]),
            "指令后填写一个账号编号；先发送 账号列表 查看",
        ),
        "账号备注" => (
            words.len() >= 2 && number(words[1]),
            "账号备注 编号 备注内容",
        ),
        "更新数据" => (
            words.len() == 1
                || (words.len() == 2 && (number(words[1]) || matches!(words[1], "全部" | "all"))),
            "更新数据\n或：更新数据 编号\n或：更新数据 全部",
        ),
        "申请绑定" | "账号列表" | "更新状态" | "重传数据" | "同意条款" | "终止当前绑定" => {
            (words.len() == 1, "本指令无需填写其他内容")
        }
        _ if first.eq_ignore_ascii_case("账号绑定taptap")
            || first.eq_ignore_ascii_case("绑定taptap") =>
        {
            (words.len() == 1, "绑定账号 TapTap")
        }
        _ => return None,
    };
    if valid {
        None
    } else {
        Some(format!(
            "指令格式不正确\n\n{usage}\n\n详细说明：/help PJSK数据上传"
        ))
    }
}
