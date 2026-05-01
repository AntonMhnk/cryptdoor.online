//! Wrap the user-provided proxy YAML with TUN-mode top-level config so mihomo
//! creates a utun device, hijacks DNS and routes everything through PROXY.

use anyhow::Result;
use serde_yaml::Value;

pub fn wrap_with_tun(user_yaml: &str) -> Result<String> {
    let mut root: Value = serde_yaml::from_str(user_yaml)?;
    let map = root
        .as_mapping_mut()
        .ok_or_else(|| anyhow::anyhow!("config root is not a mapping"))?;

    map.insert(
        Value::String("tun".into()),
        serde_yaml::from_str(
            r#"
enable: true
stack: system
dns-hijack:
  - any:53
  - tcp://any:53
auto-route: true
auto-redirect: true
auto-detect-interface: true
mtu: 9000
inet4-address:
  - 198.18.0.1/16
inet6-address:
  - "fdfe:dcba:9876::1/126"
"#,
        )?,
    );

    map.insert(Value::String("mode".into()), Value::String("rule".into()));
    map.insert(
        Value::String("log-level".into()),
        Value::String("info".into()),
    );
    map.insert(Value::String("ipv6".into()), Value::Bool(true));

    if let Some(dns_val) = map.get_mut(&Value::String("dns".into())) {
        if let Some(dns_map) = dns_val.as_mapping_mut() {
            dns_map.insert(
                Value::String("ipv6".into()),
                Value::Bool(true),
            );
        }
    }

    Ok(serde_yaml::to_string(&root)?)
}
