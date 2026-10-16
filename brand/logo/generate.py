import io
import math
import pathlib

import uharfbuzz as hb
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

HERE = pathlib.Path(__file__).resolve().parent
FONT = pathlib.Path("C:/Users/stillmvd/Projects/Kockers DS/fonts/KockersSans-Bold.woff2")
LIME = "#c6ec5a"
INK = "#1a2400"
INK_LIGHT = "#17171a"
INK_DARK = "#ececef"
SHARE = 0.72
BASE = dict(w=5.6, serif=6.0, spark=9.0, inner=0.28)
SMALL = dict(w=6.6, serif=6.6, spark=10.0, inner=0.34)


def star(cx, cy, r, inner):
    pts = []
    for i in range(8):
        rr = r if i % 2 == 0 else r * inner
        a = math.radians(-90 + i * 45)
        pts.append((cx + rr * math.cos(a), cy + rr * math.sin(a)))
    return pts


def caret(w, serif, spark, inner):
    lines = [[(0, 4), (0, 34)], [(-serif, 4), (serif, 4)], [(-serif, 34), (serif, 34)]]
    return lines, star(13, -2, spark, inner), w


def bounds(lines, spark, w):
    h = w / 2
    xs = [x + d for line in lines for x, _ in line for d in (-h, h)] + [x for x, _ in spark]
    ys = [y + d for line in lines for _, y in line for d in (-h, h)] + [y for _, y in spark]
    return min(xs), min(ys), max(xs), max(ys)


def glyph(shape, size, color, cx=32.0, cy=32.0):
    lines, spark, w = shape
    x0, y0, x1, y1 = bounds(lines, spark, w)
    s = size / max(x1 - x0, y1 - y0)

    def pt(x, y):
        return f"{cx + s * (x - (x0 + x1) / 2):.2f} {cy + s * (y - (y0 + y1) / 2):.2f}"

    d = "".join("M" + "L".join(pt(x, y) for x, y in line) for line in lines)
    star_d = "M" + "L".join(pt(x, y) for x, y in spark) + "Z"
    return (f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{w * s:.2f}" stroke-linecap="round"/>'
            f'<path d="{star_d}" fill="{color}" stroke="{color}" stroke-width="{0.8 * s:.2f}" stroke-linejoin="round"/>')


def mark_body(shape, r=32.0, cx=32.0, cy=32.0):
    return f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{LIME}"/>' + glyph(shape, 2 * r * SHARE, INK, cx, cy)


def document(body, view="0 0 64 64", width=64, height=64):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}" width="{width}" height="{height}">{body}</svg>'


def word(text, cap, x, baseline):
    font = TTFont(FONT)
    raw = io.BytesIO()
    font.flavor = None
    font.save(raw)
    hbfont = hb.Font(hb.Face(hb.Blob(raw.getvalue())))
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hbfont, buf, {"kern": True, "liga": True})
    k = cap / font["OS/2"].sCapHeight
    glyphs = font.getGlyphSet()
    order = font.getGlyphOrder()
    pen = SVGPathPen(glyphs, ntos=lambda v: f"{v:.2f}")
    pen_x = 0
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        tp = TransformPen(pen, (k, 0, 0, -k, x + (pen_x + pos.x_offset) * k, baseline - pos.y_offset * k))
        glyphs[order[info.codepoint]].draw(tp)
        pen_x += pos.x_advance
    return pen.getCommands(), pen_x * k


def lockup(shape, ink):
    mark_h = 40.0
    cap = mark_h * 0.6
    gap = mark_h * 0.36
    d, word_w = word("Ghbdtn", cap, 12 + mark_h + gap, 32 + cap / 2)
    width = 12 + mark_h + gap + word_w + 12
    body = mark_body(shape, mark_h / 2, 12 + mark_h / 2, 32) + f'<path d="{d}" fill="{ink}"/>'
    return document(body, f"0 0 {width:.2f} 64", f"{width:.2f}", 64)


def main():
    base = caret(**BASE)
    small = caret(**SMALL)
    files = {
        "mark.svg": document(mark_body(base)),
        "mark-small.svg": document(mark_body(small)),
        "mark-mono.svg": document(glyph(base, 58, "currentColor")),
        "favicon.svg": document(mark_body(small)),
        "app-icon.svg": document(mark_body(base), width=1024, height=1024),
        "lockup.svg": lockup(base, INK_LIGHT),
        "lockup-on-dark.svg": lockup(base, INK_DARK),
    }
    for name, markup in files.items():
        (HERE / name).write_text(markup, encoding="utf-8")
    print("SVG:", ", ".join(files))


if __name__ == "__main__":
    main()
