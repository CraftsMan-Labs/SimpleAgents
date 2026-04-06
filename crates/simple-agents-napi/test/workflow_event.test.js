const assert = require('node:assert');
const test = require('node:test');
const { parseWorkflowEvent } = require('../workflow_event.js');

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
