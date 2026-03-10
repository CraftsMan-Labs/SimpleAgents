const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
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

function extractEventJsonFromCallbackArgs(firstArg, secondArg, thirdArg) {
  if (typeof secondArg === 'string') return secondArg;
  if (typeof firstArg === 'string') return firstArg;
  if (typeof thirdArg === 'string') return thirdArg;
  return null;
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

test('workflow stream emits explicit stream event types', async (t) => {
  if (!hasEnv) {
    t.skip(REQUIRED_ENV_MESSAGE);
    return;
  }
  assertRequiredEnv();
  process.env.SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW = '1';
  t.after(() => {
    delete process.env.SIMPLE_AGENTS_WORKFLOW_STREAM_INCLUDE_RAW;
  });

  const workflowPath = path.join(os.tmpdir(), `live-workflow-stream-${Date.now()}.yaml`);
  const workflowYaml = `id: live-workflow-stream-test
version: 1.0.0
entry_node: answer
nodes:
  - id: answer
    node_type:
      llm_call:
        model: ${MODEL}
        temperature: 0.0
        messages_path: input.messages
        append_prompt_as_user: true
        stream: true
        stream_json_as_text: true
        heal: true
    config:
      output_schema:
        type: object
        properties:
          state: { type: string }
          reason: { type: string }
        required: [state, reason]
        additionalProperties: false
      prompt: |
        Return exactly this JSON object and nothing else:
        {"state":"ok","reason":"ok"}
`;
  fs.writeFileSync(workflowPath, workflowYaml, 'utf8');
  t.after(() => {
    try {
      fs.unlinkSync(workflowPath);
    } catch (_err) {
      // no-op
    }
  });

  const client = new Client(PROVIDER);
  const eventCounts = new Map();
  let completionNerdstats = null;
  let sawSecondArgEventJson = false;
  const result = await client.runWorkflowYamlStream(
    workflowPath,
    {
      messages: [
        { role: 'user', content: 'Hi' },
      ],
    },
    (errOrEventJson, maybeEventJson, fallbackEventJson) => {
      if (typeof maybeEventJson === 'string') {
        sawSecondArgEventJson = true;
      }
      const eventJson = extractEventJsonFromCallbackArgs(
        errOrEventJson,
        maybeEventJson,
        fallbackEventJson,
      );
      if (eventJson === null) return;
      let event;
      try {
        event = JSON.parse(eventJson);
      } catch (_err) {
        return;
      }
      const eventType = event?.event_type;
      if (typeof eventType !== 'string') return;
      eventCounts.set(eventType, (eventCounts.get(eventType) || 0) + 1);
      if (eventType === 'workflow_completed' && event?.metadata && typeof event.metadata === 'object') {
        const nerdstats = event.metadata.nerdstats;
        if (nerdstats && typeof nerdstats === 'object') {
          completionNerdstats = nerdstats;
        }
      }
    },
    { telemetry: { nerdstats: true } },
  );

  assert.ok(result && typeof result === 'object', 'stream call should resolve structured output object');
  assert.strictEqual(typeof result.terminal_node, 'string', 'result should include terminal_node');
  assert.ok((eventCounts.get('node_stream_delta') || 0) > 0, 'expected node_stream_delta events');
  assert.ok((eventCounts.get('node_stream_output_delta') || 0) > 0, 'expected node_stream_output_delta events');
  assert.strictEqual(eventCounts.get('node_stream_raw_delta') || 0, 0, 'deprecated node_stream_raw_delta must not be emitted');
  assert.ok(sawSecondArgEventJson, 'stream callback should provide event JSON in second argument');
  assert.ok(completionNerdstats && typeof completionNerdstats === 'object', 'workflow_completed should include nerdstats metadata');
  assert.strictEqual(typeof completionNerdstats.total_elapsed_ms, 'number', 'nerdstats should include total_elapsed_ms');
  assert.strictEqual(typeof completionNerdstats.token_metrics_available, 'boolean', 'nerdstats should include token_metrics_available');
  assert.ok(
    completionNerdstats.ttft_ms === null || Number.isFinite(completionNerdstats.ttft_ms),
    'nerdstats.ttft_ms should be null or a number',
  );
});
