let rustModulePromise;

export async function loadRustModule() {
  if (!rustModulePromise) {
    rustModulePromise = (async () => {
      try {
        const moduleValue = await import("../pkg/simple_agents_wasm.js");
        const wasmUrl = new URL("../pkg/simple_agents_wasm_bg.wasm", import.meta.url);
        await moduleValue.default({ module_or_path: wasmUrl });
        return moduleValue;
      } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        throw new Error(
          `[simple-agents-wasm] Failed to load Rust WASM backend. ` +
          `Build artifacts are required (run "npm run build" in bindings/wasm/simple-agents-wasm). ` +
          `Original error: ${reason}`
        );
      }
    })();
  }

  return rustModulePromise;
}
