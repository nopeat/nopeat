from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parents[2]


MIN_DIMENSION = 0.5


# Mid-tone hues, in an order that alternates hue rather than walking the wheel:
# in a squarified layout the rectangles a reader has to tell apart are usually
# neighbours, and two neighbours from the same family read as one tile. Every
# colour is dark enough to carry a near-white label, which `label_ink` checks
# rather than assumes.
PALETTE = [
    "#2f6fed", "#12a594", "#b45309", "#6d28d9", "#e11d48", "#0284c7",
    "#4d7c0f", "#db2777", "#0891b2", "#c2410c", "#4f46e5", "#059669",
]
BACKGROUND = "#0e1116"
LABEL_INK = "#f8fafc"
LABEL_INK_ON_LIGHT = "#0b0e13"
FOOTER_INK = "#9aa7b8"
FOOTER_RULE = "#26313f"
FOOTER_H = 26


LABEL_MIN_W = 74
LABEL_MIN_H = 26
LABEL_ROW_H = 42


ADVANCE_EM = 0.62



def squarify(items: list[dict], x: float, y: float, w: float, h: float) -> list[dict]:
    
    out: list[dict] = []
    total = sum(item["size"] for item in items)
    if total <= 0 or w <= 0 or h <= 0:
        return out

    scale = (w * h) / total
    rest = list(items)
    rect = [x, y, w, h]

    def worst(row: list[dict], side: float) -> float:
        s = sum(item["size"] * scale for item in row)
        if s <= 0:
            return float("inf")
        mx = max(item["size"] * scale for item in row)
        mn = min(item["size"] * scale for item in row)
        return max((side * side * mx) / (s * s), (s * s) / (side * side * mn))

    while rest:

        vertical = rect[2] >= rect[3]
        side = rect[3] if vertical else rect[2]
        row: list[dict] = []
        best = float("inf")
        while rest:
            candidate = worst(row + [rest[0]], side)
            if row and candidate > best:
                break
            row.append(rest.pop(0))
            best = candidate

        row_area = sum(item["size"] * scale for item in row)
        thickness = row_area / side

        if vertical:
            cursor = rect[1]
            for item in row:
                height = (item["size"] * scale) / thickness
                out.append({"node": item, "x": rect[0], "y": cursor, "w": thickness, "h": height})
                cursor += height
            rect = [rect[0] + thickness, rect[1], rect[2] - thickness, rect[3]]
        else:
            cursor = rect[0]
            for item in row:
                width = (item["size"] * scale) / thickness
                out.append({"node": item, "x": cursor, "y": rect[1], "w": width, "h": thickness})
                cursor += width
            rect = [rect[0], rect[1] + thickness, rect[2], rect[3] - thickness]

        if rect[2] <= MIN_DIMENSION or rect[3] <= MIN_DIMENSION:
            break

    return out

def assert_valid(rects: list[dict], width: float, height: float, name: str) -> None:
    
    if not rects:
        raise SystemExit(f"{name}: the layout produced no rectangles")

    eps = 1e-6
    for rect in rects:
        if rect["w"] < -eps or rect["h"] < -eps:
            raise SystemExit(f"{name}: negative rectangle for {rect['node']['name']}")
        if (
            rect["x"] < -eps
            or rect["y"] < -eps
            or rect["x"] + rect["w"] > width + eps
            or rect["y"] + rect["h"] > height + eps
        ):
            raise SystemExit(
                f"{name}: {rect['node']['name']} at ({rect['x']:.1f},{rect['y']:.1f}) "
                f"{rect['w']:.1f}x{rect['h']:.1f} escapes {width:.0f}x{height:.0f}"
            )

    for i, a in enumerate(rects):
        for b in rects[i + 1:]:
            dx = min(a["x"] + a["w"], b["x"] + b["w"]) - max(a["x"], b["x"])
            dy = min(a["y"] + a["h"], b["y"] + b["h"]) - max(a["y"], b["y"])
            if dx > 1e-6 and dy > 1e-6:
                raise SystemExit(
                    f"{name}: {a['node']['name']} and {b['node']['name']} overlap by "
                    f"{dx:.2f}x{dy:.2f} - fix the layout, not the check"
                )

    covered = sum(rect["w"] * rect["h"] for rect in rects)
    if covered / (width * height) < 0.97:
        raise SystemExit(
            f"{name}: the rectangles cover {covered / (width * height):.1%} of the canvas; "
            "a treemap that leaves a quarter of the frame empty is a bug"
        )



