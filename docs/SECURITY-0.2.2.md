# 0.2.2 credential and publication review — 17 November 2026

The reviewed desktop source and settings schema contain no confidential developer client-secret field or embedded personal client-secret value. The identifiers used by the desktop are public OAuth client IDs. A confidential secret belongs on a controlled server, not inside a downloadable native executable; see [RFC 8252, section 8.5](https://www.rfc-editor.org/rfc/rfc8252.html#section-8.5). The optional broker reads its secret from the server environment and is not bundled in the desktop executable. Its upstream token request uses the fixed TIDAL HTTPS endpoint.

## Changes in this release

- Token exchange, refresh and device-code requests now disable HTTP redirects. In particular, a 307/308 response cannot forward a code, PKCE verifier, device code or refresh token to another destination. The optional broker also disables upstream redirects, protecting its confidential authentication request.
- Credential transport errors strip the request URL, including private broker query parameters. Unsuccessful token exchanges continue to omit response bodies, which could echo credentials. Broker failures return generic responses rather than upstream error details.
- Local regression tests verify both body-preserving redirect statuses with synthetic credentials and a separate destination listener. The destination receives no request. A separate test verifies that a failed request's private query value is absent from the returned error. No real credentials or TIDAL account are used by these tests.

## Existing protections retained

The registered-app and compatibility browser flows retain fresh S256 PKCE verifiers, transaction binding and their existing route validation. TIDAL documents PKCE in its [authorization guide](https://developer.tidal.com/documentation/api-sdk/api-sdk-authorization). The native compatibility return can omit state only on the existing fixed return route with the current PKCE verifier; this is not an unbound token import. Device-code approval remains on the validated TIDAL website. Browser approval and account entitlements are still required.

Access and refresh tokens are persisted as Windows DPAPI ciphertext for the current Windows user. Session types zeroize their owned token strings on drop; diagnostic settings omit developer identifiers, broker URLs, profile text and private paths. Normal TLS certificate validation remains enabled. No new password collection, analytics, credential upload or telemetry is introduced.

Release compilation remaps local compiler paths. Publication guards inspect source and executable data, including UTF-16 strings, configured secret values, private-key markers, credential/session files and local home paths. Packaging uses a source allowlist and excludes account settings, DPAPI sessions, caches, private captures, work files and Git history. Archives are checked against the clean independent checkout, and SHA-256 digests identify the tested executable and uploaded assets. The source repository remains private.

Before committing or publishing, the workflow verifies the effective author and committer, relevant environment overrides, staged files, commit messages/trailers and outgoing commits. The allowed identity is the account's verified GitHub noreply identity. Personal contact addresses are not added to source, metadata, release notes or distribution logs. Third-party attribution is preserved.

## Findings and limits

A fresh RustSec scan of the pinned lockfile on 17 November reports zero known vulnerability findings. The existing informational `ttf-parser` maintenance advisory remains visible; it is not suppressed or treated as proof of an exploitable issue. Dependency versions are unchanged in this release.

No configured confidential TIDAL secret value was available in the inspected environment or supported runtime-settings locations for an exact-value comparison. The audit therefore cannot certify an unknown historical key or remove copies that may have been shared previously. No real account sign-in or live entitlement/provider validation was performed during this release audit.

These checks are not a guarantee that the application has no unknown vulnerabilities. DPAPI protects stored data, not a session from malware already running as the same Windows user or an administrator. Tokens necessarily exist in memory while making authenticated requests. A secret previously distributed in any executable or repository must be rotated with its issuer; making a repository private or deleting a file does not revoke old copies. Nothing found here establishes that such a personal secret was distributed by this release.
