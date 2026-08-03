## TeeForge-CD Dev v0.5.0-ed21b09

### 构建信息 Build Info
- Version: `v0.5.0-ed21b09`
- Version Code: `124`
- Commit: `ed21b09`

### 最近提交 Recent Commits
- ed21b09 fix(verify): 保存并恢复调用者 set -e 状态，修复安装失败 fix(verify): save/restore caller's set -e state to fix install failure
- 6894315 fix(ci): 稳定构建产物改固定名 + commit message 精细化 fix(ci): fixed-name release zip + richer commit message
- d3c4cad docs: 修复文档与代码脱节 docs: fix doc-code drift
- 6fb2899 chore: 移除残留的根 CHANGELOG.md 与 obsolete dev 分支 chore: remove stale root CHANGELOG.md and obsolete dev branch
- a71221a refactor: 版本号统一为 teeforge.h 单点维护 refactor: unify version to single source teeforge.h
- 8b80208 fix(verify): 修复 MODDIR 使完整性校验生效 fix(verify): fix MODDIR so integrity check actually runs
- 2913355 docs: add keybox sync workflow fix report
- e5354b3 fix(ci): remove deprecated key-control and key-hash from keybox sync
- 52029a3 fix(package): handle sha256sum binary mode asterisk in sed pattern
- c1caf0f feat: add verify.sh for file integrity check + package README in module
- 7d711bd docs: improve CLAUDE.md with WebUI, config loading order, static arrays, CI version injection notes
- eaad5c7 fix(webui): set cwd for ksu.exec so teeforge finds ./sys.conf
- f661b39 fix(webui): spawn→exec, lang sync, glassmorphism theme overhaul
- c99311f ci: integrate WebUI build into package.sh and CI workflows
- a02a932 feat(webui): add KernelSU WebUI with Astro, i18n, dual themes
- c501752 docs: update CLAUDE.md with refactored keybox/blhide implementation details
- 636c3cd fix(blhide): move is_category_enabled and del_props before bl_build_script
- 441e03c fix(keybox): reorder download fallback wget→curl, add user-facing logs
- e4e3451 fix: code review findings — security, correctness, performance
- 01f337d docs: update CLAUDE.md with config split, install flow, root detection
