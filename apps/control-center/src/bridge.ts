// Thin wrapper over the Tauri shell contract (frontend/tauri-ui.md):
// one invoke command `core_command` plus three event channels.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export const CORE_EVENT = "core-event";
export const CONNECTION_EVENT = "connection-changed";
export const SNAPSHOT_EVENT = "control-snapshot";

export interface CoreReply {
  version: number;
  id: string;
  kind: "response" | "error";
  name: string;
  payload?: Record<string, unknown>;
  code?: string;
  message?: string;
}

export async function coreCommand(
  name: string,
  payload: Record<string, unknown> = {},
): Promise<CoreReply> {
  return invoke<CoreReply>("core_command", { invocation: { name, payload } });
}

export function onCoreEvent(
  handler: (name: string, payload: Record<string, unknown>) => void,
): Promise<UnlistenFn> {
  return listen<{ name: string; payload: Record<string, unknown> }>(
    CORE_EVENT,
    (event) => handler(event.payload.name, event.payload.payload),
  );
}

export function onConnectionChanged(
  handler: (payload: Record<string, unknown>) => void,
): Promise<UnlistenFn> {
  return listen<Record<string, unknown>>(CONNECTION_EVENT, (event) =>
    handler(event.payload),
  );
}

export function onSnapshot(
  handler: (payload: Record<string, unknown>) => void,
): Promise<UnlistenFn> {
  return listen<Record<string, unknown>>(SNAPSHOT_EVENT, (event) =>
    handler(event.payload),
  );
}
