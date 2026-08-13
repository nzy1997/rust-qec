/* tslint:disable */
/* eslint-disable */

export class ShotSession {
    free(): void;
    [Symbol.dispose](): void;
    clear(seed_low: number, seed_high: number): string;
    constructor(source: string, seed_low: number, seed_high: number);
    redo(): string;
    restoreNoise(event_id: string): string;
    sample(seed_low: number, seed_high: number): string;
    setNoise(event_id: string, outcome: string): string;
    snapshot(): string;
    undo(): string;
    static withLimits(source: string, seed_low: number, seed_high: number, max_expanded_operations: number, max_noise_events: number, max_measurements: number, max_svg_nodes: number): ShotSession;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_shotsession_free: (a: number, b: number) => void;
    readonly shotsession_clear: (a: number, b: number, c: number) => [number, number, number, number];
    readonly shotsession_new: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly shotsession_redo: (a: number) => [number, number, number, number];
    readonly shotsession_restoreNoise: (a: number, b: number, c: number) => [number, number, number, number];
    readonly shotsession_sample: (a: number, b: number, c: number) => [number, number, number, number];
    readonly shotsession_setNoise: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly shotsession_snapshot: (a: number) => [number, number, number, number];
    readonly shotsession_undo: (a: number) => [number, number, number, number];
    readonly shotsession_withLimits: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly rust_zstd_wasm_shim_calloc: (a: number, b: number) => number;
    readonly rust_zstd_wasm_shim_free: (a: number) => void;
    readonly rust_zstd_wasm_shim_malloc: (a: number) => number;
    readonly rust_zstd_wasm_shim_memcmp: (a: number, b: number, c: number) => number;
    readonly rust_zstd_wasm_shim_memcpy: (a: number, b: number, c: number) => number;
    readonly rust_zstd_wasm_shim_memmove: (a: number, b: number, c: number) => number;
    readonly rust_zstd_wasm_shim_memset: (a: number, b: number, c: number) => number;
    readonly rust_zstd_wasm_shim_qsort: (a: number, b: number, c: number, d: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
