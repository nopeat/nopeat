#!/usr/bin/env python3
"""Draw the README charts.

    python bench/harness/make-charts.py

Standard library only, on purpose: the reproducibility gate in CI runs this and
diffs `docs/assets`, and an import would make a committed figure a function of
somebody else's release schedule. Every number comes from
`docs/assets/charts-data.json`; nothing here measures anything.

Three figures, a light and a dark theme each:

    chart-pipeline-*.svg       wall clock and peak memory, whole pipeline
    chart-source-maps-*.svg    attribution time and memory against source count
    chart-memory-budget-*.svg  peak memory against the published ceilings

House rules, because `bench/harness/check-chart-scales.mjs` reads the SVG back:

  * a bar is `<rect x y width height fill rx/>` - that attribute order is load
    bearing, the checker matches on it and finds nothing if it changes;
  * background bands are filled with the theme's `track` colour, the one colour
    the checker skips, so a band never counts as a bar;
  * the pipeline bars stay linear, one scale per panel, with no minimum drawn
    width: a 1.87 s bar is about a thirtieth of a 63.6 s bar and has to look it.
"""

from __future__ import annotations

import json
import math
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA = ROOT / "docs" / "assets" / "charts-data.json"
OUT = ROOT / "docs" / "assets"

# The layout facts `charts-data.json` records per figure. `bench/harness/
# chart-report.py` reads them back to tell a reader how small the smallest label
# gets at README width, which is the one thing a diff of an SVG cannot show.
FIGURE_NOTES = {
    "pipeline": (
        "two linear panels, wall clock and peak memory, each on its own scale; "
        "the bands mark how far each axis runs, so a bar a couple of pixels long "
        "reads as a small value rather than as a broken render"
    ),
    "source-maps": (
        "log-log, with the area between the two series tinted and every gap "
        "labelled with its own multiple, because the geometry of a log axis "
        "cannot draw a ratio of 2,690 as 2,690 times anything"
    ),
    "memory-budget": (
        "log-log markers, deliberately unconnected; B4 and B8 are the same 1 GB "
        "input, dodged sideways so their ceiling lines do not lie on top of each "
        "other"
    ),
}

FONT = (
    "-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,'Helvetica Neue',"
    "Arial,'Noto Sans',sans-serif"
)

THEMES = {
    "light": {
        "bg": "#ffffff",
        "ink": "#111827",
        "muted": "#6b7280",
        "accent": "#1d4ed8",
        "accent_ink": "#1e40af",
        "ref": "#cbd5e1",
        "ref_ink": "#64748b",
        "grid": "#e5e7eb",
        "rule": "#d1d5db",
        # `track` is load bearing: check-chart-scales.mjs skips rectangles in this
        # colour, and it is the only way a rectangle taller than 15px may exist
        # here without being read as a bar.
        "track": "#f1f5f9",
    },
    "dark": {
        "bg": "#0b1120",
        "ink": "#e5e7eb",
        "muted": "#9ca3af",
        "accent": "#60a5fa",
        "accent_ink": "#93c5fd",
        "ref": "#334155",
        "ref_ink": "#94a3b8",
        "grid": "#1e293b",
        "rule": "#334155",
        "track": "#182236",
    },
}


def esc(text: str) -> str:
    """Escape the three characters that would otherwise break the XML."""
    return str(text).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


