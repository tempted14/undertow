# Desktop redesign validation

Undertow’s desktop reference is TIDAL 2.48.0 on Windows. The updated native UI uses the reference palette, spacing, and authorized Nationale typefaces. Undertow’s app name, available navigation, and plugin settings remain part of the interface. The queue and lyric interactions were reviewed against [TIDAL’s desktop queue guide](https://support.tidal.com/hc/en-us/articles/360004182777-Play-Queue) and [lyrics guide](https://support.tidal.com/hc/en-us/articles/4404289436433-View-lyrics). Rendered verification captures use generated local audio and artwork.

- **Navigation and Home.** The library rail collapses at narrow effective widths. Discovery uses a more compact shortcut grid, responsive artwork shelves, and static loading placeholders. The sidebar keeps a shared playlist snapshot and lays out visible rows only.
- **Catalog.** Search results and preview share the new control palette. Artist pages bring tracks closer to the portrait header. Albums and playlists use smaller headers in short windows, and playlist background tint follows the existing setting.
- **Playback.** Transport, queue and lyrics share the design system. The seek rail has separate elapsed and total labels, hover time, and a wider hit target. The smaller stacked player protects space above it. The interface continues to use native Rust, egui and the existing renderers.

## Earlier playback measurements

These measurements compare 0.2.2 to the earlier native layout, before the TIDAL 2.48.0 visual alignment. Matched 18-second hidden Windows runs at 1600×1000 used generated audio, muted playback and isolated data. The local settings enabled lyric glow, blur, cover tint, inline and floating lyrics. Discord presence was disabled. CPU is the percentage of one logical core. Memory uses decimal MB.

| Workload | 0.2.2 working set | Redesign working set | Redesign private commit | CPU before → after |
| --- | ---: | ---: | ---: | ---: |
| Library with 800 playlists, software | 61.2 MB | 53.4 MB | 38.5 MB | 4.88% → 2.76% |
| Active lyrics, software | 66.6 MB | 67.1 MB | 52.0 MB | 20.02% → 17.15% |
| Active lyrics, OpenGL | 116.0 MB | 115.8 MB | 219.5 MB | 8.69% → 8.97% |

All six runs reported zero steady playback underruns. The library memory reduction also appeared in the earlier comparison. Process CPU readings varied between runs. The sampled OpenGL GPU engine mean was about 1.20% in both runs. All tested working sets met the 200 MB goal. OpenGL exceeded 200 MB of private committed memory. Live TIDAL services and a matched run of the official TIDAL player were outside the benchmark.

## Verification

The Rust suite passed 119 tests and the Python release privacy suite passed three checks. Formatting, strict Clippy, source hygiene and the path-remapped Windows build passed. The transport layout test checks that the seek area and other control regions stay separate over the supported width range. The visual review used native captures at desktop size and at simulated 150% and 200% effective scale. Changing DPI by moving the window between physical monitors was outside the capture run.

On Windows, compare the two builds with `python scripts/benchmark-desktop.py BASELINE_EXE CANDIDATE_EXE --output artifacts/desktop-benchmark`. Generate screenshots with `python scripts/ui-smoke.py --sidebar --size 1600x1000` and add `--scale 1.5` or `--scale 2` for effective scale checks on the redesign build. These commands use hidden windows and isolated settings.
