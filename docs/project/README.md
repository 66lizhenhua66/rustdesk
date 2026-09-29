# 项目文档入口

这里管理本产品的需求、阶段计划、进度、技术决策和调研，随当前 RustDesk fork 一起进入 Git。父目录仅作为工作空间，不新增父 Git 仓库。

## 从哪里继续

1. 读 [PROGRESS.md](PROGRESS.md)，了解当前状态和下一步。
2. 读 [SPEC.md](SPEC.md)，确认产品范围、被控端改造、权限/公网安全和多端架构。
3. 按 [ROADMAP.md](ROADMAP.md) 找到当前阶段，再读对应阶段规格。
4. 遇到架构取舍时先查 [DECISIONS.md](DECISIONS.md)。

| 文档 | 职责 |
| --- | --- |
| [SPEC.md](SPEC.md) | 产品需求与架构主文档：被控改造、认证权限、公网部署、鸿蒙优先的多端架构和验收 |
| [ROADMAP.md](ROADMAP.md) | 阶段顺序、依赖、退出条件与代码入口 |
| [PROGRESS.md](PROGRESS.md) | 唯一的当前进度入口，记录结果、证据、阻塞和下一步 |
| [DECISIONS.md](DECISIONS.md) | 已确认方向、提议及其取舍，避免反复讨论 |
| [specs/001-baseline-and-harmony-feasibility.md](specs/001-baseline-and-harmony-feasibility.md) | 第一阶段范围草案与验收清单 |
| [research/](research/) | 带日期和来源的调研快照，不代表当前实现能力 |

## 维护约定

- 产品范围用 `REQ-xxx`，阶段用 `M0`、`M1`，阶段规格用三位编号；在相关 PR/提交描述中引用对应编号。
- 需求变化先更新 SPEC；阶段进度只在 PROGRESS 维护，避免多个看板出现不同结论。
- 文档状态分为「草案」「已确认」「已取代」；任务状态分为「未开始」「进行中」「待验证」「阻塞」「完成」。
- 「完成」必须附提交、命令结果或设备验证记录。源码已有、编译通过、测试通过、真机通过是不同证据，不互相替代。
- 阻塞项写明缺少什么、影响哪一步，以及解除条件。未经测试的兼容性不能写成已支持。
- 每次代码任务收尾，同步更新相关规格和 PROGRESS。仅更新有变化的记录，不要求无意义的每日提交。
- 测量/真机证据记录日期、提交 SHA、设备与系统/API、远端版本、网络路径、步骤、结果和日志位置。只提交脱敏摘要；密钥、凭据、设备标识和原始诊断日志不进 Git。
- 实现方案按阶段细化。不要把全产品路线图当成已经批准的逐文件实现计划。

## 代码索引

本仓库已建立 GitNexus 本地索引，MCP 仓库名为 `rustdesk`。定位调用链、查询符号上下游或评估改动影响时可先使用 GitNexus，再核对当前源码；索引不是完整运行时依赖证明。

在本仓库根目录执行：

```text
gitnexus status
gitnexus analyze --index-only --name rustdesk --workers 4
```

先用 `status` 检查新鲜度，在源码、分支或提交变动后按需刷新。`--index-only` 保留已有 AGENTS.md、CLAUDE.md 和技能文件；`.gitnexus/` 由本机 `.git/info/exclude` 排除，不提交索引数据库。新 clone 需各自建立索引。当前没有生成 embeddings，不能把图查询与全文检索误认为已启用向量语义检索。

## 历史调研

- [开源远控选型（2026-09-29）](research/2026-09-29-remote-desktop-options.md)
- [RemoteDeskHarmonyOS 完成度（2026-09-29）](research/2026-09-29-remotedeskharmonyos-completeness.md)
- [当前 RustDesk 认证与权限源码调查（2026-09-30）](research/2026-09-30-auth-permissions-audit.md)
- [当前 RustDesk 控制端可移植性调查（2026-09-30）](research/2026-09-30-controller-portability-audit.md)

前两份报告由工作空间根目录归档而来，根目录原始文件保留为历史快照；后续维护统一在本目录进行。新增源码调查也直接保存于此，不再双份更新。