class Svg:
    """A tiny SVG writer. One instance per figure; every coordinate is absolute."""

    def __init__(self, width: float, height: float, theme: dict) -> None:
        self.w = float(width)
        self.h = float(height)
        self.t = theme
        self.parts: list[str] = []

    def rect(self, x, y, w, h, fill, rx=0.0, opacity=None):
        if w <= 0 or h <= 0:
            return
        r = f' rx="{rx:.1f}"' if rx else ""
        o = f' fill-opacity="{opacity}"' if opacity is not None else ""
        self.parts.append(
            f'<rect x="{x:.2f}" y="{y:.2f}" width="{w:.2f}" height="{h:.2f}"'
            f' fill="{fill}"{r}{o}/>'
        )

    def line(self, x1, y1, x2, y2, stroke, width=1.0, dash=""):
        d = f' stroke-dasharray="{dash}"' if dash else ""
        self.parts.append(
            f'<line x1="{x1:.2f}" y1="{y1:.2f}" x2="{x2:.2f}" y2="{y2:.2f}"'
            f' stroke="{stroke}" stroke-width="{width}"{d}/>'
        )

    def text(
        self, x, y, content, size=12, fill=None, anchor="start",
        weight="400", opacity=None, halo=False,
    ):
        fill = fill or self.t["ink"]
        o = f' fill-opacity="{opacity}"' if opacity is not None else ""
        # A halo is how a label stays readable where it has to sit on a line or a
        # gridline, and it costs one attribute instead of a chip behind the text.
        h = (
            f' stroke="{self.t["bg"]}" stroke-width="3" paint-order="stroke"'
            if halo
            else ""
        )
        self.parts.append(
            f'<text x="{x:.2f}" y="{y:.2f}" font-family="{FONT}" font-size="{size}"'
            f' font-weight="{weight}" fill="{fill}" text-anchor="{anchor}"{o}{h}'
            f'>{esc(content)}</text>'
        )

    def circle(self, x, y, r, fill, stroke=None, sw=1.5):
        s = f' stroke="{stroke}" stroke-width="{sw}"' if stroke else ""
        self.parts.append(
            f'<circle cx="{x:.2f}" cy="{y:.2f}" r="{r:.2f}" fill="{fill}"{s}/>'
        )

    def polyline(self, points, stroke, width=2.0):
        d = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
        self.parts.append(
            f'<polyline points="{d}" fill="none" stroke="{stroke}"'
            f' stroke-width="{width}" stroke-linejoin="round" stroke-linecap="round"/>'
        )

    def polygon(self, points, fill, opacity):
        d = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
        self.parts.append(f'<polygon points="{d}" fill="{fill}" fill-opacity="{opacity}"/>')

    def render(self, title: str, desc: str) -> str:
        body = "\n  ".join(self.parts)
        return (
            '<?xml version="1.0" encoding="UTF-8"?>\n'
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w:.0f}"'
            f' height="{self.h:.0f}" viewBox="0 0 {self.w:.0f} {self.h:.0f}"'
            ' role="img" aria-labelledby="t d">\n'
            f'  <title id="t">{esc(title)}</title>\n'
            f'  <desc id="d">{esc(desc)}</desc>\n'
            f'  <rect width="{self.w:.0f}" height="{self.h:.0f}"'
            f' fill="{self.t["bg"]}"/>\n'
            f"  {body}\n</svg>\n"
        )


def text_width(s: str, size: float) -> float:
    """A rough advance width, good enough to keep labels off the frame."""
    narrow = sum(c in " ,.:;'|ilj" for c in s)
    return (len(s) - narrow) * size * 0.6 + narrow * size * 0.3


def short(v: float) -> str:
    """Three significant digits, thousands separated once there are four."""
    if v == 0:
        return "0"
    if abs(v) >= 1000:
        return f"{v:,.0f}"
    return f"{v:.3g}"


def seconds(v: float) -> str:
    if v == 0:
        return "0"
    if abs(v) < 1:
        return f"{v * 1000:.3g} ms"
    return f"{short(v)} s"


def megabytes(v: float) -> str:
    return "0 MB" if v == 0 else f"{short(v)} MB"


def ratio(ours: float, theirs: float) -> str:
    return f"{theirs / ours:.2g}\u00d7"


def plain(v: float) -> str:
    """Axis ticks for a single-unit panel: the unit is in the panel heading, and
    spelling it out on five ticks crowds them into each other."""
    return f"{v:,.0f}"


