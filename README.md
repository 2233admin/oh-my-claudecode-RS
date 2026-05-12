# oh-my-claudecode-RS

[English](#english) | [中文](#中文)

---

<a id="english"></a>

[![CI](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/2233admin/oh-my-claudecode-RS/actions)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org)
[![Tests](https://img.shields.io/badge/tests-816-brightgreen.svg)](#build--test)
[![Binary Size](https://img.shields.io/badge/binary-397%20KB-brightgreen.svg)](#performance)

## What is OMC-RS

A **Rust rewrite** of [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) — multi-agent orchestration, hooks, skills, MCP routing, statusline, and context injection for Claude Code.

**No npm. No node_modules. No upstream churn dependency.**

| Metric | Value |
|--------|-------|
| Crates | **17** |
| Lines of Rust | **42,000+** |
| Tests | **816** |
| HUD cold-start median | **3.81ms** (Win11, Ryzen 9800X3D) |
| Binary size (HUD) | **397 KB** |
| Edition | Rust 2024, rustc 1.85+ |

## Quick Start

```bash
# Clone
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS

# Build
cargo build --release

# Test
cargo test --workspace
```

### Claude Code Integration

Add to `~/.claude/settings.json`:

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/target/release/omc-hud"
  }
}
```

### CLI Usage

```bash
# HUD
cargo run -p omc-hud

# CLI
cargo run -p omc-cli -- --help

# Team
cargo run -p omc-team -- init
cargo run -p omc-team -- start ./task.md --team-size 3
```

## Key Features

### Agent Orchestration
- **DAG task graph** with dependency resolution
- **8-state lifecycle FSM**: spawn → idle → busy → waiting → blocked → done → failed → terminated
- **3-layer fault tolerance**: retry, circuit breaker, fallback
- **Priority scheduler** for task execution

### Claude Code Integration
- **Statusline** with 13 elements, i18n, CJK width support
- **Hooks system** with 15 event types
- **Skills system** with 40+ built-in templates
- **MCP tool server** with JSON-RPC over stdio

### Multi-Provider Support
- **Git providers**: GitHub, GitLab, Gitea, Bitbucket, Azure DevOps
- **Notifications**: Slack, tmux, QQ, Feishu
- **Host adapters**: Claude Code, Codex CLI

## Architecture

```
omc-shared          (foundation -- types, config, routing, resilience)
  │
  ├── omc-macros    (proc macros: #[derive(Tool)])
  │
  ├── omc-hooks     (15 event types)
  ├── omc-skills    (40+ templates)
  ├── omc-context   (AGENTS.md, rules injection)
  ├── omc-hud       (13 elements, i18n)
  ├── omc-mcp       (tool registry)
  ├── omc-git-provider (6 providers)
  ├── omc-notifications
  ├── omc-wiki
  ├── omc-interop
  ├── omc-autoresearch
  ├── omc-python
  └── omc-xcmd
  │
  ├── omc-team      (DAG, lifecycle, comms, governance)
  ├── omc-host     (Claude/Codex adapters)
  │
  ├── omc-cli      (26+ commands)
  └── omc-installer
```

## Crate Reference

| Crate | Lines | Tests | Description |
|-------|---|---|---|
| omc-shared | 7,931 | 100 | Types, config, routing, tools, state, memory, circuit breaker |
| omc-team | 14,218 | 234 | DAG task graph, 8-state FSM, priority scheduler, fault tolerance |
| omc-hud | 4,449 | 205 | Statusline binary, 13 elements, CJK width, i18n |
| omc-mcp | 2,732 | 11 | MCP tool server, JSON-RPC over stdio |
| omc-interop | 2,399 | 43 | Cross-tool interoperability (OMC/OMX) |
| omc-hooks | 1,853 | 58 | Claude Code hooks (15 events) |
| omc-skills | 1,729 | 31 | Skills loader (40+ templates) |
| omc-git-provider | 1,463 | 10 | Git hosting abstraction |
| omc-notifications | 1,292 | 22 | Multi-platform notifications |
| omc-autoresearch | 1,218 | 10 | Autoresearch runtime |
| omc-wiki | 1,213 | 4 | Wiki knowledge layer |
| omc-context | 1,119 | 7 | Context injection |
| omc-cli | 559 | 11 | CLI dispatcher |
| omc-installer | 578 | 9 | Installation, update |
| omc-python | 335 | 2 | Persistent Python REPL |
| omc-macros | 378 | 0 | Proc macros |
| omc-xcmd | 243 | 0 | x-cmd integration |
| omc-host | - | 68 | Host abstraction layer |

## Build & Test

```bash
cargo build --release                # optimized binary (~400 KB)
cargo test --workspace               # all 816 tests
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## Performance

Measured on Windows 11 (Ryzen 9800X3D):

| Process | Time | Memory |
|---------|------|--------|
| `omc-hud` cold-start (median) | **3.81ms** | **7.4 MB** |
| `node hub.mjs` (upstream) | ~390-502ms | 84.5 MB |

```powershell
# Reproduce
$n = Start-Process node -ArgumentList "$env:USERPROFILE/.omc/mcp-hub/hub.mjs" -PassThru
Start-Sleep 4; (Get-Process -Id $n.Id).WorkingSet64 / 1MB  # ~84 MB

$r = Start-Process "target/release/omc-hud.exe" -PassThru
Start-Sleep 4; (Get-Process -Id $r.Id).WorkingSet64 / 1MB  # ~7 MB
```

## Documentation

| Document | Description |
|----------|-------------|
| [README.md](README.md) | This file |
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture and design decisions |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Developer guide, building, testing |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines |
| [CHANGELOG.md](CHANGELOG.md) | Version history |
| [CLAUDE.md](CLAUDE.md) | AI agent instructions |

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Quick Contribution Steps

```bash
# 1. Fork and clone
git clone https://github.com/YOUR_NAME/oh-my-claudecode-RS.git

# 2. Create feature branch
git checkout -b feat/your-feature

# 3. Make changes
cargo build --release
cargo test --workspace
cargo clippy --workspace -- -D warnings

# 4. Commit and push
git commit -m "feat: add your feature"
git push origin feat/your-feature

# 5. Open PR
```

## License

MIT — see [LICENSE](LICENSE).

Independent re-implementation. No source code copying from upstream.

---

<a id="中文"></a>

# oh-my-claudecode-RS

[![CI](https://github.com/2233admin/oh-my-claudecode-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/2233admin/oh-my-claudecode-RS/actions)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.85+-orange.svg)](https://www.rust-lang.org)

## 是什么

[oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) 的 **Rust 重写版** — 专注于 Claude Code 的多 Agent 编排、钩子、Skills、MCP 路由、状态栏和上下文注入。

**无 npm。无 node_modules。无上游依赖。**

| 指标 | 数值 |
|------|------|
| Crates | **17 个** |
| Rust 代码 | **42,000+ 行** |
| 测试 | **816 个** |
| HUD 冷启动 | **3.81ms** (Win11, Ryzen 9800X3D) |
| 二进制大小 | **397 KB** |

## 快速开始

```bash
# 克隆
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS

# 构建
cargo build --release

# 测试
cargo test --workspace
```

### Claude Code 集成

在 `~/.claude/settings.json` 中添加：

```json
{
  "statusLine": {
    "type": "command",
    "command": "/absolute/path/to/target/release/omc-hud"
  }
}
```

## 核心功能

### Agent 编排
- **DAG 任务图** 带依赖解析
- **8 状态生命周期 FSM**
- **3 层容错**：重试、熔断、兜底
- **优先级调度器**

### Claude Code 集成
- **状态栏**：13 个元素、i18n、CJK 宽度支持
- **钩子系统**：15 种事件类型
- **Skill 系统**：40+ 内置模板
- **MCP 工具服务器**

## 文档

| 文档 | 说明 |
|------|------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | 系统架构和设计决策 |
| [DEVELOPMENT.md](DEVELOPMENT.md) | 开发者指南、构建、测试 |
| [CONTRIBUTING.md](CONTRIBUTING.md) | 贡献指南 |
| [CHANGELOG.md](CHANGELOG.md) | 版本历史 |

## 贡献

欢迎贡献！请查看 [CONTRIBUTING.md](CONTRIBUTING.md) 了解指南。

## 许可证

MIT — 见 [LICENSE](LICENSE)。
