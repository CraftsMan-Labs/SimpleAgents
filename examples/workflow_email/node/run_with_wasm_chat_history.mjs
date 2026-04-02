import fs from 'node:fs'
import path from 'node:path'
import readline from 'node:readline/promises'
import { stdin, stdout } from 'node:process'
import { Client } from '../../../bindings/wasm/simple-agents-wasm/index.js'

function parseArgs(argv) {
  const options = {
    workflow: 'examples/workflow_email/email-chat-draft-or-clarify.yaml',
    maxTurns: 3,
    stream: true,
    includeEvents: false,
    nerdstats: true,
    message: null,
    model: null,
  }

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i]
    if (arg === '--workflow' && argv[i + 1]) {
      options.workflow = argv[i + 1]
      i += 1
      continue
    }
    if (arg === '--max-turns' && argv[i + 1]) {
      const parsed = Number.parseInt(argv[i + 1], 10)
      if (Number.isFinite(parsed) && parsed > 0) {
        options.maxTurns = parsed
      }
      i += 1
      continue
    }
    if (arg === '--include-events') {
      options.includeEvents = true
      continue
    }
    if (arg === '--nerdstats') {
      options.nerdstats = true
      continue
    }
    if (arg === '--no-nerdstats') {
      options.nerdstats = false
      continue
    }
    if (arg === '--stream') {
      options.stream = true
      continue
    }
    if (arg === '--no-stream') {
      options.stream = false
      continue
    }
    if (arg === '--message' && argv[i + 1]) {
      options.message = argv[i + 1]
      i += 1
      continue
    }
    if (arg === '--model' && argv[i + 1]) {
      options.model = argv[i + 1]
      i += 1
      continue
    }
  }

  return options
}

function loadDotEnv(filePath) {
  if (!fs.existsSync(filePath)) return
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/)
  for (const line of lines) {
    const trimmed = line.trim()
    if (trimmed === '' || trimmed.startsWith('#')) continue
    const splitIndex = trimmed.indexOf('=')
    if (splitIndex <= 0) continue
    const key = trimmed.slice(0, splitIndex).trim()
    const value = trimmed.slice(splitIndex + 1).trim()
    if (process.env[key] === undefined) {
      process.env[key] = value
    }
  }
}

function loadConfig() {
  loadDotEnv(path.resolve(process.cwd(), 'examples/.env'))
  loadDotEnv(path.resolve(process.cwd(), '.env'))

  const provider = process.env.WORKFLOW_PROVIDER || 'openai'
  if (provider !== 'openai' && provider !== 'openrouter') {
    throw new Error("WASM runner supports WORKFLOW_PROVIDER=openai|openrouter only.")
  }

  const baseUrl =
    process.env.WORKFLOW_API_BASE ||
    process.env.CUSTOM_API_BASE ||
    (provider === 'openrouter' ? process.env.OPENROUTER_API_BASE : process.env.OPENAI_API_BASE) ||
    ''
  const apiKey =
    process.env.WORKFLOW_API_KEY ||
    process.env.CUSTOM_API_KEY ||
    (provider === 'openrouter' ? process.env.OPENROUTER_API_KEY : process.env.OPENAI_API_KEY) ||
    ''

  if (baseUrl === '' || apiKey === '') {
    throw new Error('Set WORKFLOW_API_BASE and WORKFLOW_API_KEY (or CUSTOM_API_BASE/CUSTOM_API_KEY).')
  }

  return { provider, baseUrl, apiKey }
}

function resolveWorkflowPath(workflow) {
  const direct = path.resolve(process.cwd(), workflow)
  if (fs.existsSync(direct)) return direct

  const repoRelative = path.resolve(process.cwd(), 'examples', workflow)
  if (fs.existsSync(repoRelative)) return repoRelative

  throw new Error(`workflow file not found: ${workflow}`)
}

function initialMessages() {
  return [
    {
      role: 'system',
      content:
        'You are a friendly email drafting assistant for new users. Ask one clear follow-up question when context is missing and draft concise professional emails when enough detail is available.',
    },
  ]
}

function renderAssistantReply(value) {
  if (typeof value === 'string') return value
  if (value === null || value === undefined) return ''
  if (typeof value === 'object') {
    if (typeof value.question === 'string') return value.question
    if (typeof value.message === 'string') return value.message
    if (typeof value.subject === 'string' && typeof value.body === 'string') {
      return `Subject: ${value.subject}\n\n${value.body}`
    }
    return JSON.stringify(value, null, 2)
  }
  return String(value)
}

function pickWorkflowTerminalOutput(result) {
  if (result && typeof result === 'object') {
    if (result.output !== undefined && result.output !== null) {
      return result.output
    }
    if (result.terminal_output !== undefined && result.terminal_output !== null) {
      return result.terminal_output
    }
    if (
      typeof result.terminal_node === 'string' &&
      result.terminal_node.length > 0 &&
      result.outputs &&
      typeof result.outputs === 'object' &&
      Object.prototype.hasOwnProperty.call(result.outputs, result.terminal_node)
    ) {
      return result.outputs[result.terminal_node]
    }
  }
  return null
}

