# 0.2.0 performance and credential audit — 13 November 2026

The main change is demand-driven lyric repainting. Audio playback alone no longer forces display-rate repainting of settled lines. Line timestamps wake the UI, scroll springs and fades remain animated, and true word fills still run continuously. A boundary-aware first spring step avoids jumping after an idle interval. Background refresh, audio, Discord and floating lyrics remain serviced.

Installed system fonts now use read-only file mappings while a retained Windows handle denies writing/deletion. All original font bytes, faces, fallback order and language coverage are retained. An owned-byte fallback remains if mapping/locking is unavailable. Custom user fonts are unchanged. This is not working-set trimming or omitted glyph coverage.

## Matched process measurements

Each workload ran for 18 seconds in a hidden 1600×1000 window, with original generated audio/art/lyrics, muted output and isolated account-free settings. The requested rate is 120 fps with display sync enabled; the actual display rate can differ and actual UI-frame counts are retained. CPU is relative to one logical core, MB is decimal, memory is the peak process value during the pass. No compiler was running during the final comparison.

| Workload | CPU before → final | Working set MB before → final | Private MB before → final | UI frames before → final |
|---|---:|---:|---:|---:|
| tracks | 3.14% → 3.07% | 78.0 → 53.2 | 55.8 → 38.4 | 131 → 135 |
| line-lyrics | 22.73% → 6.68% | 83.0 → 65.8 | 68.5 → 51.8 | 2742 → 234 |
| word-lyrics | 23.01% → 23.38% | 85.6 → 63.3 | 70.6 → 48.6 | 2747 → 2741 |
| gpu-line-lyrics | 17.14% → 3.19% | 205.1 → 118.9 | 334.0 → 219.3 | 2942 → 273 |

All before/final workloads reported zero audio underrun samples and consumed roughly 18 seconds of audio. OpenGL line-lyrics CPU fell 81.4% in this pass; working set fell 42.0% and private memory 34.4%. Continuous word-fill CPU did not improve; its frame count remains essentially unchanged and that result is retained. These are combined changes and short observations, not a universal CPU/memory guarantee or a controlled attribution of each saved byte.

## Per-process GPU measurements

Windows counters sampled only the test PID at one-second intervals: mean summed engine utilization **7.769% → 0.745%** over 14 samples per pass (90.4% lower in this workload). This sums that process's engine instances; it is not overall GPU utilization. Dedicated GPU allocation stayed approximately 116.9 → 115.9 MB, with shared allocation 2.7 → 9.1 MB. Reduced rendering activity should not be confused with freeing the graphics driver's retained memory.

The GPU baseline used a separate identical hidden OpenGL pass. Other applications, clocks, driver state and scheduling affect results. A hidden context does not establish visible presentation latency or 120 Hz frame pacing. The GPU build still exceeds 90 MB of process memory on this system; software-only memory figures do not represent the GPU renderer.

## Verification and reproduction

104 Rust app tests and five renderer tests pass, with the explicit conversion benchmark intentionally ignored; three Python release-privacy tests pass, including optimized Python execution. Formatting, strict all-feature Clippy, desktop/broker release builds and dependency audit pass. RustSec retains the informational ttf-parser maintenance warning; no known vulnerability findings were reported. Font byte equality/immutability, installed-script glyph coverage, word timing, scroll spring behavior and diagnostic redaction have regression checks.

Use scripts/performance-compare.py PHASE EXE, optionally --only-gpu, for generated hidden workloads. Results and isolated fixtures go to ignored artifacts/performance. GPU counter access may require suitable Windows permissions. For the baseline, use tag v0.1.1 plus benchmarks/v0.2.0/baseline-instrumentation.patch; it only enables hidden OpenGL and the UI-frame count and changes no performance behavior.

Diagnostics omit private text settings while local settings and DPAPI sessions remain intact. Current desktop source/configuration contains no confidential developer client secret field. The optional broker receives its secret only from the server environment. Publication guards reject configured secret values and credential files. No configured secret environment value was available to compare by exact value, so the audit cannot certify an unknown historical key; see SECURITY.md for that boundary. Live TIDAL sign-in/playlists/provider/Discord delivery and exclusive audio hardware were not revalidated by these isolated fixtures.
