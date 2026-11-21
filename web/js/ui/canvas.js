// Scene rendering: raster guide image + vector overlay layers from the analysis.
// Used by the workspace, locked reference mode, AR overlay and exports.

export const LAYER_COLORS = { shapes: '#c2185b', silhouette: '#d84315', internal: '#1565c0', frame: '#6d6a63', subject: '#ef6c00', landmark: '#2e7d32', light: '#f9a825' };

/** Contain-fit an image into a stage, then apply view zoom/pan (CSS px). */
export function computeRect(stageW, stageH, imgW, imgH, view = { zoom: 1, panX: 0, panY: 0 }, pad = 16) {
  const s = Math.min((stageW - 2 * pad) / imgW, (stageH - 2 * pad) / imgH);
  const w = imgW * s * view.zoom, h = imgH * s * view.zoom;
  return { x: (stageW - w) / 2 + view.panX, y: (stageH - h) / 2 + view.panY, w, h };
}

/** Resize a canvas backing store for crisp drawing; returns the 2D context scaled to CSS px. */
export function fitCanvas(canvas) {
  const dpr = Math.min(window.devicePixelRatio || 1, 3);
  const r = canvas.getBoundingClientRect();
  const W = Math.max(1, Math.round(r.width * dpr)), H = Math.max(1, Math.round(r.height * dpr));
  if (canvas.width !== W || canvas.height !== H) { canvas.width = W; canvas.height = H; }
  const ctx = canvas.getContext('2d');
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { ctx, w: r.width, h: r.height };
}

function polyline(ctx, pts, rect, closed) {
  if (!pts.length) return;
  ctx.beginPath();
  pts.forEach(([x, y], i) => (i ? ctx.lineTo : ctx.moveTo).call(ctx, rect.x + x * rect.w, rect.y + y * rect.h));
  if (closed) ctx.closePath();
  ctx.stroke();
}

function label(ctx, text, x, y, color, lw) {
  ctx.save();
  ctx.font = `600 ${Math.max(11, lw * 6)}px system-ui, sans-serif`;
  const w = ctx.measureText(text).width + 8, hh = Math.max(16, lw * 8);
  ctx.fillStyle = color; ctx.globalAlpha = 0.9;
  ctx.fillRect(x - w / 2, y - hh / 2, w, hh);
  ctx.globalAlpha = 1; ctx.fillStyle = '#fff'; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.fillText(text, x, y + 0.5);
  ctx.restore();
}

/**
 * Draw vector layers for an analysis into `rect` (where the whole image maps).
 * layers: frame | grid | subjectBox | shapes | measures | silhouette | internal | landmarks | light
 */