function extractNerdstatsFromEvents(events) {
  if (!Array.isArray(events)) return null
  for (let i = events.length - 1; i >= 0; i -= 1) {
    const event = events[i]
    if (!event || event.event_type !== 'workflow_completed') continue
    if (!event.metadata || typeof event.metadata !== 'object') continue
    const nerdstats = event.metadata.nerdstats
    if (nerdstats && typeof nerdstats === 'object') return normalizeNerdstatsPayload(nerdstats)
  }
  return null
}

function normalizeNerdstatsPayload(payload) {
  if (!payload || typeof payload !== 'object') {
    return fallbackNerdstatsFromResult(null)
  }

  const rawStepDetails = Array.isArray(payload.step_details) ? payload.step_details : []
  const stepDetails = rawStepDetails
    .filter((step) => step && typeof step === 'object')
    .map((step) => {
      const base = {
        elapsed_ms: Number.isFinite(step.elapsed_ms) ? step.elapsed_ms : 0,
        node_id: typeof step.node_id === 'string' ? step.node_id : '',
        node_kind: typeof step.node_kind === 'string' ? step.node_kind : 'unknown',
      }
      if (base.node_kind === 'llm_call') {
        if (typeof step.model_name === 'string') {
          base.model_name = step.model_name
        }
        if (Number.isFinite(step.prompt_tokens)) {
          base.prompt_tokens = step.prompt_tokens
        }
        if (Number.isFinite(step.completion_tokens)) {
          base.completion_tokens = step.completion_tokens
        }
        if (Number.isFinite(step.total_tokens)) {
          base.total_tokens = step.total_tokens
        }
        if (Number.isFinite(step.reasoning_tokens)) {
          base.reasoning_tokens = step.reasoning_tokens
        }
        if (Number.isFinite(step.tokens_per_second)) {
          base.tokens_per_second = step.tokens_per_second
        }
      }
      return base
    })

  const traceId = typeof payload.trace_id === 'string' ? payload.trace_id : ''

  const normalized = {
    llm_nodes_without_usage: Array.isArray(payload.llm_nodes_without_usage)
      ? payload.llm_nodes_without_usage
      : [],
    step_details: stepDetails,
    terminal_node: typeof payload.terminal_node === 'string' ? payload.terminal_node : '',
    token_metrics_available: Boolean(payload.token_metrics_available),
    token_metrics_source:
      payload.token_metrics_source === 'provider_usage' ? 'provider_usage' : 'unavailable',
    tokens_per_second: Number.isFinite(payload.tokens_per_second) ? payload.tokens_per_second : 0,
    total_elapsed_ms: Number.isFinite(payload.total_elapsed_ms) ? payload.total_elapsed_ms : 0,
    total_input_tokens: Number.isFinite(payload.total_input_tokens) ? payload.total_input_tokens : 0,
    total_output_tokens: Number.isFinite(payload.total_output_tokens)
      ? payload.total_output_tokens
      : 0,
    total_reasoning_tokens: Number.isFinite(payload.total_reasoning_tokens)
      ? payload.total_reasoning_tokens
      : 0,
    total_tokens: Number.isFinite(payload.total_tokens) ? payload.total_tokens : 0,
    ttft_ms: Number.isFinite(payload.ttft_ms) ? payload.ttft_ms : 0,
    workflow_id: typeof payload.workflow_id === 'string' ? payload.workflow_id : '',
  }
  if (traceId !== '') {
    normalized.trace_id = traceId
  }

  return normalized
}

function fallbackNerdstatsFromResult(result) {
  if (!result || typeof result !== 'object') {
    return {
      workflow_id: '',
      terminal_node: '',
      total_elapsed_ms: 0,
      ttft_ms: 0,
      step_details: [],
      total_input_tokens: 0,
      total_output_tokens: 0,
      total_tokens: 0,
      total_reasoning_tokens: 0,
      tokens_per_second: 0,
      trace_id: '',
      token_metrics_available: false,
      token_metrics_source: 'unavailable',
      llm_nodes_without_usage: [],
    }
  }
  const stepDetails = Array.isArray(result.step_timings) ? result.step_timings : []
  const hasTokenMetrics =
    Number.isFinite(result.total_input_tokens) && Number.isFinite(result.total_output_tokens)
  const llmNodesWithoutUsage = stepDetails
    .filter((step) => step && step.node_kind === 'llm_call' && !Number.isFinite(step.total_tokens))
    .map((step) => step.node_id)

  return {
    workflow_id: typeof result.workflow_id === 'string' ? result.workflow_id : '',
    terminal_node: typeof result.terminal_node === 'string' ? result.terminal_node : '',
    total_elapsed_ms: Number.isFinite(result.total_elapsed_ms) ? result.total_elapsed_ms : 0,
    ttft_ms: Number.isFinite(result.ttft_ms) ? result.ttft_ms : 0,
    step_details: stepDetails,
    total_input_tokens: Number.isFinite(result.total_input_tokens) ? result.total_input_tokens : 0,
    total_output_tokens: Number.isFinite(result.total_output_tokens) ? result.total_output_tokens : 0,
    total_tokens: Number.isFinite(result.total_tokens) ? result.total_tokens : 0,
    total_reasoning_tokens: Number.isFinite(result.total_reasoning_tokens)
      ? result.total_reasoning_tokens
      : 0,
    tokens_per_second: Number.isFinite(result.tokens_per_second) ? result.tokens_per_second : 0,
    trace_id: typeof result.trace_id === 'string' ? result.trace_id : '',
    token_metrics_available: hasTokenMetrics,
    token_metrics_source: hasTokenMetrics ? 'provider_usage' : 'unavailable',
    llm_nodes_without_usage: llmNodesWithoutUsage,
  }
}

