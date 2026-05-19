# Agent Orchestration Layer — 扩展计划

> 基于 omc-hub-rs + KohakuTerrarium 的轻量级 Agent 编排方案

## Context

**目标**: 将 omc-hub-rs 从 MCP Hub 扩展为完整的 Agent 编排层，并集成 KohakuTerrarium 的核心功能

**动机**:
- omc-hub-rs 已有成熟的 MCP Hub 架构（skill 生命周期、工具分发、注册表模式）
- KohakuTerrarium 有完整的 Agent 框架（垂直/水平编排分离、Channel 通信、sub-agent 系统）
- 需要一个轻量级（<5MB）、原生支持 Claude Code subagent 的方案

---

## 两个项目的核心能力

### omc-hub-rs (已有)

| 能力 | 说明 |
|------|------|
| MCP Hub | JSON-RPC over stdio，支持 stdio + HTTP 两种传输 |
| Skill 生命周期 | load/unload/reload，自动工具注册表 |
| 工具分发 | namespace 前缀 (skill__*)，世代号追踪 |
| 26 内置工具 | state, notepad, memory, trace, ast_grep |
| 轻量 | 2.5MB binary, 7.4MB 内存 |

### KohakuTerrarium (待集成)

| 能力 | 说明 |
|------|------|
| Channel 通信 | SubAgentChannel (queue) + AgentChannel (broadcast) |
| Sub-agent 系统 | 10 内置类型 (coordinator/explorer/worker/critic 等) |
| 垂直编排 | Controller + Sub-agents 层级 |
| 水平编排 | Terrarium 多 Agent 协调 |
| 非阻塞执行 | 工具并行，LLM 流式不阻塞 |
| 配置驱动 | YAML 配置文件 |

---

## 架构设计

### 总体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     Claude Code                              │
│                   (subagent / task)                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      omc-hub-rs                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐   │
│  │   MCP Hub   │  │ Agent Layer │  │   Terrarium     │   │
│  │ (已有)      │  │  (新增)     │  │   Channel       │   │
│  │             │  │             │  │   (部分移植)     │   │
│  └─────────────┘  └─────────────┘  └─────────────────┘   │
│                                                             │
│  工具: hub_*, omc_*, agent_*, skill__*                     │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │ Skill A  │   │ Skill B  │   │ Skill N  │
        │ (stdio)  │   │ (HTTP)   │   │ (stdio)  │
        └──────────┘   └──────────┘   └──────────┘
