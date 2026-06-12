import { create } from "zustand";
import { coreCommand } from "./bridge";

export interface ModelFile {
  path: string;
  sha256: string;
  size_bytes?: number;
}

export interface ModelItem {
  profile: {
    id: string;
    label: string;
    kind: string;
    backend: string;
    version: string;
    license: string;
    languages: string[];
    streaming: boolean;
    recommended: boolean;
    min_ram_mb: number;
  };
  source: { url: string; size_bytes: number };
  local: {
    state: "not_installed" | "ready" | "active" | "broken";
    path: string;
    manifest_present: boolean;
    total_size_bytes: number;
    issues: string[];
  };
  profile_issues: string[];
}

export interface DownloadProgress {
  task_id: string;
  model_id: string;
  phase: "downloading" | "verifying" | "installing" | "done" | "failed";
  downloaded?: number;
  total?: number;
  speed_bps?: number;
  eta_s?: number;
  code?: string;
  message?: string;
}

export type ConnectionState =
  | "connecting"
  | "connected"
  | "disconnected"
  | "retrying";

interface ControlState {
  connection: ConnectionState;
  connectionError: string | null;
  snapshot: Record<string, unknown> | null;
  models: ModelItem[];
  progress: Record<string, DownloadProgress>;
  lastError: string | null;
  setConnection: (state: ConnectionState, error: string | null) => void;
  setSnapshot: (snapshot: Record<string, unknown>) => void;
  applyCoreEvent: (name: string, payload: Record<string, unknown>) => void;
  refreshModels: () => Promise<void>;
  download: (modelId: string) => Promise<void>;
  pause: (modelId: string) => Promise<void>;
  cancel: (modelId: string) => Promise<void>;
  activate: (modelId: string) => Promise<void>;
  remove: (modelId: string) => Promise<void>;
  importLocal: (
    modelId: string,
    path: string,
    mode: "copy" | "symlink",
  ) => Promise<void>;
}

async function checked(
  set: (partial: Partial<ControlState>) => void,
  name: string,
  payload: Record<string, unknown>,
): Promise<boolean> {
  const reply = await coreCommand(name, payload);
  if (reply.kind === "error") {
    set({ lastError: `${reply.code}: ${reply.message ?? ""}` });
    return false;
  }
  set({ lastError: null });
  return true;
}

export const useControlStore = create<ControlState>((set, get) => ({
  connection: "connecting",
  connectionError: null,
  snapshot: null,
  models: [],
  progress: {},
  lastError: null,

  setConnection: (connection, connectionError) =>
    set({ connection, connectionError }),

  setSnapshot: (snapshot) => set({ snapshot }),

  applyCoreEvent: (name, payload) => {
    if (name === "model.progress") {
      const progress = payload as unknown as DownloadProgress;
      set({
        progress: { ...get().progress, [progress.model_id]: progress },
      });
      if (progress.phase === "done") {
        void get().refreshModels();
      }
    } else if (name === "model.state_changed") {
      void get().refreshModels();
    }
  },

  refreshModels: async () => {
    const reply = await coreCommand("model.list");
    if (reply.kind === "response" && reply.payload) {
      set({ models: (reply.payload.models as ModelItem[]) ?? [] });
    }
  },

  download: async (modelId) => {
    if (await checked(set, "model.download", { model_id: modelId })) {
      set({
        progress: {
          ...get().progress,
          [modelId]: {
            task_id: "",
            model_id: modelId,
            phase: "downloading",
            downloaded: 0,
          },
        },
      });
    }
  },

  pause: async (modelId) => {
    await checked(set, "model.pause", { model_id: modelId });
  },

  cancel: async (modelId) => {
    if (await checked(set, "model.cancel", { model_id: modelId })) {
      const progress = { ...get().progress };
      delete progress[modelId];
      set({ progress });
    }
  },

  activate: async (modelId) => {
    if (await checked(set, "model.activate", { model_id: modelId })) {
      await get().refreshModels();
    }
  },

  remove: async (modelId) => {
    if (await checked(set, "model.delete", { model_id: modelId })) {
      await get().refreshModels();
    }
  },

  importLocal: async (modelId, path, mode) => {
    if (
      await checked(set, "model.import", { model_id: modelId, path, mode })
    ) {
      await get().refreshModels();
    }
  },
}));
