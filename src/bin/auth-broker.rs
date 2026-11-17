//! Optional server-side component. Never distribute this process or its secret environment to desktop users.
use anyhow::{Result, ensure};
use std::{
    collections::HashMap,
    io::Read,
    time::{Duration, Instant},
};
use tiny_http::{Header, Response, Server, StatusCode};
use zeroize::Zeroizing;

fn main() -> Result<()> {
    let secret = Zeroizing::new(
        std::env::var("TIDAL_CLIENT_SECRET")
            .map_err(|_| anyhow::anyhow!("Set TIDAL_CLIENT_SECRET on the server"))?,
    );
    let id = std::env::var("TIDAL_CLIENT_ID")
        .map_err(|_| anyhow::anyhow!("Set TIDAL_CLIENT_ID on the server"))?;
    let redirect = std::env::var("TIDAL_REDIRECT_URI")
        .unwrap_or_else(|_| "http://127.0.0.1:47832/callback".into());
    let bind = std::env::var("UNDERTOW_BROKER_BIND").unwrap_or_else(|_| "127.0.0.1:9015".into());
    ensure!(
        bind.starts_with("127.0.0.1:"),
        "Bind to loopback and terminate HTTPS with a reverse proxy"
    );
    let server = Server::http(&bind).map_err(|_| anyhow::anyhow!("Cannot bind token broker"))?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        .build()?;
    let mut window = Instant::now();
    let mut requests = 0u32;
    for mut request in server.incoming_requests() {
        if window.elapsed() > Duration::from_secs(1) {
            window = Instant::now();
            requests = 0;
        }
        requests += 1;
        let result = (|| -> Result<(u16, String)> {
            ensure!(
                request.url() == "/token" && request.method().as_str() == "POST",
                "Not found"
            );
            if requests > 20 {
                return Ok((429, "{\"error\":\"rate_limited\"}".into()));
            }
            let mut body = Zeroizing::new(String::new());
            request.as_reader().take(24577).read_to_string(&mut body)?;
            ensure!(body.len() <= 24576, "Request too large");
            let form: HashMap<String, String> = url::form_urlencoded::parse(body.as_bytes())
                .into_owned()
                .collect();
            ensure!(
                form.get("client_id").map(String::as_str) == Some(id.as_str()),
                "Unknown client"
            );
            let grant = form.get("grant_type").map(String::as_str).unwrap_or("");
            let mut upstream = vec![("grant_type", grant.to_string()), ("client_id", id.clone())];
            match grant {
                "authorization_code" => {
                    let verifier = form
                        .get("code_verifier")
                        .filter(|s| {
                            s.len() >= 43
                                && s.len() <= 128
                                && s.chars()
                                    .all(|c| c.is_ascii_alphanumeric() || "-._~".contains(c))
                        })
                        .ok_or_else(|| anyhow::anyhow!("Invalid PKCE verifier"))?;
                    ensure!(
                        form.get("redirect_uri") == Some(&redirect),
                        "Unregistered redirect"
                    );
                    let code = form
                        .get("code")
                        .filter(|s| !s.is_empty() && s.len() < 4096)
                        .ok_or_else(|| anyhow::anyhow!("Invalid code"))?;
                    upstream.extend([
                        ("code", code.clone()),
                        ("code_verifier", verifier.clone()),
                        ("redirect_uri", redirect.clone()),
                    ]);
                }
                "refresh_token" => {
                    let token = form
                        .get("refresh_token")
                        .filter(|s| !s.is_empty() && s.len() < 16384)
                        .ok_or_else(|| anyhow::anyhow!("Invalid refresh token"))?;
                    upstream.push(("refresh_token", token.clone()));
                }
                _ => anyhow::bail!("Unsupported grant"),
            }
            let r = client
                .post("https://auth.tidal.com/v1/oauth2/token")
                .basic_auth(&id, Some(secret.as_str()))
                .form(&upstream)
                .send()?;
            if !r.status().is_success() {
                return Ok((
                    r.status().as_u16(),
                    "{\"error\":\"token_exchange_failed\"}".into(),
                ));
            }
            let v: serde_json::Value = r.json()?;
            let mut out = serde_json::Map::new();
            for key in [
                "access_token",
                "refresh_token",
                "expires_in",
                "token_type",
                "scope",
            ] {
                if let Some(val) = v.get(key) {
                    out.insert(key.into(), val.clone());
                }
            }
            ensure!(out.contains_key("access_token"), "Missing access token");
            Ok((200, serde_json::to_string(&out)?))
        })();
        let (status, body) =
            result.unwrap_or_else(|_| (400, "{\"error\":\"invalid_request\"}".into()));
        let response = Response::from_string(body)
            .with_status_code(StatusCode(status))
            .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
            .with_header(Header::from_bytes("Cache-Control", "no-store").unwrap())
            .with_header(Header::from_bytes("Pragma", "no-cache").unwrap());
        let _ = request.respond(response);
    }
    Ok(())
}
