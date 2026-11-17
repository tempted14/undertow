# 0.2.2 playback overhead and resource lifecycle — 17 November 2026

## Changes and retained behavior

Decoded PCM is now copied into the existing reusable packet buffers in contiguous spans, instead of advancing the decoder iterator once per sample. The iterator remains as a scalar reference. Exact output comparisons cover packet boundaries, partial batches, accurate seeks and EOF, including the generated 24-bit/192 kHz FLAC fixture. The incremental fragmented-stream test now exercises the bulk path before the last segment arrives.

The consumed-audio clock has one writer, so its per-sample update now uses an atomic load/store instead of a locked read-modify-write. Its precision and visibility are retained for every sample. The separate playback clock still uses a read-modify-write because the seek worker also updates it. Neither clock is batched, rounded or estimated. Queue depth, sample precision, playback format, streaming limits and all integrations remain unchanged.

Focused word lyrics retain at most one line of word layouts and reuse each shape for the base and clipped highlight passes. This removes repeated string allocation and font-layout cache lookups during animation. The cache is invalidated with lyric/font/wrapping/DPI layout changes and on new tracks. Existing complex-script shaping remains intact. Pixel and hit-rectangle comparisons cover wrapping, glow and partial word fills, including Japanese text. No effects or frame-rate settings were reduced.

Cancellation now wakes a decoder blocked on spool data or an unknown stream length instead of leaving it at the existing 30-second condition-variable timeout. The notification synchronizes with the reader's check-and-wait so a wake cannot be lost. It runs on control/decoder threads; the audio callback does not acquire the progress mutex. Dropping an unused prepared stream also marks its downloader cancelled. A weak reference avoids an ownership cycle, and tests verify the spool is released. An already-blocked HTTP read can still take until the existing request timeout or next I/O completion; this is not an instantaneous network-abort claim.

## Whole-player comparison

Official 0.2.1 and the exact final 0.2.2 executable each ran original generated audio/art/lyrics in invisible, muted 1600×1000 windows for 18 seconds per case, with isolated data and no compiler running. CPU is relative to one logical core, and memory is peak untrimmed process memory in decimal MB. Requested 120 fps/display sync is not a presentation-rate guarantee; actual frame counts remain in the JSON.

| Workload | CPU before → final | Working set MB | Private MB |
|---|---:|---:|---:|
| tracks | 2.62% → 2.26% | 60.8 → 53.3 | 38.2 → 38.3 |
| line-lyrics | 5.47% → 5.47% | 64.9 → 65.7 | 50.5 → 51.3 |
| word-lyrics | 17.12% → 18.89% | 64.8 → 69.6 | 49.9 → 54.4 |
| gpu-line-lyrics | 1.69% → 1.86% | 119.1 → 119.1 | 219.5 → 219.4 |
| gpu-word-lyrics | 13.12% → 14.13% | 123.6 → 122.2 | 219.4 → 219.5 |
| high-rate-tracks | 3.33% → 3.77% | 56.9 → 56.7 | 39.4 → 39.3 |

All six final steady cases reported zero underrun samples and at most 18 PCM-vector allocations. PCM packet storage remains approximately 288 KiB including producer/consumer buffers. High-rate playback uses generated 32-bit PCM, 192 kHz stereo, with normal shared-device resampling; it does not certify exclusive output or live TIDAL entitlement.

The initial performance candidate, before credential-transport hardening, showed a large high-rate improvement in one pair, but a reversed-order confirmation measured only **5.48% → 5.25%** one-core CPU. Tracks and GPU word lyrics were also repeated in ABBA order because their first readings moved upward. Their ranges overlap between versions, so no reliable whole-player improvement is claimed for those workloads. Those historical observations and exact candidate hashes are retained separately. The six-case table above was refreshed for the final hardened executable on 17 November. Short process observations are not universal savings.

Steady RAM is broadly unchanged. This release targets repeated work and prompt release of abandoned decoder state, not working-set trimming, smaller font coverage or omitted artwork. GPU driver allocations still put these OpenGL runs above 90 MB. GPU utilization is not inferred from CPU, and no general GPU-utilization saving is claimed. Background applications, clocks and scheduling can affect results.

## Isolated hot paths

Seven alternating-order batches measured 2,000 copies of already-decoded 4096-float stereo packets and 8,192,000 exact clock updates per batch. Scalar copy: **9.694 ms**, span copy: **0.486 ms**. Two-RMW clocks: **25.874 ms**, single-writer consumed clock: **12.721 ms**. These measure isolated algorithms, not a corresponding multiplier for decoding, streaming or total application performance. The explicit test is ignored by ordinary test execution and has no timing-based pass/fail threshold.

## Verification and limits

114 Rust app tests, five renderer integration tests and three Python privacy tests pass (122 automated checks). The conversion and hot-path benchmarks are explicitly ignored by ordinary test execution. Formatting, strict all-feature Clippy, remapped desktop/broker release builds and publication guards pass. Stalled read/length-seek cancellation tests require completion within one second and verify no strong ownership cycle or retained spool file. Credential-post redirect and private-URL error regression tests also pass. A refreshed RustSec scan reports zero known vulnerabilities and retains the existing font-parser maintenance advisory; dependency versions are unchanged. See the [security review](SECURITY-0.2.2.md).

Validation also found an intermittent panic when a transiently collapsed lyric viewport made the scrollbar's upper clamp bound smaller than its minimum. Invalid/collapsed viewports now retain the reading position without constructing a scrollbar, and tiny positive viewports use a bounded thumb. The progress-tooltip position has the equivalent collapsed-width safeguard. Deterministic tests cover negative/zero/tiny dimensions, and three additional hidden GPU word-lyric runs passed on the fixed executable. This was a bounds assertion, not evidence of native memory corruption; no crash dump or account data is distributed.

An 11-second rapid forward/backward seek and resync check restored automatic following and retained the expected clocks. Its three queue resets produced 663 cumulative padding samples (about 15.03 ms in the mono 44.1 kHz fixture), with no additional underruns during the final three seconds. A separate two-second file reached EOF normally in a seven-second diagnostic. Lyrics, Tracks and General settings captures and the GPU word-lyric capture were visually reviewed. These checks do not certify gapless seeks, every OS-input/DPI/monitor configuration, long-term memory stability, live account sign-in, remote providers or Discord acknowledgment.

Use `python scripts/performance-compare.py PHASE EXE` for the original four cases. Add `--cases tracks line-lyrics word-lyrics gpu-line-lyrics gpu-word-lyrics high-rate-tracks` for this six-case matrix. Explicit case runs retain other results only when their executable hash matches. For the isolated hot paths, use `cargo test --release --locked --bin undertow pcm_hot_path_benchmark -- --ignored --nocapture`. Sanitized evidence is in `benchmarks/v0.2.2`; raw local fixtures and reports stay under ignored artifacts. Account data and confidential credentials are excluded. The [credential audit](PERFORMANCE-0.2.0.md) and [0.2.1 navigation behavior](PERFORMANCE-0.2.1.md) remain applicable.
