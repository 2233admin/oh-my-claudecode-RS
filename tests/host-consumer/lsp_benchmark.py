#!/usr/bin/env python3
"""Measure cold/warm project LSP latency through one real MCP process."""

import argparse
import json
import math
import pathlib
import statistics
import subprocess
import tempfile
import time


def rpc(process, request):
    started = time.perf_counter()
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    elapsed_ms = (time.perf_counter() - started) * 1000
    envelope = json.loads(response["result"]["content"][0]["text"])
    if not envelope.get("ok"):
        raise RuntimeError(envelope)
    return elapsed_ms, envelope["data"]


def percentile_95(samples):
    ordered = sorted(samples)
    return ordered[max(0, math.ceil(len(ordered) * 0.95) - 1)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--omc", required=True)
    parser.add_argument("--warm-iterations", type=int, default=5)
    parser.add_argument("--warm-p95-budget-ms", type=float, default=1000)
    args = parser.parse_args()
    if args.warm_iterations < 2:
        parser.error("--warm-iterations must be at least 2")

    with tempfile.TemporaryDirectory(prefix="omc-lsp-benchmark-") as directory:
        root = pathlib.Path(directory)
        (root / "src").mkdir()
        (root / "Cargo.toml").write_text(
            "[package]\nname='omc-lsp-benchmark'\nversion='0.1.0'\nedition='2024'\n",
            encoding="utf-8",
        )
        (root / "src" / "lib.rs").write_text("pub fn answer() -> u32 { 42 }\n", encoding="utf-8")
        process = subprocess.Popen(
            [args.omc, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, text=True, encoding="utf-8",
        )
        try:
            def request(identifier):
                return rpc(process, {"jsonrpc": "2.0", "id": identifier, "method": "tools/call", "params": {
                    "name": "lsp_document_symbols", "arguments": {
                        "workingDirectory": str(root), "file": "src/lib.rs", "timeoutMs": 60000,
                    },
                }})

            cold_ms, cold = request(1)
            warm_samples = []
            warm_payloads = []
            for identifier in range(2, args.warm_iterations + 2):
                elapsed, payload = request(identifier)
                warm_samples.append(elapsed)
                warm_payloads.append(payload)
        finally:
            process.stdin.close()
            process.wait(timeout=5)

    warm_p95 = percentile_95(warm_samples)
    same_process = all(item["serverProcessId"] == cold["serverProcessId"] for item in warm_payloads)
    reused = all(item["sessionReused"] is True for item in warm_payloads)
    within_budget = warm_p95 <= args.warm_p95_budget_ms and warm_p95 < cold_ms and same_process and reused
    report = {
        "schemaVersion": "omc.lsp-benchmark.v1",
        "coldMs": round(cold_ms, 2),
        "warm": {
            "medianMs": round(statistics.median(warm_samples), 2),
            "p95Ms": round(warm_p95, 2),
            "samples": len(warm_samples),
        },
        "warmP95BudgetMs": args.warm_p95_budget_ms,
        "sameProcess": same_process,
        "sessionReused": reused,
        "withinBudget": within_budget,
    }
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if within_budget else 1)


if __name__ == "__main__":
    main()
