## TeeForge-CD Dev v0.3.2-39cfbd9

### 构建信息 Build Info
- Version: `v0.3.2-39cfbd9`
- Version Code: `92`
- Commit: `39cfbd9`

### 最近提交 Recent Commits
- 39cfbd9 fix: download fallback now checks actual data, not popen return
- 78032a2 refactor: remove shell description update from customize.sh, rely on C binary via service.sh
- 5654f36 fix: show full ABI arch (arm64-v8a/armeabi-v7a) in description
- b32bd1e feat: dynamic module description with status info
- dbc568d docs: update TODO and README with latest features
- ab3b378 docs: update CLAUDE.md — download fallback, CDN zip distribution
- 6a82185 feat: release zip pushed to page branch, zipUrl uses self-hosted CDN
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
