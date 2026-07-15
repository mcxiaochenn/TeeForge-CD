## TeeForge-CD v0.5.0

### 构建信息 Build Info
- Version: `v0.5.0`
- Version Code: `113`
- Commit: `eaad5c7b56ed3d6e1083ab90230f521ae41a9659`

### 更新内容 Changes
- eaad5c7 fix(webui): set cwd for ksu.exec so teeforge finds ./sys.conf
- f661b39 fix(webui): spawn→exec, lang sync, glassmorphism theme overhaul
- c99311f ci: integrate WebUI build into package.sh and CI workflows
- a02a932 feat(webui): add KernelSU WebUI with Astro, i18n, dual themes
- c501752 docs: update CLAUDE.md with refactored keybox/blhide implementation details
- 636c3cd fix(blhide): move is_category_enabled and del_props before bl_build_script
- 441e03c fix(keybox): reorder download fallback wget→curl, add user-facing logs
- e4e3451 fix: code review findings — security, correctness, performance
- 01f337d docs: update CLAUDE.md with config split, install flow, root detection
