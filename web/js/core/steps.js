// Turns an analysis (see analysis.js) into an image-specific step-by-step
// tutorial using the shared text templates in shared/strings.<lang>.json.
// Mirrored in rust/crates/piccasso-core/src/steps.rs.

import { posCode } from './analysis.js';

/** Fill "{name}" placeholders. Unknown names are left untouched. */
export function fill(tpl, vars) {
  return tpl.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
}

/** One decimal, symmetric rounding (matches Rust fmt1). */
export function fmt1(v) {
  const r = Math.floor(Math.abs(v) * 10 + 0.5) / 10;
  return (v < 0 ? '-' : '') + r.toFixed(1);
}

export function joinList(items) {
  if (items.length <= 1) return items.join('');
  return items.slice(0, -1).join(', ') + ' and ' + items[items.length - 1];
}

export function aspectPhrase(a, S) {
  if (a < 0.83) return fill(S.aspect.taller, { r: fmt1(1 / a) });
  if (a > 1.2) return fill(S.aspect.wider, { r: fmt1(a) });
  return S.aspect.square;
}

/** Drawing-area size on paper (cm) for an image of aspect `imgAspect`. */
export function paperLayout(paperCfg, presets, imgAspect, defaultMargin) {
  let preset = presets.find((p) => p.id === (paperCfg && paperCfg.preset)) || presets[0];
  if (paperCfg && paperCfg.preset === 'custom' && paperCfg.widthMm > 0 && paperCfg.heightMm > 0) {
    preset = { id: 'custom', label: 'Custom', widthMm: Math.min(paperCfg.widthMm, paperCfg.heightMm), heightMm: Math.max(paperCfg.widthMm, paperCfg.heightMm) };
  }
  const margin = paperCfg && paperCfg.marginMm != null ? paperCfg.marginMm : defaultMargin;
  let orient = paperCfg && paperCfg.orientation;
  if (orient !== 'portrait' && orient !== 'landscape') orient = imgAspect > 1.05 ? 'landscape' : 'portrait';
  let pw = preset.widthMm, ph = preset.heightMm;
  if (orient === 'landscape') [pw, ph] = [ph, pw];
  const aw = Math.max(10, pw - 2 * margin), ah = Math.max(10, ph - 2 * margin);
  let dw, dh;
  if (imgAspect > aw / ah) { dw = aw; dh = aw / imgAspect; } else { dh = ah; dw = ah * imgAspect; }
  return { preset: preset.id, label: preset.label, orient, paperW: pw, paperH: ph, margin, drawW: dw, drawH: dh,
    drawX: (pw - dw) / 2, drawY: (ph - dh) / 2 };
}

const shapeArea = (s) => s.rx * s.ry;

function sizeWord(s, largest, S) {
  const r = shapeArea(s) / Math.max(shapeArea(largest), 1e-9);
  return r > 0.5 ? S.sizes[0] : r > 0.15 ? S.sizes[1] : S.sizes[2];
}

/** Count vertices where the outline turns by more than 35°. */
export function countTurns(points, aspect) {
  const n = points.length;
  if (n < 3) return n;
  let turns = 0;
  for (let i = 0; i < n; i++) {
    const a = points[(i + n - 1) % n], b = points[i], c = points[(i + 1) % n];
    const ux = (b[0] - a[0]) * aspect, uy = b[1] - a[1];
    const vx = (c[0] - b[0]) * aspect, vy = c[1] - b[1];
    const lu = Math.hypot(ux, uy), lv = Math.hypot(vx, vy);
    if (lu === 0 || lv === 0) continue;
    const cos = (ux * vx + uy * vy) / (lu * lv);
    if (cos < Math.cos((35 * Math.PI) / 180)) turns++;
  }
  return turns;
}

/**
 * @param analysis  result of analyze()
 * @param prep      project.prep
 * @param shared    { difficulty, strings, glossary, paper, stepKinds }
 */