def linear_axis(vmax: float, target: int = 6) -> tuple[float, list[float]]:
    """A round scale top at or above `vmax`, and the tick values under it."""
    if vmax <= 0:
        return 1.0, [1.0]
    base = 10 ** math.floor(math.log10(vmax / target))
    step = 10 * base
    for m in (1, 2, 2.5, 5, 10):
        if m * base * target >= vmax:
            step = m * base
            break
    top = math.ceil(vmax / step - 1e-9) * step
    return top, [round(i * step, 10) for i in range(1, int(round(top / step)) + 1)]


def headline(c: Svg, title: str, lines: list[str], x: float = 0.0) -> None:
    """A figure title and its standfirst, flush with the left edge of the frame."""
    c.text(x, 28, title, size=16, weight="700")
    for i, line in enumerate(lines):
        c.text(x, 47 + i * 16, line, size=11.5, fill=c.t["muted"])


# --------------------------------------------------------------------------
# 1. the whole pipeline
# --------------------------------------------------------------------------


def chart_pipeline(theme: dict, rows) -> str:
    W, H = 880.0, 310.0
    c = Svg(W, H, theme)
    pad_l, pad_r, gap = 176.0, 20.0, 48.0
    panel_w = (W - pad_l - pad_r - gap) / 2
    bar_h, bar_gap, group_gap = 16.0, 6.0, 26.0
    group_h = 2 * bar_h + bar_gap
    head_y, first_y = 74.0, 88.0
    axis_y = first_y + 2 * group_h + group_gap + 10.0

    headline(
        c,
        "One pass: parse stats, measure every asset, fuse the maps, render the report",
        [
            "synthetic fixtures \u00b7 median of 3 runs on the reference machine \u00b7 "
            "each panel has its own linear scale",
        ],
    )

    panels = [
        ("Wall clock", "seconds, linear", "ob_s", "ref_s", seconds, "faster"),
        ("Peak memory", "megabytes, linear", "ob_mb", "ref_mb", megabytes, "smaller"),
    ]

    # Both panels reserve the same room for the value labels after the longest
    # bar, so the two gridlines land on the same columns.
    label_room = 12.0 + max(
        text_width(fmt(float(row[key])), 11)
        for _, _, ours_key, theirs_key, fmt, _ in panels
        for row in rows
        for key in (ours_key, theirs_key)
    )
    plot_w = panel_w - label_room

    for index, (heading, unit, ours_key, theirs_key, fmt, word) in enumerate(panels):
        x0 = pad_l + index * (panel_w + gap)
        peak = max(
            max(float(row[ours_key]), float(row[theirs_key])) for row in rows
        )
        top, ticks = linear_axis(peak * 1.06)
        sx = lambda v, x_start=x0: x_start + (v / top) * plot_w

        c.text(x0, head_y, heading, size=13.5, weight="600")
        c.text(
            x0 + text_width(heading, 13.5) + 8, head_y, unit,
            size=11, fill=theme["muted"],
        )

        # The band marks how far the axis runs, so a bar three pixels long reads
        # as a small value instead of as a rendering fault.
        for row_index in range(len(rows)):
            band_y = first_y + row_index * (group_h + group_gap)
            c.rect(x0 - 6, band_y - 6, plot_w + 12, group_h + 12, theme["track"], rx=7)

        for tick in ticks:
            gx = sx(tick)
            c.line(gx, first_y - 6, gx, axis_y, theme["grid"], 1.0)
            c.text(gx, axis_y + 16, plain(tick), size=10, fill=theme["muted"],
                   anchor="middle")
        c.line(x0 - 6, axis_y, x0 + plot_w + 6, axis_y, theme["rule"], 1.0)
        c.text(sx(0.0), axis_y + 16, "0", size=10, fill=theme["muted"], anchor="middle")

        for row_index, row in enumerate(rows):
            y = first_y + row_index * (group_h + group_gap)
            ours, theirs = float(row[ours_key]), float(row[theirs_key])

            for offset, value, colour, ink in (
                (0.0, theirs, theme["ref"], theme["ref_ink"]),
                (bar_h + bar_gap, ours, theme["accent"], theme["accent_ink"]),
            ):
                bar_y = y + offset
                length = (value / top) * plot_w
                c.rect(x0, bar_y, length, bar_h, colour, rx=2.5)
                c.text(
                    x0 + length + 8, bar_y + bar_h / 2 + 4, fmt(value),
                    size=11, fill=ink,
                    weight="600" if colour == theme["accent"] else "400",
                )

            c.text(
                x0 + plot_w + 6, y + bar_h + bar_gap + bar_h / 2 + 4,
                f"{ratio(ours, theirs)} {word}",
                size=11, fill=theme["accent_ink"], anchor="end", weight="600",
            )

    # Row labels once, in the left margin: the rows are at the same height in both
    # panels, so repeating them would only add ink.
    for row_index, row in enumerate(rows):
        mid = first_y + row_index * (group_h + group_gap) + group_h / 2
        c.text(pad_l - 14, mid - 2, row["label"], size=12, anchor="end",
               weight="600")
        c.text(pad_l - 14, mid + 14, row["detail"], size=10, anchor="end",
               fill=theme["muted"])

    legend_y = axis_y + 48
    c.rect(0, legend_y - 9, 11, 11, theme["accent"], rx=2.5)
    c.text(17, legend_y, "Nopeat", size=11)
    c.rect(150, legend_y - 9, 11, 11, theme["ref"], rx=2.5)
    c.text(167, legend_y, "webpack-bundle-analyzer 4.10.2", size=11,
           fill=theme["muted"])
    c.text(0, legend_y + 28,
           "both panels are linear and to scale: a 1.87 s bar is about a "
           "thirtieth of a 63.6 s bar, and is drawn that way",
           size=10.5, fill=theme["muted"])

    desc = (
        "Two panels, each on its own linear scale. Left, wall clock in seconds. "
        "Right, peak memory in megabytes. In both panels Nopeat is the "
        "accent bar of each pair and carries the multiple: 34x faster, 29x "
        "faster, 18x smaller, 3.8x smaller."
    )
    return c.render("Full pipeline against webpack-bundle-analyzer", desc)


