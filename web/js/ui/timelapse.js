// Timelapse of a drawing session: a still from the camera every few seconds,
// played back afterwards as a short video (MediaRecorder on a canvas). Frames
// stay in memory on this device until you save the video.

const pickType = () => {
  if (typeof MediaRecorder === 'undefined') return null;
  for (const t of ['video/mp4;codecs=avc1', 'video/mp4', 'video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm']) {
    if (MediaRecorder.isTypeSupported(t)) return t;
  }
  return null;
};

export const timelapseSupported = () => !!pickType() && typeof HTMLCanvasElement.prototype.captureStream === 'function';

export class Timelapse {
  /** source: a <video>, <img> or <canvas> to photograph. */
  constructor(source, { everyMs = 2000, maxSide = 720, maxFrames = 900 } = {}) {
    this.source = source; this.everyMs = everyMs; this.maxSide = maxSide; this.maxFrames = maxFrames;
    this.frames = []; this.timer = 0; this.size = null;
  }
  get recording() { return !!this.timer; }
  get count() { return this.frames.length; }

  start() {
    if (this.timer) return;
    this.grab();
    this.timer = setInterval(() => this.grab(), this.everyMs);
  }
  stop() { clearInterval(this.timer); this.timer = 0; }
  clear() { this.stop(); this.frames = []; this.size = null; }

  grab() {
    const s = this.source;
    const w = s.videoWidth || s.naturalWidth || s.width, hgt = s.videoHeight || s.naturalHeight || s.height;
    if (!w || !hgt) return;
    // every frame has the size of the first, so the video doesn't jump
    if (!this.size) { const k = Math.min(1, this.maxSide / Math.max(w, hgt)); this.size = [Math.round(w * k) & ~1, Math.round(hgt * k) & ~1]; }
    const c = document.createElement('canvas');
    [c.width, c.height] = this.size;
    c.getContext('2d').drawImage(s, 0, 0, c.width, c.height);
    c.toBlob((b) => {
      if (!b) return;
      this.frames.push(b);
      if (this.frames.length > this.maxFrames) this.frames = this.frames.filter((_, i) => i % 2 === 0); // thin out evenly
    }, 'image/jpeg', 0.85);
  }

  /** Play the frames into a video; about `seconds` long, last frame held. */
  async render({ seconds = 15, onProgress } = {}) {
    const type = pickType();
    if (!type) throw new Error('This browser can\'t make videos.');
    if (!this.frames.length || !this.size) throw new Error('Nothing was recorded yet.');
    const fps = Math.max(2, Math.min(30, Math.round(this.frames.length / seconds)));
    const c = document.createElement('canvas');
    [c.width, c.height] = this.size;
    const g = c.getContext('2d');
    const stream = c.captureStream(fps);
    const rec = new MediaRecorder(stream, { mimeType: type, videoBitsPerSecond: 4_000_000 });
    const chunks = [];
    rec.ondataavailable = (e) => { if (e.data.size) chunks.push(e.data); };
    const done = new Promise((res) => { rec.onstop = res; });
    const show = async (blob) => { const bmp = await createImageBitmap(blob); g.drawImage(bmp, 0, 0, c.width, c.height); bmp.close && bmp.close(); };
    await show(this.frames[0]);
    rec.start(250);
    const step = 1000 / fps;
    let t = performance.now();
    for (let i = 0; i < this.frames.length; i++) {
      await show(this.frames[i]);
      if (onProgress) onProgress((i + 1) / this.frames.length);
      t += step;
      await new Promise((r) => setTimeout(r, Math.max(0, t - performance.now())));
    }
    await new Promise((r) => setTimeout(r, 1500)); // hold the finished drawing
    rec.stop();
    await done;
    stream.getTracks().forEach((tr) => tr.stop());
    return new Blob(chunks, { type: type.split(';')[0] });
  }
}

/** Save (or share, on phones) a finished timelapse video. */
export async function saveVideo(blob, name = 'piccasso-timelapse') {
  const ext = blob.type.includes('mp4') ? 'mp4' : 'webm';
  const file = new File([blob], `${name}.${ext}`, { type: blob.type });
  if (navigator.canShare && navigator.canShare({ files: [file] })) {
    try { await navigator.share({ files: [file], title: 'Piccasso timelapse' }); return; } catch (e) { if (e.name === 'AbortError') return; }
  }
  const a = document.createElement('a');
  a.href = URL.createObjectURL(file);
  a.download = file.name;
  document.body.appendChild(a); a.click(); a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 60_000);
}
