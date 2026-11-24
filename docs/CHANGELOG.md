# Changelog

## 0.3.7 — 2026-11-24

- Use the lightweight native window renderer by default on Windows. It composites into the retained Windows buffer and avoids a separate full-window canvas. OpenGL with VSync remains available through `--gpu`.
- Smooth lyric skips with distance-based timing, a soft landing, and continuous movement when another tap moves in the same direction.
- Keep the play and pause icons the same size on hover. Put volume in a compact vertical popup above the speaker, with drag, arrow-key, and mute controls.

## 0.3.6 — 2026-11-24

- Redraw the player’s shuffle, previous, play, pause, next, and repeat icons. Preserve the button positions, actions, and repeat-one indicator.
- Compact sparse software-rendered layers by dropping unused transparent pixels from their cached rows.
- Release lyric glow and native text textures when leaving full-screen Lyrics. Collect performance history only during diagnostic sessions.

## 0.3.5 — 2026-11-23

- Reduce work in the audio callback by giving it sole ownership of the playback and consumed-sample clocks. The timeline shows the requested seek until matching audio arrives, and then advances sample by sample.
- Copy only changed pixels from the software canvas into the retained Windows backbuffer. New buffers and resized windows still receive a full frame.
- Cache renderer selection at launch to avoid repeatedly reading command-line arguments during rendering.

## 0.3.4 — 2026-11-22

- Remove word and syllable highlighting and their timing lookups. Lyrics highlight the whole line and retain click-to-seek, manual scrolling, and Resync.
- Use a soft glow around the active lyric line at every display scale.
- Add the Undertow wave icon to the Windows executable and taskbar.

## 0.3.3 — 2026-11-22

- Collection opens beside the sidebar with Mixes & Radio, Playlists, Albums, Tracks, and Save for Later. Playlists and saved albums have browsable artwork grids.
- Make sidebar and page transitions quicker, with mouse and keyboard navigation for the Collection flyout.
- Restore highlighting for genuinely timed syllables. Show line-timed lyrics first and replace them when the optional Radiant Lyrics provider supplies finer timing.
- Disclose AI-assisted development in the README.

## 0.3.2 — 2026-11-20

- Keep wrapped lyrics in the same position as karaoke highlighting advances. Start following a tapped lyric while the audio seek is still being processed, and let a new seek interrupt the scroll.
- Restore the wider Colorama Lyrics glow with optional cover-art tint. Reuse shaped text and combine adjacent word highlights to limit repeated drawing during playback.
- Click the song title in the player bar to open its album and scroll to its highlighted track. Album lists keep offscreen rows virtualized.

## 0.3.1 — 2026-11-20

- Show decoded artwork immediately and save its disk cache in the background. Keep a download worker available for visible artwork when prefetch requests are already running.
- Reuse cached artwork across display sizes and prefetch nearby covers as you scroll Home and Search.
- Keep hover highlights steady over artwork, links, and menus in Recently Played, track lists, the sidebar, and the queue. Keep hover animations independent when the same item appears in multiple shelves.
- Cover artwork delivery, download priority, and nested hover targets with regression tests.

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
