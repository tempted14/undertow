# Changelog

## 0.1.1 — 2026-11-13

- Reuse interleaved audio sample storage and remove the redundant per-packet copy.
- Keep library refresh reads separate from playback/edits, invalidating stale results before mutations and rejecting already-queued stale events at the UI boundary.
- Bound album metadata to 32 recently used entries.
- Wrap long settings titles and reduce header spacing in short windows.
- Update XML and rustix dependencies to patched versions.
- Add generated UI fixtures, regression checks, audit evidence, compiler path remapping, release privacy checks, clean packaging and Windows CI.

Existing native features and integration entry points remain present. External services and entitlement-dependent scenarios have separate verification limits.
