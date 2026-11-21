// AR / camera capability evaluation. `evaluateAr` is pure (testable);
// `probeAr` gathers the facts from the browser.

/**
 * @param f { secure, hasXR, immersiveAr (true|false|null), hitTest, hasCamera, cameraPermission ('granted'|'denied'|'prompt'|'unknown') }
 * @returns { mode: 'webxr'|'camera'|'manual', status, reasons[] }
 *   webxr  – real AR session (plane hit-test + anchor) available
 *   camera – live camera overlay with manual / paper-follow alignment
 *   manual – no camera: align over a photo of the paper instead
 */
export function evaluateAr(f) {
  const reasons = [];
  if (!f.secure) reasons.push('Camera and AR need a secure (https://) page.');
  const cameraOk = f.secure && f.hasCamera && f.cameraPermission !== 'denied';
  if (f.secure && !f.hasCamera) reasons.push('This browser does not expose a camera.');
  if (f.cameraPermission === 'denied') reasons.push('Camera permission was denied. You can allow it in your browser\'s site settings.');
  if (cameraOk && f.hasXR && f.immersiveAr === true) {
    return { mode: 'webxr', status: 'AR available (WebXR)', reasons };
  }
  if (f.hasXR && f.immersiveAr === false) reasons.push('WebXR is present but immersive AR is not supported on this device.');
  if (!f.hasXR) reasons.push('WebXR AR is not available in this browser (common on iPhone/iPad and desktop).');
  if (cameraOk) return { mode: 'camera', status: 'Camera overlay (no AR tracking)', reasons };
  return { mode: 'manual', status: 'AR unavailable', reasons };
}

export async function probeAr(nav = globalThis.navigator, win = globalThis) {
  const f = {
    secure: !!win.isSecureContext,
    hasXR: !!(nav && nav.xr),
    immersiveAr: null,
    hasCamera: !!(nav && nav.mediaDevices && nav.mediaDevices.getUserMedia),
    cameraPermission: 'unknown',
  };
  if (f.hasXR) {
    try { f.immersiveAr = await nav.xr.isSessionSupported('immersive-ar'); } catch { f.immersiveAr = false; }
  }
  try {
    if (nav.permissions) f.cameraPermission = (await nav.permissions.query({ name: 'camera' })).state;
  } catch { /* Firefox/Safari may not support querying camera */ }
  return { facts: f, ...evaluateAr(f) };
}
