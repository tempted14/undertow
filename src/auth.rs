use crate::config::{Settings, data_dir};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zeroize::{Zeroize, ZeroizeOnDrop};

mod device;
pub use device::DevicePrompt;
mod manual;
pub use manual::{ManualPrompt, submit_return};

// Public native-client identifier, also documented by Lumitide. No client secret is used.
pub const DESKTOP_CLIENT_ID: &str = "mhPVJJEBNRzVjr2p";
// Public limited-input client ID documented by Lumitide. No shared key is bundled.
pub const DEVICE_CLIENT_ID: &str = "fX2JxdmntZWK0ixT";
// Public PKCE identifier documented by python-tidal. No client secret is used.
pub const WEB_PKCE_CLIENT_ID: &str = "6BDSRdpK9hqEBTgU";
pub const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) TIDAL/2.41.3 Chrome/142.0.7444.265 Electron/39.8.0 Safari/537.36 Undertow/0.1";
const TOKEN_URL: &str = "https://auth.tidal.com/v1/oauth2/token";
static REFRESH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
static CANCEL_LOGIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub fn cancel_login() {
    CANCEL_LOGIN.store(true, std::sync::atomic::Ordering::Release);
    login_status("cancelled");
}
pub fn login_status(stage: &str) {
    // Deliberately excludes redirect URLs, codes, state values, tokens and account identity.
    let _ = crate::config::write_json(
        &data_dir().join("signin-status.json"),
        &serde_json::json!({"stage":stage,"time":now()}),
    );
}

#[derive(Serialize, Deserialize)]
struct LoginContext {
    port: u16,
    state: String,
    owner_pid: u32,
    expires_at: u64,
}
fn callback_path(state: &str) -> Result<std::path::PathBuf> {
    callback_path_in(&data_dir(), state)
}
fn callback_path_in(directory: &std::path::Path, state: &str) -> Result<std::path::PathBuf> {
    ensure!(
        state.len() >= 16
            && state.len() <= 128
            && state
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Invalid callback transaction identifier"
    );
    Ok(directory.join(format!("callback-{state}.dpapi")))
}
fn receive_file_callback(state: &str) -> Result<Option<String>> {
    receive_file_callback_in(&data_dir(), state)
}
fn receive_file_callback_in(directory: &std::path::Path, state: &str) -> Result<Option<String>> {
    let path = callback_path_in(directory, state)?;
    if !path.exists() {
        return Ok(None);
    }
    ensure!(
        std::fs::metadata(&path)?.len() <= 32768,
        "Callback exceeds size limit"
    );
    let encrypted = std::fs::read(&path)?;
    let _ = std::fs::remove_file(&path);
    let mut raw = unprotect(&encrypted)?;
    let parsed = std::str::from_utf8(&raw)
        .context("Invalid callback encoding")
        .and_then(|uri| parse_native_callback(uri, state));
    raw.zeroize();
    parsed.map(Some)
}

#[cfg(windows)]
pub struct Instance(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Instance {
    pub fn acquire() -> Result<Option<Self>> {
        let name = format!(
            "Local\\Undertow-{:x}",
            Sha256::digest(data_dir().to_string_lossy().to_lowercase().as_bytes())
        );
        Self::named(&name)
    }
    fn named(name: &str) -> Result<Option<Self>> {
        use windows::{
            Win32::{
                Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError},
                System::Threading::CreateMutexW,
            },
            core::PCWSTR,
        };
        let wide = name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        unsafe {
            let handle = CreateMutexW(None, false, PCWSTR(wide.as_ptr()))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                Ok(None)
            } else {
                Ok(Some(Self(handle)))
            }
        }
    }
}
#[cfg(windows)]
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub user_id: u64,
    pub country: String,
    pub compatibility: bool,
    pub client_id: String,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn http() -> Result<Client> {
    Ok(Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .pool_max_idle_per_host(2)
        .build()?)
}
pub fn random_urlsafe() -> String {
    let mut b = [0; 32];
    rand::thread_rng().fill_bytes(&mut b);
    URL_SAFE_NO_PAD.encode(b)
}
pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn parse_callback(raw: &str, expected_state: &str) -> Result<String> {
    let u = url::Url::parse(raw)?;
    let pairs: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
    ensure!(
        pairs.get("state").map(String::as_str) == Some(expected_state),
        "Login state did not match; sign in again"
    );
    if pairs.contains_key("error") {
        bail!("TIDAL authorization was declined");
    }
    pairs
        .get("code")
        .filter(|c| !c.is_empty())
        .cloned()
        .context("No authorization code in callback")
}