# --------------------------------------------------------------------------
# 2. source map attribution
# --------------------------------------------------------------------------


def chart_source_maps(theme: dict, points) -> str:
    W, H = 880.0, 368.0
    c = Svg(W, H, theme)
    pad_l, pad_r, gap = 84.0, 24.0, 52.0
    panel_w = (W - pad_l - pad_r - gap) / 2
    top, bottom = 100.0, 268.0
    plot_h = bottom - top

    headline(
        c,
        "Source map attribution, against the number of sources in the map",
        [
            "log scales \u00b7 the band between the lines is labelled with its "
            "multiple, because a step up this axis is a multiple, not a constant",
        ],
    )

    counts = [float(p["sources"]) for p in points]
    x_lo = math.log10(min(counts) / 1.7)
    x_hi = math.log10(max(counts) * 2.2)
    sx = lambda v, x_start=0.0: x_start + (
        (math.log10(v) - x_lo) / (x_hi - x_lo)
    ) * panel_w

    panels = [
        ("Wall clock", "seconds, log scale", "nopeat_seconds",
         "source_map_explorer_seconds", seconds),
        ("Peak memory", "megabytes, log scale", "nopeat_mb",
         "source_map_explorer_mb", megabytes),
    ]

    for index, (heading, unit, ours_key, theirs_key, fmt) in enumerate(panels):
        x0 = pad_l + index * (panel_w + gap)
        values = [
            float(p[key]) for p in points for key in (ours_key, theirs_key)
        ]
        y_lo = math.log10(min(values) / 1.6)
        y_hi = math.log10(max(values) * 1.6)
        sy = lambda v: bottom - ((math.log10(v) - y_lo) / (y_hi - y_lo)) * plot_h

        c.text(x0, 82, heading, size=13.5, weight="600")
        c.text(x0 + text_width(heading, 13.5) + 8, 82, unit, size=11,
               fill=theme["muted"])

        decade = math.ceil(y_lo)
        while decade <= math.floor(y_hi):
            gy = sy(10 ** decade)
            c.line(x0, gy, x0 + panel_w, gy, theme["grid"], 1.0)
            c.text(x0 - 10, gy + 4, fmt(10 ** decade), size=10,
                   fill=theme["muted"], anchor="end")
            decade += 1

        for tick, label in ((10, "10"), (100, "100"), (1000, "1k"),
                            (10000, "10k"), (50000, "50k")):
            if not x_lo <= math.log10(tick) <= x_hi:
                continue
            gx = sx(tick, x0)
            c.line(gx, top, gx, bottom, theme["grid"], 1.0, dash="2 3")
            c.text(gx, bottom + 18, label, size=10, fill=theme["muted"],
                   anchor="middle")
        c.line(x0, bottom, x0 + panel_w, bottom, theme["rule"], 1.0)
        c.text(x0 + panel_w / 2, bottom + 38, "sources in the map", size=11,
               fill=theme["muted"], anchor="middle")

        # On a log axis 562 s sits three and a half decades above 209 ms, which is
        # a little over half the panel: the geometry cannot show a ratio of 2,688
        # as 2,688 times anything. So the area between the curves is tinted and
        # every gap is labelled with its own multiple, and the picture stops
        # claiming a difference smaller than the one in the record.
        ours_path = [
            (sx(float(p["sources"]), x0), sy(float(p[ours_key]))) for p in points
        ]
        theirs_path = [
            (sx(float(p["sources"]), x0), sy(float(p[theirs_key]))) for p in points
        ]
        c.polygon(ours_path + theirs_path[::-1], theme["accent"], 0.10)

        for (px, ours_y), (_, theirs_y), point in zip(
            ours_path, theirs_path, points
        ):
            c.line(px, ours_y, px, theirs_y, theme["accent"], 1.2)
            multiple = float(point[theirs_key]) / float(point[ours_key])
            c.text(
                px, (ours_y + theirs_y) / 2 + 4, f"{short(multiple)}\u00d7",
                size=10.5, fill=theme["accent_ink"], anchor="middle",
                weight="600", halo=True,
            )

        last = points[-1]
        for path, colour, ink, weight in (
            (theirs_path, theme["ref"], theme["ref_ink"], "400"),
            (ours_path, theme["accent"], theme["accent_ink"], "600"),
        ):
            c.polyline(path, colour, 2.4 if weight == "600" else 2.0)
            for px, py in path:
                c.circle(px, py, 4.4 if weight == "600" else 3.6, theme["bg"],
                         stroke=colour, sw=2.2 if weight == "600" else 1.8)

            # Direct labels instead of a legend, and they carry the end value, so
            # the reader does not have to read a log axis off a curve.
            key = ours_key if weight == "600" else theirs_key
            name = "Nopeat" if weight == "600" else "source-map-explorer"
            c.text(
                sx(float(last["sources"]), x0) - 10,
                sy(float(last[key])) - 9,
                f"{name} \u00b7 {fmt(float(last[key]))}",
                size=11, fill=ink, anchor="end", weight=weight, halo=True,
            )

    last = points[-1]
    time_multiple = (
        float(last["source_map_explorer_seconds"])
        / float(last["nopeat_seconds"])
    )
    memory_multiple = (
        float(last["source_map_explorer_mb"]) / float(last["nopeat_mb"])
    )

    c.text(0, 334,
           "the 12-source point is a real preact build; 10,000 and 50,000 are "
           "synthetic \u00b7 median of 3 runs, except the single 9.4-minute "
           "reference run",
           size=10.5, fill=theme["muted"])
    c.text(0, 352,
           f"at {last['sources']:,.0f} sources source-map-explorer costs "
           f"{short(time_multiple)} times the time and {short(memory_multiple)} "
           "times the memory of Nopeat",
           size=10.5, fill=theme["muted"])

    desc = (
        "Two log-log panels. Left, wall clock in seconds. Right, peak memory in "
        "megabytes. Against the number of sources in the map. The tinted band "
        "between the two lines is labelled with the multiple at each point. At "
        f"{last['sources']:,.0f} sources source-map-explorer costs "
        f"{short(time_multiple)} times the time and {short(memory_multiple)} times "
        f"the memory: {seconds(float(last['source_map_explorer_seconds']))} against "
        f"{seconds(float(last['nopeat_seconds']))}, and "
        f"{megabytes(float(last['source_map_explorer_mb']))} against "
        f"{megabytes(float(last['nopeat_mb']))}."
    )
    return c.render("Source map attribution against source-map-explorer", desc)


