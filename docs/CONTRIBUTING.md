# Contributing

Preserve existing behavior and the native feature matrix. Keep network and decoding work off UI/audio callbacks. Use bounded caches and queues; include cancellation and stale-account tests for asynchronous reads.

Run formatting, all-feature tests, strict Clippy and a release build. Use generated fixtures for reproducible UI checks. For performance changes, compare identical workloads and publish their exact scope; a conversion benchmark is not a whole-app speedup.

Do not commit sessions, credentials, settings, private libraries or local-path diagnostics. Do not weaken lossless checks, playlist revisions, OAuth validation, download permissions or font/script coverage.
