Local changes to upstream egui_software_backend 0.0.2 (MIT OR Apache-2.0):

- Honor delayed repaint deadlines instead of requesting redraw immediately for every callback.
- Add a configurable frame-rate ceiling without sleeping in the UI thread.
- Support the standard StartDrag command on the frameless Windows title bar.
- Return actual framebuffer screenshots through egui's screenshot event.
- Call the application exit hook for a programmatic viewport close.

The upstream raster algorithms are retained. The upstream licenses and source are retained. Optional floating lyrics use a small native child window, whose memory is included in Undertow's diagnostics.

27 October 2026: Frame cadence is now timed from the start of presentation work, so rendering time is included in the 120 Hz budget instead of being added after each interval. Active lyric animation requests the next frame immediately; the backend applies its deadline. Idle pages retain delayed repainting. Native borderless resizing and screenshot support remain enabled.

27 October 2026, display pacing update: input-triggered redraws now use the same scheduling path as animation redraws, preventing wheel events from bypassing the frame limiter. Windows optionally calls DwmFlush after presenting so animations follow the desktop compositor; if that API reports failure, timed pacing remains available. Monitor refresh is sampled periodically and supplied as egui's predicted frame interval. No audio or network thread waits for presentation. Non-Windows builds retain timed pacing. No rasterizer or upstream licensing change is made.

27 October 2026, parallel drawing: enable the upstream rayon feature to parallelize primitive rasterization and dirty-tile updates. Initialize the global render pool once, capped at four workers with 256 KiB stacks; no per-frame thread creation. The upstream raster algorithms and caching behavior remain in place.


## Cache and presentation work reduction (30 October 2026)

- Check geometry/clip/scale/order/texture revision cache keys before cloning, scaling and orienting meshes; unchanged meshes reuse their raster cache.
- Include texture pixel revisions and full texture identity in cache keys. Refresh the cached zero-UV sample after partial texture updates.
- Invalidate and rebuild dirty canvas tiles for removal-only and ordering changes, while avoiding composition on entirely unchanged frames.
- Use binary search on row-ordered occupied tile lists during composition instead of scanning the complete list for every tile.
- Add `copy_canvas_to_buffer` for native presentation to an otherwise cleared surface. A single copy replaces the redundant destination clear plus full-frame alpha blend. The existing alpha-over-buffer API remains available.
- Regression tests in Undertow's `tests/renderer_cache.rs` compare replacement presentation against the original blend semantics and cached sequences against fresh renders, including partial texture updates, transparency/additive colors, clipping, movement, transposed geometry, reordering, removal and resize. Cache-key fields are mixed in order so x/y permutations cannot reuse a stale raster.


## Compact memory and responsive input (31 October 2026)

Pixel-aligned constant-color rectangles are cached as color/bounds, including alpha/additive fills, instead of retaining full-size bitmaps. Other geometry retains the raster path. Tests compare this path with direct raster output and verify no raster buffer remains for solid backgrounds. Canvas buffers release excess capacity after substantial downsizing. Retained canvas/raster/texture allocations are exposed to application diagnostics.

The Windows message/input thread no longer calls DwmFlush. Display-synchronized frame requests use a monitor-refresh deadline through the event loop; Windows DWM handles final composition. This avoids blocking input while waiting for presentation. Raster workers are capped at two. Invisible diagnostic windows receive explicit renderer ticks because Windows does not generate WM_PAINT for them; these windows are never shown or activated. Foreground presentation remains a separate verification concern.
