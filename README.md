# UNDERTOW

A native, lightweight TIDAL music player for Windows, built with Rust and egui. The interface and audio do not run in a browser engine. Undertow is an independent client, not an official TIDAL application.

## Get started

Download the Windows x64 archive from this private repository's Releases, extract it, and open Undertow.exe. Choose **Connect TIDAL** and approve sign-in on TIDAL's website. Lossless web sign-in is available in Settings → Account. A subscription and the corresponding server-side playback entitlement are required.

Sessions are encrypted for the signed-in Windows user with DPAPI. Do not share sign-in return addresses, sessions, settings exports, or account captures.

## Features

- Personalized Music discovery, saved tracks, playlists, native album/artist browsing, live search previews, sorting and filtering.
- Native streaming playback, seeking, queue/history, shuffle/repeat, metadata/quality information and authorized downloads.
- Synchronized and word-timed lyrics, manual scrolling/resync, cover themes, configurable glow, Unicode font fallback and floating lyrics.
- Editable playlists, local cover customization, profile customization, responsive layouts and keyboard shortcuts.
- Minimal Discord listening activity and all existing native plugin controls.

See [feature details](docs/FEATURES.md), [the 16-plugin compatibility matrix](docs/PLUGIN-FEATURES.md), and [known integration limits](docs/VALIDATION.md). Offline rights, DRM, exclusive audio output and external provider availability are not implied by the interface.

## Build and test

Windows x64, Visual Studio C++ Build Tools and a current Rust toolchain are required. This release was tested with Rust 1.99.1.

    ./scripts/build-release.ps1 -IncludeBroker
    cargo fmt --all --check
    cargo test --locked --all-targets --all-features
    cargo clippy --locked --all-targets --all-features -- -D warnings
    ./scripts/package.ps1 -OutputDirectory ./dist

Release builds remap local compiler paths; packaging verifies the executable contains no local Windows home paths. The optional server-side authentication broker remains available with --features auth-broker; see [deployment instructions](docs/AUTH-BROKER.md). Never bundle confidential broker credentials in the desktop application.

## Performance and UI verification

Version 0.2.1 adds eased lyric taps/resync, bounded PCM buffer recycling and wake-driven decoder waiting. See the [matched measurements and validation limits](docs/PERFORMANCE-0.2.1.md). Demand-driven lyric repainting, immutable file-backed system fonts and diagnostic redaction from [0.2.0](docs/PERFORMANCE-0.2.0.md) remain in place.

The audio conversion hot path reuses its sample storage, library reads run separately from playback/edit requests, and album metadata uses a bounded recent-item cache. Results and limitations are recorded in [the audit](docs/VALIDATION.md) and [performance measurements](docs/PERFORMANCE.md).

Generate isolated, invisible UI checks with python scripts/ui-smoke.py. This uses original test audio/artwork/text and never signs into an account or controls the desktop. Raw reports and captures go to ignored artifacts/ storage.

The repository includes the complete native source, pinned dependencies, local renderer/demuxer patches, tests, packaging and CI. Account data, local caches, historical private captures and build output are intentionally excluded.

## License

Original code: MIT. Dependencies retain their own licenses and attribution; see [third-party notices](docs/THIRD-PARTY.md). Source for the local Symphonia modifications is included under its MPL-2.0 terms.
