"""Rasterise assets/logo.svg into a JPEG for a GitHub organisation avatar.

No SVG rasteriser is installed here (no cairosvg, no ImageMagick, no Inkscape) and
pip-installing one is a dependency added for a one-off asset. The mark is four
rounded rectangles, so it is drawn directly instead: the geometry is read from the
SVG rather than retyped, which means the SVG stays the single source of truth.

GitHub avatars are displayed at 32, 40, 64, 80 and 160 px, so everything is
rendered large and downsampled, and the block boundaries are kept off the pixel
grid at small sizes.
"""

from __future__ import annotations

import pathlib
import re

from PIL import Image, ImageDraw

ROOT = pathlib.Path(__file__).resolve().parents[2]
SVG = ROOT / "assets" / "logo.svg"
OUT_DIR = ROOT / "assets"

# The SVG mark is monochrome: it uses `currentColor` so the page it sits on decides
# the ink. This rasteriser has no page, so it has to pick, and picking the chart
# accent produced a blue avatar out of a black-and-white logo.
#
# Pure white made the mark look like a sticker. The mark is drawn with a rounded
# container, so it needs a field that reads as a surface rather than as paper: a
# light grey keeps the black shapes crisp and leaves the rounded corners room to
# sit on something.
BG = (11, 17, 32)
FG = (203, 213, 225)

SIZES = [1024, 512, 256, 128, 64, 40, 32]

# Empty field around the mark, as a fraction of the square on each side. GitHub
# crops an organisation avatar to a circle, so a mark drawn edge to edge loses its
# corners and reads as a blob at 32 px.
MARGIN = 0.14


def read_rects(path: pathlib.Path) -> list[tuple[float, float, float, float, float]]:
    """(x, y, w, h, rx) for every <rect>, straight out of the SVG."""
    text = path.read_text(encoding="utf-8")
    rects = []
    for m in re.finditer(
        r'<rect\s+x="([\d.]+)"\s+y="([\d.]+)"\s+width="([\d.]+)"\s+height="([\d.]+)"\s+rx="([\d.]+)"',
        text,
    ):
        rects.append(tuple(float(g) for g in m.groups()))
    if not rects:
        raise SystemExit(f"no <rect> elements in {path}")
    return rects


def render(rects, size: int, margin: float = 0.14, supersample: int = 4) -> Image.Image:
    """Draw the mark centred in a square with `margin` of empty field around it.

    The inset is measured against the mark's own bounding box rather than the SVG
    viewBox. The viewBox is 64 units and the shapes span 4..60, so padding the
    viewBox would crop into the mark instead of clearing it.
    """
    view = 64.0
    x0 = min(r[0] for r in rects)
    y0 = min(r[1] for r in rects)
    x1 = max(r[0] + r[2] for r in rects)
    y1 = max(r[1] + r[3] for r in rects)
    mark_w, mark_h = x1 - x0, y1 - y0

    s = size * supersample
    img = Image.new("RGB", (s, s), BG)
    draw = ImageDraw.Draw(img)

    usable = s * (1 - 2 * margin)
    k = min(usable / mark_w, usable / mark_h)
    off_x = (s - mark_w * k) / 2
    off_y = (s - mark_h * k) / 2

    for x, y, w, h, rx in rects:
        draw.rounded_rectangle(
            [
                off_x + (x - x0) * k,
                off_y + (y - y0) * k,
                off_x + (x - x0 + w) * k,
                off_y + (y - y0 + h) * k,
            ],
            radius=rx * k,
            fill=FG,
        )
    return img.resize((size, size), Image.LANCZOS)


def main() -> int:
    rects = read_rects(SVG)
    print(f"  read {len(rects)} rect(s) from {SVG.name}")

    written = []
    for size in SIZES:
        img = render(rects, size, MARGIN)
        jpg = OUT_DIR / f"logo-{size}.jpg"
        img.save(jpg, "JPEG", quality=95, optimize=True, progressive=True)
        written.append(jpg)
        print(f"  {jpg.name:<14} {jpg.stat().st_size:>7,} bytes  {size}x{size}")

    # A single 512 file is what the avatar field wants.
    main_png = OUT_DIR / "avatar.jpg"
    render(rects, 512, MARGIN).save(main_png, "JPEG", quality=95, optimize=True, progressive=True)
    print(f"  {main_png.name:<14} {main_png.stat().st_size:>7,} bytes  512x512  (the one to upload)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())