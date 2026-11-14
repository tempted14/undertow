# Security and integration boundaries

Tokens are stored using Windows DPAPI. Release compilation remaps local paths and distribution checks scan the binary as well as the source. Desktop distributions contain no confidential developer client secret. Browser returns are transaction-bound. Local cover paths stay out of Discord activity. Recommendation pagination is restricted to the trusted API origin and route families.

This audit updates quick-xml to 0.41.0 and rustix to 1.1.6. RustSec reports no known vulnerabilities or unsoundness advisories in the resulting lockfile. An informational maintenance warning remains for ttf-parser 0.25.1 through the UI framework font stack (RUSTSEC-2026-0192). Replacing that stack is deferred to preserve existing font coverage and rendering behavior; the warning is not suppressed.

The compatibility stream-key constant is a documented upstream protocol constant, not a user's token or confidential application credential. Attribution is retained. TIDAL can change compatibility interfaces or revoke permissions. Protected DRM streams and unavailable entitlements are rejected rather than bypassed.

Do not post secrets or account captures in issues. Coordinate sensitive reports privately with the repository owner. A passing dependency scan does not prove the absence of unknown vulnerabilities.


## 0.2.0 credential review

The desktop uses public OAuth client identifiers, not an embedded confidential developer client secret. A client ID is not proof of authentication and can be visible in a native application. A real developer client secret must remain on a controlled server; the optional broker reads it from its environment. Do not put a secret in a desktop field, source file, command-line argument, release archive or diagnostic report. See [RFC 8252 section 8.5](https://www.rfc-editor.org/rfc/rfc8252.html#section-8.5).

Access/refresh tokens remain encrypted with Windows DPAPI. Diagnostics now omit developer identifiers, broker URLs, profile text and local paths from the settings payload. Publication guards reject runtime session files, credential-file extensions, populated JSON credential fields and configured TIDAL secret values, including UTF-16 executable strings. They remain active under optimized Python execution. This is defense in depth, not proof that an unknown historical secret never appeared elsewhere. No configured TIDAL secret environment values were present during the audit, so an unknown developer secret cannot be matched by value. Current source/configuration review found no embedded personal client secret. The private GitHub repository remains private.

Installed system fonts use file-backed mappings only while a read-only handle denies writing and deletion. If such a handle or mapping cannot be obtained, the original owned-byte path is used. Custom user font loading is unchanged.
