use super::*;
use std::sync::mpsc::{self, SyncSender};

const REDIRECT: &str = "https://tidal.com/android/login/auth";
static PENDING: std::sync::Mutex<Option<SyncSender<String>>> = std::sync::Mutex::new(None);

#[derive(Clone)]
pub struct ManualPrompt {
    pub authorize_url: String,
    pub expires_at: u64,
}
struct PendingReturn;
impl Drop for PendingReturn {
    fn drop(&mut self) {
        if let Ok(mut pending) = PENDING.lock() {
            *pending = None;
        }
    }
}
fn return_url(raw: &str) -> Result<url::Url> {
    ensure!(raw.len() <= 16384, "The pasted address is too long");
    let url = url::Url::parse(raw.trim())?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("tidal.com")
            && url.path() == "/android/login/auth"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.fragment().is_none(),
        "Paste the final tidal.com/android/login/auth address from this sign-in"
    );
    Ok(url)
}
pub fn submit_return(raw: &str) -> Result<()> {
    return_url(raw)?;
    let pending = PENDING
        .lock()
        .map_err(|_| anyhow::anyhow!("Sign-in is unavailable"))?;
    let sender = pending
        .as_ref()
        .context("No lossless sign-in is waiting. Start a new sign-in.")?;
    sender
        .try_send(raw.trim().to_owned())
        .map_err(|_| anyhow::anyhow!("A return is being checked. Please wait."))
}
fn parse_return(raw: &str, state: &str) -> Result<String> {
    let url = return_url(raw)?;
    let pairs = url.query_pairs().collect::<Vec<_>>();
    ensure!(
        !pairs.iter().any(|(k, _)| k == "error"),
        "TIDAL declined this sign-in"
    );
    ensure!(
        pairs.iter().filter(|(k, _)| k == "code").count() == 1
            && pairs.iter().filter(|(k, _)| k == "state").count() <= 1,
        "Invalid sign-in return parameters"
    );
    if let Some((_, returned)) = pairs.iter().find(|(k, _)| k == "state") {
        ensure!(
            returned.as_ref() == state,
            "This address belongs to a different sign-in"
        );
    }
    // Some native clients omit state. The server must validate our fresh S256 verifier.
    pairs
        .into_iter()
        .find(|(k, v)| k == "code" && !v.is_empty() && v.len() <= 8192)
        .map(|(_, v)| v.into_owned())
        .context("This address does not contain a sign-in code")
}
pub(super) fn login(
    settings: &Settings,
    notice: impl Fn(&str),
    show: impl Fn(ManualPrompt),
    opener: impl Fn(&str) -> Result<()>,
) -> Result<Session> {
    let verifier = zeroize::Zeroizing::new(random_urlsafe());
    let state = random_urlsafe();
    let unique = format!("{:016x}", rand::random::<u64>());
    let mut url = url::Url::parse("https://login.tidal.com/authorize")?;
    url.query_pairs_mut().extend_pairs(&[
        ("response_type", "code"),
        ("client_id", WEB_PKCE_CLIENT_ID),
        ("redirect_uri", REDIRECT),
        ("scope", "r_usr w_usr w_sub"),
        ("client_unique_key", unique.as_str()),
        ("code_challenge", pkce_challenge(&verifier).as_str()),
        ("code_challenge_method", "S256"),
        ("state", state.as_str()),
        ("appMode", "android"),
        ("lang", "EN"),
        ("restrict_signup", "true"),
    ]);
    let (sender, receiver) = mpsc::sync_channel(1);
    *PENDING
        .lock()
        .map_err(|_| anyhow::anyhow!("Sign-in is unavailable"))? = Some(sender);
    let _guard = PendingReturn;
    let deadline = Instant::now() + Duration::from_secs(1800);
    show(ManualPrompt {
        authorize_url: url.to_string(),
        expires_at: now() + 1800,
    });
    notice(
        "After signing in, paste the final TIDAL page address into Undertow. The page may show an Oops message.",
    );
    login_status("waiting_for_pasted_return");
    let _ = opener(url.as_str());
    let code = loop {
        ensure!(
            !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
            "Sign-in cancelled"
        );
        ensure!(
            Instant::now() < deadline,
            "Sign-in expired. Start a new lossless sign-in."
        );
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(mut raw) => {
                let result = parse_return(&raw, &state);
                raw.zeroize();
                match result {
                    Ok(code) => break zeroize::Zeroizing::new(code),
                    Err(_) => notice(
                        "That address was rejected. Copy the complete final address from the latest sign-in tab.",
                    ),
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => bail!("Sign-in window closed"),
        }
    };
    login_status("exchanging_code");
    let value = token_post(
        settings,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", WEB_PKCE_CLIENT_ID),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("redirect_uri", REDIRECT),
            ("client_unique_key", unique.as_str()),
            ("scope", "r_usr w_usr w_sub"),
        ],
    )?;
    ensure!(
        !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
        "Sign-in cancelled"
    );
    let mut session = token_response(value, settings, "")?;
    session.client_id = WEB_PKCE_CLIENT_ID.into();
    save_session(&session)?;
    login_status("authenticated");
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pasted_return_is_route_and_transaction_bound() {
        assert_eq!(
            parse_return(
                "https://tidal.com/android/login/auth?code=fake&state=right",
                "right"
            )
            .unwrap(),
            "fake"
        );
        for raw in [
            "https://evil.test/android/login/auth?code=fake",
            "https://tidal.com.evil.test/android/login/auth?code=fake",
            "https://tidal.com/android/login/auth?code=fake&state=wrong",
            "https://tidal.com/android/login/auth?code=a&code=b",
            "https://tidal.com/android/login/auth?error=access_denied",
            "https://tidal.com/success",
            "http://tidal.com/android/login/auth?code=fake",
        ] {
            assert!(parse_return(raw, "right").is_err());
        }
    }
}
