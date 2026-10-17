# TeeForge WebUI

KernelSU 模块 WebUI，Vite + Vue 3 + vue-router（hash）+ Tailwind 3.4。

## 开发

```bash
cd webroot
npm install
npm run dev        # 本地开发服务器
npm run build      # 输出到 module/webroot/
npm run preview
```

## 结构

```
index.html              主题初始化脚本（首帧前）+ 挂载点
src/main.ts             i18n → createApp → router → loadSession
src/router.ts           hash 路由：/ /target /keybox /rkp /update
src/lib/ksu.ts          宿主探测（hasHost/canSpawn）、exec/spawn、应用名与图标
src/lib/api.ts          唯一的命令出口：拼 --json 参数、解析信封、抛 ApiError
src/lib/i18n.ts         字典加载、t()、setLanguage；缺键返回键名
src/lib/format.ts       字节/时间/指纹格式化
src/state/session.ts    --bootstrap 结果单例，跨页共享
src/components/ui/      AppButton/AppSheet/AppBadge/SegmentedControl/AppIcon/EmptyState
src/components/layout/  AppBar/DeviceInfo/SectionCard/NavRow/PreferencesSheet
src/pages/              HomePage/TargetPage/KeyboxPage/RkpPage/UpdatePage
src/styles/tokens.css   设计令牌（明/暗两套，纯 hex/rgba）
src/styles/tailwind.css Tailwind 入口与全局基样式
public/i18n/*.json      运行时字典（zh/en/ja）
```

## 关键约定

**命令契约**：所有后端调用必须走 `src/lib/api.ts`，参数带 `--json`。成功是
`{"ok":true,"data":{...}}`，失败是 `{"ok":false,"error":{stage,code,message,hint,raw}}`。
`--json` 只允许搭配一个命令；进度日志在 stderr，不会污染 stdout。

**流式执行**：耗时命令（`--rkp-issue`、`--keybox`、`--self-update install`）用
`callStream`，stderr 直接当进度日志展示；没有 `ksu.spawn` 时自动退回一次性执行。

**降级**：`hasHost()` 为假时页面只读，不发起任何命令；`ksu.getPackagesInfo` /
`ksu://icon` 不可用时，列表退回包名与首字母占位。

**主题与语言**：`<html data-mode="auto|light|dark">` 由 `index.html` 的内联脚本在首帧前设定；
持久化键固定为 `tf-theme`（JSON `{theme,mode}`）与 `tf-lang`，升级不能改。

**样式**：颜色只通过 `tokens.css` 的 CSS 变量，Tailwind 配置把它们映射成
`bg-surface` / `text-muted` 这类工具类。不要在组件里写死色值；不要使用
`color-mix()`、`:has()`、`dvh`，构建目标是 `chrome87`。

**i18n**：新增文案必须同时补 `public/i18n/{zh,en,ja}.json` 三份，键集合保持一致；
动态键（如 `rkp.code.<code>`）用 `hasKey()` 判断后再取。

## 构建

- `npm run build` → `../module/webroot/`（`emptyOutDir` 会清空旧产物）
- `module/webroot/` 是构建产物，已 gitignore
- `package.json` 为锁定依赖，`xtask` 使用 `npm ci`

## 文档

- Astro 已移除，不要再按 Astro 方式新增页面或组件
- KernelSU WebUI: https://kernelsu.org/zh_CN/guide/module-webui.html
