# Feature reference

## Features

- Live search previews for artists, albums, tracks, and playlists. Press Enter for category filters and sorting by title, artist, or year.
- Artist pages load their portrait/header immediately, then playable top tracks while albums and biography finish. Independent bounded catalog and discovery read lanes start only when needed, keep artist/album/search/credits requests off the playback/edit worker, and share the existing HTTP connection pool. Queued obsolete searches are skipped, delayed account results are rejected after connection changes, and a six-artist cache evicts the least recently visited page instead of clearing every page at once. Search previews debounce for 180 ms. No extra playback process or browser engine is added.
- Larger 64-pixel track rows with individual play/pause, artwork, explicit/HiRes badges, metadata columns, add-to-playlist and favorites. Native album pages use the supplied black album layout with cover, artist, year, duration, Play/Shuffle, Add, Credits, Share, Report, and More.
- Persistent reference-style navigation with expanded/compact modes, playlist covers, a playback indicator, and Ctrl+B to collapse or expand.
- Upload has been removed from navigation; local playback still works through file drop and Settings. Playlist creation/editing uses a large native modal with a cover column, rounded title/description fields, a Unicode-aware 500-character description counter, visibility switch and disabled-empty Save. Saving reports service errors without dropping the draft; partial visibility/image failures are reported after the playlist text succeeds. The image chooser saves a bounded cover **on this device only**, including the sidebar and playlist page; it does not upload an image to TIDAL. Visibility changes use the connected account's playlist routes and remain subject to service authorization.
- The player queue button toggles a sliding right-side Play queue panel with bounded recent history, playing source/current track, upcoming songs, context menus, removal and Clear. History collapses automatically in short windows and can be expanded; Saved for Later remains available from the header. Closing the panel keeps the current page. All app notifications share the supplied rounded teal/charcoal toast and outlined close target; native input fields use neutral rounded borders. Captions reflow beside the queue and above a visible toast.
- Centered transport controls reflow into two rows below 1200 points; queue, lyrics, volume, floating lyrics, favorites and music information stay available. Shared geometry keeps painted buttons and click targets aligned. Animation follows the display refresh rate or a configurable 30–240 Hz limit, with frame-rate-independent lyric springs and eased page/card/tab transitions.
- Borderless native edge/corner resizing, a visible corner grip, responsive Home grids, and a sidebar that temporarily uses its compact rail below 1100 points without changing the saved preference. Album/playlist secondary actions move below Play/Shuffle when needed. Scroll areas, notices and lyric captions share the player height; wide track tables retain readable columns through horizontal scrolling.
- The minimum window is 640×480. Resize targets cover 12px edges and 26px corners, with immediate press handling; page scrollbars sit inside the edge strip. In narrow windows the transport shifts left of the auxiliary buttons and the time readout follows it. Player title, artist and source use fixed rows aligned to the cover, keeping the bottom source label visible.
- The optional caption yields while a dialog is open or the area above the player is shorter than 360px. It returns automatically when space is available; full and floating lyrics remain available.
- Load saved tracks and owned/followed playlists, and sync every minute or on demand. Staff Picks and Uploads tabs have been removed from Home.
- Keep visible artwork and the active backdrop pinned while inactive textures use an 8 MiB cache, preventing both cover/placeholder swaps and accumulation of old Hi-Res covers. Song thumbnails use 80/160-pixel sources where appropriate; playlist mosaics fetch their supported source and decode to the required physical thumbnail size; larger covers retain 320/640-pixel sources with display scaling taken into account. A small offscreen lookahead has lower priority than visible covers.
- Create, rename, describe, delete, add/remove tracks, and reorder playlists. The legacy backend carries playlist revisions; conflicts require resync. Stable item identifiers are used by the public API. Video positions are preserved when editing mixed playlists.
- Decode raw FLAC and FLAC in fragmented MP4/DASH with Symphonia. Only decoded codec headers establish lossless status. Configurable strict mode refuses lossy TIDAL fallbacks. FLAC is decoded to floating-point PCM without a lossy encoding stage.
- Bounded PCM queue (256 KiB) and a temporary disk spool (maximum 512 MiB per track). No complete decoded song is retained. Fragmented MP4 starts from its initialization data and first available audio fragment. Later fragments continue downloading to the bounded disk spool; seeking uses the same sample index and cached bytes.
- Queue, shuffle, previous/next, seeking, volume, and local FLAC/WAV/MP3/AAC playback. Drop an audio file or paste its path. Adjacent `.lrc`, `.jpg`, and `.png` files are supported.
- A shared lyrics engine for synced lyrics, genuine word timestamps with continuous word fill, copy-line/full-lyrics, original/translation toggles, romanization, color/glow, responsive spring following, a player caption, and an always-on-top native window. Seeking retargets immediately; a timing offset and animation speed are configurable.
- Lyrics automatically use installed Windows fallback fonts when the chosen font lacks characters. Complex scripts use DirectWrite for joined letters, combining marks and bidirectional text, including Punjabi, Hindi, Arabic, Urdu, Hebrew and Southeast Asian scripts. Whole-line shaping preserves word highlighting and word seeking. The caption and native floating window also support this fallback. Only visible lines receive cached masks (8 MiB/24-entry cache); ordinary supported text keeps the existing font atlas and custom font. This changes text display only. Rare scripts still require a suitable Windows font; Undertow does not bundle or download every language font.
- Collection filters have a clear button, Escape-to-clear, a match count and a recoverable empty state. Artist and album names in track lists open inside Undertow with hand cursors. Search tabs show their own result count and return to the top when the selection changes.
- Home uses TIDAL’s personalized feed: real shortcuts, recommended albums/new tracks, recently played, custom mixes, editorial playlists, and listening-based suggestions. View all fetches that section instead of opening My Tracks; albums, artists, playlists, and mixes open inside Undertow. A manual Refresh and a 15-minute in-memory refresh interval keep requests controlled. If the personalized endpoint is unavailable, compatible connections can show the service’s editorial discovery feed with its real labels. No saved-library content is relabeled as recommendations. Only visible artwork is requested. Feed metadata is bounded to 24 sections/768 items, with four expanded pages and 16 opened list headers cached; account changes discard the feed. Search result order is reused between frames. Collection filtering uses a lazy, bounded text index (up to 2 MiB of normalized text/4,096 rows); larger collections remain fully searchable and sortable. Source refreshes invalidate derived views without retaining the old track list.
- Artist pages recover missing detail-endpoint portraits using TIDAL artist search matched by exact ID. Small search hints are shared between read lanes and capped at 64 artists. The taller portrait header, side atmosphere and lower fade follow the reference; fan counts appear when the service supplies them.
- Artwork colors and a cached low-resolution frosted backdrop. Visibility, typeface, effect strength, and frame cap are adjustable. No full-screen video or continuously recomputed Gaussian blur is used.
- Lyrics has Resync and Hide UI icons beside its tabs. Resync reloads lyric timing and returns following to the current playhead without seeking/restarting audio; failed refreshes keep the existing text. Hide UI fades/slides the bottom controls away and expands the lyric area. Use the remaining eye icon or Esc to restore controls. The native window title/resize controls stay available. `--hide-lyrics-ui` starts this view with its controls hidden.
- The player spans the complete bottom edge without side/bottom gaps. Settings → General → Player background → Blur adds a frosted artwork layer beneath the bar, reusing the cached 48px blur rather than capturing/blurring the desktop or page every frame. Color switches this effect off.
- Discord Rich Presence: public app ID configured once for all users, a neutral Music activity name, song/artist, album artwork, progress and pause status. The compact Discord status uses the song title. Quality labels and the extra TIDAL button are omitted. The client checks Discord's IPC acknowledgment and handles disconnects. Discord desktop must be running; no Discord token or password is used.
- Sequential download queue with stop-after-current, lossless checks, and native FLAC metadata writing. Offline requests depend on TIDAL entitlement. Unsupported or denied downloads fail explicitly; arbitrary DRM bypass is not implemented.