async function runTurn(client, workflowYaml, input, includeEvents, stream, nerdstats) {
  const streamState = { nodeId: null }
  const streamedEvents = []
  const workflowOptions = {}
  if (includeEvents) {
    workflowOptions.includeEvents = true
  }
  if (nerdstats) {
    workflowOptions.telemetry = { nerdstats: true }
  }
  if (stream) {
    workflowOptions.onEvent = (event) => {
      if (event && typeof event === 'object') {
        streamedEvents.push(event)
      }
      if (!event || event.eventType !== 'node_stream_delta') return
      const delta = typeof event.delta === 'string' ? event.delta : ''
      if (delta.length === 0) return
      const nodeId = typeof event.nodeId === 'string' ? event.nodeId : 'workflow'
      if (streamState.nodeId !== nodeId) {
        if (streamState.nodeId !== null) {
          process.stdout.write('\n')
        }
        process.stdout.write(`\nStep: ${nodeId}\nStreaming: `)
        streamState.nodeId = nodeId
      }
      process.stdout.write(delta)
    }
  }

  const result = await client.runWorkflowYamlString(
    workflowYaml,
    input,
    Object.keys(workflowOptions).length > 0 ? workflowOptions : undefined,
  )
  if (stream && streamState.nodeId !== null) {
    process.stdout.write('\n')
  }

  if (nerdstats) {
    const nerdstatsPayload = normalizeNerdstatsPayload(
      extractNerdstatsFromEvents(streamedEvents) ?? fallbackNerdstatsFromResult(result),
    )
    if (nerdstatsPayload !== null) {
      console.log(`Nerdstats: ${JSON.stringify(nerdstatsPayload)}`)
    }
  }

  return result
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  const { provider, baseUrl, apiKey } = loadConfig()
  const workflowPath = resolveWorkflowPath(args.workflow)
  const workflowYaml = fs.readFileSync(workflowPath, 'utf8')
  const messages = initialMessages()
  const client = new Client(provider, {
    apiKey,
    baseUrl,
    fetchImpl: globalThis.fetch,
  })

  console.log('WASM Chat Email Assistant')
  console.log("Type your request. Type 'exit' to quit.\n")
  console.log(`Workflow: ${workflowPath}`)

  if (args.message !== null && args.message.trim() !== '') {
    messages.push({ role: 'user', content: args.message })
    const result = await runTurn(
      client,
      workflowYaml,
      { messages, model: args.model || undefined, email_text: args.message },
      args.includeEvents,
      args.stream,
      args.nerdstats,
    )
    const reply = renderAssistantReply(pickWorkflowTerminalOutput(result))
    console.log(`\nAssistant: ${reply}\n`)
    if (args.includeEvents && Array.isArray(result.events)) {
      for (const event of result.events) {
        console.log(`- ${event.stepId} (${event.stepType}) -> ${event.status}`)
      }
    }
    return
  }

  const rl = readline.createInterface({ input: stdin, output: stdout })
  try {
    for (let turn = 1; turn <= args.maxTurns; turn += 1) {
      const rawInput = await rl.question('You: ')
      const userInput = rawInput.trim()
      if (userInput === '') continue
      if (userInput.toLowerCase() === 'exit' || userInput.toLowerCase() === 'quit') {
        console.log('Bye!')
        return
      }

      messages.push({ role: 'user', content: userInput })
      const result = await runTurn(
        client,
        workflowYaml,
        { messages, model: args.model || undefined, email_text: userInput },
        args.includeEvents,
        args.stream,
        args.nerdstats,
      )
      const reply = renderAssistantReply(pickWorkflowTerminalOutput(result))
      console.log(`\nAssistant: ${reply}\n`)
      messages.push({ role: 'assistant', content: reply })
    }
  } finally {
    rl.close()
  }

  console.log('Reached max turns. Restart to continue.')
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
