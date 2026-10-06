export const MIN_DIMENSION = 0.5;

export function squarify(items, x, y, w, h) {
  const out = [];
  const total = items.reduce((a, b) => a + b.size, 0);
  if (total <= 0 || w <= 0 || h <= 0) return out;

  const scale = (w * h) / total;
  const rest = items.slice();
  let rect = { x, y, w, h };

  const worst = (row, side) => {
    const s = row.reduce((a, b) => a + b.size * scale, 0);
    if (s <= 0) return Infinity;
    const mx = row.reduce((a, b) => Math.max(a, b.size * scale), 0);
    const mn = row.reduce((a, b) => Math.min(a, b.size * scale), Infinity);
    return Math.max((side * side * mx) / (s * s), (s * s) / (side * side * mn));
  };

  while (rest.length) {
    const vertical = rect.w >= rect.h;
    const side = vertical ? rect.h : rect.w;
    const row = [];
    let best = Infinity;
    while (rest.length) {
      const candidate = worst(row.concat([rest[0]]), side);
      if (row.length && candidate > best) break;
      row.push(rest.shift());
      best = candidate;
    }

    const rowArea = row.reduce((a, b) => a + b.size * scale, 0);
    const thickness = rowArea / side;

    if (vertical) {
      let cy = rect.y;
      for (const it of row) {
        const hgt = (it.size * scale) / thickness;
        out.push({ node: it, x: rect.x, y: cy, w: thickness, h: hgt });
        cy += hgt;
      }
      rect = { x: rect.x + thickness, y: rect.y, w: rect.w - thickness, h: rect.h };
    } else {
      let cx = rect.x;
      for (const it of row) {
        const wid = (it.size * scale) / thickness;
        out.push({ node: it, x: cx, y: rect.y, w: wid, h: thickness });
        cx += wid;
      }
      rect = { x: rect.x, y: rect.y + thickness, w: rect.w, h: rect.h - thickness };
    }
    if (rect.w <= MIN_DIMENSION || rect.h <= MIN_DIMENSION) break;
  }
  return out;
}