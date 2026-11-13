use super::*;
use serde_json::Value;

const DEVICE_ENDPOINT: &str = "https://auth.tidal.com/v1/oauth2/device_authorization";
const SCOPE: &str = "r_usr w_usr w_sub";

#[derive(Clone)]
pub struct DevicePrompt {
    pub user_code: String,
    pub verification_url: String,
    pub expires_at: u64,
}

#[derive(Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(rename_all = "camelCase")]
struct Grant {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default)]
    verification_uri_complete: String,
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}
fn default_interval() -> u64 {
    5
}

fn verification_url(raw: &str) -> Result<String> {
    ensure!(raw.len() <= 4096, "TIDAL verification link is too long");
    let raw = if raw.starts_with("https://") {
        raw.to_owned()
    } else {
        format!("https://{raw}")
    };
    let url = url::Url::parse(&raw)?;
    ensure!(
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && matches!(
                url.host_str(),
                Some("link.tidal.com" | "login.tidal.com" | "tidal.com" | "www.tidal.com")
            ),
        "TIDAL returned an untrusted verification link"
    );
    Ok(url.into())
}
impl Grant {
    fn prompt(&self) -> Result<DevicePrompt> {
        ensure!(
            !self.device_code.is_empty() && self.device_code.len() <= 4096,
            "TIDAL omitted the device transaction"
        );
        ensure!(
            !self.user_code.is_empty()
                && self.user_code.len() <= 64
                && self
                    .user_code
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "TIDAL returned an invalid display code"
        );
        ensure!(
            (5..=3600).contains(&self.expires_in),
            "TIDAL returned an invalid device expiry"
        );
        let base = verification_url(&self.verification_uri)?;
        let url = if self.verification_uri_complete.is_empty() {
            base
        } else {
            verification_url(&self.verification_uri_complete)?
        };
        Ok(DevicePrompt {
            user_code: self.user_code.clone(),
            verification_url: url,
            expires_at: now() + self.expires_in,
        })
    }
}

