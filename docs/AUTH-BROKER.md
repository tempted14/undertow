# Keeping the developer client secret server-side

Undertow's desktop executable contains the **public client ID only**. A desktop binary cannot securely conceal a shared client secret from its users. Obfuscation, an encrypted constant, or a secret bundled alongside the executable would not solve that problem.

The normal user login is authorization code + S256 PKCE, with a fresh state and verifier for every login. Access/refresh tokens belong to each user and are saved using Windows DPAPI.

If your registered TIDAL client requires confidential authentication, run the optional token broker on your own server:

```powershell
cargo build --release --locked --features auth-broker --bin undertow-auth-broker
```

Configure these values **in the server's secret manager / protected service environment**:

- `TIDAL_CLIENT_ID`: the public ID of your registered application.
- `TIDAL_CLIENT_SECRET`: the confidential developer secret. Never put the real value in source, a release archive, or a desktop user's settings.
- `TIDAL_REDIRECT_URI`: `http://127.0.0.1:47832/callback`, or the exact loopback redirect you register and configure in the client.
- `UNDERTOW_BROKER_BIND`: optional, defaults to `127.0.0.1:9015`.

The broker intentionally binds only to loopback. Put a maintained HTTPS reverse proxy in front of `/token`, enforce request/rate limits, and disable request/response body logging for this route. In the desktop settings, use your HTTPS endpoint as **Token broker URL**. Do not distribute the broker process, its service environment, or the secret with the desktop application.

The broker accepts only authorization-code and refresh-token grants for the configured client. It checks the exact redirect and PKCE verifier shape, caps incoming request size, limits global request rate, authenticates the upstream token request with the server secret, strips unrelated response fields, and marks responses `no-store`. It does not log credentials or echo upstream error bodies. OAuth codes remain protected by PKCE. It cannot grant scopes or full playback access that TIDAL has not approved.

This component compiles and is linted, but production deployment, HTTPS termination, provider-specific confidential-client behavior, and a live token exchange still require validation on your server. No developer secret has been embedded in this repository to make a local demonstration appear authenticated.
