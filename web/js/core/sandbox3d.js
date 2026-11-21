// 3D construction sandbox: simple meshes (cube, sphere, cylinder, cone,
// pyramid), an orbit camera with perspective or orthographic projection,
// flat shading from a movable light, shadows on the ground, and vanishing
// points of an object's edges. Plain geometry; no WebGL needed.

export const v3 = {
  add: (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]],
  sub: (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]],
  mul: (a, k) => [a[0] * k, a[1] * k, a[2] * k],
  dot: (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2],
  cross: (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]],
  len: (a) => Math.hypot(a[0], a[1], a[2]),
  norm: (a) => { const l = Math.hypot(a[0], a[1], a[2]) || 1; return [a[0] / l, a[1] / l, a[2] / l]; },
};
const rad = (d) => (d * Math.PI) / 180;

/** Rotation (degrees, applied X then Y then Z) of a point. */
export function rotate(p, [rx, ry, rz]) {
  let [x, y, z] = p;
  let c = Math.cos(rad(rx)), s = Math.sin(rad(rx)); [y, z] = [y * c - z * s, y * s + z * c];
  c = Math.cos(rad(ry)); s = Math.sin(rad(ry)); [x, z] = [x * c + z * s, -x * s + z * c];
  c = Math.cos(rad(rz)); s = Math.sin(rad(rz)); [x, y] = [x * c - y * s, x * s + y * c];
  return [x, y, z];
}

// ----- meshes: unit size, resting on y = 0 (y is up) -----
export function mesh(kind, seg = 24) {
  const V = [], F = [];
  const ring = (y, r, n) => Array.from({ length: n }, (_, i) => [Math.cos((i / n) * Math.PI * 2) * r, y, Math.sin((i / n) * Math.PI * 2) * r]);
  if (kind === 'cube') {
    for (const x of [-0.5, 0.5]) for (const y of [0, 1]) for (const z of [-0.5, 0.5]) V.push([x, y, z]);
    // index = xi*4 + yi*2 + zi
    F.push([0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]);
  } else if (kind === 'pyramid') {
    V.push([-0.5, 0, -0.5], [0.5, 0, -0.5], [0.5, 0, 0.5], [-0.5, 0, 0.5], [0, 1, 0]);
    F.push([0, 1, 2, 3], [0, 4, 1], [1, 4, 2], [2, 4, 3], [3, 4, 0]);
  } else if (kind === 'cylinder' || kind === 'cone') {
    const bottom = ring(0, 0.5, seg);
    V.push(...bottom);
    if (kind === 'cylinder') {
      V.push(...ring(1, 0.5, seg));
      for (let i = 0; i < seg; i++) { const j = (i + 1) % seg; F.push([i, seg + i, seg + j, j]); }
      F.push(Array.from({ length: seg }, (_, i) => seg + (seg - 1 - i)).reverse());
    } else {
      V.push([0, 1, 0]);
      for (let i = 0; i < seg; i++) F.push([i, seg, (i + 1) % seg]);
    }
    F.push(Array.from({ length: seg }, (_, i) => seg - 1 - i));
  } else if (kind === 'sphere') {
    const rows = Math.max(6, seg >> 1);
    for (let r = 0; r <= rows; r++) {
      const phi = (r / rows) * Math.PI, y = 0.5 - Math.cos(phi) * 0.5, rr = Math.sin(phi) * 0.5;
      for (let i = 0; i < seg; i++) V.push([Math.cos((i / seg) * Math.PI * 2) * rr, y, Math.sin((i / seg) * Math.PI * 2) * rr]);
    }
    for (let r = 0; r < rows; r++) for (let i = 0; i < seg; i++) {
      const j = (i + 1) % seg, a = r * seg + i, b = r * seg + j, c = (r + 1) * seg + j, d = (r + 1) * seg + i;
      F.push([a, d, c, b]);
    }
  }
  // sharp edges for construction lines: cube/pyramid all edges; round shapes only rims and silhouettes
  return { V, F, kind };
}

/** World-space vertices of an object {kind, pos, rot, scale:[sx,sy,sz]}. */
export function worldVerts(obj, m) {
  return m.V.map((p) => v3.add(rotate([p[0] * obj.scale[0], p[1] * obj.scale[1], p[2] * obj.scale[2]], obj.rot), obj.pos));
}

/** Orbit camera: target, yaw/pitch (degrees), distance, focal (perspective) or ortho. */
export function cameraBasis(cam) {
  const yaw = rad(cam.yaw), pitch = rad(cam.pitch);
  const dir = [Math.cos(pitch) * Math.sin(yaw), Math.sin(pitch), Math.cos(pitch) * Math.cos(yaw)]; // from target to eye
  const eye = v3.add(cam.target, v3.mul(dir, cam.dist));
  const fwd = v3.norm(v3.sub(cam.target, eye));
  const right = v3.norm(v3.cross(fwd, [0, 1, 0]));
  const up = v3.cross(right, fwd);
  return { eye, fwd, right, up };
}

