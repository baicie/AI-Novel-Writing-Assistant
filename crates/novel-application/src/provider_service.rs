//! P16.2B：Provider 应用服务。
//!
//! 职责边界：
//! - 这里是唯一的业务编排层：CAS 校验、凭据三态、删除保护、发现确认都在本层做；
//! - 存储与密钥库通过端口注入（`ProviderRegistry` / `CredentialVault`），本层不碰 SQL/Keychain；
//! - 返回值一律脱敏，任何 DTO 都不含明文密钥；
//! - 终态错误用稳定机器码表达，前端按码提示。

mod error;
mod model_route;
mod service;
#[cfg(test)]
mod tests;

pub use error::*;
pub use model_route::*;
