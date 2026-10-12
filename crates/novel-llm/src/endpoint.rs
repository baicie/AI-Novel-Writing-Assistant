use std::net::IpAddr;

use url::Url;

use crate::error::LlmError;

pub fn authorize_endpoint(endpoint: &str, allow_localhost_http: bool) -> Result<(), LlmError> {
    let url =
        Url::parse(endpoint).map_err(|_| LlmError::EndpointRejected("模型地址无法识别".into()))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err(LlmError::EndpointRejected("模型地址不能携带密钥".into()));
    }
    let host = url.host_str().unwrap_or_default().trim_matches(['[', ']']);
    let local = is_loopback(host);
    match url.scheme() {
        "https" if !is_blocked_host(host) => Ok(()),
        "http" if local && allow_localhost_http => Ok(()),
        "http" if local => Err(LlmError::EndpointRejected(
            "本地模型地址需要单独允许".into(),
        )),
        "https" => Err(LlmError::EndpointRejected(
            "不能把密钥发到内网或元数据地址".into(),
        )),
        _ => Err(LlmError::EndpointRejected("模型地址只接受 HTTPS".into())),
    }
}

fn is_loopback(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn is_blocked_host(host: &str) -> bool {
    if is_loopback(host) {
        return false;
    }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) => {
            let octets = ip.octets();
            ip.is_private()
                || ip.is_link_local()
                || ip.is_unspecified()
                || octets[0] == 169 && octets[1] == 254
                || octets[0] == 100 && (64..=127).contains(&octets[1])
        }
        Ok(IpAddr::V6(ip)) => {
            ip.is_unique_local() || ip.is_unicast_link_local() || ip.is_unspecified()
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_defaults_to_https_and_localhost_is_explicit() {
        assert!(authorize_endpoint("https://api.example.com/v1/chat/completions", false).is_ok());
        assert!(authorize_endpoint("http://127.0.0.1:11434/v1/chat/completions", false).is_err());
        assert!(authorize_endpoint("http://127.0.0.1:11434/v1/chat/completions", true).is_ok());
        assert!(authorize_endpoint("http://localhost:11434/api/chat", true).is_ok());
        assert!(authorize_endpoint("https://192.168.1.8/v1/chat/completions", true).is_err());
        assert!(authorize_endpoint("https://10.1.2.3/v1/chat/completions", false).is_err());
        assert!(authorize_endpoint("https://169.254.169.254/latest", false).is_err());
        assert!(authorize_endpoint("https://user:secret@api.example.com/v1", false).is_err());
        assert!(authorize_endpoint("file:///tmp/model", false).is_err());
    }
}