export function drawLayers(ctx, analysis, layers, rect, { labels = true, alpha = 1, grid = null } = {}) {
  if (!analysis || !layers || !layers.length) return;
  const lw = Math.max(1.5, rect.w / 420);
  const has = (l) => layers.includes(l);
  ctx.save();
  ctx.globalAlpha = alpha;
  ctx.lineJoin = 'round'; ctx.lineCap = 'round';
  const [bx0, by0, bx1, by1] = analysis.subject.bbox;
  const sb = { x: rect.x + bx0 * rect.w, y: rect.y + by0 * rect.h, w: (bx1 - bx0) * rect.w, h: (by1 - by0) * rect.h };

  if (has('frame')) {
    ctx.strokeStyle = LAYER_COLORS.frame; ctx.lineWidth = lw; ctx.setLineDash([lw * 4, lw * 3]);
    ctx.strokeRect(rect.x, rect.y, rect.w, rect.h); ctx.setLineDash([]);
    ctx.fillStyle = LAYER_COLORS.frame;
    for (const [x, y] of [[0, 0], [1, 0], [1, 1], [0, 1]]) { ctx.beginPath(); ctx.arc(rect.x + x * rect.w, rect.y + y * rect.h, lw * 2.5, 0, 7); ctx.fill(); }
  }
  if (has('grid')) {
    ctx.strokeStyle = LAYER_COLORS.frame; ctx.globalAlpha = alpha * 0.45; ctx.lineWidth = lw * 0.7;
    for (const f of [1 / 3, 2 / 3]) {
      polyline(ctx, [[f, 0], [f, 1]], rect, false); polyline(ctx, [[0, f], [1, f]], rect, false);
    }
    ctx.globalAlpha = alpha;
  }
  if (has('subjectBox')) {
    ctx.strokeStyle = LAYER_COLORS.subject; ctx.lineWidth = lw; ctx.setLineDash([lw * 3, lw * 2]);
    ctx.strokeRect(sb.x, sb.y, sb.w, sb.h); ctx.setLineDash([]);
  }
  if (has('measures')) {
    ctx.strokeStyle = LAYER_COLORS.subject; ctx.lineWidth = lw;
    const off = lw * 6;
    ctx.beginPath();
    ctx.moveTo(sb.x, sb.y - off); ctx.lineTo(sb.x + sb.w, sb.y - off);
    ctx.moveTo(sb.x - off, sb.y); ctx.lineTo(sb.x - off, sb.y + sb.h);
    ctx.stroke();
    ctx.globalAlpha = alpha * 0.5; ctx.setLineDash([lw * 2, lw * 3]);
    ctx.beginPath(); ctx.moveTo(sb.x + sb.w / 2, sb.y); ctx.lineTo(sb.x + sb.w / 2, sb.y + sb.h);
    ctx.moveTo(sb.x, sb.y + sb.h / 2); ctx.lineTo(sb.x + sb.w, sb.y + sb.h / 2); ctx.stroke();
    ctx.setLineDash([]); ctx.globalAlpha = alpha;
    const a = analysis.subject.aspect;
    if (labels) {
      label(ctx, a >= 1 ? `${(Math.round(a * 10) / 10).toFixed(1)} wide` : '1 wide', sb.x + sb.w / 2, sb.y - off, LAYER_COLORS.subject, lw);
      label(ctx, a >= 1 ? '1 tall' : `${(Math.round(10 / a) / 10).toFixed(1)} tall`, sb.x - off, sb.y + sb.h / 2, LAYER_COLORS.subject, lw);
    }
  }
  if (has('shapes')) {
    ctx.strokeStyle = LAYER_COLORS.shapes; ctx.lineWidth = lw * 1.2;
    analysis.shapes.forEach((s, i) => {
      const cx = rect.x + s.cx * rect.w, cy = rect.y + s.cy * rect.h, rx = s.rx * rect.w, ry = s.ry * rect.w;
      ctx.beginPath();
      if (s.kind === 'box') ctx.rect(cx - rx, cy - ry, rx * 2, ry * 2);
      else ctx.ellipse(cx, cy, Math.max(rx, 1), Math.max(ry, 1), (s.angle * Math.PI) / 180, 0, Math.PI * 2);
      ctx.stroke();
      ctx.beginPath(); ctx.moveTo(cx - lw * 3, cy); ctx.lineTo(cx + lw * 3, cy); ctx.moveTo(cx, cy - lw * 3); ctx.lineTo(cx, cy + lw * 3); ctx.stroke();
      if (labels) label(ctx, String(i + 1), cx, cy - ry - lw * 5, LAYER_COLORS.shapes, lw);
    });
  }
  if (has('silhouette')) {
    ctx.strokeStyle = LAYER_COLORS.silhouette; ctx.lineWidth = lw * 1.6;
    for (const c of analysis.silhouettes) polyline(ctx, c.points, rect, c.closed);
  }
  if (has('internal')) {
    ctx.strokeStyle = LAYER_COLORS.internal; ctx.lineWidth = lw * 1.1; ctx.setLineDash([lw * 3, lw * 2]);
    for (const c of analysis.internals) polyline(ctx, c.points, rect, c.closed);
    ctx.setLineDash([]);
  }
  if (has('landmarks')) {
    for (const l of analysis.landmarks) {
      const x = rect.x + l.x * rect.w, y = rect.y + l.y * rect.h;
      ctx.fillStyle = LAYER_COLORS.landmark; ctx.beginPath(); ctx.arc(x, y, lw * 2.6, 0, 7); ctx.fill();
      ctx.strokeStyle = '#fff'; ctx.lineWidth = lw * 0.8; ctx.stroke();
    }
  }
  if (has('cmgrid') && grid && grid.fx > 0 && grid.fy > 0) {
    ctx.save();
    ctx.strokeStyle = '#2f7466'; ctx.globalAlpha = alpha * 0.55; ctx.lineWidth = Math.max(1, lw * 0.6);
    ctx.beginPath();
    for (let x = grid.fx; x < 1 - 1e-6; x += grid.fx) { ctx.moveTo(rect.x + x * rect.w, rect.y); ctx.lineTo(rect.x + x * rect.w, rect.y + rect.h); }
    for (let y = grid.fy; y < 1 - 1e-6; y += grid.fy) { ctx.moveTo(rect.x, rect.y + y * rect.h); ctx.lineTo(rect.x + rect.w, rect.y + y * rect.h); }
    ctx.stroke();
    ctx.restore();
  }
  const tech = analysis.technique;
  if (has('pressure') && tech && tech.bands.length) {
    const order = tech.bands.map((b) => b.band).sort((a, b) => b - a); // lightest first → 1
    for (const b of tech.bands) {
      const n = order.indexOf(b.band) + 1;
      const x = rect.x + b.x * rect.w, y = rect.y + b.y * rect.h, r = Math.max(11, lw * 6);
      const shade = Math.round(235 - (195 * (n - 1)) / Math.max(1, order.length - 1));
      ctx.beginPath(); ctx.arc(x, y, r, 0, 7);
      ctx.fillStyle = `rgb(${shade},${shade},${shade})`; ctx.fill();
      ctx.lineWidth = lw; ctx.strokeStyle = '#24221e'; ctx.stroke();
      ctx.fillStyle = shade < 128 ? '#fff' : '#24221e';
      ctx.font = `700 ${Math.round(r * 1.05)}px system-ui, sans-serif`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
      ctx.fillText(String(n), x, y + 0.5);
    }
  }
  if (has('palette') && tech && tech.palette.length) {
    for (const c of tech.palette) {
      const x = rect.x + c.x * rect.w, y = rect.y + c.y * rect.h, r = Math.max(10, lw * 5.5);
      ctx.beginPath(); ctx.arc(x, y, r, 0, 7);
      ctx.fillStyle = `rgb(${c.rgb.join(',')})`; ctx.fill();
      ctx.lineWidth = lw * 1.2; ctx.strokeStyle = '#fff'; ctx.stroke();
      ctx.lineWidth = lw * 0.6; ctx.strokeStyle = '#24221e'; ctx.beginPath(); ctx.arc(x, y, r + lw * 0.9, 0, 7); ctx.stroke();
    }
  }
  if (has('light') && analysis.values.lightDir !== 'even') {
    const code = analysis.values.lightDir;
    const dx = code[1] === 'l' ? -1 : code[1] === 'r' ? 1 : 0, dy = code[0] === 't' ? -1 : code[0] === 'b' ? 1 : 0;
    const cx = sb.x + sb.w / 2, cy = sb.y + sb.h / 2, len = Math.max(sb.w, sb.h) * 0.62;
    const sx = cx + dx * len, sy = cy + dy * len;
    ctx.strokeStyle = LAYER_COLORS.light; ctx.fillStyle = LAYER_COLORS.light; ctx.lineWidth = lw * 1.6;
    ctx.beginPath(); ctx.arc(sx, sy, lw * 6, 0, 7); ctx.fill();
    const ex = cx + dx * len * 0.35, ey = cy + dy * len * 0.35;
    ctx.beginPath(); ctx.moveTo(sx - dx * lw * 8, sy - dy * lw * 8); ctx.lineTo(ex, ey); ctx.stroke();
    const ang = Math.atan2(ey - sy, ex - sx);
    ctx.beginPath(); ctx.moveTo(ex, ey);
    ctx.lineTo(ex - Math.cos(ang - 0.5) * lw * 6, ey - Math.sin(ang - 0.5) * lw * 6);
    ctx.lineTo(ex - Math.cos(ang + 0.5) * lw * 6, ey - Math.sin(ang + 0.5) * lw * 6); ctx.closePath(); ctx.fill();
  }
  ctx.restore();
}

