#!/usr/bin/env python3
"""Gate cold profile validation and warm in-process MCP discovery latency."""

import argparse
import json
import statistics
import subprocess
import time


def p95(samples):
    return sorted(samples)[max(0, int(len(samples) * 0.95) - 1)]


def rpc(process, request):
    started = time.perf_counter()
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    if "error" in response:
        raise RuntimeError(response["error"])
    return (time.perf_counter() - started) * 1000, response["result"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omc", required=True)
    parser.add_argument("--profile", default="schemas/profile-fixtures/unknown-runtime-v1.json")
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--cold-p95-budget-ms", type=float, default=250)
    parser.add_argument("--warm-p95-budget-ms", type=float, default=50)
    args = parser.parse_args()
    if args.iterations < 2:
        parser.error("--iterations must be at least 2")

    cold = []
    for _ in range(args.iterations):
        started = time.perf_counter()
        result = subprocess.run(
            [args.omc, "profile", "validate", "--profile", args.profile, "--json"],
            capture_output=True,
            text=True,
            encoding="utf-8",
            check=False,
        )
        if result.returncode != 0 or not json.loads(result.stdout).get("ok"):
            raise RuntimeError(f"profile validation failed: {result.stderr}")
        cold.append((time.perf_counter() - started) * 1000)

    process = subprocess.Popen(
        [args.omc, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, text=True, encoding="utf-8"
    )
    try:
        rpc(process, {"jsonrpc": "2.0", "id": 0, "method": "initialize"})
        warm = []
        tool_count = 0
        for index in range(args.iterations):
            elapsed, listed = rpc(process, {
                "jsonrpc": "2.0", "id": index + 1, "method": "tools/list"
            })
            warm.append(elapsed)
            tool_count = len(listed["tools"])
    finally:
        process.stdin.close()
        process.wait(timeout=5)

    report = {
        "schemaVersion": "omc.profile-performance.v1",
        "iterations": args.iterations,
        "coldResolutionValidation": {
            "medianMs": round(statistics.median(cold), 2), "p95Ms": round(p95(cold), 2)
        },
        "warmDiscovery": {
            "medianMs": round(statistics.median(warm), 2), "p95Ms": round(p95(warm), 2)
        },
        "toolCount": tool_count,
        "budgetsMs": {"coldP95": args.cold_p95_budget_ms, "warmP95": args.warm_p95_budget_ms},
    }
    report["withinBudget"] = (
        report["coldResolutionValidation"]["p95Ms"] <= args.cold_p95_budget_ms
        and report["warmDiscovery"]["p95Ms"] <= args.warm_p95_budget_ms
        and tool_count == 32
    )
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report["withinBudget"] else 1)


if __name__ == "__main__":
    main()