// The legacy desktop route can omit state. This exception is only used with our
// fresh S256 verifier and the fixed TIDAL native redirect. A code is not a session:
// TIDAL must validate the verifier before we save tokens or report a connection.
// RFC 9700 §2.1 permits CSRF protection through an enforced PKCE binding.
pub fn parse_native_callback(raw: &str, expected_state: &str) -> Result<String> {
    let uri = url::Url::parse(raw)?;
    ensure!(
        uri.scheme() == "tidal" && uri.host_str() == Some("login") && uri.path() == "/auth",
        "Unexpected native sign-in redirect"
    );
    let pairs = uri.query_pairs().collect::<Vec<_>>();
    ensure!(
        pairs.iter().filter(|(k, _)| k == "state").count() <= 1
            && pairs.iter().filter(|(k, _)| k == "code").count() <= 1,
        "Ambiguous sign-in parameters"
    );
    ensure!(
        !pairs.iter().any(|(k, _)| k == "error"),
        "TIDAL authorization was declined"
    );
    if let Some((_, state)) = pairs.iter().find(|(k, _)| k == "state") {
        ensure!(
            state == expected_state,
            "Login state did not match; use the latest sign-in tab"
        );
    }
    pairs
        .into_iter()
        .find(|(k, v)| k == "code" && !v.is_empty() && v.len() <= 8192)
        .map(|(_, v)| v.into_owned())
        .context("No authorization code in callback")
}

pub fn open_browser(u: &str) -> Result<()> {
    ensure!(
        u.starts_with("https://") || u.starts_with("http://127.0.0.1:"),
        "Refusing untrusted browser URL"
    );
    #[cfg(windows)]
    {
        std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(u)
            .spawn()?;
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("xdg-open").arg(u).spawn()?;
    }
    Ok(())
}

fn token_response(v: serde_json::Value, settings: &Settings, fallback: &str) -> Result<Session> {
    ensure!(
        v["access_token"].as_str().is_some(),
        "TIDAL did not return an access token"
    );
    Ok(Session {
        access_token: v["access_token"].as_str().unwrap_or_default().into(),
        refresh_token: v["refresh_token"].as_str().unwrap_or(fallback).into(),
        expires_at: now() + v["expires_in"].as_u64().unwrap_or(300),
        user_id: v
            .pointer("/user/userId")
            .and_then(|x| x.as_u64())
            .unwrap_or(0),
        country: v
            .pointer("/user/countryCode")
            .and_then(|x| x.as_str())
            .unwrap_or(&settings.country)
            .into(),
        compatibility: settings.compatibility,
        client_id: if settings.compatibility && settings.paste_login {
            WEB_PKCE_CLIENT_ID.into()
        } else if settings.compatibility && settings.device_login {
            DEVICE_CLIENT_ID.into()
        } else if settings.compatibility {
            DESKTOP_CLIENT_ID.into()
        } else {
            settings.client_id.clone()
        },
    })
}

fn token_post(settings: &Settings, form: &[(&str, &str)]) -> Result<serde_json::Value> {
    let client = http()?;
    let endpoint = if settings.compatibility || settings.token_broker.is_empty() {
        TOKEN_URL
    } else {
        &settings.token_broker
    };
    ensure!(
        endpoint.starts_with("https://"),
        "Token broker must use HTTPS"
    );
    let r = client.post(endpoint).form(form).send()?;
    let status = r.status();
    if !status.is_success() {
        login_status(&format!("token_exchange_http_{}", status.as_u16()));
    }
    // Do not include response bodies: an auth provider can echo credentials.
    ensure!(
        status.is_success(),
        "TIDAL token exchange returned HTTP {}. Check registered redirects, scopes and app access; confidential clients need a token broker.",
        status.as_u16()
    );
    Ok(r.json()?)
}