/** Project a world point to screen [x, y, depth] (depth > 0 in front). */
export function project(p, cam, basis, W, H) {
  const d = v3.sub(p, basis.eye);
  const x = v3.dot(d, basis.right), y = v3.dot(d, basis.up), z = v3.dot(d, basis.fwd);
  const s = Math.min(W, H);
  if (cam.ortho) { const k = s / (cam.orthoSize || 4); return [W / 2 + x * k, H / 2 - y * k, z]; }
  const f = (cam.focal || 1.6) * s / 2;
  return [W / 2 + (x / Math.max(z, 1e-3)) * f, H / 2 - (y / Math.max(z, 1e-3)) * f, z];
}

/**
 * Vanishing point of a world direction: where parallel lines in that
 * direction meet on screen. null for orthographic views or directions parallel
 * to the picture plane (those lines stay parallel).
 */
export function vanishingPoint(dir, cam, basis, W, H) {
  if (cam.ortho) return null;
  const x = v3.dot(dir, basis.right), y = v3.dot(dir, basis.up), z = v3.dot(dir, basis.fwd);
  if (Math.abs(z) < 1e-6) return null;
  const f = (cam.focal || 1.6) * Math.min(W, H) / 2;
  return [W / 2 + (x / z) * f, H / 2 - (y / z) * f];
}

/** The three edge directions of an object (its local axes in the world). */
export const objectAxes = (obj) => [[1, 0, 0], [0, 1, 0], [0, 0, 1]].map((a) => rotate(a, obj.rot));

/** Direction towards the light from azimuth/elevation in degrees. */
export const lightDir = (az, el) => v3.norm([Math.cos(rad(el)) * Math.sin(rad(az)), Math.sin(rad(el)), Math.cos(rad(el)) * Math.cos(rad(az))]);

/** Shadow of a world point on the ground (y = 0) cast by a directional light. */
export function shadowPoint(p, L) {
  if (L[1] <= 1e-3) return null; // light at or below the horizon: no ground shadow
  const t = p[1] / L[1];
  return [p[0] - L[0] * t, 0, p[2] - L[2] * t];
}

/**
 * Draw list for a scene: faces in back-to-front order with their shade
 * (0 = dark .. 1 = lit) and screen points; shadows; edges.
 */
export function buildScene(objects, cam, light, W, H) {
  const basis = cameraBasis(cam);
  const faces = [], shadows = [];
  objects.forEach((obj, oi) => {
    const m = mesh(obj.kind);
    const wv = worldVerts(obj, m);
    const sv = wv.map((p) => project(p, cam, basis, W, H));
    const centre = wv.reduce((a, p) => v3.add(a, p), [0, 0, 0]).map((c) => c / wv.length);
    for (const f of m.F) {
      // Newell's method: a sound normal even for faces with repeated points (sphere poles)
      let nx = 0, ny = 0, nz = 0;
      for (let q = 0; q < f.length; q++) {
        const a = wv[f[q]], b = wv[f[(q + 1) % f.length]];
        nx += (a[1] - b[1]) * (a[2] + b[2]); ny += (a[2] - b[2]) * (a[0] + b[0]); nz += (a[0] - b[0]) * (a[1] + b[1]);
      }
      if (Math.hypot(nx, ny, nz) < 1e-12) continue;
      let n = v3.norm([nx, ny, nz]);
      const fc = f.reduce((acc, i) => v3.add(acc, wv[i]), [0, 0, 0]).map((x) => x / f.length);
      if (v3.dot(n, v3.sub(fc, centre)) < 0) n = v3.mul(n, -1); // outward
      const toEye = cam.ortho ? v3.mul(basis.fwd, -1) : v3.sub(basis.eye, fc);
      if (v3.dot(n, toEye) <= 0) continue; // back face
      const shade = Math.max(0, v3.dot(n, light));
      const depth = f.reduce((s, i) => s + sv[i][2], 0) / f.length;
      faces.push({ obj: oi, pts: f.map((i) => sv[i]), shade, depth, n });
    }
    // shadow: every face's projection on the ground (union drawn with one flat colour)
    for (const f of m.F) {
      const pts = f.map((i) => shadowPoint(wv[i], light));
      if (pts.some((p) => !p)) continue;
      shadows.push(pts.map((p) => project(p, cam, basis, W, H)));
    }
  });
  faces.sort((p, q) => q.depth - p.depth);
  return { faces, shadows, basis };
}

/** Starter scene. */
export function defaultScene() {
  return {
    objects: [
      { kind: 'cube', pos: [-1.2, 0, 0], rot: [0, 25, 0], scale: [1.4, 1.4, 1.4] },
      { kind: 'sphere', pos: [0.9, 0, 0.6], rot: [0, 0, 0], scale: [1.2, 1.2, 1.2] },
      { kind: 'cylinder', pos: [0.6, 0, -1.4], rot: [0, 0, 0], scale: [0.9, 1.8, 0.9] },
    ],
    cam: { target: [0, 0.6, 0], yaw: 35, pitch: 18, dist: 5.6, focal: 1.6, ortho: false, orthoSize: 6 },
    light: { az: -50, el: 45 },
  };
}
