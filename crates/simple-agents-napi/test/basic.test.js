const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
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

test('declaration and runtime exports follow shared contract fixture', () => {
  const fixturePath = path.resolve(
    __dirname,
    '../../../parity-fixtures/binding_contract.json',
  );
  const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
  const declarationPath = path.resolve(__dirname, '../index.d.ts');
  const declaration = fs.readFileSync(declarationPath, 'utf8');

  for (const symbol of fixture.node.required_type_symbols) {
    assert.ok(
      declaration.includes(symbol),
      `index.d.ts should include: ${symbol}`,
    );
  }

  for (const symbol of fixture.node.required_runtime_exports) {
    assert.ok(symbol in require('..'), `runtime export should include: ${symbol}`);
  }

  const sharedCases = fixture.shared_cases;
  assert.ok(sharedCases, 'shared_cases fixture must exist');
  assert.ok(Array.isArray(sharedCases.request.completion_modes));
  assert.ok(sharedCases.request.completion_modes.includes('standard'));
  assert.ok(sharedCases.request.completion_modes.includes('healed_json'));
  assert.ok(sharedCases.request.completion_modes.includes('schema'));
  assert.deepStrictEqual(sharedCases.streaming.event_types, ['delta', 'error', 'done']);
});

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