```

### 新增模块

| 模块 | 文件 | 职责 |
|------|------|------|
| AgentCore | `src/agent/core.rs` | AgentSession、AgentManager |
| Channel | `src/agent/channel.rs` | QueueChannel、BroadcastChannel |
| SubAgent | `src/agent/subagent.rs` | SubAgent 生命周期 |
| ExecutionPool | `src/agent/pool.rs` | 并行执行控制 |

### 新增工具 (14 个)

| 工具 | 功能 |
|------|------|
| `agent_create` | 创建 Agent 会话 |
| `agent_delete` | 删除 Agent 会话 |
| `agent_list` | 列出所有会话 |
| `agent_get_state` | 获取会话状态 |
| `agent_execute` | 单任务执行 |
| `agent_execute_parallel` | 多任务并行 |
| `agent_interrupt` | 中断执行 |
| `agent_update_context` | 更新上下文 |
| `agent_clone` | 克隆会话 |
| `agent_preserve` | 持久化到磁盘 |
| `agent_restore` | 从磁盘恢复 |
| `channel_create` | 创建 Channel |
| `channel_send` | 发送消息 |
| `channel_receive` | 接收消息 |

---

## 实现计划

### Phase 1: Agent 核心 (P0)

1. **src/agent/mod.rs** - 模块入口
2. **src/agent/session.rs** - AgentSession 数据结构
3. **src/agent/manager.rs** - AgentManager 生命周期
4. **Hub 集成** - 新增 agent_* 工具分发

### Phase 2: Channel 通信 (P1)

1. **src/agent/channel.rs** - Channel trait + Queue + Broadcast
2. **channel_* 工具** - 创建/发送/接收

### Phase 3: Sub-Agent 集成 (P1)

1. **src/agent/subagent.rs** - SubAgent 生命周期
2. **复用 KohakuTerrarium 的 10 种 sub-agent 角色定义**
3. **集成其配置文件解析逻辑**

### Phase 4: Claude Code 兼容 (P0)

1. **实现 Task tool 兼容接口**
2. **支持 subagent 嵌套**
3. **集成到 omc-hub-rs 的 skill 系统**

---

## 关键设计决策

### 1. Channel vs Skill

| 机制 | Channel | Skill |
|------|---------|-------|
| 用途 | Agent 间通信 | 工具集封装 |
| 模式 | 队列/广播 | 命名空间前缀 |
| 持久化 | 可选 | 已支持 |

### 2. Sub-agent 实现

- 复用 KohakuTerrarium 的 `SubAgentManager` 逻辑
- 适配 omc-hub-rs 的 ChildMcp 机制
- 支持 Claude Code 原生 Task tool

### 3. 后端选择

- **优先**: Claude Code Task tool（原生支持 subagent 嵌套）
- **保留扩展**: 自建 LLM 调用接口（支持更多模型）

### 4. 向后兼容

- 所有改动都是**新增**
- 不修改现有 26 个工具的行为
- 现有 skill 系统保持不变

---

## 关键文件

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/agent/mod.rs` | 新建 | Agent 模块入口 |
| `src/agent/session.rs` | 新建 | AgentSession |
| `src/agent/manager.rs` | 新建 | AgentManager |
| `src/agent/channel.rs` | 新建 | Channel 通信 |
| `src/agent/subagent.rs` | 新建 | SubAgent 集成 |
| `src/agent/pool.rs` | 新建 | ExecutionPool |
| `src/hub.rs` | 编辑 | 集成 Agent 层 |
| `src/main.rs` | 编辑 | 模块引入 |
| `Cargo.toml` | 编辑 | 新增依赖 |

---

## KohakuTerrarium 集成参考

### Channel 通信模型

```rust
// QueueChannel - 点对点，一个消费者
// AgentChannel - 广播，所有订阅者

pub trait Channel {
    async fn send(&self, msg: Message) -> Result<(), ChannelError>;
    async fn recv(&self) -> Result<Message, ChannelError>;
}

pub enum ChannelType {
    Queue,      // SubAgentChannel
    Broadcast,  // AgentChannel
}
```

### Sub-Agent 角色 (10 种)

| 角色 | 职责 |
|------|------|
| coordinator | 多 Agent 编排协调 |
| explorer | 代码库探索（只读） |
| plan | 实现规划 |
| worker | 代码实现/修复 |
| critic | 代码审查 |
| summarize | 内容摘要 |
| research | 深度研究 |
| memory_read | 记忆读取 |
| memory_write | 记忆写入 |
| response | 响应生成 |

### Terrarium 配置示例

```yaml
terrarium:
  root:
    config: creatures/root
  creatures:
    - name: brainstorm
      channels:
        listen: [seed, team_chat]
        can_send: [ideas, team_chat]
  channels:
    seed: { type: queue }
    ideas: { type: queue }
    team_chat: { type: broadcast }
```

---

## 验证步骤

1. `cargo test` - 现有测试 + 新增测试
2. `cargo clippy -- -D warnings` - 代码质量
3. MCP 协议测试 - 验证 agent_* 工具可用
4. Claude Code 集成测试 - subagent 嵌套

---

## 相关文档

- [KohakuTerrarium](https://github.com/Kohaku-Lab/KohakuTerrarium) - 参考框架
- [omc-hub-rs](https://github.com/2233admin/omc-hub-rs) - MCP Hub 基础实现
