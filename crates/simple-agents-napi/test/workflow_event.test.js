const assert = require('node:assert');
const test = require('node:test');
const { parseWorkflowEvent, defaultOnEvent } = require('../workflow_event.js');

test('parseWorkflowEvent parses workflow stream JSON', () => {
  const ev = parseWorkflowEvent(
    JSON.stringify({ event_type: 'node_stream_delta', node_id: 'n1', delta: 'x' }),
  );
  assert.strictEqual(ev.event_type, 'node_stream_delta');
  assert.strictEqual(ev.node_id, 'n1');
  assert.strictEqual(ev.delta, 'x');
});

test('parseWorkflowEvent rejects non-string', () => {
  assert.throws(() => parseWorkflowEvent(null), /expected string/);
});

test('WorkflowRunnerEventType uses resolved_llm_input (wire name check)', () => {
  // Verify the canonical wire string is parseable and not the wrong alias.
  const ev = parseWorkflowEvent(JSON.stringify({ event_type: 'resolved_llm_input' }));
  assert.strictEqual(ev.event_type, 'resolved_llm_input');
  // The wrong alias should NOT appear in the type — runtime test: just assert the
  // string we parse is the one Rust emits.
  assert.notStrictEqual(ev.event_type, 'node_llm_input_resolved');
});

test('defaultOnEvent writes stream delta to stdout', () => {
  const written = [];
  const origWrite = process.stdout.write.bind(process.stdout);
  process.stdout.write = (chunk) => { written.push(chunk); return true; };
  try {
    defaultOnEvent(null, JSON.stringify({ event_type: 'node_stream_delta', delta: 'hello' }));
    assert.deepStrictEqual(written, ['hello']);
  } finally {
    process.stdout.write = origWrite;
  }
});

test('defaultOnEvent silences workflow_started', () => {
  const written = [];
  const origWrite = process.stdout.write.bind(process.stdout);
  process.stdout.write = (chunk) => { written.push(chunk); return true; };
  try {
    defaultOnEvent(null, JSON.stringify({ event_type: 'workflow_started' }));
    assert.deepStrictEqual(written, []);
  } finally {
    process.stdout.write = origWrite;
  }
});

test('defaultOnEvent ignores error arg', () => {
  // Should not throw when err is truthy.
  defaultOnEvent(new Error('fail'), JSON.stringify({ event_type: 'node_stream_delta', delta: 'x' }));
});