# --------------------------------------------------------------------------
# 3. peak memory against the ceilings the project publishes
# --------------------------------------------------------------------------


def chart_budget(theme: dict, points) -> str:
    W, H = 880.0, 320.0
    c = Svg(W, H, theme)
    pad_l, pad_r = 78.0, 34.0
    top, bottom = 92.0, 244.0
    plot_w = W - pad_l - pad_r
    plot_h = bottom - top

    headline(
        c,
        "Peak memory stays inside the published budget",
        [
            "B3 and B4 are one workload at two input sizes; B8 is the 1 GB input "
            "with a 36.5 MB source map added.",
            "Each marker is a measurement; the dashed line above it is that "
            "target's published ceiling, not a reading.",
        ],
    )

    x_lo, x_hi = math.log10(250), math.log10(1900)
    y_lo, y_hi = math.log10(70), math.log10(800)
    sx = lambda v: pad_l + ((math.log10(v) - x_lo) / (x_hi - x_lo)) * plot_w
    sy = lambda v: bottom - ((math.log10(v) - y_lo) / (y_hi - y_lo)) * plot_h

    for ceiling in (100, 200, 400, 800):
        c.line(pad_l, sy(ceiling), pad_l + plot_w, sy(ceiling), theme["grid"], 1.0)
        c.text(pad_l - 10, sy(ceiling) + 4, megabytes(ceiling), size=10,
               fill=theme["muted"], anchor="end")

    for value, label in ((363, "363 MB"), (1049, "1 GB")):
        c.line(sx(value), top, sx(value), bottom, theme["grid"], 1.0, dash="2 3")
        c.text(sx(value), bottom + 18, label, size=10.5, fill=theme["muted"],
               anchor="middle")
    c.line(pad_l, bottom, pad_l + plot_w, bottom, theme["rule"], 1.0)
    c.text(pad_l + plot_w / 2, bottom + 34, "input size", size=11,
           fill=theme["muted"], anchor="middle")

    # B4 and B8 are the same 1 GB input, so they are drawn side by side rather
    # than stacked on one x, where two ceiling lines would lie on top of each
    # other and the reader would read B4's marker as part of B8's.
    offsets = (0.0, -11.0, 11.0)
    for (value, measured, ceiling, label), offset in zip(points, offsets):
        x = sx(value) + offset
        c.line(x, sy(measured), x, sy(ceiling), theme["rule"], 1.2, dash="3 3")
        # The ceiling label sits to the right of its line and above it, where the
        # only thing it can cross is the neighbouring dashed line, which the halo
        # keeps it legible over.
        c.text(x + 9, sy(ceiling) - 6, megabytes(ceiling), size=10,
               fill=theme["muted"], halo=True)
        c.circle(x, sy(measured), 5.5, theme["accent"])
        # The left marker of a side-by-side pair labels to the left, or its text
        # would sit across the right marker's ceiling line.
        c.text(x + (-12.0 if offset < 0 else 12.0), sy(measured) + 4,
               f"{label} \u00b7 {megabytes(measured)}", size=11, weight="600",
               anchor="end" if offset < 0 else "start")

    c.text(0, 306,
           "median of 3 runs, sampled every 25 ms \u00b7 the working set moves "
           "about 15% between runs \u00b7 B4 and B8 share the 1 GB input",
           size=10.5, fill=theme["muted"])

    desc = (
        "Log-log scatter of peak memory against input size. B3 reads 127 MB "
        "against a 200 MB ceiling, B4 376 MB against 400 MB, B8 137 MB against "
        "500 MB. The three markers are not joined, because they are not a line."
    )
    return c.render("Peak memory against the published budget", desc)


