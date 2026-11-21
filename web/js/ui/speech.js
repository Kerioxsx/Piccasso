// Optional audio readout of instructions via the Web Speech API.
export const speechSupported = () => 'speechSynthesis' in window && 'SpeechSynthesisUtterance' in window;
export function speak(text, { rate = 0.95 } = {}) {
  if (!speechSupported()) return false;
  window.speechSynthesis.cancel();
  const u = new SpeechSynthesisUtterance(text);
  u.rate = rate; u.lang = document.documentElement.lang || 'en';
  window.speechSynthesis.speak(u);
  return true;
}
export function stopSpeaking() { if (speechSupported()) window.speechSynthesis.cancel(); }
