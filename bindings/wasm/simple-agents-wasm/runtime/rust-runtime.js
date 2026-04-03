let rustModulePromise;

export async function loadRustModule() {
  if (!rustModulePromise) {
    rustModulePromise = (async () => {
      try {
        const moduleValue = await import("../pkg/simple_agents_wasm.js");
        const wasmUrl = new URL("../pkg/simple_agents_wasm_bg.wasm", import.meta.url);
        await moduleValue.default({ module_or_path: wasmUrl });
        return moduleValue;
      } catch {
        return null;
      }
    })();
  }

  return rustModulePromise;
}