pub fn login(
    settings: &Settings,
    notice: impl Fn(&str),
    prompt: impl Fn(DevicePrompt),
    manual_prompt: impl Fn(ManualPrompt),
) -> Result<Session> {
    login_with_events(settings, notice, prompt, manual_prompt, open_browser)
}
pub fn login_with_opener(
    settings: &Settings,
    notice: impl Fn(&str),
    opener: impl Fn(&str) -> Result<()>,
) -> Result<Session> {
    login_with_events(settings, notice, |_| {}, |_| {}, opener)
}
fn login_with_events(
    settings: &Settings,
    notice: impl Fn(&str),
    prompt: impl Fn(DevicePrompt),
    manual_prompt: impl Fn(ManualPrompt),
    opener: impl Fn(&str) -> Result<()>,
) -> Result<Session> {
    CANCEL_LOGIN.store(false, std::sync::atomic::Ordering::Release);
    if settings.compatibility && settings.paste_login {
        return manual::login(settings, notice, manual_prompt, opener);
    }
    if settings.compatibility && settings.device_login {
        return device::login(settings, notice, prompt, opener);
    }
    let verifier = random_urlsafe();
    let state = random_urlsafe();
    let unique_key = format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        rand::random::<u32>(),
        rand::random::<u16>(),
        rand::random::<u16>() & 0xfff,
        rand::random::<u16>() & 0xfff,
        rand::random::<u64>() & 0xffffffffffff
    );
    let mut redirect = settings.redirect_uri.clone();
    let listener;
    let mut bridge = None;
    let client_id;
    let scope;
    if settings.compatibility {
        redirect = "tidal://login/auth".into();
        client_id = DESKTOP_CLIENT_ID;
        scope = "r_usr w_usr";
        listener = TcpListener::bind("127.0.0.1:0")?;
        bridge = Some(ProtocolBridge::install(
            listener.local_addr()?.port(),
            &state,
        )?);
    } else {
        client_id = settings.client_id.as_str();
        scope = settings.scopes.as_str();
        let r = url::Url::parse(&redirect)?;
        ensure!(
            r.scheme() == "http" && r.host_str() == Some("127.0.0.1") && r.path() == "/callback",
            "Register a loopback redirect http://127.0.0.1:PORT/callback"
        );
        listener = TcpListener::bind(("127.0.0.1", r.port_or_known_default().unwrap_or(47832)))?;
    }
    listener.set_nonblocking(true)?;
    let mut u = url::Url::parse("https://login.tidal.com/authorize")?;
    u.query_pairs_mut().extend_pairs(&[
        ("response_type", "code"),
        ("client_id", client_id),
        ("redirect_uri", &redirect),
        ("scope", scope),
        ("code_challenge", &pkce_challenge(&verifier)),
        ("code_challenge_method", "S256"),
        ("state", &state),
        ("client_unique_key", &unique_key),
    ]);
    notice(
        "Complete TIDAL sign-in in your browser, then allow Open app. You can cancel here. Waiting up to 30 minutes…",
    );
    login_status("waiting_for_browser");
    opener(u.as_str())?;
    let deadline = Instant::now() + Duration::from_secs(1800);
    let code = loop {
        ensure!(
            !CANCEL_LOGIN.load(std::sync::atomic::Ordering::Acquire),
            "Sign-in cancelled"
        );
        ensure!(
            Instant::now() < deadline,
            "Sign-in timed out; your previous TIDAL protocol handler has been restored"
        );
        if settings.compatibility {
            match receive_file_callback(&state) {
                Ok(Some(code)) => break code,
                Ok(None) => {}
                Err(_) => {
                    login_status("return_rejected");
                    notice("A sign-in return was rejected. Use the latest sign-in tab.");
                }
            }
        }
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                let mut buf = [0u8; 16384];
                let n = stream.read(&mut buf)?;
                let request = String::from_utf8_lossy(&buf[..n]);
                let target = request
                    .lines()
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or("");
                let callback = if settings.compatibility {
                    url::Url::parse(&format!("http://127.0.0.1{target}"))?
                        .query_pairs()
                        .find(|(k, _)| k == "callback")
                        .map(|(_, v)| v.into_owned())
                        .unwrap_or_default()
                } else {
                    format!(
                        "{}{}",
                        &redirect[..redirect.find("/callback").unwrap_or(redirect.len())],
                        target
                    )
                };
                let parsed = if settings.compatibility {
                    parse_native_callback(&callback, &state)
                } else {
                    parse_callback(&callback, &state)
                };
                let ok = parsed.is_ok();
                if !ok {
                    login_status("return_rejected");
                    notice(
                        "A sign-in return was rejected. Use the browser tab opened by the current Undertow sign-in.",
                    );
                }
                let body = if ok {
                    "TIDAL sign-in received. You can return to Undertow."
                } else {
                    "Invalid sign-in callback. Return to Undertow and try again."
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    if ok { "200 OK" } else { "400 Bad Request" },
                    body.len(),
                    body
                );
                if let Ok(code) = parsed {
                    break code;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100))
            }
            Err(e) => return Err(e.into()),
        }
    };
    drop(bridge);
    login_status("exchanging_code");
    let v = token_post(
        settings,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client_id),
            ("code", &code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("client_unique_key", &unique_key),
            ("scope", scope),
        ],
    )?;
    let mut session = token_response(v, settings, "")?;
    if session.compatibility && session.user_id == 0 {
        let r: serde_json::Value = http()?
            .get("https://api.tidal.com/v1/sessions")
            .bearer_auth(&session.access_token)
            .send()?
            .error_for_status()?
            .json()?;
        session.user_id = r["userId"].as_u64().unwrap_or(0);
        session.country = r["countryCode"]
            .as_str()
            .unwrap_or(&settings.country)
            .into();
    }
    save_session(&session)?;
    login_status("authenticated");
    Ok(session)
}