See [PLUGIN-FEATURES.md](PLUGIN-FEATURES.md) for all sixteen source plugins and the exact scope of each native equivalent. Some upstream features depend on private services or unsupported protections and are documented as limitations.

## Settings and profile

Click the avatar to open **Profile**, **Settings**, and **Log Out**. The same menu appears on playlist pages. Explore has been removed; the search box still searches the TIDAL catalog. Settings uses the reference's large grouped cards and switches, with General, Account, Plugins, and About tabs. Only functional player options are presented; normalization, autoplay recommendations, tray/startup and other unsupported TIDAL settings are not simulated.

Profile lets you choose/remove a PNG or JPEG and set an Undertow display name. Both are **local to this device**, not uploaded to your TIDAL profile. The native chooser runs away from the input thread; pictures are decoded with input limits, center-cropped to 256px and persisted as a small PNG. Avatar rendering uses the existing texture in circle geometry, without another masked bitmap. Native profile upload/synchronization is not implemented.

Plugins consolidates the sixteen requested behaviors: lyric glow/color/motion blur, floating lyrics and captions, provider options, copy/translate controls, total duration, fonts, artwork actions, downloader, NoBuffer, RealMAX, tags, cover theme and visibility. Custom fonts can be enabled/disabled without losing their path, selected with a Windows file chooser, applied or reset. Invalid fonts are rejected before reaching the renderer. Existing provider/offline limitations still apply.

