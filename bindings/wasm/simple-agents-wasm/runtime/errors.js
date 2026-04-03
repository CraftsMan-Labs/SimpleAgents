export function configError(message) {
  return new Error(`simple-agents-wasm config error: ${message}`);
}

export function runtimeError(message) {
  return new Error(`simple-agents-wasm runtime error: ${message}`);
}
