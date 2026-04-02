const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

function outgoingEdgesForNode(node) {
  switch (node.type) {
    case 'start':
      return [node.next];
    case 'llm':
    case 'tool':
    case 'subgraph':
      return node.next ? [node.next] : [];
    case 'condition':
      return [node.on_true, node.on_false];
    case 'loop':
      return [node.body, node.next];
    case 'parallel':
      return [...node.branches, node.next];
    case 'batch':
    case 'filter':
    case 'merge':
    case 'map':
    case 'reduce':
      return [node.next];
    case 'end':
      return [];
    default:
      throw new Error(`unsupported node type in fixture: ${node.type}`);
  }
}

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

test('runEmailWorkflowYaml declaration supports workflowOptions', () => {
  const declarationPath = path.resolve(__dirname, '../index.d.ts');
  const declaration = fs.readFileSync(declarationPath, 'utf8');
  assert.ok(
    declaration.includes('runEmailWorkflowYaml(workflowPath: string, emailText: string, workflowOptions?: { telemetry?: Record<string, unknown>; trace?: Record<string, unknown> }): { workflow_id: string;'),
    'runEmailWorkflowYaml should accept optional workflowOptions in TypeScript declaration',
  );
});

test('workflow DSL fixture preserves canonical IR wires', () => {
  const fixturePath = path.resolve(
    __dirname,
    '../../../parity-fixtures/workflow_dsl_ir_golden.json',
  );
  const fixture = JSON.parse(fs.readFileSync(fixturePath, 'utf8'));
  const dslNodes = fixture.workflow_dsl.nodes;
  const canonicalNodes = fixture.canonical_ir.nodes;
  const canonicalById = new Map(canonicalNodes.map((node) => [node.id, node]));

  assert.deepStrictEqual(
    new Set(Object.keys(dslNodes)),
    new Set(canonicalById.keys()),
  );
  assert.ok(canonicalById.has(fixture.workflow_dsl.entry));
  assert.deepStrictEqual(
    new Set(canonicalNodes.map((node) => node.type)),
    new Set(fixture.required_node_types),
  );

  for (const wireExpectation of fixture.wire_expectations) {
    const node = canonicalById.get(wireExpectation.node_id);
    assert.ok(node, `missing node ${wireExpectation.node_id}`);
    const actual = outgoingEdgesForNode(node).sort();
    const expected = [...wireExpectation.outgoing].sort();
    assert.deepStrictEqual(
      actual,
      expected,
      `${wireExpectation.node_id} outgoing wires should match fixture`,
    );
  }

  for (const mergeExpectation of fixture.merge_source_expectations) {
    const node = canonicalById.get(mergeExpectation.node_id);
    assert.ok(node, `missing merge node ${mergeExpectation.node_id}`);
    assert.strictEqual(node.type, 'merge');
    assert.deepStrictEqual(
      [...node.sources].sort(),
      [...mergeExpectation.sources].sort(),
    );
  }
});