export function generateSteps(analysis, prep, shared) {
  const S = shared.strings;
  const level = shared.difficulty.levels[prep.difficulty] || shared.difficulty.levels.easy;
  const kindsById = Object.fromEntries(shared.stepKinds.kinds.map((k) => [k.id, k]));
  const P = (code) => S.positions[code] || S.positions.mc;

  let kinds = level.steps.slice();
  if (analysis.internals.length < 2) kinds = kinds.filter((k) => k !== 'forms');
  if (analysis.internals.length === 0 && analysis.detail.density < 0.02) kinds = kinds.filter((k) => k !== 'features');
  if (prep.style === 'contour_only') kinds = kinds.filter((k) => k !== 'values' && k !== 'details');
  const tech = analysis.technique || { bands: [], palette: [] };
  if (tech.bands.length < 2 || prep.style === 'contour_only') kinds = kinds.filter((k) => k !== 'pressure');
  if (prep.medium === 'color' && tech.palette.length && prep.style !== 'contour_only') {
    const at = kinds.indexOf('values');
    kinds.splice(at < 0 ? kinds.length : at + 1, 0, 'color');
  }
  if (prep.style === 'shading_guide' && !kinds.includes('values')) {
    const at = kinds.indexOf('clean');
    kinds.splice(at < 0 ? kinds.length : at + 1, 0, 'values');
  }

  const layout = paperLayout(prep.paper, shared.paper.presets, analysis.aspect, shared.paper.defaultMarginMm);
  const shapes = analysis.shapes.slice().sort((a, b) => shapeArea(b) - shapeArea(a) || (a.id < b.id ? -1 : 1));
  const largest = shapes[0];
  const kindWord = (s) => S.shapeKinds[s.kind] || s.kind;
  const shapePhrases = shapes.slice(0, 3).map((s) =>
    fill(S.shapePhrase || 'a {size} {kind} near {pos}', { size: sizeWord(s, largest, S), kind: kindWord(s), pos: P(posCode(s.cx, s.cy)) }));

  const aspectP = aspectPhrase(analysis.subject.aspect, S);
  let compareList;
  if (shapes.length >= 2) {
    const a = shapes[0], b = shapes[1];
    const r = Math.sqrt(shapeArea(a) / Math.max(shapeArea(b), 1e-9));
    const ratio = r < 1.25 ? S.ratioWords.same : r < 2.5 ? S.ratioWords.twice : fill(S.ratioWords.x, { n: Math.round(r) });
    compareList = fill(S.compareItem, { a: sizeWord(a, largest, S), kindA: kindWord(a), b: sizeWord(b, largest, S), kindB: kindWord(b), ratio });
  } else {
    compareList = fill(S.compareSingle, { aspectPhrase: aspectP });
  }

  const pressureList = joinList(tech.bands.slice().sort((a, b) => b.band - a.band).map((b) => {
    const e = pressureFor(tech.bands, b, shared.pressure);
    return fill(S.pressureItem, { label: S.pressureLabels[e.label] || e.label, pct: e.pct, grade: e.grade, pos: P(b.pos) });
  }));
  const seen = new Set();
  const paletteList = joinList(tech.palette.map((c) => ({ name: colorName(c.rgb, shared.colors.colors), pos: c.pos }))
    .filter((c) => (seen.has(c.name) ? false : seen.add(c.name)))
    .map((c) => fill(S.paletteItem, { name: c.name, where: P(c.pos) })));
  const gridMm = prep.paper && prep.paper.gridCm > 0 ? prep.paper.gridCm * 10 : 0;
  const sil = analysis.silhouettes[0];
  const topLm = analysis.landmarks.find((l) => l.name === 'top');
  const bigInternal = analysis.internals[0];
  const vars = {
    orient: layout.orient, marginCm: fmt1(layout.margin / 10),
    drawWcm: fmt1(layout.drawW / 10), drawHcm: fmt1(layout.drawH / 10),
    shapeList: joinList(shapePhrases), aspectPhrase: aspectP, compareList,
    startPos: P(topLm ? posCode(topLm.x, topLm.y) : 'tc'),
    cornerCount: sil ? countTurns(sil.points, analysis.aspect) : 4,
    internalCount: analysis.internals.length,
    internalPos: P(bigInternal ? posCode(bigInternal.centroid[0], bigInternal.centroid[1], analysis.subject.bbox) : 'mc'),
    detailPos: P(analysis.detail.pos),
    darkPos: P(analysis.values.darkPos),
    lightSentence: analysis.values.lightDir === 'even' ? S.light.even : fill(S.light.dir, { dir: P(analysis.values.lightDir) }),
    bandCount: tech.bands.length, pressureList, highlightTip: S.highlightTip,
    shadeMethod: S.shadeMethods[prep.difficulty] || S.shadeMethods.easy,
    paletteList, colorTip: S.colorTip,
  };

  return kinds.map((kind, i) => {
    const T = S.steps[kind];
    const def = kindsById[kind] || { base: 'light', layers: [] };
    let instruction = fill(T.instruction, vars);
    if (kind === 'page' && analysis.lowContrast) instruction += ' ' + S.fallback.lowContrast;
    let layers = def.layers.slice();
    if (kind === 'page' && gridMm) {
      instruction += ' ' + fill(S.grid, { gridCm: fmt1(gridMm / 10), cols: Math.ceil(layout.drawW / gridMm - 1e-9), rows: Math.ceil(layout.drawH / gridMm - 1e-9) });
      layers.push('cmgrid');
    }
    let base = def.base;
    if (kind === 'contours' && analysis.silhouettes.length === 0) { base = 'outline'; layers = ['shapes']; }
    return {
      id: `step-${i + 1}-${kind}`, kind, title: T.title, instruction, tip: T.tip,
      checks: (T.checks || []).map((c) => fill(c, vars)),
      motion: T.motion || '',
      base, layers, terms: (shared.glossary.stepTerms[kind] || []).slice(),
    };
  });
}

/**
 * Pressure entry for a band, by its rank among the bands present (darkest first),
 * using the table for that many tones (2 tones: the extremes of the 3-tone table).
 */
export function pressureFor(bands, band, pressure) {
  const order = bands.map((b) => b.band).sort((a, b) => a - b);
  const n = order.length, rank = order.indexOf(band.band);
  const table = pressure.byLevels[String(Math.min(5, Math.max(3, n)))];
  if (n < 3) return rank === 0 ? table[0] : table[table.length - 1];
  return table[rank];
}

const chroma = (c) => Math.max(c[0], c[1], c[2]) - Math.min(c[0], c[1], c[2]);

/** Nearest plain colour name ("redmean" weighted RGB + 2×chroma² difference; first wins on ties). */
export function colorName(rgb, table) {
  let best = table[0][0], bd = Infinity;
  for (const [name, c] of table) {
    const rm = (rgb[0] + c[0]) / 2, dr = rgb[0] - c[0], dg = rgb[1] - c[1], db = rgb[2] - c[2], dc = chroma(rgb) - chroma(c);
    const d = (2 + rm / 256) * dr * dr + 4 * dg * dg + (2 + (255 - rm) / 256) * db * db + 2 * dc * dc;
    if (d < bd) { bd = d; best = name; }
  }
  return best;
}

/** Grid cell size as fractions of the drawing (for the `cmgrid` layer). */
export function gridFractions(layout, gridCm) {
  const mm = gridCm * 10;
  return { fx: mm / layout.drawW, fy: mm / layout.drawH };
}
