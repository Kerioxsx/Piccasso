// Screen Wake Lock with automatic re-acquire when the page becomes visible again.
// Browsers release wake locks whenever the tab is hidden; we re-request on return.
export class WakeLock {
  constructor(onChange = () => {}) { this.sentinel = null; this.wanted = false; this.onChange = onChange; this._vis = () => this._reacquire(); }
  static get supported() { return 'wakeLock' in navigator; }
  get active() { return !!this.sentinel && !this.sentinel.released; }
  async request() {
    this.wanted = true;
    document.addEventListener('visibilitychange', this._vis);
    if (!WakeLock.supported) { this.onChange({ active: false, reason: 'unsupported' }); return false; }
    try {
      this.sentinel = await navigator.wakeLock.request('screen');
      this.sentinel.addEventListener('release', () => this.onChange({ active: false, reason: this.wanted ? 'released' : 'stopped' }));
      this.onChange({ active: true });
      return true;
    } catch (e) {
      this.onChange({ active: false, reason: 'denied', message: e.message });
      return false;
    }
  }
  async _reacquire() { if (this.wanted && document.visibilityState === 'visible' && !this.active) await this.request(); }
  async release() {
    this.wanted = false;
    document.removeEventListener('visibilitychange', this._vis);
    if (this.sentinel) { try { await this.sentinel.release(); } catch { /* already released */ } }
    this.sentinel = null;
  }
}
