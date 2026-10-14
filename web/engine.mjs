// Worker-local ABI adapter. Refresh memory views after every Wasm call/growth.
export class Engine {
  constructor(instance) { this.exports = instance.exports; }
  static async load(url = new URL('./minipy_native.wasm', import.meta.url)) {
    const start = performance.now();
    const bytes = await (await fetch(url)).arrayBuffer();
    const {instance} = await WebAssembly.instantiate(bytes, {});
    const engine = new Engine(instance);
    engine.initialization_ms = performance.now() - start;
    return engine;
  }
  command(command) {
    const start = performance.now();
    const bytes = new TextEncoder().encode(JSON.stringify(command));
    if (bytes.length > 65536) throw new Error('command exceeds 64 KiB');
    const offset = this.exports.command_buffer(bytes.length);
    if (!offset) throw new Error('command allocation rejected');
    new Uint8Array(this.exports.memory.buffer, offset, bytes.length).set(bytes);
    const length = this.exports.command_run();
    const result = JSON.parse(new TextDecoder().decode(new Uint8Array(
      this.exports.memory.buffer, this.exports.result_buffer(), length)));
    if (!result.ok) throw new Error(result.error);
    return {...result.value, linear_memory_bytes: this.exports.memory.buffer.byteLength,
      timing: {command_ms: performance.now() - start}};
  }
}
