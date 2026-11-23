# Implementation research

## GUI decision

| Framework | Relevant properties from primary documentation | Decision |
|---|---|---|
| [Slint](https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/) | Retained declarative UI; software renderer supports partial rendering and very small devices. That renderer documents limits for shadows, clipping/rounding combinations, transformations, and text/scripts. Other renderers add GPU capabilities. | Strong candidate for a stricter embedded-style UI. Not benchmarked here; no memory figure is claimed. |
| [Iced](https://docs.iced.rs/iced_tiny_skia/struct.Renderer.html) | Native declarative architecture and a tiny-skia software renderer with damage regions. | Viable alternative. Not benchmarked here; no unsupported RAM ranking is claimed. |
| [Tauri](https://v2.tauri.app/concept/process-model/) | Rust core plus platform WebView processes; Windows uses WebView2. Small binary size does not imply a single process or a small total working set. | Rejected for this strict total-app RAM target and native-Rust feature request. |
| [egui](https://github.com/emilk/egui) | Native Rust widgets and low-level painting; custom effects and shared model state without a browser. | Selected after actual tests of software and OpenGL renderers. |
| [egui software backend](https://github.com/DGriffin91/egui_software_backend) | CPU rasterizer, softbuffer presentation, and cached primitives; 0.0.2 is compatible with egui 0.33. | Default. A documented local patch fixes delayed repaint scheduling and adds a frame cap, screenshot return, exit hook, and title-bar dragging. |

The first OpenGL release playback workload reached roughly 179 MB resident and 268 MB private committed memory on this machine. The corrected software-renderer workload with 24-bit/192 kHz FLAC and word-timed lyric test data peaked below 80 MB resident and 63 MB private, with zero reported audio underruns. See the final validation files for exact measurements and workload boundaries. CPU is reported as a percentage of **one logical core**, not the whole machine. These are measured workloads, not universal guarantees or comparisons with unbuilt frameworks.

## TIDAL access

- [Official OAuth documentation](https://developer.tidal.com/documentation/api-sdk/api-sdk-authorization): authorization-code PKCE and refresh grants; no desktop-shared client secret is assumed safe.
- [Official API/SDK clarification](https://github.com/orgs/tidal-music/discussions/78): public documented API access and playback SDK access are distinct. App scopes/access tiers constrain what a successful login can do.
- [TIDAL SDK source](https://github.com/tidal-music/tidal-sdk-web), including the current OpenAPI specification, was inspected for public playlist CRUD, item identifiers, pagination, user identity, and internal-only lyrics classification. Spec presence does not override a client's granted access tier.
- [Lumitide](https://github.com/BreakLime/lumitide) and its [FLAC protocol description](https://github.com/BreakLime/lumitide/blob/main/docs/how-flac-works.md) explain the unofficial Windows PKCE/internal-token route, legacy encrypted BTS streams, and platform-specific quality. Its source was inspected. Undertow adds state checking, DPAPI token storage, exact protocol-value recovery, paginated collections, revision-aware writes, a native GUI, and bounded audio buffers. No downloaded installer or upstream executable was executed.
- [python-tidal playlist implementation](https://github.com/tamland/python-tidal/blob/master/tidalapi/playlist.py) was inspected for legacy revision headers and mixed playlist item indices.

Compatibility mode can break when TIDAL changes internals. The registered public API backend implements account/catalog/playlist requests but cannot manufacture a native lossless playback grant. Protected DASH requiring a separate approved DRM module is rejected. Segmented MP4 is currently spooled to disk before decode. Playback uses Windows shared output; exclusive device access is not implemented.

### Login-method comparison, checked 26 October 2026

| Method | Evidence and tradeoff | Undertow decision |
|---|---|---|
| Lumitide Windows desktop PKCE | Its [authentication source](https://github.com/BreakLime/lumitide/blob/main/src/auth.rs) uses a public desktop ID, S256 verifier, `client_unique_key`, `r_usr w_usr`, and `tidal://login/auth`. Windows launches a second process with `--auth-callback`; Lumitide writes the return to a temporary file. | Same native compatibility flow, now with the same simple launch-command shape and a per-transaction, Windows-DPAPI-encrypted file. The return is consumed once. A supplied state must match; a missing state requires the one-time PKCE verifier to pass the server exchange. Original registry data is restored exactly. |
| Device-code login | The desktop ID rejects this grant. The public limited-input client documented by Lumitide accepted code issuance, approval, token polling and refresh without a shared key. Real library reads and playlist write round trips passed. Both lossless quality requests returned AAC for this grant. | Default simple account connection. Lossless-only mode refuses its AAC fallback. No shared key or user password is bundled. |
| HTTPS return + PKCE | [python-tidal's session source](https://github.com/tamland/python-tidal/blob/master/tidalapi/session.py) documents a public PKCE client with `https://tidal.com/android/login/auth`. The user copies the final address from the browser to the waiting client. | Added as Lossless web sign-in. Exact origin/path and supplied state are checked; S256 binds the code to the waiting app. A real token exchange, refresh, authenticated library and decoded 16-bit/44.1 kHz FLAC stream with seeking passed. No OS scheme or local HTTP callback is needed. |
| Registered-app authorization code + PKCE | The [official authorization documentation](https://developer.tidal.com/documentation/api-sdk/api-sdk-authorization) defines authorization-code, refresh and app-only client-credentials grants. Redirects must match app registration; API scopes and playback access remain separate. | Loopback flow remains available. A confidential exchange, if required, belongs on the optional server broker; developer registration is done once for the application. |
| App-only client credentials | The [TIDAL Auth specification](https://github.com/tidal-music/tidal-sdk/blob/main/Auth.md) distinguishes client credentials from user credentials. | Does not sign a user into their private library. |
| Direct password collection | The official documented user grant is browser authorization with PKCE. | Undertow does not collect, store or submit account passwords. |

The Windows protocol self-test now exercises both the previous loopback relay and the exact production encrypted-file return through Windows URL dispatch. Both passed without credentials or TIDAL requests. This proves the local dispatch plumbing; it does not prove that Chrome delivered a real authorization response or that TIDAL issued a session. Browser success alone is never recorded as native login success.

## Lyrics, presence, and resource control

- [LRCLIB API](https://lrclib.net/docs) provides plain/synced lyrics; genuine timing is preserved. Plain text is never given invented timestamps.
- The requested Radiant and translation sources were inspected. The main Radiant provider relies on credentials injected at its build time; those are not public app configuration and are not copied. The app uses TIDAL and LRCLIB for online line timing. Translation uses the upstream plugin's unofficial Google endpoint with explicit on-demand activation and exact per-line alignment.
- [Discord Rich Presence documentation](https://discord.com/developers/docs/rich-presence/overview): a public developer application identity and local IPC are used, without a Discord user/bot token. Activity acknowledgment is verified rather than inferred from a pipe write.
- Audio decode is performed off the audio callback. PCM capacity is bounded, encoded data is disk-spooled, artwork texture count is capped, lyrics/artwork disk caches are bounded, inactive track pages are evicted, and idle repainting is delayed. A native lyric child is measured along with the parent.

All sixteen original plugin source links and native scope differences are listed in [PLUGIN-FEATURES.md](PLUGIN-FEATURES.md). No JavaScript plugin engine is shipped.

## Catalog UI update

The compatibility search interface was compared with [python-tidal's session source](https://github.com/tamland/python-tidal/blob/master/tidalapi/session.py), and album fields with its [album source](https://github.com/tamland/python-tidal/blob/master/tidalapi/album.py). The implementation makes a single multi-category request after a 350 ms typing delay, rejects stale responses, limits returned items, and virtualizes the result list. Album metadata is fetched on demand. The [favorites interface](https://github.com/tamland/python-tidal/blob/master/tidalapi/user.py) documents saving an album; its native Add action uses that authenticated request. No upstream Python source is embedded. Full credits, reports, artist pages, and public search-playlist pages use service links where a native view is not implemented.

## Streaming and animation update — 27 October 2026

The Symphonia MP4 reader's eager constructor scan and required final extent forced complete segmented downloads before playback. The modified component now accepts a growing seekable source with no final length and stops the constructor scan when fragmented media starts. Existing atom/sample parsing and seek code remain in use. A localhost regression test withholds the final segment while requiring real decoded FLAC samples and a backward seek; it passes without releasing that segment. The complete modified MPL-2.0 source is distributed in vendor/symphonia-format-isomp4.

The first FLAC frame is decoded on the preparation thread. Audio output begins with a primed PCM packet, replacing the former fixed 80 ms UI-thread sleep. Tracks already tagged HiRes skip RealMAX's redundant alternate-version search; lower quality catalog tracks retain that search. Software rendering schedules frame starts against the requested cadence instead of adding render time to the delay. Lyrics use subpixel positioning, an exact critically damped spring, and active-line-only glow. Non-lyrics pages retain delayed repainting.

## Native artist browsing — 30 October 2026
Artist identity, top-track, album and biography routes follow the existing authenticated web connection. Source reference: https://minim.readthedocs.io/en/dev/api-ref/minim.api.tidal.PrivateArtistsAPI.html . Native UI work reuses existing playback, collection, context-menu and artwork components; artist metadata is bounded independently of library metadata. Live evidence is recorded separately from unit tests.
