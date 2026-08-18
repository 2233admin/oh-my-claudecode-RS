#!/usr/bin/env python3
"""Measure OMC-RS discovery latency with no third-party dependencies."""

import argparse
import json
import statistics
import subprocess
import time


def summary(samples):
    ordered = sorted(samples)
    p95 = ordered[max(0, int(len(ordered) * 0.95) - 1)]
    return {"min_ms": round(ordered[0], 2), "median_ms": round(statistics.median(ordered), 2), "p95_ms": round(p95, 2), "max_ms": round(ordered[-1], 2)}


def timed_cli(binary, iterations):
    samples = []
    count = 0
    for _ in range(iterations):
        started = time.perf_counter()
        result = subprocess.run([binary, "tool", "capabilities"], check=True, capture_output=True, text=True, encoding="utf-8")
        samples.append((time.perf_counter() - started) * 1000)
        count = len(json.loads(result.stdout)["data"]["capabilities"])
    return samples, count


def rpc(process, request):
    started = time.perf_counter()
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    response = json.loads(process.stdout.readline())
    if "error" in response:
        raise RuntimeError(response["error"])
    return (time.perf_counter() - started) * 1000, response["result"]


def timed_mcp(binary, iterations):
    process = subprocess.Popen([binary, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8")
    try:
        rpc(process, {"jsonrpc": "2.0", "id": 0, "method": "initialize"})
        list_ms, listed = rpc(process, {"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        samples = []
        for index in range(iterations):
            elapsed, _ = rpc(process, {"jsonrpc": "2.0", "id": index + 2, "method": "tools/call", "params": {"name": "agent_capabilities", "arguments": {}}})
            samples.append(elapsed)
        return samples, list_ms, len(listed["tools"])
    finally:
        process.stdin.close()
        process.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--omc", required=True)
    parser.add_argument("--iterations", type=int, default=20)
    parser.add_argument("--cli-p95-budget-ms", type=float, default=250)
    parser.add_argument("--mcp-p95-budget-ms", type=float, default=50)
    args = parser.parse_args()
    if args.iterations < 2:
        parser.error("--iterations must be at least 2")

    cli, capability_count = timed_cli(args.omc, args.iterations)
    mcp, list_ms, tool_count = timed_mcp(args.omc, args.iterations)
    report = {
        "schema_version": "omc.performance-baseline.v1",
        "iterations": args.iterations,
        "capability_count": capability_count,
        "tool_count": tool_count,
        "cli_cold_discovery": summary(cli),
        "mcp_warm_discovery": summary(mcp),
        "mcp_tools_list_ms": round(list_ms, 2),
        "budgets_ms": {"cli_cold_p95": args.cli_p95_budget_ms, "mcp_warm_p95": args.mcp_p95_budget_ms},
    }
    report["within_budget"] = report["cli_cold_discovery"]["p95_ms"] <= args.cli_p95_budget_ms and report["mcp_warm_discovery"]["p95_ms"] <= args.mcp_p95_budget_ms and capability_count == 16 and tool_count == 32
    print(json.dumps(report, indent=2))
    raise SystemExit(0 if report["within_budget"] else 1)


if __name__ == "__main__":
    main()