pub fn refresh(session: &mut Session, settings: &Settings) -> Result<()> {
    refresh_inner(session, settings, false, None)
}
pub fn refresh_now(session: &mut Session, settings: &Settings) -> Result<()> {
    refresh_inner(session, settings, true, None)
}
pub(crate) fn refresh_cancellable(
    session: &mut Session,
    settings: &Settings,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Result<()> {
    if cancelled.is_none() {
        return refresh(session, settings);
    }
    refresh_inner(session, settings, false, cancelled)
}
fn refresh_inner(
    session: &mut Session,
    settings: &Settings,
    force: bool,
    cancelled: Option<&std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let check = || -> Result<()> {
        ensure!(
            !cancelled.is_some_and(|c| c.load(std::sync::atomic::Ordering::Acquire)),
            "Account request cancelled"
        );
        Ok(())
    };
    check()?;
    if !force && session.expires_at > now() + 60 {
        return Ok(());
    }
    let _guard = REFRESH_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Session refresh lock failed"))?;
    check()?;
    if let Ok(latest) = load_session()
        && latest.client_id == session.client_id
        && latest.user_id == session.user_id
        && latest.expires_at > session.expires_at
    {
        *session = latest;
    }
    if !force && session.expires_at > now() + 60 {
        return Ok(());
    }
    let mut effective = settings.clone();
    effective.compatibility = session.compatibility;
    effective.device_login = session.client_id == DEVICE_CLIENT_ID;
    effective.paste_login = session.client_id == WEB_PKCE_CLIENT_ID;
    effective.client_id = session.client_id.clone();
    let v = token_post(
        &effective,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &session.client_id),
            ("refresh_token", &session.refresh_token),
            (
                "scope",
                if effective.device_login || effective.paste_login {
                    "r_usr w_usr w_sub"
                } else if session.compatibility {
                    "r_usr w_usr"
                } else {
                    settings.scopes.as_str()
                },
            ),
        ],
    )?;
    check()?;
    let mut next = token_response(v, &effective, &session.refresh_token)?;
    next.client_id = session.client_id.clone();
    if next.user_id == 0 {
        next.user_id = session.user_id;
    }
    *session = next;
    save_session(session)
}

pub fn save_session(s: &Session) -> Result<()> {
    std::fs::create_dir_all(data_dir())?;
    let mut bytes = serde_json::to_vec(s)?;
    let protected = protect(&bytes)?;
    bytes.zeroize();
    let p = data_dir().join("session.dpapi");
    let temp = p.with_extension("tmp");
    std::fs::write(&temp, protected)?;
    std::fs::rename(temp, p)?;
    Ok(())
}
pub fn load_session() -> Result<Session> {
    let mut bytes = unprotect(&std::fs::read(data_dir().join("session.dpapi"))?)?;
    let value = serde_json::from_slice(&bytes);
    bytes.zeroize();
    Ok(value?)
}
pub fn sign_out() -> Result<()> {
    let _guard = REFRESH_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("Session refresh lock failed"))?;
    let p = data_dir().join("session.dpapi");
    if p.exists() {
        std::fs::remove_file(p)?;
    }
    Ok(())
}

