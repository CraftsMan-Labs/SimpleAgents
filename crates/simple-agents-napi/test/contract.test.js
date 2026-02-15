const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

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
