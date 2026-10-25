/* tslint:disable */
/* eslint-disable */

/**
 * A Spare Construct Transport session: the open project, the active scenario, the results and the undo history.
 */
export class Session {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Stops sending progress events.
     */
    clearProgress(): void;
    /**
     * Runs one command with its parameters as JSON text and gives the result as JSON text; an error is thrown as an `Error`
     * whose message says what went wrong and which has `code` and `details` properties.
     */
    command(name: string, params_json: string): string;
    /**
     * Every command as JSON text: name, description, whether it changes the project and the JSON Schema of its parameters.
     */
    static commands(): string;
    /**
     * Like `command`, but never throws: it gives `{"ok":true,"result":...}` or `{"ok":false,"error":{...}}` as JSON text.
     */
    execute(name: string, params_json: string): string;
    /**
     * A session with a new empty project.
     */
    constructor();
    /**
     * Sets a function called with each progress event as JSON text while a long command runs.
     */
    onProgress(callback: Function): void;
    /**
     * A number that goes up each time the project changes.
     */
    revision(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_session_free: (a: number, b: number) => void;
    readonly session_clearProgress: (a: number) => void;
    readonly session_command: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly session_commands: (a: number) => void;
    readonly session_execute: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly session_new: () => number;
    readonly session_onProgress: (a: number, b: number) => void;
    readonly session_revision: (a: number) => number;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export2: (a: number, b: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
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
