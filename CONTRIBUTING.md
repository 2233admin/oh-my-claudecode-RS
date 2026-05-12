# Contributing to oh-my-claudecode-RS

[English](#english) | [中文](#中文)

---

<a id="english"></a>

## Welcome!

Thank you for your interest in contributing to oh-my-claudecode-RS. This is a Rust rewrite of oh-my-claudecode, focused on multi-agent orchestration, hooks, skills, and MCP routing for Claude Code.

## Project Structure

```
oh-my-claudecode-RS/
├── crates/
│   ├── omc-shared/        # Foundation: types, config, routing, tools
│   ├── omc-team/          # Agent orchestration: DAG, lifecycle, comms
│   ├── omc-hud/           # Statusline: 13 elements, i18n
│   ├── omc-hooks/         # Hook system: 15 event types
│   ├── omc-skills/        # Skill system: 40+ templates
│   ├── omc-mcp/           # MCP tool server
│   ├── omc-host/          # Host abstraction: Claude + Codex
│   └── ...
├── tests/
├── docs/
├── scripts/
└── templates/
```

## Development Setup

### Prerequisites

- **Rust 1.85+** (edition 2024)
- **git**
- **cargo** (comes with Rust)

### Build

```bash
# Clone
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS

# Build
cargo build --release

# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

### IDE Setup

We recommend **Rust Analyzer** for VS Code or JetBrains IDEs:

```json
{
  "rust-analyzer.linkedProjects": ["./Cargo.toml"],
  "rust-analyzer.cargo.allTargets": true
}
```

## Code Conventions

### Naming

- Functions/variables: `snake_case`
- Types/traits: `PascalCase`
- Constants: `SCREAMING_SNAKE_CASE`
- Crate names: `kebab-case` (in Cargo.toml)

### Error Handling

- **Library code**: Use `thiserror` for typed error enums
- **Application code**: Use `anyhow` for ergonomic propagation
- **No panics in library code**: Return `Result` instead

### Async

- Use `tokio` runtime
- Use `async-trait` for trait objects
- All I/O is async

### Testing

- Every module gets a `#[cfg(test)] mod tests` block
- Integration tests go in `tests/` per crate
- Use `tempfile` for filesystem tests
- Test behavior, not implementation

## Branching Strategy

```
master          # Stable releases only
  └── feat/*   # Feature branches
  └── fix/*    # Bug fix branches
  └── docs/*   # Documentation branches
  └── refactor/*  # Refactoring branches
```

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add agent DAG task graph
fix: resolve HUD CJK width calculation
docs: update architecture diagram
refactor: extract routing logic to omc-shared
test: add integration tests for skills loader
chore: update dependencies
```

## Pull Request Process

1. **Fork** the repository
2. **Create** a feature branch: `git checkout -b feat/my-feature`
3. **Make** your changes with tests
4. **Ensure** all tests pass: `cargo test --workspace`
5. **Run** clippy: `cargo clippy --workspace -- -D warnings`
6. **Format**: `cargo fmt --`
7. **Open** a PR against `master`

### PR Template

```markdown
## Summary

Brief description of the changes.

## Changes

- Change 1
- Change 2

## Testing

How was this tested?

## Checklist

- [ ] Tests pass
- [ ] Clippy clean
- [ ] Format checked
- [ ] Documentation updated (if needed)
```

## Areas to Contribute

### High Priority

| Area | Description | Crate |
|------|-------------|-------|
| Integration layer | Wire omc-team to Claude Code subprocess | omc-team |
| Agent execution | Implement subagent spawn lifecycle | omc-team |
| Hook binary | Build hook executable for Claude Code hooks | omc-hooks |

### Medium Priority

| Area | Description | Crate |
|------|-------------|-------|
| More skills | Add more built-in skill templates | omc-skills |
| Git providers | Complete GitLab/Bitbucket/Azure integration | omc-git-provider |
| i18n | Add more locale support | omc-hud |

### Good First Issues

- [ ] Add tests for existing functions
- [ ] Improve error messages
- [ ] Documentation fixes
- [ ] Code comments for complex logic

## Getting Help

- **Issues**: Open a GitHub issue for bugs or feature requests
- **Discussions**: Use GitHub Discussions for questions
- **README**: Check [README.md](README.md) for project overview

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

<a id="中文"></a>

# 贡献指南

## 欢迎！

感谢您对 oh-my-claudecode-RS 的关注！这是一个用 Rust 重写的 oh-my-claudecode，专注于 Claude Code 的多 Agent 编排、钩子、Skills 和 MCP 路由。

## 项目结构

```
oh-my-claudecode-RS/
├── crates/
│   ├── omc-shared/        # 基础：类型、配置、路由、工具
│   ├── omc-team/          # Agent 编排：DAG、生命周期、通信
│   ├── omc-hud/           # 状态栏：13 个元素、i18n
│   ├── omc-hooks/         # 钩子系统：15 种事件类型
│   ├── omc-skills/        # Skill 系统：40+ 模板
│   ├── omc-mcp/           # MCP 工具服务器
│   ├── omc-host/          # 主机抽象：Claude + Codex
│   └── ...
├── tests/
├── docs/
├── scripts/
└── templates/
```

## 开发环境

### 前置要求

- **Rust 1.85+** (edition 2024)
- **git**
- **cargo** (随 Rust 安装)

### 构建

```bash
# 克隆
git clone https://github.com/2233admin/oh-my-claudecode-RS.git
cd oh-my-claudecode-RS

# 构建
cargo build --release

# 运行测试
cargo test --workspace

# 代码检查
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

## 分支策略

```
master          # 稳定版本
  └── feat/*   # 功能分支
  └── fix/*    # 修复分支
  └── docs/*   # 文档分支
```

## 提交规范

遵循 [Conventional Commits](https://www.conventionalcommits.org/zh-hans/):

```
feat: 添加 Agent DAG 任务图
fix: 修复 HUD CJK 宽度计算
docs: 更新架构图
refactor: 提取路由逻辑到 omc-shared
test: 添加技能加载器集成测试
chore: 更新依赖
```

## Pull Request 流程

1. **Fork** 仓库
2. **创建** 功能分支: `git checkout -b feat/my-feature`
3. **编写** 代码和测试
4. **确保** 所有测试通过: `cargo test --workspace`
5. **运行** clippy: `cargo clippy --workspace -- -D warnings`
6. **格式化**: `cargo fmt --`
7. **提交** PR 到 `master`

## 贡献领域

### 高优先级

| 领域 | 描述 | 涉及 Crate |
|------|------|-----------|
| 集成层 | 将 omc-team 连接到 Claude Code 子进程 | omc-team |
| Agent 执行 | 实现 subagent spawn 生命周期 | omc-team |
| Hook 二进制 | 为 Claude Code hooks 构建可执行文件 | omc-hooks |

### 中等优先级

| 领域 | 描述 | 涉及 Crate |
|------|------|-----------|
| 更多 skills | 添加更多内置 skill 模板 | omc-skills |
| Git providers | 完成 GitLab/Bitbucket/Azure 集成 | omc-git-provider |
| i18n | 添加更多语言支持 | omc-hud |

## 获取帮助

- **Issues**: 使用 GitHub Issues 报告 bug 或请求功能
- **Discussions**: 使用 GitHub Discussions 提问

## 许可证

贡献即表示您同意您的贡献将根据 MIT 许可证授权。
