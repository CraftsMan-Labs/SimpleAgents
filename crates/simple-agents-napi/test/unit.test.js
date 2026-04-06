const assert = require('node:assert');
const test = require('node:test');

const binding = require('..');

test('runtime exports include Client', () => {
  assert.ok('Client' in binding);
  assert.strictEqual(typeof binding.Client, 'function');
});

test('Client prototype includes workflow parity methods', () => {
  const methods = ['runWorkflow', 'streamWorkflow'];
  for (const method of methods) {
    assert.strictEqual(typeof binding.Client.prototype[method], 'function');
  }
});
