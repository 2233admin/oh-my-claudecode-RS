#!/usr/bin/env python3
"""Consume OMC-RS through JSON only, without importing OMC-RS internals."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from typing import Any


SCHEMA_VERSION = "omc.tool.v1"


class ConsumerError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ConsumerError(message)


def decode_json(raw: str, label: str) -> dict[str, Any]:
    try:
        value = json.loads(raw)
    except json.JSONDecodeError as error:
        raise ConsumerError(f"{label} was not JSON: {error}: {raw!r}") from error
    require(isinstance(value, dict), f"{label} must be a JSON object")
    return value


def validate_tool_response(value: dict[str, Any], request_id: str) -> dict[str, Any]:
    require(value.get("schema_version") == SCHEMA_VERSION, "wrong tool schema version")
    require(value.get("request_id") == request_id, "request ID was not preserved")
    return value


def validate_code_intel_response(value: dict[str, Any], request_id: str) -> dict[str, Any]:
    response = validate_tool_response(value, request_id)
    require(response.get("ok") is True, "Code Intel query was not successful")
    data = response.get("data")
    require(isinstance(data, dict), "Code Intel data is not an object")
    require(
        data.get("schemaVersion") == "omc.code-intel.query.v1",
        "Code Intel query schema missing",
    )
    require(data.get("readOnly") is True, "Code Intel query was not read-only")
    refs = data.get("artifactRefs")
    require(isinstance(refs, list) and refs, "Code Intel query returned no artifact refs")
    result = data.get("result")
    require(isinstance(result, dict), "Code Intel result is not an object")
    require(result.get("runOutcome") == "completed", "Code Intel run was not completed")
    require(
        result.get("authority", {}).get("status") == "committed",
        "Code Intel result was not committed",
    )
    require(
        result.get("freshness", {}).get("status") == "current",
        "Code Intel result was not current",
    )
    return response


def validate_interop_response(value: dict[str, Any], request_id: str) -> dict[str, Any]:
    response = validate_tool_response(value, request_id)
    require(response.get("ok") is True, "interop snapshot was not successful")
    data = response.get("data")
    require(isinstance(data, dict), "interop snapshot data is not an object")
    require(
        data.get("schemaVersion") == "omc.interop.snapshot.v1",
        "interop snapshot schema missing",
    )
    require(data.get("readOnly") is True, "interop snapshot was not read-only")
    require(isinstance(data.get("sharedTaskCount"), int), "interop task count missing")
    normalized = data.get("normalizedTasks")
    require(isinstance(normalized, list), "normalized interop task list missing")
    statuses = {"pending", "blocked", "in_progress", "completed", "failed"}
    require(
        all(isinstance(task, dict) and task.get("status") in statuses for task in normalized),
        "normalized interop task status was not portable",
    )
    require(isinstance(data.get("omxTeams"), list), "interop OMX team list missing")
    return response


def validate_bridge_response(value: dict[str, Any], request_id: str) -> dict[str, Any]:
    response = validate_tool_response(value, request_id)
    require(response.get("ok") is True, "interop bridge was not successful")
    data = response.get("data")
    require(isinstance(data, dict), "interop bridge data is not an object")
    require(
        data.get("schemaVersion") == "omc.interop.bridge.v1",
        "interop bridge schema missing",
    )
    require(data.get("readOnly") is False, "interop bridge unexpectedly read-only")
    require(data.get("writeEnabled") is True, "interop bridge write gate was not reported")
    require(data.get("source") != data.get("target"), "interop bridge endpoints collapsed")
    require(isinstance(data.get("task"), dict) or isinstance(data.get("message"), dict), "interop bridge payload missing")
    return response


def run_cli(
    omc: str,
    artifact_root: str | None,
    repo: str,
    repo_path: str,
    interop_root: str,
    require_active: bool,
) -> dict[str, Any]:
    success = subprocess.run(
        [omc, "tool", "capabilities", "--request-id", "host-consumer-cli"],
        capture_output=True,
        text=True,
        check=False,
    )
    require(success.returncode == 0, f"CLI capabilities failed: {success.stderr}")
    capabilities = validate_tool_response(
        decode_json(success.stdout, "CLI capabilities"), "host-consumer-cli"
    )
    require(capabilities.get("ok") is True, "CLI capabilities was not successful")
    data = capabilities.get("data")
    require(isinstance(data, dict), "CLI capabilities data is not an object")
    require(data.get("product") == "omc-rs", "unexpected OMC product")
    names = sorted(
        item.get("name")
        for item in data.get("capabilities", [])
        if isinstance(item, dict) and isinstance(item.get("name"), str)
    )
    require("agent_route" in names, "CLI catalog omitted agent_route")
    require("team_observability" in names, "CLI catalog omitted team_observability")

    team = subprocess.run(
        [
            omc,
            "tool",
            "team-observability",
            "--view",
            "sessions",
            "--root",
            ".",
            "--request-id",
            "host-consumer-team",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    require(team.returncode == 0, f"CLI team observability failed: {team.stderr}")
    team_response = validate_tool_response(
        decode_json(team.stdout, "CLI team observability"), "host-consumer-team"
    )
    require(team_response.get("ok") is True, "CLI team observability was not successful")
    require(
        team_response.get("data", {}).get("schemaVersion") == "omc.team-observability.v1",
        "CLI team observability schema missing",
    )

    interop = subprocess.run(
        [
            omc,
            "tool",
            "interop-snapshot",
            "--root",
            ".",
            "--limit",
            "5",
            "--request-id",
            "host-consumer-interop",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    require(interop.returncode == 0, f"CLI interop snapshot failed: {interop.stderr}")
    interop_response = validate_interop_response(
        decode_json(interop.stdout, "CLI interop snapshot"), "host-consumer-interop"
    )

    bridge = subprocess.run(
        [
            omc,
            "tool",
            "interop-bridge",
            "--action",
            "send_message",
            "--source",
            "omc",
            "--target",
            "omx",
            "--content",
            "host consumer bridge",
            "--root",
            interop_root,
            "--allow-side-effects",
            "--request-id",
            "host-consumer-bridge",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    require(bridge.returncode == 0, f"CLI interop bridge failed: {bridge.stderr}")
    bridge_value = validate_tool_response(
        decode_json(bridge.stdout, "CLI interop bridge"), "host-consumer-bridge"
    )
    if require_active:
        validate_bridge_response(bridge_value, "host-consumer-bridge")
    else:
        require(bridge_value.get("ok") is False, "CLI bridge unexpectedly wrote state")
        require(
            bridge_value.get("error", {}).get("code") == "side_effects_not_allowed",
            "CLI bridge denial did not expose a stable error code",
        )

    code_intel_response: dict[str, Any] | None = None
    if artifact_root is not None:
        code_intel = subprocess.run(
            [
                omc,
                "tool",
                "code-intel-query",
                "--repo",
                repo,
                "--artifact-root",
                artifact_root,
                "--repo-path",
                repo_path,
                "--artifact-type",
                "code_evidence.agent_slice",
                "--limit",
                "3",
                "--request-id",
                "host-consumer-code-intel",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        require(code_intel.returncode == 0, f"CLI Code Intel failed: {code_intel.stderr}")
        code_intel_response = validate_code_intel_response(
            decode_json(code_intel.stdout, "CLI Code Intel"),
            "host-consumer-code-intel",
        )

    denied = subprocess.run(
        [
            omc,
            "tool",
            "python-repl",
            "--action",
            "execute",
            "--session-id",
            "host-consumer-denied",
            "--code",
            "print(1)",
            "--request-id",
            "host-consumer-failure",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    require(denied.returncode == 0, f"CLI failure fixture process failed: {denied.stderr}")
    failure = validate_tool_response(
        decode_json(denied.stdout, "CLI failure fixture"), "host-consumer-failure"
    )
    require(failure.get("ok") is False, "CLI failure fixture unexpectedly succeeded")
    require(
        failure.get("error", {}).get("code") == "side_effects_not_allowed",
        "CLI failure did not expose a stable error code",
    )
    result = {
        "capability_count": len(names),
        "failure_code": failure["error"]["code"],
        "team_view": team_response["data"]["view"],
        "interop_mode": interop_response["data"]["interopMode"],
        "interop_omx_team_count": len(interop_response["data"]["omxTeams"]),
        "interop_normalized_task_count": interop_response["data"]["normalizedTaskCount"],
        "interop_bridge": "written" if require_active else "denied",
    }
    if code_intel_response is not None:
        code_intel_data = code_intel_response["data"]
        result["code_intel_artifact_count"] = len(code_intel_data["artifactRefs"])
        result["code_intel_run"] = code_intel_data["result"]["run"]
    return result


class McpClient:
    def __init__(self, omc: str) -> None:
        self.process = subprocess.Popen(
            [omc, "mcp"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        require(self.process.stdin is not None, "MCP stdin was not opened")
        require(self.process.stdout is not None, "MCP stdout was not opened")

    def call(self, request_id: int, method: str, params: dict[str, Any]) -> dict[str, Any]:
        request = {"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}
        assert self.process.stdin is not None
        assert self.process.stdout is not None
        self.process.stdin.write(json.dumps(request) + "\n")
        self.process.stdin.flush()
        line = self.process.stdout.readline()
        require(line != "", f"MCP closed before replying to {method}")
        response = decode_json(line, f"MCP {method}")
        require(response.get("id") == request_id, f"MCP ID mismatch for {method}")
        require("error" not in response, f"MCP returned an error for {method}: {response}")
        result = response.get("result")
        require(isinstance(result, dict), f"MCP {method} result is not an object")
        return result

    def close(self) -> None:
        if self.process.stdin is not None:
            self.process.stdin.close()
        self.process.wait(timeout=10)
        require(self.process.returncode == 0, "MCP process did not exit cleanly")


def run_mcp(
    omc: str,
    artifact_root: str | None,
    repo: str,
    repo_path: str,
    interop_root: str,
    require_active: bool,
) -> dict[str, Any]:
    client = McpClient(omc)
    try:
        initialized = client.call(1, "initialize", {})
        require(initialized.get("serverInfo", {}).get("name") == "omc-mcp", "wrong MCP server")

        listed = client.call(2, "tools/list", {})
        tools = listed.get("tools")
        require(isinstance(tools, list), "MCP tools/list did not return an array")
        names = sorted(
            tool.get("name")
            for tool in tools
            if isinstance(tool, dict) and isinstance(tool.get("name"), str)
        )
        require("agent_capabilities" in names, "MCP catalog omitted agent_capabilities")
        require("team_observability" in names, "MCP catalog omitted team_observability")
        if artifact_root is not None:
            require(
                "code_intel_artifact_query" in names,
                "MCP catalog omitted code_intel_artifact_query",
            )

        called = client.call(
            3,
            "tools/call",
            {
                "name": "agent_capabilities",
                "arguments": {"requestId": "host-consumer-mcp"},
            },
        )
        content = called.get("content")
        require(isinstance(content, list) and content, "MCP tool result has no content")
        block = content[0]
        require(isinstance(block, dict) and isinstance(block.get("text"), str), "MCP text block missing")
        response = validate_tool_response(
            decode_json(block["text"], "MCP agent_capabilities"), "host-consumer-mcp"
        )
        require(response.get("ok") is True, "MCP capabilities was not successful")
        team_called = client.call(
            4,
            "tools/call",
            {
                "name": "team_observability",
                "arguments": {
                    "view": "sessions",
                    "workingDirectory": ".",
                    "requestId": "host-consumer-team-mcp",
                },
            },
        )
        team_content = team_called.get("content")
        require(
            isinstance(team_content, list) and team_content,
            "MCP team observability result has no content",
        )
        team_block = team_content[0]
        require(
            isinstance(team_block, dict) and isinstance(team_block.get("text"), str),
            "MCP team observability text block missing",
        )
        team_response = validate_tool_response(
            decode_json(team_block["text"], "MCP team observability"),
            "host-consumer-team-mcp",
        )
        require(team_response.get("ok") is True, "MCP team observability was not successful")
        require(
            team_response.get("data", {}).get("schemaVersion")
            == "omc.team-observability.v1",
            "MCP team observability schema missing",
        )
        interop_called = client.call(
            5,
            "tools/call",
            {
                "name": "interop_snapshot",
                "arguments": {
                    "workingDirectory": ".",
                    "limit": 5,
                    "requestId": "host-consumer-interop-mcp",
                },
            },
        )
        interop_content = interop_called.get("content")
        require(
            isinstance(interop_content, list) and interop_content,
            "MCP interop snapshot result has no content",
        )
        interop_block = interop_content[0]
        require(
            isinstance(interop_block, dict) and isinstance(interop_block.get("text"), str),
            "MCP interop snapshot text block missing",
        )
        interop_response = validate_interop_response(
            decode_json(interop_block["text"], "MCP interop snapshot"),
            "host-consumer-interop-mcp",
        )
        bridge_called = client.call(
            6,
            "tools/call",
            {
                "name": "interop_bridge",
                "arguments": {
                    "action": "send_message",
                    "source": "omc",
                    "target": "omx",
                    "content": "host consumer bridge",
                    "workingDirectory": interop_root,
                    "allowSideEffects": True,
                    "requestId": "host-consumer-bridge-mcp",
                },
            },
        )
        bridge_content = bridge_called.get("content")
        require(
            isinstance(bridge_content, list) and bridge_content,
            "MCP interop bridge result has no content",
        )
        bridge_block = bridge_content[0]
        require(
            isinstance(bridge_block, dict) and isinstance(bridge_block.get("text"), str),
            "MCP interop bridge text block missing",
        )
        bridge_value = validate_tool_response(
            decode_json(bridge_block["text"], "MCP interop bridge"),
            "host-consumer-bridge-mcp",
        )
        if require_active:
            validate_bridge_response(bridge_value, "host-consumer-bridge-mcp")
        else:
            require(bridge_value.get("ok") is False, "MCP bridge unexpectedly wrote state")
            require(
                bridge_value.get("error", {}).get("code") == "side_effects_not_allowed",
                "MCP bridge denial did not expose a stable error code",
            )
        code_intel_response: dict[str, Any] | None = None
        if artifact_root is not None:
            code_intel_called = client.call(
                7,
                "tools/call",
                {
                    "name": "code_intel_artifact_query",
                    "arguments": {
                        "repo": repo,
                        "artifactRoot": artifact_root,
                        "repoPath": repo_path,
                        "artifactType": "code_evidence.agent_slice",
                        "limit": 3,
                        "requestId": "host-consumer-code-intel-mcp",
                    },
                },
            )
            code_intel_content = code_intel_called.get("content")
            require(
                isinstance(code_intel_content, list) and code_intel_content,
                "MCP Code Intel result has no content",
            )
            code_intel_block = code_intel_content[0]
            require(
                isinstance(code_intel_block, dict)
                and isinstance(code_intel_block.get("text"), str),
                "MCP Code Intel text block missing",
            )
            code_intel_response = validate_code_intel_response(
                decode_json(code_intel_block["text"], "MCP Code Intel"),
                "host-consumer-code-intel-mcp",
            )

        result = {
            "tool_count": len(names),
            "capability_count": len(response["data"]["capabilities"]),
            "team_view": team_response["data"]["view"],
            "interop_mode": interop_response["data"]["interopMode"],
            "interop_omx_team_count": len(interop_response["data"]["omxTeams"]),
            "interop_normalized_task_count": interop_response["data"]["normalizedTaskCount"],
            "interop_bridge": "written" if require_active else "denied",
        }
        if code_intel_response is not None:
            code_intel_data = code_intel_response["data"]
            result["code_intel_artifact_count"] = len(code_intel_data["artifactRefs"])
            result["code_intel_run"] = code_intel_data["result"]["run"]
        return result
    finally:
        client.close()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--omc", default="omc", help="omc executable or PATH name")
    parser.add_argument(
        "--artifact-root",
        help="published Code Intel artifact root; enables real Code Intel CLI/MCP checks",
    )
    parser.add_argument("--repo", default="omc-rs-src", help="Code Intel repository key")
    parser.add_argument(
        "--repo-path",
        default=".",
        help="checkout used for Code Intel freshness evaluation",
    )
    parser.add_argument(
        "--interop-root",
        default=".",
        help="temporary project root used by the interop bridge fixture",
    )
    parser.add_argument(
        "--require-active-interop",
        action="store_true",
        help="require the active bridge to write a real task/message record",
    )
    args = parser.parse_args()
    try:
        result = {
            "consumer": "json-only-host-consumer",
            "cli": run_cli(
                args.omc,
                args.artifact_root,
                args.repo,
                args.repo_path,
                args.interop_root,
                args.require_active_interop,
            ),
            "mcp": run_mcp(
                args.omc,
                args.artifact_root,
                args.repo,
                args.repo_path,
                args.interop_root,
                args.require_active_interop,
            ),
        }
    except (ConsumerError, OSError, subprocess.SubprocessError) as error:
        print(f"host-consumer failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
