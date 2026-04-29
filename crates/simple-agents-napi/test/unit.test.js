const assert = require('node:assert');
const test = require('node:test');

const binding = require('..');

test('runtime exports include Client', () => {
  assert.ok('Client' in binding);
  assert.strictEqual(typeof binding.Client, 'function');
});

test('Client prototype includes workflow parity methods', () => {
  const methods = [
    'runWorkflow',
    'streamWorkflow',
    'run',
    'stream',
    'resume',
    'runWorkflowYaml',
    'runWorkflowYamlWithEvents',
    'runWorkflowYamlStream',
    'executeWorkflowYaml',
    'executeWorkflowYamlStream',
  ];
  for (const method of methods) {
    assert.strictEqual(typeof binding.Client.prototype[method], 'function');
  }
});

test('package self-reference exposes workflow YAML compatibility aliases', () => {
  const packageBinding = require('simple-agents-node');
  assert.strictEqual(typeof packageBinding.Client.prototype.executeWorkflowYaml, 'function');
});

test('parseWorkflowYamlExecutionRequest accepts workflowOptions.includeEvents', () => {
  const parsed = binding.parseWorkflowYamlExecutionRequest(
    'workflow.yaml',
    [{ role: 'user', content: 'hello' }],
    { healing: false, workflowStreaming: false, nodeLlmStreaming: false },
    undefined,
    { includeEvents: true },
  );
  assert.strictEqual(parsed.workflowOptions.include_events, true);
});
