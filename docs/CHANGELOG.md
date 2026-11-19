# Changelog

## 0.3.0 — 2026-11-18

- Align the Windows desktop layout, typography, and controls with the installed TIDAL 2.48.0 reference. Keep the native Rust renderer, Undertow branding, and plugin controls.
- Refine the Music shelves, search suggestions, album and artist headers, settings, playlist dialog, docked queue, and synchronized lyrics.
- Preview the position during scrubbing and seek once on release. Hold the new position during decoder preparation and isolate end-of-stream markers by seek generation.
- Bundle the authorized Nationale faces and include them in the buildable source archive.
- Add regression checks for scrubbing, playback completion during a seek, responsive control placement, and primary-button contrast.

## 0.2.2 — 2026-11-17

- Copy decoded PCM spans into the bounded packet pool and preserve exact per-sample clocks with one fewer locked counter update.
- Reuse the last active word-line layouts for base/highlight rendering; retain existing glow, wrapping, shaping and click targets.
- Wake stalled spool readers on cancellation; cancel unused prepared streams and release spool ownership without a strong cycle. Existing HTTP timeouts still apply to in-flight blocking reads.
- Add exact-output, pixel/hit-target and cancellation-lifecycle tests, plus high-rate and GPU word-lyric comparisons with repeat measurements.
- Reject redirects on credential-bearing token/device-code exchanges and the optional broker's upstream request. Omit private broker URL parameters from transport errors.
- Fix collapsed lyric-scrollbar and progress-tooltip bounds during transient window sizes; preserve the reading position until the viewport returns.

## 0.2.1 — 2026-11-14

- Ease lyric line/word clicks and resync over 340 ms, with rapid retargeting, manual-scroll cancellation, reduced-motion support and layout-change cancellation.
- Recycle bounded PCM packets instead of allocating a fresh vector for each decode batch. Retain the 16-packet queue and 4,096-sample packets; the pool covers producer/consumer buffers as well.
- Wake the decoder on queue consumption, seeking and cancellation instead of polling full queues every 2 ms or completed streams every 5 ms.
- Keep audio callbacks free of blocking waits; preserve exact sample counters, stale-seek rejection and download/spool limits. Seek failures now surface as playback errors.
- Add transition, sample ordering, recycling, wake-up and cancellation regression checks, plus a hidden rapid-seek/resync diagnostic.

## 0.2.0 — 2026-11-13

- Repaint synchronized lyrics at display rate only during spring motion, fades, scrolling or live word fill. Settled lines sleep until the next timed change; audio and background integrations continue.
- Keep the first post-idle spring step tied to the lyric boundary so waking does not jump over the animation.
- Keep installed fonts in immutable, read-only file mappings instead of duplicate committed heap storage. All original font bytes, glyphs, fallback order and custom-font behavior are preserved; a guarded owned-data fallback remains.
- Remove private identifiers, broker addresses, profile text and file paths from diagnostic settings without changing local persistence.
- Add known-secret and private-key checks to publication guards, including regression tests under optimized Python.
- Add invisible OpenGL diagnostics and CPU/GPU/memory comparison evidence.

## 0.1.1 — 2026-11-13

- Reuse interleaved audio sample storage and remove the redundant per-packet copy.
- Keep library refresh reads separate from playback/edits, invalidating stale results before mutations and rejecting already-queued stale events at the UI boundary.
- Bound album metadata to 32 recently used entries.
- Wrap long settings titles and reduce header spacing in short windows.
- Update XML and rustix dependencies to patched versions.
- Add generated UI fixtures, regression checks, audit evidence, compiler path remapping, release privacy checks, clean packaging and Windows CI.

Existing native features and integration entry points remain present. External services and entitlement-dependent scenarios have separate verification limits.