Hover the player volume icon to reveal the slider. The popup stays open while crossing the gap or dragging; leaving closes it after a short grace period. Click pins/unpins it; right-click still mutes. General settings adds animation speed, and Plugins adds a lyric timing offset shared by the focused, caption and floating lyric views. Motion follows the display and supports reduced motion.

Account offers lossless and quick-code sign-in directly. Lossless still requires approving on TIDAL's actual website and returning its final address once; **Paste sign-in link** focuses the private field and requests a paste only when clicked, then submits automatically. Quick-code approval returns automatically but its client may be limited to lossy playback. Session refresh and DPAPI storage are unchanged. An embedded browser/password form is not included: browser approval follows native OAuth guidance (https://www.rfc-editor.org/rfc/rfc8252.html#section-8.12) and avoids a resident web engine. Current verification scope is recorded in VALIDATION.md.

## Playback quality

The format badge shows actual decoded codec, bit depth when available, and source sample rate. Click it for the signal path. Windows shared audio can resample to the device's configured rate. Exclusive WASAPI / bit-perfect device output is not implemented; source-lossless playback must not be confused with exclusive output.

## Memory and validation

The design target is **around 90 MB** during playback for the documented software workloads. Reports include a separate 90,000,000-byte target result and retain the older 200-MB soft target / 256-MB visual headroom fields for comparison with previous measurements. Reports retain separate results for the original target and the headroom budget, using both untrimmed process working set and private committed bytes. On Windows, floating lyrics now uses a small native window in the same process; it does not launch another copy of the player. Its memory is included in the main process. Older reports measured the separate helper and conservatively counted shared pages twice. There is no working-set trimming or hidden browser/audio helper process. GPU/DWM allocations are separate and are not represented as process RAM.

The native desktop renderer uses OpenGL with VSync for visible playback. `--software` selects the cached CPU backend; invisible `--background-benchmark` diagnostics use that backend because there is no visible presentation surface. The earlier around-90-MB results apply to the documented software workloads, not a guarantee for GPU driver allocations or arbitrary window/DPI/font combinations. Other Rust frameworks were researched; see [RESEARCH.md](RESEARCH.md).

Use `scripts/benchmark.ps1` on the release executable with an actual audio file. Diagnostics clearly identify local versus TIDAL audio, active integrations, underruns, source format, and measured peaks. A local synthetic workload is not proof of the full authenticated, all-integrations scenario. See `VALIDATION.md` for the results and remaining live checks.

After signing in, `--autoplay-favorites` starts the first saved track automatically. For explicit live diagnostics, `Undertow.exe --validate-live RESULT.json` checks the stored session, collection reads, a real stream, seeking, lyrics/translation, and offline-manifest authorization. Add `--playlist-roundtrip` to create, edit, add/reorder/remove items in, and delete one uniquely named temporary validation playlist. The report records any cleanup failure; existing playlists are never used for mutation tests. These diagnostics do not contain tokens or account identity.

## Keyboard

| Shortcut | Action |
|---|---|
| Space | Play / pause |
| Ctrl+L | Lyrics |
| Ctrl+B | Collapse / expand sidebar |
| Ctrl+, | Settings, even with the sidebar hidden |
| Ctrl+Left / Ctrl+Right | Previous / next |
| Double-click track | Play |
| Right-click track | Queue, save, playlist edits, cover, download |