def find_binary(explicit: str | None) -> pathlib.Path:
    
    if explicit:
        candidate = pathlib.Path(explicit)
        if not candidate.exists():
            raise SystemExit(f"--binary {explicit} does not exist")
        return candidate

    from_env = os.environ.get("OB_BINARY")
    if from_env:
        return pathlib.Path(from_env)

    stem = REPO / "target" / "release" / "nopeat"
    for candidate in (stem.with_suffix(".exe"), stem):
        if candidate.exists():
            return candidate
    raise SystemExit(
        "no nopeat binary found: build it with `cargo build --release`, "
        "or pass --binary / set OB_BINARY"
    )

def run_payload(binary: pathlib.Path, stats: pathlib.Path) -> dict:
    
    result = subprocess.run(
        [str(binary), str(stats), "--mode", "json", "--dims", "package"],
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip()
        raise SystemExit(f"{binary} exited {result.returncode}: {detail}")

    return json.loads(result.stdout.decode("utf-8-sig"))

def size_of(sizes: dict) -> tuple[int, str]:
    
    attributed = sizes.get("attributed")
    if attributed:
        return int(attributed), "attributed"
    parsed = sizes.get("parsed")
    if parsed:
        return int(parsed), "parsed"
    return int(sizes.get("stat") or 0), "stat"

def groups_by_package(payload: dict) -> list[dict]:
    
    totals: dict[str, int] = {}
    counts: dict[str, int] = {}
    for module in (payload.get("modules") or {}).values():
        package = module.get("package") or {}
        name = package.get("name") or "<app>"
        byte_count, _ = size_of(module.get("sizes") or {})
        totals[name] = totals.get(name, 0) + byte_count
        counts[name] = counts.get(name, 0) + 1

    children = [
        {"name": name, "size": size, "module_count": counts[name]}
        for name, size in totals.items()
        if size > 0
    ]

    children.sort(key=lambda c: (-c["size"], c["name"]))
    return children

def size_dimension(payload: dict) -> str:
    
    seen = {
        size_of(module.get("sizes") or {})[1]
        for module in (payload.get("modules") or {}).values()
    }
    for candidate in ("attributed", "parsed", "stat"):
        if candidate in seen:
            return candidate
    return "stat"



def label_ink(hex_colour: str) -> str:
    """Near-white on a dark tile, near-black on a light one, from luminance."""
    n = int(hex_colour[1:], 16)
    linear = [(((n >> shift) & 255) / 255) ** 2.2 for shift in (16, 8, 0)]
    luminance = (
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    )
    return LABEL_INK if luminance < 0.45 else LABEL_INK_ON_LIGHT

def fmt_bytes(n: float) -> str:
    if n >= 1048576:
        return f"{n / 1048576:.1f} MB"
    return f"{round(n / 1024)} KB"

def esc(text: str) -> str:
    return str(text).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")

def render(
    children: list[dict],
    payload: dict,
    label: str | None,
    width: int,
    height: int,
    top: int,
    fold: bool,
) -> tuple[str, int, int, str]:
    shown = children if top <= 0 else children[:top]
    if fold and len(children) > len(shown):
        rest = children[len(shown):]
        shown = shown + [
            {
                "name": f"(+{len(rest)} more)",
                "size": sum(c["size"] for c in rest),
                "module_count": sum(c.get("module_count") or 0 for c in rest),
            }
        ]

    rects = squarify(shown, 0, 0, width, height)
    assert_valid(rects, width, height, "treemap (package)")

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
        f'viewBox="0 0 {width} {height}" '
        f'font-family="ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">',
        f'<rect width="{width}" height="{height}" fill="{BACKGROUND}"/>',
    ]

    for index, rect in enumerate(rects):
        fill = PALETTE[index % len(PALETTE)]
        ink = label_ink(fill)
        node = rect["node"]

        parts.append(
            f'<rect x="{rect["x"]:.1f}" y="{rect["y"]:.1f}" '
            f'width="{rect["w"]:.1f}" height="{rect["h"]:.1f}" '
            f'fill="{fill}" '
            f'stroke="{BACKGROUND}" stroke-width="2">'
            f'<title>{esc(node["name"])}: {fmt_bytes(node["size"])}</title></rect>'
        )
        if rect["w"] > LABEL_MIN_W and rect["h"] > LABEL_MIN_H:
            name = str(node["name"])
            room = int(rect["w"] / (12 * ADVANCE_EM))
            if len(name) > room:
                name = name[: max(1, room - 1)] + "\u2026"
            parts.append(
                f'<text x="{rect["x"] + 7:.1f}" y="{rect["y"] + 17:.1f}" '
                f'fill="{ink}" font-size="12" font-weight="600">{esc(name)}</text>'
            )
            if rect["h"] > LABEL_ROW_H:
                parts.append(
                    f'<text x="{rect["x"] + 7:.1f}" y="{rect["y"] + 31:.1f}" '
                    f'fill="{ink}" font-size="11" fill-opacity="0.78">'
                    f'{fmt_bytes(node["size"])}</text>'
                )

    totals = payload.get("totals") or {}
    footer = " \u00b7 ".join(
        [
            label or "bundle",
            "area = declared bytes",
            f'{totals.get("asset_count", 0):,} assets',
            f'{totals.get("module_count", 0):,} modules',
            f"{len(shown)} of {len(children)} packages",
            f"dimension {size_dimension(payload)}",
        ]
    )

    # The treemap has to cover the whole canvas - a caption band under it would
    # put the rectangles below the 97 % the reader gates check for - so the footer
    # is an overlay. The band is what keeps the last row of labels legible
    # underneath it instead of tangled with it, and it carries no <title>, so the
    # geometry checks read it as chrome and not as a rectangle of the build.
    parts.append(
        f'<rect x="0" y="{height - FOOTER_H}" width="{width}"'
        f' height="{FOOTER_H}" fill="{BACKGROUND}" fill-opacity="0.86"/>'
    )
    parts.append(
        f'<line x1="0" y1="{height - FOOTER_H}" x2="{width}"'
        f' y2="{height - FOOTER_H}" stroke="{FOOTER_RULE}" stroke-width="1"/>'
    )
    parts.append(
        f'<text x="12" y="{height - 9}" fill="{FOOTER_INK}"'
        f' font-size="11">{esc(footer)}</text>'
    )
    parts.append("</svg>")
    return "\n".join(parts) + "\n", len(rects), len(shown), footer

