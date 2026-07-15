## TeeForge-CD v0.4.0

### 构建信息 Build Info
- Version: `v0.4.0`
- Version Code: `104`
- Commit: `c5e3648df1b103aa276394b95ded768052bebd52`

### 更新内容 Changes
- c5e3648 fix: code review findings — null term, dedup paths, stderr redirect
- 8ee0860 fix(ui): reorder install prompts — options at end, add spacing
- b148220 fix: review findings — timeout logic, cached detection, dead code
- 8d7f7b1 docs: update CLAUDE.md with resetprop tool selection and fallback
- f5904b3 feat: add resetprop tool selection during installation
- c2f87e4 docs: document blhide feature toggles in CLAUDE.md
- 2cdc393 feat: add blhide feature toggles (master + per-category switches)
- 413cb92 fix: move kaomoji before status icon in description
- 3c38e1d docs: fix CLAUDE.md and README.md review findings
- f5e8b5e fix: medium C code issues from review
- b7ef71b fix(ci): keybox-sync critical issues
- cdd225f fix: description retry root detection, fix keybox path, add kaomoji
- 39cfbd9 fix: download fallback now checks actual data, not popen return
- 78032a2 refactor: remove shell description update from customize.sh, rely on C binary via service.sh
- 5654f36 fix: show full ABI arch (arm64-v8a/armeabi-v7a) in description
- b32bd1e feat: dynamic module description with status info
- dbc568d docs: update TODO and README with latest features
- ab3b378 docs: update CLAUDE.md — download fallback, CDN zip distribution
