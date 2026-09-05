# AGENTS.md

本文件是 TeeForge-CD 的唯一 AI 协作规范入口。每轮开始必须完整阅读本文件；其他文档只按当前任务需要读取，不默认加载整个 `docs/` 或根目录 `README.md`。

## 长期硬规范

1. 中文优先；用户可见日志和代码注释遵循项目既有的中英双语约定。
2. 复杂任务先只读检查、复述需求和输出计划，确认后再修改。
3. 只做任务要求的精准变更，不顺手重构无关代码或文档。
4. 不覆盖、删除或回滚用户已有修改；不把 `.claude/` 等用户文件纳入本次变更。
5. 未明确授权不得 push、发版、打标签、强制覆盖或执行其他危险操作。
6. 用户说“提交”只表示本地 commit，不自动 push。
7. 连接、调试或修改用户主力设备前必须取得明确授权。
8. 自动化构建结果与真机验收结果必须分别报告，构建通过不代表设备行为通过。
9. Keybox 私密维护细节不得写入公开文档、提交、日志或最终回复。
10. 所有外部输入、下载内容、命令退出状态和平台差异都必须按实际结果处理，不能凭假设放行。
11. 每轮结束必须判断是否需要更新项目文档或踩坑记录，并在最终回复中说明判断结果。

## 按需文档导航

不要默认读取全部文档，只读取当前任务相关内容：

- 项目定位与源码入口：`docs/project/OVERVIEW.md`
- 当前技术架构：`docs/project/ARCHITECTURE.md`
- 构建、测试、CI、发版：`docs/development/BUILD_AND_RELEASE.md`
- 安装、ABI、设备验收：`docs/operations/INSTALLATION.md`
- 当前任务与待验收项：`docs/maintenance/ROADMAP.md`
- 已知问题与踩坑记录：`docs/maintenance/LESSONS.md`
- 文档维护规则：`docs/maintenance/DOCUMENTATION.md`
- Rust 重构决策：`docs/decisions/0001-rust-four-abi-migration.md`
- 无法确定文档位置时：`docs/README.md`

`AGENTS.md` 只在长期规范或文档入口变化时修改，不记录版本、架构快照、临时状态、详细命令或具体故障过程。
