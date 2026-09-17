use serde::Deserialize;

/// Response from `POST https://github.com/login/device/code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in_secs: u64,
    /// Minimum polling interval GitHub requires.
    pub interval_secs: u64,
}

/// One tick of `POST https://github.com/login/oauth/access_token` while a
/// device flow is outstanding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenPoll {
    Authorized {
        access_token: String,
        scopes: Vec<String>,
    },
    /// User has not finished yet; wait and poll again.
    Pending {
        /// Seconds to wait before the next poll (raised on slow_down).
        retry_after_secs: u64,
    },
    /// The device code expired before the user completed authorization.
    Expired,
    /// The user denied the request or GitHub rejected the client.
    Denied { message: String },
}

#[derive(Debug, Deserialize)]
struct RawDeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default = "default_expires")]
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_expires() -> u64 {
    900
}

fn default_interval() -> u64 {
    5
}

#[derive(Debug, Deserialize)]
struct RawTokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

pub fn parse_device_code(json: &str) -> Result<DeviceCodeResponse, String> {
    let raw: RawDeviceCode = serde_json::from_str(json)
        .map_err(|error| format!("malformed device code response: {error}"))?;
    if raw.device_code.is_empty() || raw.user_code.is_empty() || raw.verification_uri.is_empty() {
        return Err("device code response is missing required fields".into());
    }
    Ok(DeviceCodeResponse {
        device_code: raw.device_code,
        user_code: raw.user_code,
        verification_uri: raw.verification_uri,
        expires_in_secs: raw.expires_in,
        interval_secs: raw.interval.max(1),
    })
}

pub fn parse_token_poll(json: &str) -> Result<TokenPoll, String> {
    let raw: RawTokenResponse =
        serde_json::from_str(json).map_err(|error| format!("malformed token response: {error}"))?;
    if let Some(token) = raw.access_token {
        return Ok(TokenPoll::Authorized {
            access_token: token,
            // GitHub echoes granted scopes as a comma list ("repo,read:user").
            scopes: raw
                .scope
                .unwrap_or_default()
                .split([',', ' '])
                .map(str::trim)
                .filter(|scope| !scope.is_empty())
                .map(str::to_owned)
                .collect(),
        });
    }
    match raw.error.as_deref() {
        Some("authorization_pending") => Ok(TokenPoll::Pending {
            retry_after_secs: default_interval(),
        }),
        Some("slow_down") => Ok(TokenPoll::Pending {
            // RFC 8628 slow_down: add 5 seconds to the previous interval.
            retry_after_secs: default_interval() + 5,
        }),
        Some("expired_token") => Ok(TokenPoll::Expired),
        Some(other) => Ok(TokenPoll::Denied {
            message: raw.error_description.unwrap_or_else(|| other.to_owned()),
        }),
        None => Err("token response carried neither token nor error".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_code_response() {
        let parsed = parse_device_code(
            r#"{"device_code":"d1","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","expires_in":900,"interval":5}"#,
        )
        .unwrap();
        assert_eq!(parsed.device_code, "d1");
        assert_eq!(parsed.user_code, "ABCD-1234");
        assert_eq!(parsed.interval_secs, 5);
        assert_eq!(parsed.expires_in_secs, 900);
    }

    #[test]
    fn applies_defaults_for_missing_timing_fields() {
        let parsed = parse_device_code(
            r#"{"device_code":"d1","user_code":"AB12","verification_uri":"https://github.com/login/device"}"#,
        )
        .unwrap();
        assert_eq!(parsed.expires_in_secs, 900);
        assert_eq!(parsed.interval_secs, 5);
    }

    #[test]
    fn rejects_device_code_response_without_fields() {
        assert!(parse_device_code(r#"{"error":"bad"}"#).is_err());
    }

    #[test]
    fn parses_success_with_scope_list() {
        let parsed = parse_token_poll(
            r#"{"access_token":"tok","token_type":"bearer","scope":"repo,read:user,workflow"}"#,
        )
        .unwrap();
        match parsed {
            TokenPoll::Authorized {
                access_token,
                scopes,
            } => {
                assert_eq!(access_token, "tok");
                assert_eq!(
                    scopes,
                    vec![
                        "repo".to_owned(),
                        "read:user".to_owned(),
                        "workflow".to_owned()
                    ]
                );
            }
            other => panic!("expected authorized, got {other:?}"),
        }
    }

    #[test]
    fn maps_pending_and_slow_down() {
        let pending = parse_token_poll(r#"{"error":"authorization_pending"}"#).unwrap();
        assert_eq!(
            pending,
            TokenPoll::Pending {
                retry_after_secs: 5
            }
        );

        let slowed = parse_token_poll(r#"{"error":"slow_down"}"#).unwrap();
        assert_eq!(
            slowed,
            TokenPoll::Pending {
                retry_after_secs: 10
            }
        );
    }

    #[test]
    fn maps_expiry_denial_and_garbage() {
        assert_eq!(
            parse_token_poll(r#"{"error":"expired_token"}"#).unwrap(),
            TokenPoll::Expired
        );

        let denied = parse_token_poll(
            r#"{"error":"access_denied","error_description":"The user has denied your application access."}"#,
        )
        .unwrap();
        assert!(matches!(denied, TokenPoll::Denied { .. }));

        assert!(parse_token_poll(r#"{}"#).is_err());
    }
}