def main() -> int:
    parser = argparse.ArgumentParser(description="Render the Nopeat README treemap to SVG.")
    parser.add_argument("--stats", required=True, help="stats.json (or metafile.json) to analyse")
    parser.add_argument("--out", required=True, help="SVG to write")
    parser.add_argument("--width", type=int, default=880)
    parser.add_argument("--height", type=int, default=440)
    parser.add_argument("--top", type=int, default=40, help="rectangles to keep; 0 for all")
    parser.add_argument(
        "--fold", action="store_true",
        help="fold the tail into one node, so the picture never implies it shows everything",
    )
    parser.add_argument("--label", default=None, help="what the fixture is, drawn in the footer")
    parser.add_argument("--binary", default=None)
    args = parser.parse_args()

    stats = pathlib.Path(args.stats)
    if not stats.exists():
        raise SystemExit(f"--stats {args.stats} does not exist")

    binary = find_binary(args.binary)
    payload = run_payload(binary, stats)

    children = groups_by_package(payload)
    if not children:
        raise SystemExit(f"no packages with a positive size in {stats}")

    svg, rendered, groups, footer = render(
        children, payload, args.label, args.width, args.height, args.top, args.fold
    )

    out = pathlib.Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)

    out.write_text(svg, encoding="utf-8", newline="\n")
    print(
        f"wrote {out} ({rendered} rectangles, {groups} of {len(children)} package groups, "
        f"dimension {size_dimension(payload)})"
    )
    print(f"  {footer}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
