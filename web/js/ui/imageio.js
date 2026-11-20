// Image import/export helpers (browser).
export const MAX_SOURCE_DIM = 2048;

/** Decode a File/Blob into a canvas, honouring EXIF orientation, capped at MAX_SOURCE_DIM. */
export async function fileToCanvas(file) {
  if (!file.type.startsWith('image/')) throw new Error('That file is not an image. Please choose a JPG, PNG or WebP picture.');
  let bmp;
  try {
    bmp = await createImageBitmap(file, { imageOrientation: 'from-image' });
  } catch {
    try { bmp = await loadViaImg(file); } catch {
      throw new Error(/heic|heif/i.test(file.type || file.name)
        ? 'This browser cannot open HEIC photos. On iPhone, choose "Most Compatible" in Settings → Camera → Formats, or export the photo as JPEG.'
        : 'This image could not be opened. It may be damaged or in an unsupported format.');
    }
  }
  const s = Math.min(1, MAX_SOURCE_DIM / Math.max(bmp.width, bmp.height));
  const c = document.createElement('canvas');
  c.width = Math.max(1, Math.round(bmp.width * s)); c.height = Math.max(1, Math.round(bmp.height * s));
  c.getContext('2d').drawImage(bmp, 0, 0, c.width, c.height);
  if (bmp.close) bmp.close();
  return c;
}

function loadViaImg(blob) {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(blob);
    const img = new Image();
    img.onload = () => { URL.revokeObjectURL(url); resolve(img); };
    img.onerror = () => { URL.revokeObjectURL(url); reject(new Error('decode failed')); };
    img.src = url;
  });
}

/** Canvas → { mime, width, height, data(base64) } for the project format. */
export async function canvasToSource(canvas, preferPng = false) {
  const mime = preferPng ? 'image/png' : 'image/jpeg';
  const blob = await new Promise((r) => canvas.toBlob(r, mime, 0.9));
  return { mime, width: canvas.width, height: canvas.height, data: await blobToBase64(blob) };
}

export function blobToBase64(blob) {
  return new Promise((resolve, reject) => {
    const fr = new FileReader();
    fr.onload = () => resolve(String(fr.result).split(',')[1]);
    fr.onerror = () => reject(fr.error);
    fr.readAsDataURL(blob);
  });
}

export const sourceToDataUrl = (src) => `data:${src.mime};base64,${src.data}`;

/** Project source → RGBA image object for the core engine. */
export async function sourceToRGBA(src) {
  const img = await loadViaImg(await (await fetch(sourceToDataUrl(src))).blob());
  const c = document.createElement('canvas');
  c.width = img.naturalWidth; c.height = img.naturalHeight;
  const ctx = c.getContext('2d', { willReadFrequently: true });
  ctx.drawImage(img, 0, 0);
  const d = ctx.getImageData(0, 0, c.width, c.height);
  return { width: d.width, height: d.height, data: d.data };
}

export function rgbaToCanvas(img, canvas = document.createElement('canvas')) {
  canvas.width = img.width; canvas.height = img.height;
  canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(img.data.buffer || img.data, img.data.byteOffset || 0, img.width * img.height * 4), img.width, img.height), 0, 0);
  return canvas;
}

export function canvasToRGBA(canvas) {
  const d = canvas.getContext('2d', { willReadFrequently: true }).getImageData(0, 0, canvas.width, canvas.height);
  return { width: d.width, height: d.height, data: d.data };
}

/** Small square-ish JPEG thumbnail data URL for project lists. */
export function thumbnail(canvasOrImg, size = 160) {
  const w = canvasOrImg.width, hh = canvasOrImg.height;
  const s = size / Math.max(w, hh);
  const c = document.createElement('canvas');
  c.width = Math.round(w * s); c.height = Math.round(hh * s);
  c.getContext('2d').drawImage(canvasOrImg, 0, 0, c.width, c.height);
  return c.toDataURL('image/jpeg', 0.75);
}
