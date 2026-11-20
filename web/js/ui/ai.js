// Optional AI step writer (off by default). Sends a downscaled copy of the
// prepared image plus the local analysis to Claude using the user's own API key,
// and asks for image-specific wording of the existing steps. The local steps,
// overlays and layers are kept; only the text is replaced. Any failure falls
// back to the local steps — AI is never required.

export const SDK_URL = 'https://cdn.jsdelivr.net/npm/@anthropic-ai/sdk@1.4.0/+esm';
export const DEFAULT_MODEL = 'claude-opus-5-5';

const SYSTEM = `You are a patient drawing teacher for complete beginners.
You receive a reference picture and an automatic analysis of it (normalized 0-1 coordinates, x to the right, y down).
Rewrite each listed tutorial step so it talks about what is actually in THIS picture (name the real objects you see).
Rules: plain words, no art jargon without a short explanation, 2-4 short sentences per instruction, never invent details
that are not visible in the picture, keep the same step kinds and order, keep tips short and practical.`;

function schemaFor(kinds) {
  return {
    type: 'object', additionalProperties: false, required: ['subject', 'steps'],
    properties: {
      subject: { type: 'string', description: 'A short plain description of the main subject.' },
      steps: {
        type: 'array',
        items: {
          type: 'object', additionalProperties: false, required: ['kind', 'title', 'instruction', 'tip'],
          properties: { kind: { type: 'string', enum: kinds }, title: { type: 'string' }, instruction: { type: 'string' }, tip: { type: 'string' } },
        },
      },
    },
  };
}

/** Summary of the analysis small enough to send. */
export function analysisSummary(a) {
  return {
    subjectBox: a.subject.bbox, subjectAspect: a.subject.aspect, lowContrast: a.lowContrast,
    objects: a.objects.map((o) => ({ id: o.id, bbox: o.bbox, areaFrac: o.areaFrac })),
    shapes: a.shapes.map((s) => ({ kind: s.kind, cx: s.cx, cy: s.cy, rx: s.rx, ry: s.ry })),
    lightComesFrom: a.values.lightDir, darkestArea: a.values.darkPos, mostDetail: a.detail.pos,
  };
}

/** Merge AI text into local steps by kind (pure; exported for tests). */
export function mergeAiSteps(localSteps, ai) {
  const byKind = new Map((ai.steps || []).map((s) => [s.kind, s]));
  return localSteps.map((s) => {
    const r = byKind.get(s.kind);
    if (!r || !r.instruction) return s;
    return { ...s, title: r.title || s.title, instruction: r.instruction, tip: r.tip || s.tip, source: 'ai', localInstruction: s.instruction };
  });
}

/**
 * @param jpegBase64 downscaled prepared image
 * @returns {Promise<{subject, steps}>}
 */
export async function aiRewriteSteps({ apiKey, model = DEFAULT_MODEL, jpegBase64, analysis, steps, signal }) {
  if (!apiKey) throw new Error('No API key set.');
  const { default: Anthropic } = await import(SDK_URL);
  const client = new Anthropic({ apiKey, dangerouslyAllowBrowser: true });
  const kinds = steps.map((s) => s.kind);
  const resp = await client.beta.messages.create({
    model, max_tokens: 16000,
    betas: ['server-side-fallback-2026-07-01'], fallbacks: 'default',
    output_config: { effort: 'medium', format: { type: 'json_schema', schema: schemaFor(kinds) } },
    system: SYSTEM,
    messages: [{
      role: 'user',
      content: [
        { type: 'image', source: { type: 'base64', media_type: 'image/jpeg', data: jpegBase64 } },
        { type: 'text', text: `Analysis:\n${JSON.stringify(analysisSummary(analysis))}\n\nSteps to rewrite (kind: current text):\n${steps.map((s) => `- ${s.kind}: ${s.title}. ${s.instruction}`).join('\n')}` },
      ],
    }],
  }, { signal });
  if (resp.stop_reason === 'refusal') throw new Error('The AI service declined this image. Using the built-in steps instead.');
  if (resp.stop_reason === 'max_tokens') throw new Error('The AI response was cut off. Using the built-in steps instead.');
  const text = resp.content.filter((b) => b.type === 'text').map((b) => b.text).join('');
  let parsed;
  try { parsed = JSON.parse(text); } catch { throw new Error('The AI response could not be read. Using the built-in steps instead.'); }
  if (!parsed || !Array.isArray(parsed.steps)) throw new Error('The AI response was incomplete.');
  return { subject: String(parsed.subject || ''), steps: parsed.steps.filter((s) => kinds.includes(s.kind)), model: resp.model };
}
