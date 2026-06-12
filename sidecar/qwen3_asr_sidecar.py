#!/usr/bin/env python3
"""Qwen3-ASR streaming sidecar for voxflow-core (D-22 default backend).

Speaks line-delimited JSON over stdin/stdout. The Rust side
(`voxflow-asr-qwen3`) is the only intended client.

Protocol (one JSON object per line):
  -> {"cmd":"init", "model":..., "gpu_memory_utilization":..., "chunk_size_sec":...,
      "unfixed_chunk_num":..., "unfixed_token_num":..., "max_new_tokens":...}
  <- {"event":"ready"} | {"event":"error","message":...}
  -> {"cmd":"start"}
  <- {"event":"started"}
  -> {"cmd":"audio","sample_rate":16000,"pcm_i16_b64":"..."}
  <- {"event":"partial","text":...,"language":...}
  -> {"cmd":"finish"}
  <- {"event":"final","text":...,"language":...}
  -> {"cmd":"shutdown"}

Weights are resolved from the local Hugging Face cache (predownloaded by
scripts/deploy-local.sh); nothing is fetched at dictation time.
"""

import base64
import json
import os
import sys

import numpy as np

# vLLM and its dependencies write progress/logs to stdout, which would corrupt
# the JSONL protocol. Keep a private duplicate of the original stdout for
# protocol replies and point fd 1 at stderr so even C-level writes are safe.
_PROTOCOL_OUT = os.fdopen(os.dup(1), "w", buffering=1)
os.dup2(2, 1)
sys.stdout = sys.stderr


def reply(obj):
    _PROTOCOL_OUT.write(json.dumps(obj, ensure_ascii=False) + "\n")
    _PROTOCOL_OUT.flush()


def main():
    asr = None
    state = None
    init_cfg = None

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError as err:
            reply({"event": "error", "message": f"bad json: {err}"})
            continue
        cmd = msg.get("cmd")

        if cmd == "init":
            try:
                from qwen_asr import Qwen3ASRModel  # heavy import deferred

                init_cfg = msg
                asr = Qwen3ASRModel.LLM(
                    model=msg.get("model", "Qwen/Qwen3-ASR-1.7B"),
                    gpu_memory_utilization=float(
                        msg.get("gpu_memory_utilization", 0.8)
                    ),
                    max_new_tokens=int(msg.get("max_new_tokens", 32)),
                    max_model_len=int(msg.get("max_model_len", 16384)),
                )
                reply({"event": "ready"})
            except Exception as err:  # noqa: BLE001 — report anything to Rust
                reply({"event": "error", "message": f"init failed: {err}"})
        elif cmd == "start":
            if asr is None:
                reply({"event": "error", "message": "not initialized"})
                continue
            state = asr.init_streaming_state(
                unfixed_chunk_num=int(init_cfg.get("unfixed_chunk_num", 2)),
                unfixed_token_num=int(init_cfg.get("unfixed_token_num", 5)),
                chunk_size_sec=float(init_cfg.get("chunk_size_sec", 2.0)),
            )
            reply({"event": "started"})
        elif cmd == "audio":
            if asr is None or state is None:
                reply({"event": "error", "message": "no active session"})
                continue
            try:
                pcm = np.frombuffer(
                    base64.b64decode(msg["pcm_i16_b64"]), dtype=np.int16
                )
                wav = pcm.astype(np.float32) / 32767.0
                asr.streaming_transcribe(wav, state)
                reply(
                    {
                        "event": "partial",
                        "text": state.text or "",
                        "language": getattr(state, "language", None),
                    }
                )
            except Exception as err:  # noqa: BLE001
                reply({"event": "error", "message": f"transcribe failed: {err}"})
        elif cmd == "finish":
            if asr is None or state is None:
                reply({"event": "error", "message": "no active session"})
                continue
            try:
                asr.finish_streaming_transcribe(state)
                reply(
                    {
                        "event": "final",
                        "text": state.text or "",
                        "language": getattr(state, "language", None),
                    }
                )
            except Exception as err:  # noqa: BLE001
                reply({"event": "error", "message": f"finish failed: {err}"})
            state = None
        elif cmd == "shutdown":
            reply({"event": "bye"})
            return
        else:
            reply({"event": "error", "message": f"unknown cmd: {cmd}"})


if __name__ == "__main__":
    main()
