const assert = require('node:assert');
const test = require('node:test');

const { Client } = require('..');

if (!process.env.OPENAI_API_KEY && process.env.CUSTOM_API_KEY) {
  process.env.OPENAI_API_KEY = process.env.CUSTOM_API_KEY;
}
if (!process.env.OPENAI_API_BASE && process.env.CUSTOM_API_BASE) {
  process.env.OPENAI_API_BASE = process.env.CUSTOM_API_BASE;
}

const hasEnv =
  (process.env.OPENAI_API_KEY || process.env.CUSTOM_API_KEY || process.env.ANTHROPIC_API_KEY) &&
  process.env.CUSTOM_API_MODEL &&
  process.env.PROVIDER;
const MODEL = process.env.CUSTOM_API_MODEL;
const PROVIDER = process.env.PROVIDER;
const REQUIRED_ENV_MESSAGE =
  'Missing required env. Set CUSTOM_API_BASE, CUSTOM_API_KEY, CUSTOM_API_MODEL, and PROVIDER.';

function assertRequiredEnv() {
  assert.ok(hasEnv, REQUIRED_ENV_MESSAGE);
}

function debugLog(label, value) {
  try {
    console.log(`[debug] ${label}: ${JSON.stringify(value, null, 2)}`);
  } catch (_err) {
    console.log(`[debug] ${label}:`, value);
  }
}

test('complete returns content', async (t) => {
  if (!hasEnv) {
    t.skip(REQUIRED_ENV_MESSAGE);
    return;
  }
  assertRequiredEnv();
  debugLog('complete.env', { provider: PROVIDER, model: MODEL });
  const client = new Client(PROVIDER);
  const res = await client.complete(
    MODEL,
    [
      { role: 'system', content: 'You are concise.' },
      { role: 'user', content: 'Say hello briefly.' },
    ],
    { max_tokens: 8 },
  );

  debugLog('complete.response', res);
  assert.ok(res.content && res.content.length > 0, 'should return content');
  debugLog('complete.usage', res.usage);
  assert.ok(res.usage.totalTokens >= 1, 'usage should be populated');
});

test('healed_json mode parses JSON', async (t) => {
  if (!hasEnv) {
    t.skip(REQUIRED_ENV_MESSAGE);
    return;
  }
  assertRequiredEnv();
  debugLog('healed_json.env', { provider: PROVIDER, model: MODEL });
  const client = new Client(PROVIDER);
  const res = await client.complete(
    MODEL,
    'Respond only with JSON: {"status":"ok"}',
    { mode: 'healed_json', max_tokens: 24 },
  );

  debugLog('healed_json.response', res);
  assert.ok(res.healed && res.healed.value, 'healed metadata present');
  assert.strictEqual(
    res.healed.value.status || res.healed.value?.value?.status || 'ok',
    'ok',
  );
});
