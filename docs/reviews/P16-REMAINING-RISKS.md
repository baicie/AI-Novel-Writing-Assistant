# P16 剩余风险与未完成项

## 1. 原 P1（已关闭）：Director 运行链路按环节使用绑定模型

- 现状：`RunContract.step_routes` 在 Run 创建时冻结各环节路由；worker 的正文/审校/修复三个生成点按环节取路由，旧合同（无该表）与未绑定环节自动回落单一路由。
- 证据：`provider_service/tests.rs` 三条用例、`cargo test -p novel-application --lib` 186 passed、workspace 全量 exit 0。
- 残留观察点：新增环节（例如将来加入的图像任务）需要在 `task_type_for_step` 里显式登记，否则按 writer 处理；这条已写进代码注释。

## 2. 安全取舍（已记录，需产品确认）

1. **明文 HTTP**：授权绑定到主机（换主机需再确认），但允许指向内网/公网明文地址。理由：自建网关现实需求；元数据与链路本地地址始终拒绝。
2. **本地密钥文件**：钥匙库不可靠时，密钥用 ChaCha20-Poly1305 加密，主密钥以 0600 文件为唯一权威（钥匙库仅副本）。它防的是意外暴露、其他本机用户与备份外泄，**不防**拿到本机文件系统的攻击者。要提升到「钥匙库为权威」，需要先解决本地 ad-hoc 签名下钥匙库不可读的环境问题。
3. **Legacy 与注册表各写一份密钥**：`save_model_setup` 会同时写 `openai-compatible` 与注册表 `default` 两个槽位，属兼容期冗余存储；清理需先确认没有旧版本回读路径。

## 3. 测试覆盖边界

- 所有 Provider 发现/生成测试用本地 mock server，**不能**作为「某厂商线上可用」的证明；真实厂商连通性需用户在本机验证。
- Windows / Linux 桌面包与 E2E 当前环境无法执行，属独立待验收门禁，不声称通过。
- 远端 CI 未运行（禁止 push），本地已执行等价命令。

## 4. 受保护文件的过程违规

`crates/novel-http/tests/integration_runs.rs` 在 `7b89c0fd` 被提交入库。本轮未再修改、未删除、未做破坏性 Git 操作。
建议由用户决定：保留跟踪、或单独一次提交把它移出版本库（`git rm --cached`）并加入忽略规则。

## 5. 依赖变更

新增 `chacha20poly1305 0.10`（RustCrypto，纯 Rust）。若团队不接受新增依赖，需要退回「0600 + 完整性校验」方案，届时密钥不再是加密存储。

## 6. 判定

代码层面：P0 与 P1 全清，P2 已记录。
对外状态：本次报告 **IMPLEMENTATION_COMPLETE_EXTERNAL_VALIDATION_PENDING** —— 实现与本地验收完成，远端 CI 因禁止 push 未运行。
