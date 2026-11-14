# 0.2.1 lyric navigation and playback performance — 13 November 2026

Tapping a synchronized lyric line or word now uses a short 340 ms quintic transition instead of snapping the reading position. A second tap retargets from the current position. Manual scrolling cancels the transition; resync eases back to the current line. The animation follows the existing motion-speed setting and reduced-motion preference. Resizing or changing lyric layout clears stale geometry. Audio seeks still use the exact lyric timestamps and timing offset. The view waits briefly for the decoder position before beginning a seek transition and resumes demand-driven repainting when settled.

## Playback changes

The decoder reuses PCM vectors through a bounded return queue. Its existing 16-packet queue still holds at most 256 KiB (4096 float samples per packet); including the consumer and producer, the observed high-water allocation is 18 vectors, approximately 288 KiB. This is PCM packet storage, not total application memory. Decoded sample precision, per-sample clocks, audio quality, streaming/spool limits, seek epoch rejection, download behavior and all UI features remain intact.

The full-queue producer now parks and is unparked by packet consumption, seeks and cancellation. EOF waiting uses the same notification pattern. This removes the previous 2 ms full-queue / 5 ms EOF polling loops. The audio source remains nonblocking on empty queues. Decoder seek failures now report the actual error instead of silently ending. Diagnostics count buffer allocations and park attempts; park attempts are not a measurement of operating-system wakeups.

## Matched process observations

The official 0.2.0 executable and final 0.2.1 executable each ran the same original 18-second audio/artwork/lyric fixtures in an invisible 1600×1000 window, muted, with isolated settings and no compiler running. CPU percentages refer to one logical core; memory is peak untrimmed process memory in decimal MB. Requested 120 fps is a setting, not a frame-pacing guarantee; actual frame counts are retained in the JSON evidence.

| Workload | CPU 0.2.0 → 0.2.1 | Working set MB | Private MB |
|---|---:|---:|---:|
| Tracks, software | 3.53% → 2.35% | 61.8 → 53.2 | 38.3 → 39.1 |
| Line lyrics, software | 6.85% → 6.33% | 65.3 → 65.5 | 50.3 → 50.8 |
| Word lyrics, software | 25.10% → 20.43% | 64.8 → 69.3 | 50.3 → 54.3 |
| Line lyrics, OpenGL | 3.28% → 2.22% | 181.8 → 118.5 | 293.3 → 219.2 |

All four steady final workloads used 18 PCM vectors, consumed approximately 18 seconds of audio, and reported zero underrun samples. Each recorded 196 decoder park attempts. Source sample counters and the word-fill frame count remain unchanged in behavior.

These are short process observations, not universal savings or a controlled attribution of each byte. Software memory is broadly similar and word-lyrics memory increased in this pass. Graphics allocation varies: this identical 0.2.0 binary previously measured around 119 MB working set, so the larger OpenGL difference here must not be attributed solely to the PCM change. GPU utilization counters were unavailable in this comparison; no GPU percentage improvement is claimed. The visible GPU application can still exceed 90 MB. The change bounds PCM storage and reduces allocation churn rather than trimming the process or removing functionality.

## Navigation, regressions and limitations

A separate 11-second invisible check performs three forward/backward audio seeks, manual lyric browsing and resync. Its trace contains intermediate offsets during transitions, settles with automatic following restored, and retains the expected playback position. Three intentional queue resets produced 302 cumulative underrun samples (about 6.85 ms at the fixture's mono 44.1 kHz); the final three seconds added none. This is not gapless-seek certification.

107 Rust app tests, five renderer integration tests and three Python privacy tests pass (115 total). The explicit audio-conversion benchmark remains intentionally ignored by ordinary test execution. Formatting, strict all-feature Clippy, both remapped release targets and publication privacy checks pass. RustSec reports zero known vulnerability findings and the existing ttf-parser maintenance warning. New regression tests cover PCM recycling/order/clocks/stale epochs, producer notification/cancellation, and bounded time-based transitions in either direction. Invisible captures of Lyrics, Tracks and General settings were visually reviewed.

The fixtures do not revalidate live TIDAL sign-in, entitlements, remote provider delivery, Discord acknowledgment, every input gesture, every monitor/DPI configuration or long-term memory behavior. No account data or secrets are included in the evidence. Existing DPAPI storage, public-client authentication and server-only confidential broker credentials are unchanged; the [0.2.0 credential audit](PERFORMANCE-0.2.0.md) still applies.

## Reproduce

Run `python scripts/performance-compare.py before PATH_TO_020_EXE`, then the same command with `final` and the 0.2.1 release. These generated runs create no visible windows. Raw local reports live under ignored `artifacts/performance`; sanitized summaries and the seek trace are in `benchmarks/v0.2.1`. For the navigation diagnostic, add `--preview-lyric-clicks --lyrics` to an isolated `--background-benchmark --benchmark 11 --mute --open ORIGINAL_AUDIO --report REPORT.json` run with its timestamped LRC fixture. This diagnostic never runs during normal playback.