def figure_facts(doc: str, name: str) -> dict:
    """The layout facts `chart-report.py` reads back, taken from the figure."""
    width = float(re.search(r'<svg[^>]*width="([\d.]+)"', doc).group(1))
    sizes = [float(size) for size in re.findall(r'font-size="([\d.]+)"', doc)]
    return {
        "view_box": re.search(r'viewBox="([^"]+)"', doc).group(1),
        "figure_width_pt": round(width * 0.75),
        "labels": doc.count("<text "),
        "font_pt_min": round(min(sizes) * 0.75, 1),
        "font_pt_max": round(max(sizes) * 0.75, 1),
        # What the smallest label comes out as once the README scales the figure
        # to the width it renders at, which is the number that decides whether a
        # reader can read it at all.
        "smallest_px_at_880px_wide": round(min(sizes) * (880.0 / width), 1),
        "note": FIGURE_NOTES[name],
    }


def main() -> int:
    data = json.loads(DATA.read_text(encoding="utf-8"))

    rows = []
    for label, values in data["stats_pipeline"].items():
        size, _, detail = label.partition(" stats")
        rows.append({
            "label": f"{size} stats",
            "detail": detail.strip().lstrip(",").strip(),
            "ob_s": values["ob_s"], "ref_s": values["ref_s"],
            "ob_mb": values["ob_mb"], "ref_mb": values["ref_mb"],
        })

    budget = [
        (363.0, 126.8, 200.0, "B3"),
        (1049.0, 375.9, 400.0, "B4"),
        (1049.0, 137.0, 500.0, "B8"),
    ]

    written = []
    facts: dict[str, dict[str, dict]] = {}
    for theme_name, theme in THEMES.items():
        for stem, doc in (
            ("chart-pipeline", chart_pipeline(theme, rows)),
            ("chart-source-maps", chart_source_maps(theme, data["source_maps"])),
            ("chart-memory-budget", chart_budget(theme, budget)),
        ):
            path = OUT / f"{stem}-{theme_name}.svg"
            path.write_text(doc, encoding="utf-8", newline="\n")
            written.append(path)
            name = stem.removeprefix("chart-")
            facts.setdefault(name, {})[theme_name] = figure_facts(doc, name)

    # The layout facts live beside the numbers they describe, and this script is
    # the thing that writes them: a figure that shrinks its smallest label has to
    # say so in the same run that moves it, or `chart-report.py` reads a stale
    # number and reports a legibility that is no longer there.
    data["figures"] = facts
    with DATA.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(json.dumps(data, indent=2, ensure_ascii=False) + "\n")

    for path in written:
        print(f"  {path.relative_to(ROOT)}  {path.stat().st_size:,} bytes")
    print(f"  {DATA.relative_to(ROOT)}  figures block rewritten")
    for name, themes in sorted(facts.items()):
        light = themes["light"]
        print(
            f"    {name:<14} {light['figure_width_pt']:>4}pt wide  "
            f"{light['labels']:>3} labels  "
            f"fonts {light['font_pt_min']}-{light['font_pt_max']}pt  "
            f"smallest {light['smallest_px_at_880px_wide']}px"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
