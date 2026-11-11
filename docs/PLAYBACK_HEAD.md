# Bright playback-head refinement

Scope is limited to `ui/waveform.rs` and the supporting playback-head intensity
function in `modules/waveform_lighting.rs`. Wave shape generation, layout,
artwork, media integration, progress mapping, and control behavior are retained.

The previous head mixed its core into a low-opacity feathered stroke and drew
additional supporting highlights afterwards. The refined head has a dedicated
final Canvas geometry pass and four independently composited elements:

1. A curved backward trail, approximately 14–20 logical px expanded, following
   the already prepared primary wave samples.
2. A cyan/blue/violet outer halo with smooth elliptical falloff, approximately
   18–22 px across in the expanded view.
3. A concentrated 8–10 px inner glow with at least 78% peak opacity.
4. A sharp white core, normally 3.5 px expanded / 2.8 px compact, with full
   opacity independent of the glow intensity or paused playback state.

Glow is approximated by composited elliptical shells, without full-surface blur.
Ellipses flatten when the primary wave approaches the top/bottom of the slot;
the core and sparkle radii also adapt to the available clearance. The existing
14/25 px scissor remains in place as a safety boundary. Horizontal edge fading
and the notch content opacity remain applicable.

The existing animation clock drives a 1.95-second breathing cycle and seeded
2–5 second flare intervals. A flare lasts 170–205 ms and introduces a small
four-point sparkle, a slightly larger halo, and a stronger core. Pointer-down
and drag impulses use the existing seek feedback; reduced motion disables
breathing and sparkle animation. Paused playback retains the bright core.

The scheduling is deterministic and independent of render frequency. No new
timer, thread, GUI dependency, or media command is introduced.

At the user's standing request, visual QA, screenshots, Clippy, and automated
tests are not run. Source formatting is applied as an edit and the application
is started normally with `cargo run`. Brightness at actual notch size remains
for the user's visual acceptance; no claim of verified visibility is made.
