# Third-party notices

Undertow's original code is MIT-licensed. Dependency licenses remain their respective authors' licenses. Cargo.lock fixes the dependency versions. `THIRD-PARTY-LICENSES.txt` contains the available license/notice files from resolved packages; source repositories and package versions are identified in that file.

- egui / eframe: MIT OR Apache-2.0, https://github.com/emilk/egui
- egui_software_backend 0.0.2: MIT OR Apache-2.0, https://github.com/DGriffin91/egui_software_backend. Local changes are documented in `vendor/egui_software_backend/UNDERTOW-PATCHES.md`; original licenses and source are retained.
- Symphonia 0.5.5: MPL-2.0, https://github.com/pdeljanov/Symphonia. Other, unmodified dependency sources are available through the locked crates at https://crates.io/crates/symphonia/0.5.5 and associated symphonia-* packages listed in Cargo.lock.
- Rodio: MIT OR Apache-2.0, https://github.com/RustAudio/rodio
- Reqwest: MIT OR Apache-2.0, https://github.com/seanmonstar/reqwest
- Lumitide: MIT, https://github.com/BreakLime/lumitide. The public native-client identifier and legacy FLAC compatibility algorithm were researched from its protocol description/source. Its MIT license is retained in `licenses/Lumitide-MIT.txt`.

The sixteen TidaLuna plugins were studied for behavior. Their JS/CSS source is not embedded in Undertow, and their names here indicate feature inspiration rather than a claim of affiliation, identical behavior, or ownership. Their original authors and source links are preserved in `PLUGIN-FEATURES.md`.

TIDAL and Discord names and service artwork remain their owners' marks/content. No catalog music or copyrighted lyric fixture is bundled. Audio test fixtures are short generated tones; lyric workload fixtures are explicitly labeled benchmark text.

The authorized Nationale UI fonts retain their original names and copyright notices. See the [font provenance](../assets/tidal-ui/README.md).

The MP4 demuxer is a locally modified MPL-2.0 component. Its complete modified source and license are included at `vendor/symphonia-format-isomp4/` in Undertow-source.zip. The patch accepts an unknown final length for a seekable growing source and stops eager scanning at the first fragmented media atom. Existing fragment parsing and sample-based seeking remain in use. Original copyright/license headers are retained; changes are described in UNDERTOW-PATCHES.md.

- Rayon 1.12 / rayon-core 1.13 and Crossbeam: MIT OR Apache-2.0; https://github.com/rayon-rs/rayon and https://github.com/crossbeam-rs/crossbeam. The software renderer enables upstream parallel drawing using a bounded two-worker pool with small stacks. Either 1.19 is MIT OR Apache-2.0 (https://github.com/rayon-rs/either). Their resolved license files are retained in THIRD-PARTY-LICENSES.txt.

The local software-renderer patch also uses an ordered 64-bit mesh fingerprint with a single final avalanche. Position, UV, color, index order, clip, DPI, primitive order, texture identity and texture revisions remain part of cache invalidation; the upstream license is preserved. No additional package was introduced for this change.
