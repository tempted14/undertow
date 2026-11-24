# Performance evidence

## 0.3.5 playback and software rendering

The audio callback publishes both clocks with single-writer stores. The decoder attaches the starting sample to each packet, and the timeline follows the latest seek until the first sample from that seek plays. An isolated release-mode comparison of 8,192,000 clock updates per run took **11.37 ms in 0.3.4 and 3.32 ms in 0.3.5** (median of nine alternating runs). The comparison covers clock updates only. Run the equivalent in-tree benchmark with `cargo test --release --bin undertow pcm_hot_path_benchmark -- --ignored --nocapture`.

On Windows, the software renderer updates changed tiles in a retained backbuffer. A pixel comparison covers unchanged frames, moved and removed elements, partial edge tiles, and resize. An unchanged frame copies zero pixels into the backbuffer, and the renderer still presents the full surface so Windows can repaint exposed areas.

One paired pass with 18 seconds of muted fixture playback at 1600×1000 used the same software renderer, audio files, and view settings for each version. CPU is relative to one logical core. Memory is peak working set and peak private bytes in decimal MB.

| Workload | Average CPU, 0.3.4 → 0.3.5 | Working set MB | Private MB |
|---|---:|---:|---:|
| Tracks, 44.1 kHz | 3.31% → 3.04% | 61.77 → 62.91 | 39.28 → 39.76 |
| Lyrics, 44.1 kHz | 6.50% → 5.90% | 64.32 → 64.43 | 48.66 → 48.70 |
| Tracks, 192 kHz | 2.90% → 3.41% | 58.27 → 57.81 | 41.26 → 40.16 |

All six runs reported zero underrun samples. These short whole-process runs include scheduling and background-load variation. The figures cover local software-renderer playback only. Use `scripts/performance-compare.py` with the `tracks`, `line-lyrics`, and `high-rate-tracks` cases to repeat them.

## 13 November 2026 measurements

### Audio conversion

The decoder previously allocated a conversion buffer for every packet and copied the result into a second buffer. It now keeps one canonical interleaved buffer, grows only when required, and preserves the original sample conversion/channel order. A sample-for-sample regression test covers packet sizes and channel changes.

An optimized standalone harness compared the exact old algorithm with the production reusable buffer. For 10,000 packets of 4096-frame stereo f32 audio, nine alternating-order batches produced median conversion times of **51.802 ms before and 44.6602 ms after**: **13.8% less time / 1.16× throughput**. Conversion buffer allocations fell from **10,000 to one**, and the redundant 32,768-byte packet store/copy was removed. These are conversion-path results, not a 13.8% whole-player improvement.

Reproduce the included explicit microbenchmark:

    cargo test --release --bin undertow audio::buffer::tests::conversion_benchmark -- --ignored --nocapture

### Hidden playback samples

Four identical eight-second workloads used generated local PCM/audio/art/lyrics, a 1,200-track fixture, muted output, an invisible software-rendering window and a 30 fps cap. The baseline is the previous 0.1.0 executable. Peaks and CPU averages from one pass per workload are below; MB means decimal bytes / 1,000,000. CPU is relative to one logical core.

| Workload | Peak working set MB, before → after | Peak private MB, before → after | Average CPU, before → after |
|---|---:|---:|---:|
| tracks | 78.85 → 69.52 | 55.90 → 54.60 | 6.01% → 4.66% |
| filtered | 70.32 → 70.00 | 55.66 → 56.13 | 4.27% → 4.85% |
| small | 47.04 → 47.66 | 32.34 → 32.85 | 3.11% → 2.72% |
| lyrics | 85.58 → 85.56 | 70.32 → 70.08 | 37.16% → 32.49% |

Both passes reported **zero audio underrun samples**. Filtered-view CPU increased in this short sample; this result is retained rather than excluded. Scheduling, allocator and background-load variance make these directional observations, not statistically established whole-app gains. Measurements precede the final queued-library-event guard, About version label and compiler path-remapping changes. Those edits leave the measured decoder/rendering workload unchanged, but these figures are not a repeated measurement of the final release binary. Measured hashes are retained in the JSON evidence.

These samples do not measure visible GPU/DWM memory, 120 Hz frame pacing, live TIDAL bandwidth, startup latency, long-session memory growth or all plugins enabled together. They do not establish a universal 90 MB ceiling. No working-set trimming or disabled product feature is used to obtain these numbers. Use scripts/benchmark.ps1 with an actual audio file for longer, machine-specific measurements.

### Bounded work and responsiveness

- Album metadata retains 32 recent pages; browsing 5,000 fixture IDs verifies eviction and reload behavior.
- Library refresh, favorites and collection-state reads use a lazy independent lane. Mock HTTP tests hold a refresh while playback requests proceed, then verify that stale refresh results cannot overwrite later edits. A separate queued-event regression covers results already waiting for the UI before invalidation.
- Existing bounded streaming PCM/spool queues, artwork caches, account cancellation, search debouncing, native font fallback, renderer selection and plugin controls remain intact.
- Public evidence contains aggregate numbers and original fixtures only. Runtime account data and machine-specific reports are excluded.

See the JSON files in ../benchmarks for exact values and the retained dependency scan. The security report is a point-in-time result, not a guarantee against unknown advisories.
