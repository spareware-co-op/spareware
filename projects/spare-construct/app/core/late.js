// Names a file uses from a file that loads after it, bound once that file has loaded (note
// A96-tool-architecture.md). The core never imports a tool, so where the core still calls a tool by
// name it calls it here, as `late.<name>`; each such name is a place still to become a registry hook.
// Written by the split; a getter, so a value that changes is read as it is now.

export const late = {};

export function provide(getters) {
  Object.defineProperties(late, Object.getOwnPropertyDescriptors(getters));
}
