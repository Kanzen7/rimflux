# Part 3.1 — Integrated waveform lighting

The straight vertical playback marker and circular hover marker were removed.
`ui/waveform.rs` now places the playback light on the main ribbon's interpolated
centerline, at the exact normalized playback/seek position. A short feathered
streak, translucent halo strokes, and a 2.4 px curved highlight form the expanded
head. Compact uses localized waveform brightness without the prominent core.

## Visual changes

- Expanded: seven independently moving layers (main ribbon, secondary ribbon,
  cyan/blue/violet/magenta lines, and fine prism detail). Compact keeps three.
- Each layer combines three smooth oscillators, independent speed and phase,
  slow amplitude variation, and ribbon thickness variation. Stable track
  profiles and horizontal playback coordinates remain unchanged.
- Both horizontal ends use a 10% smoothstep alpha envelope, sampled into native
  gradient stops. Lighting uses the same envelope. No opaque fading rectangles
  or full-surface blur are used.
- Base glow uses translucent curved strokes. Local glow follows the strongest
  lobes in separate sections of the main ribbon.
- Sparse decorative flares travel along supporting curves, have no white
  playback core, and fade over short lifetimes. They are suppressed near the
  real playback head, reduced during seeking, and disabled while paused or in
  reduced-motion mode.
- All geometry and lighting use the existing logical 14/25 px visual scissor.
  Notch dimensions, metadata, artwork, duration labels, controls, and GSMTC
  session integration are unchanged.

## Interaction and lifetime

`modules/waveform_lighting.rs` owns a bounded eight-sample seek trail, pointer
direction, pointer-down impulse, and a 750 ms release envelope. The trail is
short in both distance and time and follows the prepared ribbon curve. A large
pointer jump clears old history rather than drawing an uncontrolled trail.

Hover adds a faint curved preview. Dragging strengthens the actual head and
briefly brightens nearby thin waves. Release still uses the existing
capability-checked media command and authoritative timeline reconciliation;
lighting never confirms or simulates a successful seek.

Paused playback freezes wave motion and dims the head. Transient seek feedback
may request frames briefly while paused, then stops. Reduced motion keeps direct
seek feedback but omits trails, animated release glow, and decorative motion.

Prepared point arrays are reused for paths, glow, and pulse interpolation in a
frame. Played and remaining regions share identical geometry. Small lighting
paths are reused across halo passes. No extra GUI, rendering, or audio
dependencies were added.

## Execution status

The user's standing instruction leaves validation and visual QA to them. No
Clippy, test suite, formatting check, screenshots, desktop input, or media-control
QA is performed for this revision. Source formatting is applied as an edit, and
the updated application is started normally through `cargo run`.

Actual appearance and runtime performance have not been measured or accepted.
Audio motion remains procedural unless an optional RMS provider is supplied;
this revision does not start WASAPI capture.
