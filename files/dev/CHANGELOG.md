## TeeForge-CD Dev v0.5.0-a7fd758

### 构建信息 Build Info
- Version: `v0.5.0-a7fd758`
- Version Code: `128`
- Commit: `a7fd758`

### 最近提交 Recent Commits
- a7fd758 docs: 脱敏公开文档中的 keybox 加解密细节
- fc25eea docs: CLAUDE.md 的 backup/ 描述更新为加解密维护指南
- d7fa00f fix(ci): 状态文件不再混入 CDN files/keybox/
- ddee09c fix(keybox): 修正 CDN URL 为自建 CDN，修复 keybox 下载 404
- 3b945de fix(verify): 保存并恢复调用者 set -e 状态，修复安装失败 fix(verify): save/restore caller's set -e state to fix install failure
- 1e3fdc9 fix(ci): 稳定构建产物改固定名 + commit message 精细化 fix(ci): fixed-name release zip + richer commit message
- c465b5d docs: 修复文档与代码脱节 docs: fix doc-code drift
- bbc08df chore: 移除残留的根 CHANGELOG.md 与 obsolete dev 分支 chore: remove stale root CHANGELOG.md and obsolete dev branch
- 1a5ff2f refactor: 版本号统一为 teeforge.h 单点维护 refactor: unify version to single source teeforge.h
- 30174c6 fix(verify): 修复 MODDIR 使完整性校验生效 fix(verify): fix MODDIR so integrity check actually runs
- 748d139 docs: add keybox sync workflow fix report
- 86d5095 fix(ci): remove deprecated key-control and key-hash from keybox sync
- 44a8f68 fix(package): handle sha256sum binary mode asterisk in sed pattern
- 2f47163 feat: add verify.sh for file integrity check + package README in module
- 8501bfe docs: improve CLAUDE.md with WebUI, config loading order, static arrays, CI version injection notes
- 08bcb59 fix(webui): set cwd for ksu.exec so teeforge finds ./sys.conf
- bc35ec3 fix(webui): spawn→exec, lang sync, glassmorphism theme overhaul
- 6b090ca ci: integrate WebUI build into package.sh and CI workflows
- 5ec130d feat(webui): add KernelSU WebUI with Astro, i18n, dual themes
- 1351802 docs: update CLAUDE.md with refactored keybox/blhide implementation details
