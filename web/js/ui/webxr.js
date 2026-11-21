// WebXR immersive-ar session: hit-test plane detection, tap-to-place with
// anchors (when supported), and a textured quad showing the guide lying on the
// detected surface. Raw WebGL to stay dependency-free.
// Supported mainly by Chrome on ARCore-capable Android phones.

const VS = `attribute vec3 p; attribute vec2 uv; uniform mat4 mvp; varying vec2 v; void main(){ v=uv; gl_Position=mvp*vec4(p,1.0); }`;
const FS_TEX = `precision mediump float; varying vec2 v; uniform sampler2D t; uniform float a; void main(){ vec4 c=texture2D(t,v); gl_FragColor=vec4(c.rgb, c.a*a); }`;
const FS_RET = `precision mediump float; varying vec2 v; void main(){ float d=distance(v,vec2(0.5)); float ring=smoothstep(0.5,0.46,d)*smoothstep(0.36,0.40,d); if(ring<0.05) discard; gl_FragColor=vec4(1.0,1.0,1.0,ring*0.9); }`;

function mul(a, b) {
  const o = new Float32Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    let s = 0;
    for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
    o[c * 4 + r] = s;
  }
  return o;
}
function rotY(rad) { const c = Math.cos(rad), s = Math.sin(rad); return new Float32Array([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]); }
function scale(x, y, z) { return new Float32Array([x, 0, 0, 0, 0, y, 0, 0, 0, 0, z, 0, 0, 0, 0, 1]); }

function program(gl, fs) {
  const mk = (type, src) => { const s = gl.createShader(type); gl.shaderSource(s, src); gl.compileShader(s); return s; };
  const pr = gl.createProgram();
  gl.attachShader(pr, mk(gl.VERTEX_SHADER, VS)); gl.attachShader(pr, mk(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(pr);
  return pr;
}

/**
 * @param o { overlayCanvas, domRoot, sizeMeters:{w,h}, get:()=>({opacity, scale, rotationDeg, locked}), onStatus(state,text), onEnd() }
 * @returns controller { end(), recenter(), refreshTexture() }
 */
export async function startXR(o) {
  const session = await navigator.xr.requestSession('immersive-ar', {
    requiredFeatures: ['hit-test', 'local'],
    optionalFeatures: ['anchors', 'dom-overlay'],
    domOverlay: o.domRoot ? { root: o.domRoot } : undefined,
  });
  const glCanvas = document.createElement('canvas');
  const gl = glCanvas.getContext('webgl', { xrCompatible: true, alpha: true });
  await gl.makeXRCompatible();
  session.updateRenderState({ baseLayer: new XRWebGLLayer(session, gl) });
  const refSpace = await session.requestReferenceSpace('local');
  const viewerSpace = await session.requestReferenceSpace('viewer');
  const hitSource = await session.requestHitTestSource({ space: viewerSpace });

  const texProg = program(gl, FS_TEX), retProg = program(gl, FS_RET);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  // unit quad in the XZ plane (y = 0), uv flipped so the image reads correctly from above
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([
    -0.5, 0, -0.5, 0, 0, 0.5, 0, -0.5, 1, 0, 0.5, 0, 0.5, 1, 1,
    -0.5, 0, -0.5, 0, 0, 0.5, 0, 0.5, 1, 1, -0.5, 0, 0.5, 0, 1]), gl.STATIC_DRAW);
  const tex = gl.createTexture();
  const upload = () => {
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, false);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, o.overlayCanvas);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  };
  upload();

  let lastHit = null, anchor = null, placed = null, status = '';
  const setStatus = (s, t) => { if (s + t !== status) { status = s + t; o.onStatus(s, t); } };
  setStatus('searching', 'Move the phone slowly over the paper…');

  session.addEventListener('select', async () => {
    if (o.get().locked || !lastHit) return;
    placed = new Float32Array(lastHit.getPose(refSpace).transform.matrix);
    if (anchor) { try { anchor.delete(); } catch { /* ignore */ } anchor = null; }
    if (lastHit.createAnchor) { try { anchor = await lastHit.createAnchor(); } catch { anchor = null; } }
  });

  function drawQuad(prog, mvp, isTex, alpha) {
    gl.useProgram(prog);
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    const pl = gl.getAttribLocation(prog, 'p'), ul = gl.getAttribLocation(prog, 'uv');
    gl.enableVertexAttribArray(pl); gl.vertexAttribPointer(pl, 3, gl.FLOAT, false, 20, 0);
    gl.enableVertexAttribArray(ul); gl.vertexAttribPointer(ul, 2, gl.FLOAT, false, 20, 12);
    gl.uniformMatrix4fv(gl.getUniformLocation(prog, 'mvp'), false, mvp);
    if (isTex) { gl.activeTexture(gl.TEXTURE0); gl.bindTexture(gl.TEXTURE_2D, tex); gl.uniform1i(gl.getUniformLocation(prog, 't'), 0); gl.uniform1f(gl.getUniformLocation(prog, 'a'), alpha); }
    gl.drawArrays(gl.TRIANGLES, 0, 6);
  }

  function onFrame(t, frame) {
    session.requestAnimationFrame(onFrame);
    const pose = frame.getViewerPose(refSpace);
    const layer = session.renderState.baseLayer;
    gl.bindFramebuffer(gl.FRAMEBUFFER, layer.framebuffer);
    gl.clearColor(0, 0, 0, 0); gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    if (!pose) { setStatus('lost', 'Tracking lost: point the camera at the paper'); return; }
    const st = o.get();
    const hits = st.locked ? [] : frame.getHitTestResults(hitSource);
    lastHit = hits.length ? hits[0] : null;
    let model = placed;
    if (anchor && frame.trackedAnchors && frame.trackedAnchors.has(anchor)) {
      const ap = frame.getPose(anchor.anchorSpace, refSpace);
      if (ap) model = new Float32Array(ap.transform.matrix);
    }
    if (!model) setStatus(lastHit ? 'searching' : 'searching', lastHit ? 'Surface found: tap to place the guide' : 'Looking for a flat surface…');
    else setStatus(pose.emulatedPosition ? 'limited' : 'tracking', st.locked ? 'Alignment locked' : pose.emulatedPosition ? 'Limited tracking' : 'Tracking');
    gl.enable(gl.BLEND); gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    for (const view of pose.views) {
      const vp = layer.getViewport(view);
      gl.viewport(vp.x, vp.y, vp.width, vp.height);
      const pv = mul(view.projectionMatrix, view.transform.inverse.matrix);
      if (!model && lastHit) drawQuad(retProg, mul(pv, mul(new Float32Array(lastHit.getPose(refSpace).transform.matrix), scale(0.08, 1, 0.08))), false);
      if (model) {
        const m = mul(model, mul(rotY((st.rotationDeg * Math.PI) / 180), scale(o.sizeMeters.w * st.scale, 1, o.sizeMeters.h * st.scale)));
        drawQuad(texProg, mul(pv, m), true, st.opacity);
      }
    }
  }
  session.requestAnimationFrame(onFrame);
  session.addEventListener('end', () => { try { hitSource.cancel(); } catch { /* ignore */ } o.onEnd(); });
  return {
    end: () => session.end().catch(() => {}),
    recenter: () => { placed = null; if (anchor) { try { anchor.delete(); } catch { /* ignore */ } anchor = null; } },
    refreshTexture: upload,
    hasDomOverlay: !!session.domOverlayState,
  };
}
