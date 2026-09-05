# TeeForge-CD 文档索引

本目录保存会随项目变化的架构、开发、运维、决策和维护资料。它只是定位入口，不是每轮任务的默认必读内容。

AI 已能从 `AGENTS.md` 判断目标文档时，应直接读取对应文件；只有无法判断位置时才读取本索引，不要默认加载整个 `docs/`。

## 当前文档

| 任务 | 文档 |
|---|---|
| 项目定位、CLI、源码入口 | [`project/OVERVIEW.md`](project/OVERVIEW.md) |
| Rust 架构、数据流、配置和兼容性 | [`project/ARCHITECTURE.md`](project/ARCHITECTURE.md) |
| 构建、测试、CI、打包和发布 | [`development/BUILD_AND_RELEASE.md`](development/BUILD_AND_RELEASE.md) |
| 安装、ABI、权限和设备验收 | [`operations/INSTALLATION.md`](operations/INSTALLATION.md) |
| Rust 四架构迁移决策 | [`decisions/0001-rust-four-abi-migration.md`](decisions/0001-rust-four-abi-migration.md) |
| 当前状态和下一步 | [`maintenance/ROADMAP.md`](maintenance/ROADMAP.md) |
| 可复用的故障经验 | [`maintenance/LESSONS.md`](maintenance/LESSONS.md) |
| 每轮结束的文档检查 | [`maintenance/DOCUMENTATION.md`](maintenance/DOCUMENTATION.md) |
| 根 README 的结构、语言和审查要求 | [`maintenance/README_STANDARD.md`](maintenance/README_STANDARD.md) |

## 历史与私密资料

- [`history/`](history/) 只保存旧阶段快照，不代表当前实现。
- [`private/README.md`](private/README.md) 说明本地私密资料边界；真实 Keybox 维护文件被 Git 忽略，不得提交。

## 维护边界

- `AGENTS.md` 保存长期协作硬规范。
- 本目录保存可变化的项目事实和可复用经验。
- 源码、测试和工作流是实现行为的最终证据；发现文档漂移时修正文档。
- 新增文档必须放入已有分类，并同步本索引；不要在根目录新增第二套 AI 指引。
