# Compact playback control refinement

Music Compact uses dedicated white Play/Pause SVG glyphs in a 14 logical px
rectangle. They have no circular container, border or background. The existing
circular Expanded assets are unchanged. Play is optically offset within the
same rectangle; neither glyph changes layout dimensions between playback states.

`CompactMusicLayout` now allocates the row from its content edges. The 24×24 px
playback hitbox is reserved first, with 12 px between its right edge and the
notch body's inner edge. An 8 px gap separates the timeline and control. Artwork
remains 23×23 px with its existing rounded corners and 8 px metadata gap.
Metadata keeps its existing 79 px allocation at the normal size, and shrinks
before the timeline's 96 px preferred minimum at narrower diagnostic widths.
Existing shaped-text measurement truncates long titles/artists to that allocation.

At the unchanged 345×42 px Compact size, the timeline occupies x=144..288,
the hitbox x=296..320, and the visual icon x=301..315. Both the content clip and
rounded silhouette contain the complete control. The waveform's hitbox and
painting use the same allocation, retaining fixed pointer-to-time mapping.
Expanded layout, waveform rendering, audio analysis, orb and notch geometry are
unchanged. Dimensions are logical pixels; SVG rendering follows iced DPI scaling.

Canvas-local feedback changes icon opacity from 0.78 at rest to 1.0 on hover
through a 90 ms smooth transition. Pressed brightness is 0.72 of the current
brightness; unsupported/busy controls use 0.28. There is no scaling or permanent
background, and no ongoing animation subscription for the button. Local redraws
stop when the hover transition finishes. Reduced motion snaps the transition.

A press inside the control captures the event. A release inside the same enabled
control publishes exactly one existing `MediaCommand::PlayPause`, also captured.
Dragging away, losing focus, changing session, or losing capability cancels the
gesture. Disabled presses are consumed without expanding the notch. Other
Compact surface clicks retain expansion; waveform gestures retain seeking.
The existing Apple Music command dispatcher, capability checks, generation
tracking, busy state and error handling are reused without changes.

Headless validation checks the fixed target and padding inside the curved
silhouette/content clip at widths 280, 300, 345 and 456; shared waveform bounds;
very long Unicode metadata; single release activation and capture; background
expansion; drag/focus cancellation; disabled controls; hover timing and reduced
motion. Formatting, compilation, strict Clippy and all 36 automated tests pass.
Physical desktop scale, monitor DPI and real Apple Music clicks remain
user acceptance; no playback or desktop automation is performed.

Changed files: `src/ui/music_layout.rs`, `src/ui/layout.rs`,
`src/ui/compact_playback.rs`, `src/ui/icons.rs`, `src/ui/mod.rs`,
`assets/icons/compact_play.svg`, `assets/icons/compact_pause.svg`,
`tests/compact_playback.rs`, and this note.
