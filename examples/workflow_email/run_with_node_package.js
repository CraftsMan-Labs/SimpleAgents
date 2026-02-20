const fs = require('node:fs')
const path = require('node:path')

function loadDotEnv(filePath) {
  if (!fs.existsSync(filePath)) return
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const trimmed = line.trim()
    if (!trimmed || trimmed.startsWith('#')) continue
    const idx = trimmed.indexOf('=')
    if (idx <= 0) continue
    const key = trimmed.slice(0, idx).trim()
    const value = trimmed.slice(idx + 1).trim()
    if (!process.env[key]) process.env[key] = value
  }
}

function mapProviderEnv(provider, apiKey, apiBase) {
  if (provider === 'openai') {
    process.env.OPENAI_API_KEY = apiKey
    if (apiBase) process.env.OPENAI_API_BASE = apiBase
    return
  }
  if (provider === 'anthropic') {
    process.env.ANTHROPIC_API_KEY = apiKey
    return
  }
  if (provider === 'openrouter') {
    process.env.OPENROUTER_API_KEY = apiKey
    if (apiBase) process.env.OPENROUTER_API_BASE = apiBase
    return
  }
  throw new Error(`Unsupported WORKFLOW_PROVIDER: ${provider}`)
}

async function main() {
  loadDotEnv(path.resolve(__dirname, '../.env'))

  const provider = process.env.WORKFLOW_PROVIDER || 'openai'
  const apiBase = process.env.WORKFLOW_API_BASE || process.env.CUSTOM_API_BASE
  const apiKey = process.env.WORKFLOW_API_KEY || process.env.CUSTOM_API_KEY

  if (!apiKey) {
    throw new Error('Set WORKFLOW_API_KEY or CUSTOM_API_KEY')
  }

  mapProviderEnv(provider, apiKey, apiBase)

  const { Client } = require('../../crates/simple-agents-napi')
  const client = new Client(provider)

  const workflowPath = process.argv[2] || 'examples/workflow_email/email-intake-classification.yaml'
  const emailText = process.argv[3] || 'Please process supply chain replacement, order 9921 arrived damaged.'

  const result = client.runEmailWorkflowYaml(workflowPath, emailText)

  // Execute real JS custom handlers for custom_worker nodes.
  const started = Date.now()
  for (const step of result.step_timings || []) {
    if (step.node_kind !== 'custom_worker') continue
    const nodeId = step.node_id
    const topic = nodeId.startsWith('rag_') ? nodeId.slice(4) : 'clarification'
    const context = {
      input: { email_text: emailText },
      nodes: Object.fromEntries(
        Object.entries(result.outputs || {}).map(([k, v]) => [k, v.output || {}]),
      ),
    }
    const handled = getRagData(topic, { emailText, context })
    result.outputs[nodeId] = { output: handled }
    if (result.terminal_node === nodeId) {
      result.terminal_output = handled
    }
  }

  const customElapsed = Date.now() - started
  if (customElapsed > 0) {
    result.total_elapsed_ms = (result.total_elapsed_ms || 0) + customElapsed
  }

  console.log(JSON.stringify(result, null, 2))
}

function getRagData(topic, { emailText, context }) {
  const data = {
    probation: [
      'hr_policy/probation.md',
      'Collect manager review, performance evidence, and probation timeline.',
    ],
    leave_request: [
      'hr_policy/leave.md',
      'Validate leave balance, manager approval, and blackout dates.',
    ],
    supply_chain_order_assessment: [
      'supply_chain/order_assessment.md',
      'Review order specs, inventory risk, and vendor lead-time guidance.',
    ],
    supply_chain_order_replacement: [
      'supply_chain/order_replacement.md',
      'Collect order id, damage proof, and replacement SLA policy.',
    ],
    termination_first_time_offense: [
      'hr_policy/termination_first_offense.md',
      'Validate first-incident criteria and route to HRBP review.',
    ],
    termination_repeated_offense: [
      'hr_policy/termination_repeated_offense.md',
      'Collect prior warnings and escalation approvals before final action.',
    ],
    clarification: [
      'shared/request_clarification.md',
      'Request clarifying details before routing.',
    ],
  }
  const [kbSource, playbook] = data[topic] || data.clarification
  return {
    kb_source: kbSource,
    playbook,
    handler: 'GetRagData',
    topic,
    email_preview: emailText.slice(0, 120),
    context_nodes: String(Object.keys(context.nodes || {}).length),
  }
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