#[cfg(windows)]
fn protect(bytes: &[u8]) -> Result<Vec<u8>> {
    use windows::Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::*,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        Ok(v)
    }
}
#[cfg(windows)]
fn unprotect(bytes: &[u8]) -> Result<Vec<u8>> {
    use windows::Win32::{
        Foundation::{HLOCAL, LocalFree},
        Security::Cryptography::*,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut out = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        )?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut _)));
        Ok(v)
    }
}
#[cfg(not(windows))]
fn protect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("Secure session storage is currently Windows-only")
}
#[cfg(not(windows))]
fn unprotect(_: &[u8]) -> Result<Vec<u8>> {
    bail!("Secure session storage is currently Windows-only")
}

// Only the exact values changed by this app are restored; other protocol settings survive.
#[cfg(windows)]
pub struct ProtocolBridge {
    _lock: Instance,
    prior: Option<String>,
    prior_url: Option<winreg::RegValue>,
    command: String,
}
#[cfg(windows)]
impl ProtocolBridge {
    pub fn install(port: u16, nonce: &str) -> Result<Self> {
        Self::install_for(port, nonce, false)
    }
    fn install_for(port: u16, nonce: &str, probe: bool) -> Result<Self> {
        use winreg::{RegKey, enums::*};
        let lock = Instance::named("Local\\Undertow-Tidal-Auth")?
            .context("Another Undertow sign-in is already in progress")?;
        Self::recover()?;
        ensure!(
            !data_dir().join("protocol-recovery.json").exists(),
            "Another Undertow sign-in is already in progress"
        );
        let root = RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = root.create_subkey("Software\\Classes\\tidal\\shell\\open\\command")?;
        let prior: Option<String> = key.get_value("").ok();
        let prior_raw = key.get_raw_value("").ok();
        let (proto, _) = root.create_subkey("Software\\Classes\\tidal")?;
        let prior_url = proto.get_raw_value("URL Protocol").ok();
        let command = if probe {
            format!(
                "\"{}\" {} {} {} \"%1\"",
                std::env::current_exe()?.display(),
                if probe {
                    "--protocol-selftest-return"
                } else {
                    "--auth-callback"
                },
                port,
                nonce
            )
        } else {
            format!(
                "\"{}\" --auth-callback \"%1\"",
                std::env::current_exe()?.display()
            )
        };
        crate::config::write_json(
            &data_dir().join("protocol-recovery.json"),
            &serde_json::json!({"owner_pid":std::process::id(),"prior":prior,"prior_bytes":prior_raw.as_ref().map(|v|&v.bytes),"prior_type":prior_raw.as_ref().map(|v|v.vtype.clone()as u32),"command":command,"had_url":prior_url.is_some(),"url_type":prior_url.as_ref().map(|v|v.vtype.clone() as u32),"url_bytes":prior_url.as_ref().map(|v|&v.bytes)}),
        )?;
        let guard = Self {
            _lock: lock,
            prior,
            prior_url,
            command: command.clone(),
        };
        if !probe {
            crate::config::write_json(
                &data_dir().join("login-context.json"),
                &LoginContext {
                    port,
                    state: nonce.into(),
                    owner_pid: std::process::id(),
                    expires_at: now() + 1800,
                },
            )?;
        }
        proto.set_value("URL Protocol", &"")?;
        key.set_value("", &command)?;
        notify_protocol_change();
        Ok(guard)
    }
    pub fn recover() -> Result<()> {
        use winreg::{RegKey, enums::*};
        let p = data_dir().join("protocol-recovery.json");
        if !p.exists() {
            return Ok(());
        }
        let v: serde_json::Value = crate::config::read_json(&p)?;
        if let Some(pid) = v["owner_pid"].as_u64()
            && pid as u32 != std::process::id()
        {
            use windows::Win32::{
                Foundation::{CloseHandle, WAIT_TIMEOUT},
                System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
            };
            unsafe {
                if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid as u32) {
                    let alive = WaitForSingleObject(h, 0) == WAIT_TIMEOUT;
                    let _ = CloseHandle(h);
                    if alive {
                        return Ok(());
                    }
                }
            }
        }
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(
            "Software\\Classes\\tidal\\shell\\open\\command",
            KEY_READ | KEY_WRITE,
        )?;
        let current: String = key.get_value("").unwrap_or_default();
        if Some(current.as_str()) == v["command"].as_str() {
            if v["prior_bytes"].is_array() {
                let bytes: Vec<u8> = serde_json::from_value(v["prior_bytes"].clone())?;
                let vtype = registry_type(v["prior_type"].as_u64().unwrap_or(1))?;
                key.set_raw_value("", &winreg::RegValue { bytes, vtype })?;
            } else if let Some(s) = v["prior"].as_str() {
                key.set_value("", &s)?;
            } else {
                let _ = key.delete_value("");
            }
            let proto = RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey_with_flags("Software\\Classes\\tidal", KEY_READ | KEY_WRITE)?;
            if v["had_url"].as_bool() == Some(true) {
                let bytes: Vec<u8> = serde_json::from_value(v["url_bytes"].clone())?;
                // URL Protocol is normally a string. Preserve its original raw data.
                let vtype = match v["url_type"].as_u64().unwrap_or(1) {
                    0 => REG_NONE,
                    1 => REG_SZ,
                    2 => REG_EXPAND_SZ,
                    3 => REG_BINARY,
                    4 => REG_DWORD,
                    5 => REG_DWORD_BIG_ENDIAN,
                    6 => REG_LINK,
                    7 => REG_MULTI_SZ,
                    8 => REG_RESOURCE_LIST,
                    9 => REG_FULL_RESOURCE_DESCRIPTOR,
                    10 => REG_RESOURCE_REQUIREMENTS_LIST,
                    11 => REG_QWORD,
                    _ => bail!("Invalid saved registry type"),
                };
                let raw = winreg::RegValue { bytes, vtype };
                proto.set_raw_value("URL Protocol", &raw)?;
            } else {
                let _ = proto.delete_value("URL Protocol");
            }
        }
        std::fs::remove_file(p)?;
        let context_path = data_dir().join("login-context.json");
        if let Ok(context) = crate::config::read_json::<LoginContext>(&context_path)
            && Some(context.owner_pid as u64) == v["owner_pid"].as_u64()
        {
            if let Ok(callback) = callback_path(&context.state) {
                let _ = std::fs::remove_file(callback);
            }
            let _ = std::fs::remove_file(context_path);
        }
        notify_protocol_change();
        Ok(())
    }
}

