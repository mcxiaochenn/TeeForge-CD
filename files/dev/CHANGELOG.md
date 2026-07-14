## TeeForge-CD Dev v0.3.0-c07fe4a

### 构建信息 Build Info
- Version: `v0.3.0-c07fe4a`
- Version Code: `85`
- Commit: `c07fe4a`

### 最近提交 Recent Commits
- c07fe4a fix: remove stale release.json write to master (update/ dir deleted)
- 83ee3af Refactor README to eliminate redundant warnings
- d117c56 fix: debug mode not enabled in dev builds; download fallback strategy
- 9c68080 docs: update README and TODO
- 653a665 docs: update CLAUDE.md with latest CLI, config, and CI changes
- db4e160 fix: replace curl with wget for keybox download
- 5024e0c fix: banner indentation
- bf637a5 fix: remove duplicate 'v' in banner version display
- 5d399b8 fix: simplify rootdetect call — --rootdetect already skips auto detection
- 2394ec4 refactor: move root detection before config selection in install flow
- 1375599 fix: volume key prompt polluted by root detection output during install
- 92f54b0 fix: dev build failed — config.conf deleted from repo, generate dynamically
- 321835b refactor: dynamically generate config.conf at install time
- ae41301 refactor: split config into sys.conf (auto-generated) and config.conf (user)
- cefb4d9 fix: skip double root detection when --rootdetect flag is used
- c8402c2 feat: root detection runs every execution, saves to config; --no-rootdetect to skip
- 4034c05 fix: changelog commit list formatting — each commit on its own line
- f6f9b8d feat: show ASCII banner + version + root info when run without args
- 62e5389 chore: remove update/ from master (now on page branch)
- 99fa1ac refactor: push all CDN files to page branch/files/
