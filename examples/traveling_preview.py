"""Render Rust-exported preview geometry. Requires Pillow; no desktop capture."""
import csv
import sys
from collections import defaultdict
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

source = Path(sys.argv[1] if len(sys.argv) > 1 else "target/traveling-preview.csv")
destination = Path(sys.argv[2] if len(sys.argv) > 2 else "docs/previews/traveling-waves.gif")
data = defaultdict(list)
with source.open(newline="", encoding="utf-8") as stream:
    for row in csv.DictReader(stream):
        data[(int(row["frame"]), row["passage"], int(row["layer"]))].append(
            (float(row["x"]), float(row["y"]), float(row["opacity"]), float(row["width"])))

supersample, scale = 2, 3
width, height = 860, 288
try:
    font = ImageFont.truetype("C:/Windows/Fonts/segoeui.ttf", 14 * supersample)
except OSError:
    font = ImageFont.load_default()

stops = [(0.0, (20, 240, 248)), (0.38, (32, 135, 255)), (0.68, (130, 78, 255)), (1.0, (244, 45, 215))]
def color(x, opacity):
    t = max(0.0, min(1.0, x / 268.0))
    pair = next(((a, b) for a, b in zip(stops, stops[1:]) if t <= b[0]), (stops[-2], stops[-1]))
    a, b = pair
    fraction = (t - a[0]) / (b[0] - a[0])
    # Same horizontal entry/exit fade as the waveform; no opaque crop edges.
    edge = min(1.0, t / 0.10, (1.0 - t) / 0.10)
    edge = edge * edge * (3.0 - 2.0 * edge)
    return tuple(round((u + (v - u) * fraction) * opacity * edge) for u, v in zip(a[1], b[1]))

frames = []
for frame in sorted({key[0] for key in data}):
    canvas = Image.new("RGB", (width * supersample, height * supersample), "black")
    draw = ImageDraw.Draw(canvas)
    draw.text((20 * supersample, 10 * supersample), "Traveling thin waves | 3x geometry preview", font=font, fill=(210, 215, 226))
    for passage, top, label in [("quiet", 69, "Quiet / slow - synthetic input"), ("dense", 187, "Dense percussion - synthetic input")]:
        draw.text((20 * supersample, (top - 25) * supersample), label, font=font, fill=(144, 151, 168))
        # The large ribbon is a faint reference; the three traveling lines use
        # actual production points, stroke widths and opacity. No playback orb.
        for layer in [0, 3, 2, 4]:
            points = data[(frame, passage, layer)]
            def tangent(index):
                before = points[index][1] - points[index - 1][1] if index else points[1][1] - points[0][1]
                after = points[index + 1][1] - points[index][1] if index + 1 < len(points) else before
                return 0.0 if before * after <= 0 else 2 * before * after / (before + after)
            smooth = []
            for index, (a, b) in enumerate(zip(points, points[1:])):
                if not layer:
                    smooth.append(a)
                    continue
                y1, y2 = a[1] + tangent(index) / 3, b[1] - tangent(index + 1) / 3
                for fraction in [0.0, 0.25, 0.5, 0.75]:
                    t, u = fraction, 1 - fraction
                    y = u**3 * a[1] + 3*u*u*t*y1 + 3*u*t*t*y2 + t**3*b[1]
                    smooth.append((a[0] + (b[0] - a[0]) * t, y, a[2], a[3]))
            smooth.append(points[-1])
            for a, b in zip(smooth, smooth[1:]):
                opacity = a[2] if layer else 0.16
                stroke = a[3] if layer else 0.7
                xy = [((20 + x * scale) * supersample, (top + y * scale) * supersample) for x, y, *_ in (a, b)]
                draw.line(xy, fill=color((a[0] + b[0]) * 0.5, opacity), width=max(1, round(stroke * scale * supersample)))
    frames.append(canvas.resize((width, height), Image.Resampling.LANCZOS))

destination.parent.mkdir(parents=True, exist_ok=True)
# A finite preview clip; no loop extension introduces an artificial phase reset.
frames[0].save(destination, save_all=True, append_images=frames[1:], duration=[33, 33, 34] * (len(frames) // 3), optimize=False, disposal=2)
frames[len(frames) // 2].save(destination.with_suffix(".png"))
print(f"Wrote {len(frames)} frames to {destination}")