## Build

The native track and album menus include play now/next, adding tracks to editable playlists, collection actions, artist search, link sharing, audio downloads and cover-art actions. Album artwork reveals animated play and collection buttons; playlist shortcuts reveal play controls. Click the audio-quality badge for the expandable music-information panel. The redundant music-note control has been removed from the player.

Save for Later is a local list, available under Queue; it does not sync to a TIDAL server-side save-for-later service. Downloads still depend on the account's offline entitlement. Album collection state is loaded from TIDAL, and hearts update only after a successful service response.

Page entrances, sidebar collapse, hover controls and menus use short transitions. Focused lyrics have a cached cover-colored backdrop, soft active-line glow, optional inactive-line blur, and up to 640-pixel cover artwork. Motion, blur, glow and atmosphere remain configurable in Settings. Animation settings are a refresh-rate target; measured frame pacing is recorded separately.

```powershell
cargo build --release --locked --bin undertow
cargo test --locked --bin undertow
cargo clippy --locked --all-features --bins -- -D warnings
```

Builds were made with Rust 1.99.1 on Windows x64. Dependency versions are locked. The optional broker uses `cargo build --release --locked --features auth-broker --bin undertow-auth-broker`.

`UNDERTOW_DATA_DIR` overrides the normal `%LOCALAPPDATA%\Undertow\Undertow\data` location; the exact platform directory is selected by the `directories` crate. Settings and artwork/lyrics caches live there. Tokens are DPAPI ciphertext. Sign out to remove the locally stored session. Disconnecting an app at TIDAL revokes service access separately.

No account passwords are collected. No analytics or crash-reporting service is included. Lyrics lookup sends track metadata to the selected providers; clicking Translate sends lyric text to Google's unofficial translation endpoint. These external services can be unavailable or rate limited.

## Attribution

Behavior and protocol research: TidaLuna, the sixteen plugin authors, Lumitide, TIDAL's SDK/API sources, LRCLIB, and Discord's documentation. The legacy key-unwrapping compatibility algorithm follows Lumitide's MIT-licensed description. Upstream plugin JavaScript is not embedded or executed. The software renderer is a small documented local fork; upstream licenses and sources are retained. See `THIRD-PARTY.md` and the generated dependency license collection.

The now-playing header is right-aligned and switches between Similar tracks, Credits and Lyrics using white selected pills. Credits and track radio are fetched from TIDAL; the removed Translate and three-dot header buttons are replaced by translation/copy controls in Settings. The quality badge retains music information. Saved Tracks uses the larger catalog layout, a filter, sorting and real added dates. Hovering the progress bar displays the target seek time.

On Windows, Synchronize animations to the display schedules frames at the monitor refresh interval and lets DWM composite them. It no longer waits for DwmFlush on the mouse/input thread. Turn it off in Settings to use a fixed frame-rate limit. The optional OpenGL renderer also explicitly enables VSync. Homepage wheel movement retains fractional pixels and has a native draggable scrollbar. Idle pages still use delayed repainting; active scrolling and transitions request frames promptly.

Native artist pages open from artist search results, album artist names, playlist artist cells, the player artist name and Go to artist menus. The reference layout has a centered photo with blurred side fill and a black fade, circular Play/Follow/Share/Report/More actions, All / Music / About tabs, and top-track title/artist/album/time columns. Albums and EP & Singles have animated carousels and View all grids. Preview shelves group identically named same-year editions; View all retains every returned edition. Top tracks retain playback, track menus, playlist insertion and favorites. Artist follow/unfollow uses the real account collection routes; fan counts are shown only when the provider supplies them. The artist cache holds six profiles and shares its track list with playback instead of copying it during redraws. Only visible release artwork is painted/requested. A missing identifier in an older cache falls back to artist search inside Undertow. Release sections show up to the latest 50 albums and 50 singles/EPs per artist.


Rendering reuses unchanged mesh data before preparing a new raster buffer, composes only invalidated tiles, and presents the finished software canvas with one copy. Floating lyrics publish snapshots only when the text or color changes; paused playback uses a slower maintenance timer. Animation settings and audio decoding quality are unchanged by these performance improvements.