/**
 * Compose a full guide frame at the base image's resolution: raster + layers.
 * Returns a new canvas. `transparentPaper` turns white into transparency (for AR overlays).
 */
export function composeGuide(base, analysis, layers, { width, height, transparentPaper = false, baseAlpha = 1, grid = null } = {}) {
  const w = width || (base && base.width) || 1024, hh = height || (base && base.height) || Math.round(w / (analysis ? analysis.aspect : 1));
  const c = document.createElement('canvas');
  c.width = w; c.height = hh;
  const ctx = c.getContext('2d');
  if (!transparentPaper) { ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, w, hh); }
  if (base) {
    ctx.globalAlpha = baseAlpha;
    ctx.drawImage(base, 0, 0, w, hh);
    ctx.globalAlpha = 1;
    if (transparentPaper) {
      // Map luminance to alpha so white paper becomes see-through and lines stay dark.
      const d = ctx.getImageData(0, 0, w, hh);
      for (let i = 0; i < d.data.length; i += 4) {
        const l = (77 * d.data[i] + 150 * d.data[i + 1] + 29 * d.data[i + 2]) >> 8;
        d.data[i + 3] = Math.round((255 - l) * (d.data[i + 3] / 255));
      }
      ctx.putImageData(d, 0, 0);
    }
  }
  drawLayers(ctx, analysis, layers, { x: 0, y: 0, w, h: hh }, { labels: !transparentPaper, grid });
  return c;
}

