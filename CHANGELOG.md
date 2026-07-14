## TeeForge-CD v0.2.1

### 构建信息 Build Info
- Version: `v0.2.1`
- Version Code: `57`
- Commit: `63ef470f8dc14ab5c53f24e2cdc88a28f83eea91`

### 更新内容 Changes
63ef470 fix: 修正 author 字段 [Fix author field]
824fc8d fix: dev 构建写入 updateJson 指向 dev 分支 [Dev build sets updateJson to dev branch]
ee57c34 chore: 移除 master 上的 dev.json（由 dev 分支自动维护）[Remove dev.json from master]
7ff2c50 release: v0.2.0 — update release.json and CHANGELOG.md [auto]
b0a6429 fix: release 推送使用 HEAD:master 兼容 detached HEAD [Fix push for detached HEAD]
6477f0a fix: 修正自动更新 URL 路径 + dev 分支推送 + CHANGELOG 自动生成
1185658 docs: 更新 README 和 TODO，标记自动更新完成 [Update README and TODO]
c17b591 feat: KernelSU 自动更新 updateJson + CDN 加速 [KernelSU auto-update with CDN]
f75ebc7 docs: 更新 TODO，标记日志系统已完成 [Update TODO, mark logging as done]
dc566ff Update README content and formatting
d208a6c Update README with new quotes and formatting
5706105 docs: README 全面升级，酷炫风格 [README overhaul, hype style]
516ec56 fix: 移除多余的 mv 命令（源文件名已正确）[Remove redundant mv]
f86128d docs: 更新 CLAUDE.md [Update CLAUDE.md]
6230661 docs: 更新 README [Update README]
b2d4bd9 docs: 小结标题加时间戳 [Add timestamp to retrospective title]
0448606 docs: 添加反思小结 [Add retrospective summary]
090919a fix: 修复4个bug + 改进 [Fix 4 bugs + improvements]
cb84aca docs: 更新 CLAUDE.md 和 README.md [Update CLAUDE.md and README.md]
0dc7208 refactor: 移除 download 模块及相关配置 [Remove download module and config]
59b1fab refactor: 预置 resetprop-rs，移除下载逻辑 [Bundle resetprop-rs, remove download]
a706902 fix: 镜像 URL 去除末尾斜杠，下载大小验证 [Trim mirror slash, download size check]
43eee56 fix: keybox 解密需要二次 base64 解码 [Double base64 decode for keybox]
ce715e8 fix: 权限设置移到音量键检测前 [Move chmod before volume key check]
4026c35 feat: --volume 命令、配置保留逻辑、keybox SHA256 修复 [--volume cmd, config preserve, keybox SHA256 fix]
f0cd5a7 feat: 添加卸载脚本 [Add uninstall script]
94da749 fix: keybox 解密改用纯 C，隐藏 URL 日志 [Pure C decrypt, hide URL log]
442e02d fix: 修复 resetprop-rs 路径检测 [Fix resetprop-rs path detection]
fa6b2b0 feat: 日志按天存储，限制15份，每次运行空行分隔 [Daily log, limit 15, separate runs]
c8a2b71 fix: keybox 不触发地区检测 [Keybox skip region detection]
bc0ba33 fix: 仅下载时检测地区 [Only detect region for download]
533965d fix: action.sh 热更新逻辑，日志路径修复 [Fix action.sh hot update, log path]
6bc6cb9 feat: debug 日志开关，文件日志输出 [Debug switch, file logging]
0b2e941 perf: 并行测速 [Parallel speed test]
a52126c style: 日志分段空行 [Log section spacing]
551923b feat: 测速流程日志输出 [Speed test logging]
f52927d refactor: 音量键监听独立模块，优化安装日志 [Volume key standalone module, optimize install log]
92e47bc feat: 音量键选择地区 [Volume key region selection]
5516586 fix: 修复中国大陆地区检测 [Fix CN region detection]
fad5cdc ci: 修复 release 构建版本号 [Fix release build version]
aa4d568 fix: build.sh 优先使用 CI 环境变量版本号 [Prioritize CI env version]
40b3c01 ci: 修复 dev 版本命名 [Fix dev version naming]
2265dbd ci: dev 构建标记 [Dev] [Add [Dev] tag to dev build]
93273d5 chore: 更新 module.prop [Update module.prop]
a131d39 feat: service.sh 移除 keybox，新增 action.sh [Remove keybox from service.sh, add action.sh]
a885e99 fix: 安装日志中英双语 [Bilingual install logging]
ab0e03a fix: 安装脚本添加下载日志 [Add download logging to customize.sh]
