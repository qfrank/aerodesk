#!/usr/bin/env python3
"""Render the AeroDesk icon (white Material "send" paper plane on a #7D70F5
rounded-square badge) into a full Tauri icon set. Outputs PNGs + icon.ico +
icon.icns."""
import os
from PIL import Image, ImageDraw

ACCENT = (0x7D, 0x70, 0xF5, 255)
WHITE = (255, 255, 255, 255)
# Material "send" plane in a 24-unit space.
PLANE = [(2, 21), (23, 12), (2, 3), (2, 10), (17, 12), (2, 14)]

OUT = os.path.dirname(os.path.abspath(__file__))  # src-tauri/icons/


def render(size: int) -> Image.Image:
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # macOS app-icon spec: the badge occupies ~83% of the canvas (measured
    # from system squircle icons like Calculator/Notes/Maps), centered with
    # ~8.5% padding on each side. A full-bleed badge looks oversized in the
    # Dock next to system icons that honor this padding.
    pad = round(size * 0.085)
    side = size - 2 * pad
    d.rounded_rectangle([pad, pad, pad + side - 1, pad + side - 1],
                        radius=round(side * 5.5 / 24), fill=ACCENT)
    # Paper plane scaled to 0.6 of the 24-space and centered on the badge.
    scale = side / 24 * 0.6
    pts = [(x * scale, y * scale) for x, y in PLANE]
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    bw, bh = max(xs) - min(xs), max(ys) - min(ys)
    cx = size / 2
    dx = cx - (min(xs) + bw / 2)
    dy = cx - (min(ys) + bh / 2)
    poly = [(x + dx, y + dy) for x, y in pts]
    d.polygon(poly, fill=WHITE)
    return img


# Master + Tauri bundle sizes.
render(1024).save(os.path.join(OUT, "icon.png"))
render(32).save(os.path.join(OUT, "32x32.png"))
render(128).save(os.path.join(OUT, "128x128.png"))
render(256).save(os.path.join(OUT, "128x128@2x.png"))

# Windows .ico (multi-size).
ico = render(256)
ico.save(os.path.join(OUT, "icon.ico"), sizes=[(16, 16), (32, 32), (48, 48),
                                               (64, 64), (128, 128), (256, 256)])

# macOS .icns via iconutil (build an .iconset, then convert).
iconset = os.path.join(OUT, "icon.iconset")
os.makedirs(iconset, exist_ok=True)
spec = {
    "icon_16x16.png": 16, "icon_16x16@2x.png": 32,
    "icon_32x32.png": 32, "icon_32x32@2x.png": 64,
    "icon_128x128.png": 128, "icon_128x128@2x.png": 256,
    "icon_256x256.png": 256, "icon_256x256@2x.png": 512,
    "icon_512x512.png": 512, "icon_512x512@2x.png": 1024,
}
for name, sz in spec.items():
    render(sz).save(os.path.join(iconset, name))
import subprocess
r = subprocess.run(["iconutil", "-c", "icns", iconset, "-o",
                    os.path.join(OUT, "icon.icns")],
                   capture_output=True, text=True)
print("iconutil:", r.returncode, r.stderr.strip() or "ok")
subprocess.run(["rm", "-rf", iconset])

print("wrote to", OUT)
for f in sorted(os.listdir(OUT)):
    p = os.path.join(OUT, f)
    print(f"  {f}  {os.path.getsize(p)} bytes")