The installed Segoe UI styles and CJK fallback share process-lifetime font buffers instead of being duplicated by the font parser. Flat pixel-aligned backgrounds retain a color and bounds rather than full-resolution raster bitmaps; gradients, artwork, blur and glow retain their original rendering. Oversized canvas allocations are released after substantial downsizing. Native drawing is limited to two raster workers to reduce contention with input and other applications.

For unattended diagnostics, `scripts/benchmark.ps1 -Background -Mute ...` creates only invisible, non-activating native windows. This mode uses the actual audio device/decoder and software surface, skips the normal single-instance guard, and does not save transient benchmark settings. It cannot validate visible presentation/input behavior; its reports identify it explicitly. Normal startup never uses this mode.

Fixed-size TIDAL image URLs are resized after decoding to the display's required thumbnail size, even when the remote URL always serves 640–750px images. Full original image files remain in the disk cache for larger views and cover-art downloads. The 8 MiB inactive texture budget is soft when the currently visible page requires more; visible content is never discarded to force a benchmark number.


## Artwork scrolling and responsive lyrics (1 November 2026)

Artwork now has an independent disk-cache lane and two bounded CDN workers. Each frame submits only the current viewport and a small lookahead; obsolete queued requests are removed, visible/player images have priority, and failed requests retry after a cooldown instead of remaining blocked forever. Completed images are uploaded in batches of at most six per frame. Cached higher/lower-size art remains visible while a resized image loads.

A validated, versioned RGBA thumbnail cache avoids repeatedly decoding full JPEGs when scrolling back. Original cover files remain available for full views/downloads; decoded files use one quarter of the artwork disk budget and originals three quarters, pruned in batches away from the input thread. Inactive in-memory art has an 8 MiB soft budget and a 256-entry cap; visible art stays pinned. This modest increase from 4 MiB is deliberate to reduce churn. Cold covers still depend on TIDAL/CDN/network availability.

Focused lyrics use the newer reference proportions, bright white default text, a soft neutral glow, brighter previously sung lines, blurred upcoming lines and a richer cached backdrop. Custom text color and optional cover-colored glow remain available in Plugins. Word timing, motion, translation, timing offset and fonts remain functional. Text scales down in narrow windows and its layout is recalculated on resize; the redundant compact lyric caption is shown in other views. Catalog canvases track actual content height and retain scroll access below headers instead of clipping at a fixed rectangle. Favorites uses the same responsive page canvas.

For non-interactive diagnostics, `scripts/benchmark.ps1 -Background -Mute -Playlist chromatic -ArtworkScroll` exercises a deterministic forward/backward row sequence in an invisible native window. It records eligible visible covers, completion latency per step, disk/CDN counts and audio/resource measurements without injecting OS input or changing playlists. These measurements do not certify visible GPU presentation or every possible resize/monitor/driver configuration.


Songs that provide plain lyrics without timestamps stay bright and can be read with the wheel or scrollbar. They do not get invented active lines/word timing, inactive-line blur, or continuous playback-driven animation. Genuine timed lyrics retain automatic following, white active text and optional glow/blur. Lyric font sizes are rounded to half-points while resizing to avoid retaining a new raster font size for every pixel of a drag.

Performance changes on 4 November 2026 retain the existing effects, controls and playback pipeline: visible lyric layouts are reused across color/glow changes with bounded retention, favorite membership uses an updated index, artist track queues share their storage, and the software renderer uses an ordered 64-bit geometry fingerprint with less per-field mixing. Minimized windows keep audio, provider events, settings writes and floating lyrics serviced while skipping construction of the hidden main interface. Normal visible rendering continues to use GPU/VSync; software diagnostics and the optional CPU renderer retain their existing two-worker limit. See VALIDATION.md for measured scope and limits.

Synced lyrics can be browsed with the mouse wheel, touchpad or scrollbar. Scrolling holds your reading position while audio and highlighting continue. Resync returns to the current sung line and resumes automatic following immediately, without seeking audio or fetching lyrics again; when already following, it retains the existing lyric-refresh action. A new track starts in follow mode.

The player keeps its like and More actions beside the song metadata, aligned with the artist row. The group follows the text width with a bounded title area and stays clear of playback controls as the window resizes.