/// Exercises Windows URL dispatch without a login code or a request to TIDAL.
#[cfg(windows)]
pub fn protocol_selftest(path: &std::path::Path, browser: bool) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let port = listener.local_addr()?.port();
    let nonce = random_urlsafe();
    let bridge = ProtocolBridge::install_for(port, &nonce, true)?;
    let uri = format!("tidal://undertow-selftest?state={nonce}");
    if browser {
        open_browser(&format!("http://127.0.0.1:{port}/"))?;
    } else {
        std::process::Command::new("rundll32.exe")
            .arg("url.dll,FileProtocolHandler")
            .arg(&uri)
            .spawn()?;
    }
    let deadline = Instant::now() + Duration::from_secs(if browser { 600 } else { 15 });
    let result = loop {
        match listener.accept() {
            Ok((mut stream, peer)) => {
                if !peer.ip().is_loopback() {
                    continue;
                }
                stream.set_read_timeout(Some(Duration::from_secs(2)))?;
                let mut buffer = [0; 2048];
                let n = match stream.read(&mut buffer) {
                    Ok(0) | Err(_) => continue,
                    Ok(n) => n,
                };
                let request = String::from_utf8_lossy(&buffer[..n]);
                if browser && request.starts_with("GET / HTTP/") {
                    let body = format!(
                        r#"<!doctype html><meta charset="utf-8"><title>Undertow return test</title><style>body{{background:#101216;color:#f5f5f5;font:18px 'Segoe UI',sans-serif;margin:12vh auto;max-width:680px;padding:32px}}a{{display:inline-block;background:#24dfcc;color:#082220;padding:18px 28px;border-radius:32px;text-decoration:none;font-weight:600}}p{{line-height:1.6;color:#b5b8be}}</style><h1>Test the browser return</h1><p>This checks whether Chrome opens Undertow for a TIDAL link. It contains no account information, password, token or authorization code.</p><a href="{uri}">Test return to Undertow</a><p>Click the button, then Open if Chrome asks. The test does not sign you into TIDAL. Return to the chat afterward.</p>"#
                    );
                    let _ = write!(
                        stream,
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    continue;
                }
                let verified = request
                    .lines()
                    .next()
                    .is_some_and(|s| s.contains(&format!("/selftest?state={nonce}")));
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK",
                );
                if verified {
                    break true;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() > deadline {
                    break false;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(e.into()),
        }
    };
    drop(bridge);
    // Exercise the exact production command and encrypted-file transport too.
    // This synthetic value is consumed here; no token exchange is attempted.
    let file_bridge = ProtocolBridge::install(0, &nonce)?;
    let fake_code = "undertow-synthetic-dispatch-check";
    let uri = format!("tidal://login/auth?code={fake_code}&state={nonce}");
    std::process::Command::new("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(uri)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(15);
    let file_result = loop {
        if let Some(code) = receive_file_callback(&nonce)? {
            break code == fake_code;
        }
        if Instant::now() > deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    drop(file_bridge);
    crate::config::write_json(
        path,
        &serde_json::json!({"windows_url_dispatch_verified":result,"browser_dispatch_tested":browser,"encrypted_file_dispatch_verified":file_result,"credentials_used":false,"tidal_requests_made":false}),
    )?;
    ensure!(
        result && file_result,
        "Windows did not return the diagnostic link to Undertow"
    );
    Ok(())
}
#[cfg(not(windows))]
pub fn protocol_selftest(_: &std::path::Path, _: bool) -> Result<()> {
    bail!("Windows-only diagnostic")
}

pub fn protocol_selftest_return(args: &[String]) -> Result<()> {
    ensure!(args.len() == 5, "Invalid diagnostic arguments");
    let port: u16 = args[2].parse()?;
    let uri = url::Url::parse(&args[4])?;
    ensure!(
        uri.scheme() == "tidal" && uri.host_str() == Some("undertow-selftest"),
        "Invalid diagnostic URI"
    );
    ensure!(
        uri.query_pairs().any(|(k, v)| k == "state" && v == args[3]),
        "Invalid diagnostic nonce"
    );
    let mut target = url::Url::parse(&format!("http://127.0.0.1:{port}/selftest"))?;
    target.query_pairs_mut().append_pair("state", &args[3]);
    http()?.get(target).send()?.error_for_status()?;
    Ok(())
}

#[cfg(windows)]
fn notify_protocol_change() {
    use windows::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
    // Windows caches URL associations. Notify it after both installation and restoration.
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
}

#[cfg(windows)]
fn registry_type(n: u64) -> Result<winreg::enums::RegType> {
    use winreg::enums::*;
    Ok(match n {
        0 => REG_NONE,
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        5 => REG_DWORD_BIG_ENDIAN,
        6 => REG_LINK,
        7 => REG_MULTI_SZ,
        8 => REG_RESOURCE_LIST,
        9 => REG_FULL_RESOURCE_DESCRIPTOR,
        10 => REG_RESOURCE_REQUIREMENTS_LIST,
        11 => REG_QWORD,
        _ => bail!("Invalid registry type"),
    })
}
#[cfg(windows)]
impl Drop for ProtocolBridge {
    fn drop(&mut self) {
        let _ = (&self.prior, &self.prior_url, &self.command);
        let _ = Self::recover();
    }
}
#[cfg(not(windows))]
struct ProtocolBridge;
#[cfg(not(windows))]
impl ProtocolBridge {
    fn install(_: u16, _: &str) -> Result<Self> {
        bail!("Desktop compatibility login requires Windows")
    }
}

pub fn relay_callback(args: &[String]) -> Result<()> {
    login_status("callback_received");
    if args.len() == 3 {
        relay_file_callback_in(&data_dir(), &args[2])?;
        login_status("callback_delivered");
        return Ok(());
    }
    ensure!(args.len() == 5, "Invalid callback arguments");
    let port: u16 = args[2].parse()?;
    let u = url::Url::parse(&args[4])?;
    ensure!(
        u.scheme() == "tidal" && u.host_str() == Some("login"),
        "Invalid TIDAL callback"
    );
    // The independently generated state is carried through TIDAL, not inserted by the relay.
    if parse_native_callback(u.as_str(), &args[3]).is_err() {
        login_status("callback_rejected");
        bail!("Invalid native callback");
    }
    if !u.query_pairs().any(|(k, _)| k == "state") {
        login_status("callback_without_state_pkce_required");
    }
    let mut target = url::Url::parse(&format!("http://127.0.0.1:{port}/callback"))?;
    target.query_pairs_mut().append_pair("callback", u.as_str());
    let response = http()?.get(target).send();
    if !response.is_ok_and(|r| r.status().is_success()) {
        login_status("callback_delivery_failed");
        bail!("Unable to deliver the sign-in return to Undertow");
    }
    login_status("callback_delivered");
    Ok(())
}

fn relay_file_callback_in(directory: &std::path::Path, raw: &str) -> Result<()> {
    let context: LoginContext = crate::config::read_json(&directory.join("login-context.json"))?;
    ensure!(context.expires_at > now(), "The sign-in request expired");
    let uri = raw.trim().trim_matches('"');
    ensure!(uri.len() <= 16384, "Callback exceeds size limit");
    parse_native_callback(uri, &context.state)?;
    let path = callback_path_in(directory, &context.state)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, protect(uri.as_bytes())?)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc7636_pkce_vector() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }
    #[test]
    fn forged_callback_is_rejected() {
        assert!(parse_callback("tidal://login/auth?code=stolen&state=wrong", "right").is_err());
        assert_eq!(
            parse_callback("tidal://login/auth?code=ok&state=right", "right").unwrap(),
            "ok"
        );
    }
    #[test]
    fn native_pkce_callback_validates_route_and_present_state() {
        assert_eq!(
            parse_native_callback("tidal://login/auth?code=test-code", "expected").unwrap(),
            "test-code"
        );
        assert!(
            parse_native_callback("tidal://login/auth?code=test-code&state=wrong", "expected")
                .is_err()
        );
        assert!(
            parse_native_callback("https://example.com/auth?code=test-code", "expected").is_err()
        );
        assert!(parse_native_callback("tidal://login/other?code=test-code", "expected").is_err());
        assert!(parse_native_callback("tidal://login/auth?code=a&code=b", "expected").is_err());
        assert!(
            parse_native_callback(
                "tidal://login/auth?code=a&state=expected&state=wrong",
                "expected"
            )
            .is_err()
        );
        assert!(parse_callback("http://127.0.0.1/callback?code=test-code", "expected").is_err());
    }
    #[cfg(windows)]
    #[test]
    fn dpapi_round_trip() {
        let secret = b"test-refresh-token";
        let encrypted = protect(secret).unwrap();
        assert!(!encrypted.windows(secret.len()).any(|w| w == secret));
        assert_eq!(unprotect(&encrypted).unwrap(), secret);
    }
    #[cfg(windows)]
    #[test]
    fn encrypted_file_return_is_bound_to_transaction_and_consumed_once() {
        let state = random_urlsafe();
        let directory = std::env::temp_dir().join(format!("undertow-callback-test-{state}"));
        let context = LoginContext {
            port: 0,
            state: state.clone(),
            owner_pid: std::process::id(),
            expires_at: now() + 60,
        };
        crate::config::write_json(&directory.join("login-context.json"), &context).unwrap();
        assert!(
            relay_file_callback_in(&directory, "tidal://login/auth?code=fake&state=wrong").is_err()
        );
        assert!(
            receive_file_callback_in(&directory, &state)
                .unwrap()
                .is_none()
        );
        let code = "synthetic-code-never-sent-to-tidal";
        relay_file_callback_in(
            &directory,
            &format!("tidal://login/auth?code={code}&state={state}"),
        )
        .unwrap();
        let bytes = std::fs::read(callback_path_in(&directory, &state).unwrap()).unwrap();
        assert!(!bytes.windows(code.len()).any(|w| w == code.as_bytes()));
        assert_eq!(
            receive_file_callback_in(&directory, &state)
                .unwrap()
                .as_deref(),
            Some(code)
        );
        assert!(
            receive_file_callback_in(&directory, &state)
                .unwrap()
                .is_none()
        );
        let expired = LoginContext {
            expires_at: 0,
            ..context
        };
        crate::config::write_json(&directory.join("login-context.json"), &expired).unwrap();
        assert!(relay_file_callback_in(&directory, "tidal://login/auth?code=fake").is_err());
        std::fs::remove_file(directory.join("login-context.json")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
