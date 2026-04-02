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

async function runTurn(client, workflowYaml, input, includeEvents, stream) {
  const streamState = { nodeId: null }
  const workflowOptions = {}
  if (includeEvents) {
    workflowOptions.telemetry = { nerdstats: true }
  }
  if (stream) {
    workflowOptions.onEvent = (event) => {
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
    fetchImpl: (...args) => fetch(...args),
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
