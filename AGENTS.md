# Repository instructions

开发前阅读 [docs/development-conventions.md](docs/development-conventions.md)。本仓库的现有框架与项目所有者明确要求优先。

- 保持 msg_analysis → FnHandler 生命周期；父模块薄路由，系列业务按子模块划分并使用 sub_match_process。
- 帮助走 /help、FnHandler.help 和 sub_help；功能帮助与业务共用子处理器注册表。
- 跨消息用模块自己的共享状态，按 (BotQQ,UserQQ) 隔离，守卫不跨 await，处理代次、过期和终止。
- 当前消息内等待最终结果，不创建脱离消息的 QQ 结果推送任务。
- 正常结果和过程提醒用消息接口；安全错误向上返回，由框架统一日志与回复，不重复发送错误。
- message_type=private 包括群临时会话；不得因为来源群号非零而向群里回复私密操作。
- 使用易读的动作阶段、原因、下一步和必要诊断；不给用户原始凭证或网络异常。
- PJSK 协议、持久化与上传留在独立 Go 服务，Rust 只做交互，不把 Go 源码搬入此仓库。
- 不提交 HAR、真实配置、设备上下文、凭证、个人数据、数据库与密钥。不擅改原始消息日志策略。
- 按变更执行相关测试与定向格式化。SQLx 使用临时本地库；未经批准不写入或重启远程服务器。
