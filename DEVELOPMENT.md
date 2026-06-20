# Development Guide

[English](#english) | [中文](#中文)

---

<a id="english"></a>

## Prerequisites

- **Rust 1.85+** (edition 2024)
- **cargo** (comes with Rust)
- **git**

Verify your Rust version:

```bash
rustc --version  # Should be 1.85.0 or higher
rustup update    # Update if needed
```

## Building

### Full Build

```bash
cargo build --release
```

Binary outputs:
- `target/release/omc-hud` (~400 KB)
- `target/release/omc-cli`

### Incremental Build

```bash
# Build single crate
cargo build -p omc-hud

# Build with debug info
cargo build

# Build specific target
cargo build --release --bin omc-cli
```

## Testing

### Run All Tests

```bash
cargo test --workspace
```

### Run Tests for Specific Crate

```bash
cargo test -p omc-team
cargo test -p omc-hud
cargo test -p omc-skills
```

### Run with Output

```bash
cargo test --workspace -- --nocapture
```

### Test Coverage

```bash
cargo tarpaulin --workspace
```

## Code Quality

### Clippy (Linter)

```bash
cargo clippy --workspace -- -D warnings
```

### Format Check

```bash
cargo fmt --check
```

### Format and Fix

```bash
cargo fmt
```

## Project Structure

### Crate Overview

| Crate | Lines | Tests | Purpose |
|-------|-------|-------|---------|
| omc-shared | 7,931 | 100 | Foundation: types, config, routing, tools |
| omc-team | 14,218 | 234 | Agent orchestration: DAG, lifecycle, comms |
| omc-hud | 4,449 | 205 | Statusline: 13 elements, i18n |
| omc-mcp | 2,732 | 11 | MCP tool server |
| omc-hooks | 1,853 | 58 | Hook system: 15 events |
| omc-skills | 1,729 | 31 | Skill system: 40+ templates |
| omc-git-provider | 1,463 | 10 | Git providers abstraction |
| omc-interop | 2,399 | 43 | Cross-tool interop |
| omc-notifications | 1,292 | 22 | Notifications: Slack, tmux |
| omc-context | 1,119 | 7 | Context injection |
| omc-wiki | 1,213 | 4 | Wiki knowledge layer |
| omc-autoresearch | 1,218 | 10 | Autoresearch runtime |
| omc-cli | 559 | 11 | CLI dispatcher |
| omc-installer | 578 | 9 | Installer |
| omc-python | 335 | 2 | Python REPL |
| omc-macros | 378 | 0 | Proc macros |
| omc-xcmd | 243 | 0 | x-cmd integration |
| omc-host | - | 68 | Host abstraction |

### Adding a New Crate

1. Create directory: `crates/omc-myfeature/`
2. Add `Cargo.toml`:

```toml
[package]
name = "omc-myfeature"
version = "0.1.0"
edition = "2024"

[dependencies]
omc-shared = { path = "../omc-shared" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tracing = "0.1"

[dev-dependencies]
tempfile = "3"
```

3. Add to workspace `Cargo.toml`:

```toml
[workspace]
members = [
    "crates/omc-myfeature",
    # ...existing crates
]
```

4. Add module in `crates/omc-myfeature/src/lib.rs`

## Architecture Deep Dive

### Dependency Flow

```
omc-shared (foundation)
    │
    ├── omc-host (Claude/Codex adapters)
    ├── omc-team (orchestration)
    ├── omc-hooks
    ├── omc-skills
    ├── omc-context
    ├── omc-hud
    ├── omc-mcp
    ├── omc-git-provider
    ├── omc-interop
    ├── omc-notifications
    ├── omc-wiki
    ├── omc-autoresearch
    ├── omc-python
    ├── omc-xcmd
    │
    └── omc-cli (top-level)

omc-macros (build-time only)
omc-installer (standalone)
```

### omc-team Architecture

```
┌─────────────────────────────────────────┐
│                  omc-team               │
├─────────────────────────────────────────┤
│  Phase Controller                      │
│  ├── Planning Phase                    │
│  ├── Execution Phase                   │
│  └── Review Phase                      │
├─────────────────────────────────────────┤
│  Task Graph (DAG)                     │
│  ├── TaskNode { id, deps, status }    │
│  ├── DependencyResolver                │
│  └── PriorityScheduler                  │
├─────────────────────────────────────────┤
│  Agent Lifecycle                       │
│  ├── FSM: spawn→idle→busy→...→done   │
│  ├── HealthMonitor                     │
│  └── FaultTolerance                    │
├─────────────────────────────────────────┤
│  Communication                         │
│  ├── Inbox (JSONL files)              │
│  ├── Outbox (JSONL files)             │
│  └── Governance Gates                  │
└─────────────────────────────────────────┘
```

### omc-hud Architecture

```
stdin JSON (Claude Code status)
    │
    ▼
┌─────────────┐
│   Parser   │ → StatusInfo struct
└─────────────┘
    │
    ▼
┌─────────────┐
│   Cache    │ → File-based cache
└─────────────┘
    │
    ▼
┌─────────────┐
│   Sampler  │ → Random element sampling
└─────────────┘
    │
    ▼
┌─────────────┐
│  Renderer   │ → 13 elements
└─────────────┘
    │
    ▼
stdout (terminal output)
```

## Debugging

### Enable Tracing

```bash
RUST_LOG=omc_team=debug cargo run -p omc-team
```

### HUD Debug

```bash
# Test with sample input
echo '{"claude":{"model":"opus","memory_used":1234}}' | target/release/omc-hud
```

### Test Claude Code Integration

```bash
# Build HUD
cargo build --release -p omc-hud

# Add to ~/.claude/settings.json
{
  "statusLine": {
    "type": "command",
    "command": "/path/to/omc-hud"
  }
}
```

## Release Process

### Version Bump

1. Update version in `Cargo.toml` (workspace root)
2. Update version in `CHANGELOG.md`
3. Create git tag: `git tag v0.x.x`
4. Push: `git push origin master --tags`

### Build Artifacts

```bash
# Build all platforms
cargo build --release --target x86_64-pc-windows-msvc
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-apple-darwin
```

---

<a id="中文"></a>

## 前置要求

- **Rust 1.85+** (edition 2024)
- **cargo** (随 Rust 安装)
- **git**

验证 Rust 版本：

```bash
rustc --version  # 应该是 1.85.0 或更高
rustup update    # 如需更新
```

## 构建

### 完整构建

```bash
cargo build --release
```

二进制输出：
- `target/release/omc-hud` (~400 KB)
- `target/release/omc-cli`

### 增量构建

```bash
# 构建单个 crate
cargo build -p omc-hud

# 带调试信息构建
cargo build
```

## 测试

### 运行所有测试

```bash
cargo test --workspace
```

### 运行特定 Crate 的测试

```bash
cargo test -p omc-team
cargo test -p omc-hud
```

## 代码质量

### Clippy (检查器)

```bash
cargo clippy --workspace -- -D warnings
```

### 格式化检查

```bash
cargo fmt --check
```

## 项目结构

### Crate 概览

| Crate | 行数 | 测试 | 用途 |
|-------|------|------|------|
| omc-shared | 7,931 | 100 | 基础：类型、配置、路由、工具 |
| omc-team | 14,218 | 234 | Agent 编排：DAG、生命周期、通信 |
| omc-hud | 4,449 | 205 | 状态栏：13 个元素、i18n |
| omc-mcp | 2,732 | 11 | MCP 工具服务器 |
| omc-hooks | 1,853 | 58 | 钩子系统：15 种事件 |
| omc-skills | 1,729 | 31 | Skill 系统：40+ 模板 |
| omc-git-provider | 1,463 | 10 | Git providers 抽象 |
| omc-interop | 2,399 | 43 | 跨工具互操作 |
| omc-cli | 559 | 11 | CLI 分发器 |
| omc-installer | 578 | 9 | 安装器 |
| omc-python | 335 | 2 | Python REPL |
| omc-macros | 378 | 0 | 过程宏 |
| omc-xcmd | 243 | 0 | x-cmd 集成 |

## 添加新 Crate

1. 创建目录：`crates/omc-myfeature/`
2. 添加 `Cargo.toml`：
3. 添加到 workspace `Cargo.toml`：
4. 在 `crates/omc-myfeature/src/lib.rs` 添加模块

## 调试

### 启用追踪

```bash
RUST_LOG=omc_team=debug cargo run -p omc-team
```

### HUD 调试

```bash
# 使用示例输入测试
echo '{"claude":{"model":"opus","memory_used":1234}}' | target/release/omc-hud
```

## 发布流程

### 版本更新

1. 更新 `Cargo.toml` 中的版本
2. 更新 `CHANGELOG.md`
3. 创建 git tag：`git tag v0.x.x`
4. 推送：`git push origin master --tags`
