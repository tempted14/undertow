# Changelog

## 0.2.1 — 2026-11-13

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
