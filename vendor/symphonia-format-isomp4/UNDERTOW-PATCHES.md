Local modifications to Symphonia's ISO/MP4 demuxer 0.5.5, 27 October 2026:

- Accept a seekable growing media source with an unknown final byte count.
- Stop initial atom scanning when fragmented media begins; discover later segments as playback or seeking needs them.
- Preserve the upstream sample index, codec checks, and seek path.

This removes Undertow's old full-download startup barrier. Source files retain the original MPL-2.0 notices. The full component source and LICENSE are included in the source distribution.