enum Poll {
    Pending,
    SlowDown,
    Retry,
    Approved(Value),
}
fn poll_response(status: u16, value: Value) -> Result<Poll> {
    if (200..300).contains(&status) {
        ensure!(
            value["access_token"]
                .as_str()
                .is_some_and(|s| !s.is_empty()),
            "TIDAL omitted the device access token"
        );
        return Ok(Poll::Approved(value));
    }
    if status == 429 {
        return Ok(Poll::SlowDown);
    }
    if status >= 500 {
        return Ok(Poll::Retry);
    }
    match value["error"].as_str().unwrap_or("") {
        "authorization_pending" => Ok(Poll::Pending),
        "slow_down" => Ok(Poll::SlowDown),
        "access_denied" | "authorization_declined" => bail!("TIDAL device approval was declined"),
        "expired_token" => bail!("The TIDAL sign-in code expired. Connect again for a new code."),
        "invalid_client" | "unauthorized_client" => {
            bail!("TIDAL rejected this device client (HTTP {status})")
        }
        _ => bail!("TIDAL device login returned HTTP {status}. Connect again to retry."),
    }
}
fn read_auth_json(response: reqwest::blocking::Response) -> Result<Value> {
    let mut bytes = Vec::new();
    response.take(65537).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 65536,
        "TIDAL authentication response exceeds size limit"
    );
    let value = serde_json::from_slice(&bytes).context("Invalid TIDAL authentication response");
    bytes.zeroize();
    value
}
fn wait_until(next: Instant, deadline: Instant) -> Result<()> {
    while Instant::now() < next {
        ensure!(
            !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
            "Sign-in cancelled"
        );
        if Instant::now() >= deadline {
            login_status("device_code_expired");
            bail!("The TIDAL sign-in code expired. Connect again for a new code.");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    ensure!(
        !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
        "Sign-in cancelled"
    );
    Ok(())
}

pub(super) fn login(
    settings: &Settings,
    notice: impl Fn(&str),
    show: impl Fn(DevicePrompt),
    opener: impl Fn(&str) -> Result<()>,
) -> Result<Session> {
    let client = http()?;
    login_status("requesting_device_code");
    let response = client
        .post(DEVICE_ENDPOINT)
        .form(&[("client_id", DEVICE_CLIENT_ID), ("scope", SCOPE)])
        .send()?;
    let status = response.status().as_u16();
    ensure!(
        (200..300).contains(&status),
        "TIDAL could not create a device sign-in code (HTTP {status})"
    );
    let grant: Grant = serde_json::from_value(read_auth_json(response)?)?;
    let prompt = grant.prompt()?;
    let deadline = Instant::now() + Duration::from_secs(grant.expires_in);
    let mut interval = grant.interval.clamp(5, 300);
    show(prompt.clone());
    notice(
        "Approve the code on TIDAL's website. Undertow will connect automatically; no Open app prompt is needed.",
    );
    login_status("waiting_for_device_approval");
    // The code stays visible if Windows cannot launch the browser.
    let _ = opener(&prompt.verification_url);
    let mut failures = 0;
    loop {
        wait_until(Instant::now() + Duration::from_secs(interval), deadline)?;
        let response = match client
            .post(TOKEN_URL)
            .form(&[
                ("client_id", DEVICE_CLIENT_ID),
                ("device_code", grant.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("scope", SCOPE),
            ])
            .send()
        {
            Ok(r) => r,
            Err(_) => {
                failures += 1;
                ensure!(
                    failures <= 5,
                    "Cannot reach TIDAL. Check your connection and try again."
                );
                interval = (interval + 5).min(300);
                continue;
            }
        };
        let status = response.status().as_u16();
        let value = if status == 429 || status >= 500 {
            Value::Null
        } else {
            read_auth_json(response)?
        };
        ensure!(
            !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
            "Sign-in cancelled"
        );
        match poll_response(status, value)? {
            Poll::Pending => failures = 0,
            Poll::SlowDown => interval = (interval + 5).min(300),
            Poll::Retry => {
                failures += 1;
                ensure!(
                    failures <= 5,
                    "TIDAL is temporarily unavailable. Try connecting again."
                );
                interval = (interval + 5).min(300);
            }
            Poll::Approved(value) => {
                let mut session = token_response(value, settings, "")?;
                session.client_id = DEVICE_CLIENT_ID.into();
                save_session(&session)?;
                login_status("authenticated");
                return Ok(session);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn verification_links_stay_on_tidal_https() {
        assert!(verification_url("link.tidal.com/ABCD").is_ok());
        for bad in [
            "http://link.tidal.com/",
            "https://link.tidal.com.evil.test/",
            "https://user@link.tidal.com/",
            "https://link.tidal.com:444/",
        ] {
            assert!(verification_url(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn polling_respects_provider_states_and_rejects_bad_success() {
        assert!(matches!(
            poll_response(400, json!({"error":"authorization_pending"})).unwrap(),
            Poll::Pending
        ));
        assert!(matches!(
            poll_response(400, json!({"error":"slow_down"})).unwrap(),
            Poll::SlowDown
        ));
        assert!(matches!(
            poll_response(429, Value::Null).unwrap(),
            Poll::SlowDown
        ));
        assert!(matches!(
            poll_response(503, Value::Null).unwrap(),
            Poll::Retry
        ));
        assert!(poll_response(400, json!({"error":"access_denied"})).is_err());
        assert!(poll_response(400, json!({"error":"expired_token"})).is_err());
        assert!(poll_response(200, json!({})).is_err());
    }
    #[test]
    fn device_session_keeps_its_client_during_refresh() {
        let settings = Settings {
            device_login: true,
            compatibility: true,
            ..Settings::default()
        };
        let session=token_response(json!({"access_token":"test-only","expires_in":3600,"user":{"userId":4,"countryCode":"CA"}}),&settings,"").unwrap();
        assert_eq!(session.client_id, DEVICE_CLIENT_ID);
        let native = Settings {
            device_login: false,
            ..settings
        };
        assert_eq!(
            token_response(json!({"access_token":"test-only"}), &native, "")
                .unwrap()
                .client_id,
            DESKTOP_CLIENT_ID
        );
    }
}
