# Current release validation

For 0.2.1, see [lyric navigation and playback measurements](PERFORMANCE-0.2.1.md). 107 Rust app tests, five renderer tests and three Python privacy tests pass. Four matched steady playback workloads have zero underruns; a separate rapid-seek trace records brief queue-reset silence explicitly. Lyrics, Tracks and General settings captures were reviewed. Existing live-integration limits remain below. The [0.2.0 performance/credential audit](PERFORMANCE-0.2.0.md) and broader UI review remain available.

---

# Validation and feature retention — 13 November 2026

## Verified locally

Windows x64, Rust 1.99.1. All checks use the pinned lockfile and local patches.

| Check | Result |
|---|---|
| cargo fmt --all --check | Passed |
| cargo test --locked --offline --all-targets --all-features | 101 app tests and 5 renderer integration tests passed; broker target has 0 tests |
| Explicit audio conversion benchmark | Measured separately; intentionally ignored in ordinary unit tests |
| cargo clippy --locked --offline --all-targets --all-features -- -D warnings | Passed |
| Optimized desktop build | Passed |
| Optimized optional auth-broker build | Passed |
| RustSec dependency audit | 0 known vulnerabilities/unsoundness findings; 1 informational unmaintained-font-parser warning |
| Hidden playback before/after | Four generated eight-second workloads; no audio underruns |
| Synthetic UI review | 25 captures visually reviewed across 640×480, 1200×900 and 1600×1000 |

Local mock HTTP and DPAPI tests require normal Windows socket/user-encryption access. A restricted sandbox denied those operations; rerunning with that access passed the full suite. No live account authorization was needed or changed.

## UI review scope

Generated fixtures exercise Music, Tracks, playlist, album, artist, search, lyrics, credits, similar tracks, General/Account/Plugins/About settings, profile, queue, Saved for Later, create/edit/delete playlist dialogs, volume popup and recoverable errors. Compact captures cover settings/plugins/create/album at 640×480. The checks found and corrected long-label/control overlap, excessive short-window settings header spacing and a stale About version label.

Screenshots establish painted layout for the captured states. They do not prove every OS-input gesture, DPI/monitor configuration, all offscreen content or exact TIDAL pixel parity. Compact album/track content remains scrollable; large metadata tables retain horizontal scrolling. The generated test scripts never control the user's desktop.

## Preserved functionality

| Area | Retained behavior and relevant verification |
|---|---|
| Playback | Native streaming/local decoding, seeking, queue/history, shuffle/repeat, volume, lossless validation; sample equivalence, fragmented FLAC, interrupted-CDN resume and bounded queue tests |
| Account | Existing browser/lossless/code/broker paths, session refresh, Windows DPAPI, transaction-bound returns; auth and stale-account tests |
| Library and editing | Favorites, collections, playlist create/edit/delete/visibility/reorder, mixed video positions and revisions; route/revision/draft/conflict regression tests |
| Discovery and browsing | Real feed/view-all/mixes, search previews, native artist/album navigation and portrait fallback; pagination, trusted-origin and blocked-read tests |
| Artwork | Existing thumbnail sizing, pinned visible covers, disk cache and bounded inactive textures; round-trip/corruption checks |
| Lyrics and fonts | Synced/word lyrics, scrolling/resync, glow/blur/theme, captions, native floating lyrics and DirectWrite fallback remain; layout, timing, shaping/cache and resize tests |
| Integrations | All 16 native plugin controls, fonts, Discord minimal presence, downloads/tags/cover tools, provider settings and optional broker are retained |
| UI | Existing profile/settings menus, configurable pictures, dialogs/toasts, sidebar and keyboard navigation preserved; generated layout review plus geometry/settings tests |

No requested product feature was removed to meet a memory target. See FEATURES.md and PLUGIN-FEATURES.md for the existing implementation/compatibility boundaries.

## Explicit remaining limits

This audit did not perform a fresh real TIDAL sign-in, mutate the user's real playlists, confirm every live recommendation/provider route, test actual Discord delivery, verify entitlement-dependent downloads/offline authorization, or measure exclusive audio hardware/GPU frame pacing. Those external-service scenarios remain subject to credentials, subscription, permissions, DRM and server changes. The interface does not add exclusive WASAPI, unsupported protected streams, universal offline rights, automatic cloud profile pictures or a resident embedded browser.

The dependency warning is ttf-parser 0.25.1 (RUSTSEC-2026-0192), transitively required by the current egui font stack. It is documented and not suppressed; replacing that stack without font regressions needs separate validation.

## Reproduction

Run the build/test commands in README.md. Run python scripts/ui-smoke.py for invisible generated captures; outputs are ignored under artifacts/. Use an isolated UNDERTOW_DATA_DIR for live diagnostics. Existing --validate-live and optional --playlist-roundtrip checks are available for deliberate account verification; the latter creates and deletes one temporary playlist and must not be mistaken for a read-only test.

Release archives use an explicit source allowlist. Sessions, settings, account libraries, local caches, historical private diagnostics and the enclosing workspace Git history are excluded. Compiler paths in distributed binaries are remapped to neutral build prefixes and scanned before packaging. Only the verified GitHub noreply identity is used for this repository's original commits. GitHub artifact uploads are optional because the account artifact-storage quota is currently full; test/build/security failures still fail CI, and reviewed releases carry the distribution files.