/** Draw strokes progressively up to `progress` (0..1 of total length) with a pencil tip. */
export function drawMotion(ctx, strokes, rect, progress, { color = '#2f7466', reduced = false } = {}) {
  const lw = Math.max(2, rect.w / 260);
  const P = (p) => [rect.x + p[0] * rect.w, rect.y + p[1] * rect.h];
  const lens = strokes.map((s) => { let L = 0; for (let i = 1; i < s.points.length; i++) { const a = P(s.points[i - 1]), b = P(s.points[i]); L += Math.hypot(b[0] - a[0], b[1] - a[1]); } return Math.max(L, 30); });
  const total = lens.reduce((a, b) => a + b, 0);
  let remaining = reduced ? total : progress * total;
  let tip = null, tipAngle = 0;
  ctx.save();
  ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  strokes.forEach((s, si) => {
    if (remaining <= 0) return;
    const pts = s.points.map(P);
    // numbered start marker
    ctx.fillStyle = color; ctx.beginPath(); ctx.arc(pts[0][0], pts[0][1], lw * 2.2, 0, 7); ctx.fill();
    ctx.fillStyle = '#fff'; ctx.font = `700 ${Math.round(lw * 2.6)}px system-ui`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.fillText(String(si + 1), pts[0][0], pts[0][1] + 0.5);
    ctx.strokeStyle = color; ctx.lineWidth = lw; ctx.globalAlpha = 0.9;
    ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]);
    let left = Math.min(remaining, lens[si]);
    remaining -= lens[si];
    for (let i = 1; i < pts.length && left > 0; i++) {
      const a = pts[i - 1], b = pts[i], d = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1e-6;
      if (left >= d) { ctx.lineTo(b[0], b[1]); left -= d; tip = b; tipAngle = Math.atan2(b[1] - a[1], b[0] - a[0]); }
      else { const t = left / d; tip = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]; ctx.lineTo(tip[0], tip[1]); tipAngle = Math.atan2(b[1] - a[1], b[0] - a[0]); left = 0; }
    }
    if (s.kind === 'dot') { ctx.beginPath(); ctx.arc(pts[0][0], pts[0][1], lw * 1.2, 0, 7); ctx.fill(); }
    ctx.stroke();
    if (reduced && pts.length > 1) { // direction arrow at the end
      const a = pts[pts.length - 2], b = pts[pts.length - 1], ang = Math.atan2(b[1] - a[1], b[0] - a[0]);
      ctx.beginPath(); ctx.moveTo(b[0], b[1]);
      ctx.lineTo(b[0] - Math.cos(ang - 0.5) * lw * 4, b[1] - Math.sin(ang - 0.5) * lw * 4);
      ctx.lineTo(b[0] - Math.cos(ang + 0.5) * lw * 4, b[1] - Math.sin(ang + 0.5) * lw * 4); ctx.closePath(); ctx.fillStyle = color; ctx.fill();
    }
  });
  if (tip && !reduced) { // pencil tip
    ctx.translate(tip[0], tip[1]); ctx.rotate(tipAngle + Math.PI * 0.75);
    ctx.globalAlpha = 1;
    ctx.fillStyle = '#f2c14e'; ctx.strokeStyle = '#24221e'; ctx.lineWidth = 1.5;
    ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(lw * 2.4, lw * 5); ctx.lineTo(lw * 2.4, lw * 22); ctx.lineTo(-lw * 2.4, lw * 22); ctx.lineTo(-lw * 2.4, lw * 5); ctx.closePath(); ctx.fill(); ctx.stroke();
    ctx.fillStyle = '#24221e'; ctx.beginPath(); ctx.moveTo(0, 0); ctx.lineTo(lw * 1, lw * 2); ctx.lineTo(-lw * 1, lw * 2); ctx.closePath(); ctx.fill();
  }
  ctx.restore();
  return total;
}
