# WebUI v2 与 Action 改造计划

> 状态：已实现（M0–M5），真机验收待执行
> 范围：WebUI 信息架构重做、四项新功能入口、二进制接口拆分、Action 交互重做、日志截断修复
> 参照：[Duck-ToolBox](https://github.com/eltavine/Duck-ToolBox)（MIT）、[Tricky-Addon-Update-Target-List](https://github.com/KOWX712/Tricky-Addon-Update-Target-List)（Apache-2.0）
> 说明：两个项目只作交互与信息结构参考，不复制其代码、样式、图标或文案

## 0. 结论先行

> 实现结果：二进制、WebUI、Action 三段均已落地；`cargo test` 99 项通过，`vite build` 通过。
> 与原计划的差异：更新安装**要求清单提供 sha256** 才自动安装（原计划允许手动保存降级，已实现）；
> Action 按键按确认采用 TWRP 形式（音量下切换、音量上确认）；`keybox_source` 默认 `remote`。
> 真机验收（小米 17 Pro）尚未执行，`-18` 与音量键可用性需在设备上确认。

| 项 | 结论 |
|---|---|
| 技术栈 | 弃用 Astro，改为 **Vite + Vue 3 + vue-router（hash）+ Tailwind v3.4**；保留现有设计令牌 |
| 二进制 | RKP 从 `keybox` 中拆出，新增 `--rkp-*`、`--target-*`、`--self-update` 三组命令，统一 `--json` 输出 |
| 目标作用域 | 新增持久化的 `target_mode=auto|manual`，否则手动勾选会被下次开机的 `--generate` 覆盖 |
| Keybox 来源 | 由「自动优先设备端」改为**显式选择**：本机 RKP 产物 / 远程公共池；不再隐式回退 |
| Action | 改为两级音量键菜单，音量下 = 确认、音量上 = 返回，超时 = 取消且不写文件 |
| 日志截断 | 主因判断为「步骤静默期过长 + 宿主提前结束读取」，不是缓冲问题；用落盘日志 + 心跳 + trap 三项一起兜住 |
| 体积 | 预计 WebUI 从 42 KB 增至 90–120 KB（gzip 前），模块 ZIP 增量 < 100 KB，仍在 1.5 MiB 上限内 |

## 1. 现状与约束

### 1.1 现有 WebUI

- `webroot/` 为 Astro 静态站，单页三块：应用栏、状态面板、两条操作行；已重构为自绘令牌 + 普通 CSS。
- 构建：`xtask build/package` → `npm ci --ignore-scripts` → `npm run build` → 输出到 `module/webroot/`。
- 运行时只有 `ksu.exec` / `ksu.spawn` 两个能力，状态数据靠 4 次独立 shell 调用拼出来。

### 1.2 现有二进制契约

| 命令 | 行为 | 本次变化 |
|---|---|---|
| `--generate` | 扫描包列表 → 写三种后端 | 增加读取 `target_mode`，区分全量与手动集合 |
| `--keybox` | 设备签发优先，失败回落远程 CDN | 只做远程池；设备产物改用 `--keybox --source rkp` |
| `--rkp-status` | 纯文本打印前置条件 | 增加 `--json`，字段化 |
| `--rkp-ta-worker` | 内部隐藏入口 | 保留 |
| `--volume N` | 返回 1（音量+）/ 0（音量−）/ −1（超时） | 复用，不改语义 |

### 1.3 Action 日志证据

用户提供的日志：

```
[1/2] 获取 keybox ...            → 正常走完，退出码 0
[2/2] 更新目标应用配置 ...
[INFO] 正在获取已安装包列表，最多等待 30 秒...
（文件到此结束，没有退出码行，也没有收尾横幅）
```

时间戳 `1792241716` 与文件名的 `20-55-25` 相差约 9 秒。可以确定的结论：

1. 进程终止发生在 `cmd package list packages` 期间，**没有走到任何错误分支**（否则会打印超时或退出码）。
2. 因此不是「缓冲没刷出去」——在它之前的每一行都已经输出；是**读取端先结束**或**进程被外部终止**。
3. 30 秒超时没有触发，说明终止早于超时。

据此把原因归为两类，计划里两条都处理：

- **A：宿主提前结束**。用户看到长时间无输出，切走或关闭 Action 面板，管理器回收进程并保存了当时的部分日志。
- **B：管道继承导致阻塞**。若子进程的孙进程持有 stdout 写端，父进程在 `join_reader` 处永久等待，表现为卡死；用户随后关闭面板，得到同样的截断日志。

## 2. 技术选型

### 2.1 框架与构建

| 方案 | 优点 | 代价 | 结论 |
|---|---|---|---|
| A. 保留 Astro + Vue island | 构建链零改动 | Astro 只作壳，路由与共享状态仍需自建；`client:only` 后无 SSR 收益 | 否 |
| B. Vite + Vue SPA | 单入口、路由/状态自然、构建更简单 | 需替换 `astro.config.mjs`，重写现有 6 个组件 | **采用** |

具体：**Vue 3.5 + TypeScript + vue-router 4（`createWebHashHistory`）+ Tailwind CSS 3.4 + @tailwindcss/vite**。

- 路由用 hash 模式：KernelSU 通过固定入口加载 `index.html`，hash 路由不需要服务端 rewrite，也能用系统返回键。
- **不用 Pinia**：跨页只有一份会话状态（设备信息、能力探测、Keybox 来源），用 `reactive()` 单例 composable 足够，省一个依赖。
- **不用 axios**：所有网络能力都在 Rust 侧，前端只有 `ksu.exec`。
- **不用 shadcn-vue / reka-ui**：组件数不多，自建 10 个左右基础组件即可，避免多层依赖和 Radix 体积。
- **Tailwind 选 v3.4 而非 v4**：v4 依赖 `@layer`、`color-mix()`、`@property`，等于把 WebView 下限抬到 Chromium 111。本模块基线是 API 24+，老设备 WebView 可能停在更早版本；v3.4 产物是普通 CSS，风险低。等真机 WebView 版本确认 ≥ 111 后再评估升级。
- 构建目标 `build.target: 'chrome87'`，`base: './'`（相对路径，任何子路径都能加载）。

### 2.2 与现有实现的关系

- 复用 `webroot/src/styles/tokens.css` 的配色/圆角/动效令牌，Tailwind 只负责布局与间距，颜色仍走 CSS 变量，避免两套设计语言。
- 语言包继续放在 `public/i18n/{zh,en,ja}.json`，沿用现有 key，新增约 80 个键；不引 `vue-i18n`（省 12 KB，且现有 `t()` 语义够用）。
- 深浅色仍由 `<html data-mode>` + `prefers-color-scheme` 驱动，`tf-theme` / `tf-lang` 两个 localStorage 键保持不变，升级后用户选择不丢。

## 3. 信息架构

### 3.1 路由与页面

| 路由 | 页面 | 职责 |
|---|---|---|
| `/` | HomePage | 顶部主信息栏 + 四个入口 + 当前状态摘要 |
| `/target` | TargetPage | 目标作用域二级页：一键全量 / 手动勾选 |
| `/keybox` | KeyboxPage | Keybox 来源选择：本机 RKP / 远程公共池 |
| `/rkp` | RkpPage | 前置识别、签发、错误详情、产物管理与删除 |
| `/update` | UpdatePage | 模块信息、更新通道、检查与安装 |

导航形态：Home 用「列表行」进入二级页（与现有 `ActionRow` 一致），二级页顶部有返回箭头；不使用底部 Tab（入口只有四个，且 Action 场景下以单项任务为主）。

### 3.2 组件树

```
App.vue
├─ AppBar.vue            返回键 / 标题 / 偏好入口
├─ router-view
│   ├─ HomePage.vue
│   │   ├─ DeviceHeader.vue      品牌型号 + 版本徽章 + 能力 chip
│   │   └─ SectionCard + NavRow ×4
│   ├─ TargetPage.vue
│   │   ├─ Segmented（全量 / 手动）
│   │   ├─ SearchField
│   │   └─ AppList → AppRow（复选框 + 图标 + 名称）
│   ├─ KeyboxPage.vue
│   │   ├─ SourceCard（radio 卡片 ×2）
│   │   └─ ApplyBar（当前状态 + 应用按钮）
│   ├─ RkpPage.vue
│   │   ├─ BlockerAlert
│   │   ├─ KeyValueList（前置识别）
│   │   ├─ EvidenceDetails
│   │   ├─ ErrorPanel（透传 + 解释）
│   │   └─ ArtifactList（含删除）
│   └─ UpdatePage.vue
└─ 全局：Sheet / ConfirmDialog / Toast
```

### 3.3 目录结构

```
webroot/src/
├─ main.ts, App.vue, router.ts
├─ styles/tokens.css, tailwind.css
├─ lib/
│   ├─ ksu.ts          桥接：hasHost / exec / spawn / toast
│   ├─ api.ts          命令封装 + JSON 解析（唯一进出口）
│   ├─ i18n.ts         t() / 语言切换
│   └─ format.ts       字节、日期、时长
├─ state/session.ts    设备信息、能力、Keybox/RKP 状态（reactive 单例）
├─ components/         ui/* 基础件 + 业务件
└─ pages/              五个页面
```

## 4. 九项改造

### 4.1 顶部主信息栏

**目标**：替换当前「Root / 版本 / 架构 / Keybox」四宫格，改为一屏可读的设备概览。

**结构**（参考 Duck 的 DeviceWidget，字段按本项目重排）：

```
┌────────────────────────────────────────┐
│  [icon]  Xiaomi 25113PN0EC     v0.6.0  │  ← 型号 + 版本徽章
│          Android 16 · SDK 36           │
│  [KernelSU] [骁龙 8 Elite] [arm64]     │  ← chip 行
│  [安全补丁 2026-09] [SELinux Enforcing]│
└────────────────────────────────────────┘
```

- 版本徽章右侧按状态着色（有新版本时显示 `v0.6.0 → v0.7.0`）。
- chip 可横向滚动，单个 chip 不换行截断。
- 数据来源：一次 `getprop` 全量 dump（约 60 KB）+ `uname -r` + `getenforce` + `module.prop` + `sys.conf`，合并成 1–2 次 shell 调用（现状是 4 次独立调用）。
- 字段：`ro.product.brand` / `ro.product.marketname`（HyperOS 有）/ `ro.product.model`；`ro.build.version.release`、`ro.build.version.sdk`、`ro.build.version.security_patch`；`ro.product.cpu.abi`；`ro.soc.model` / `ro.soc.manufacturer`；`ro.boot.verifiedbootstate` 作为附加 chip（可选）。
- 加载态用骨架屏，失败降级为「型号未知」，不阻塞整页。

**文件**：`components/DeviceHeader.vue`、`state/session.ts`、`lib/api.ts:bootstrap()`。

### 4.2 操作栏二级菜单：更新目标作用域

**目标**：把现在的「一条命令跑全量」改成「进入二级页，一键添加或勾选细调」，参考 Tricky-Addon 的列表交互。

**二级页结构**：

1. 顶部分段控件：`全部应用`（默认，等同当前行为）/ `手动选择`。
2. 工具行：搜索框、`全选` / `清空`、`显示系统应用`开关（默认关）。
3. 列表：应用图标（`ksu://icon/<pkg>`，不可用时回退首字母占位）+ 应用名 + 包名；点击整行切换勾选；勾选状态用 checkbox 与行底色双重反馈。
4. 底部固定操作条：`已选 N 项` + `应用更改`（手动模式下启用）。

**关键设计：选择要能持久化。**
现在开机 `service.sh` 会跑 `--generate` 全量覆盖；若手动勾选只写文件，下次开机就没了。因此引入：

```ini
target_mode=auto              # auto（全量）| manual（按保存的集合）
```

- `manual` 时集合存 `/data/adb/teeforge/target-selection.json`：
  ```json
  { "version": 1, "updated_at": 1792241716, "packages": ["com.example.a", "com.example.b"] }
  ```
- `--generate` 在 manual 模式下只写集合内且仍安装的包；集合内已卸载的包在下次应用时提示并剔除。
- 三个后端仍共用同一份集合（保持现有「一次扫描写所有已存在后端」的语义），不引入按后端分叉——那是另一套产品逻辑，超出本次范围。

**二进制接口**：

| 命令 | 输出 |
|---|---|
| `--target-list --json` | `{ packages:[{name,label,uid,system,selected}], mode, backends:["tricky","teesim","omk"] }` |
| `--target-apply --json --file <path>` | 读取选择并写入受管区块，返回每个后端的结果 |

读文件而不是读 argv，避免 300 个包名撑爆命令行长度。

**实现要点**：

- 列表数据尽量走 `ksu.listPackages('all')` + `ksu.getPackagesInfo()` 拿 label（Tricky-Addon 就是这么做的）；接口不存在时退回 `--target-list --json` 只给包名，图标用 `ksu://icon`，都没有就显示包名首字母。
- 300+ 项列表用 `content-visibility: auto` + `contain-intrinsic-size`，避免一次性布局；搜索做 150ms 防抖。
- 写入仍走 `render_target_file` / `render_teesim_config` / `omk` 三处已有事务，受管区块外的内容不动。

**文件**：`pages/TargetPage.vue`、`components/target/{AppList,AppRow,Toolbar}.vue`、`crates/teeforge/src/target.rs`（新增 manual 分支与两个入口）、`crates/teeforge/src/config.rs`。

### 4.3 Keybox 配置

**目标**：把「来源」变成显式选项，两条路径互不干扰。

**页面结构**：

```
来源
┌──────────────────────────────────────┐
│ ○ 本机 RKP 签发                       │
│   使用本机 TA 申请的设备专属证书链      │
│   状态：已生成（2026-10-17，3 张证书）  │
├──────────────────────────────────────┤
│ ● 远程公共密钥池                       │
│   从 CDN 获取社区公共 keybox           │
│   状态：最近同步 2026-10-11            │
└──────────────────────────────────────┘
[ 应用所选来源 ]
```

- 选中「本机 RKP」但产物不存在时，卡片置灰并给「前往签发」链接（跳 `/rkp`）。
- 应用到哪些后端由后端探测结果决定，页面底部列出「将写入：Tricky Store / TEESimulator」。
- 应用前弹确认：会覆盖现有 `keybox.xml`，并说明备份行为。

**配置**：

```ini
keybox_source=remote          # remote | rkp
```

`rkp_mode` 标记为废弃但仍解析，映射为 `auto|device → rkp`、`remote → remote`，首次运行时写回新键并保留旧键一版。

**二进制**：

| 命令 | 行为 |
|---|---|
| `--keybox` | 远程池下载 + 校验 + 安装（恢复为纯远程） |
| `--keybox --source rkp` | 读取 `/data/adb/teeforge/RKP/keybox.xml`，校验后安装 |
| `--keybox --json` | 返回来源、安装目标、每目标结果 |

不再有「设备失败自动回落远程」的隐式行为——回退由用户在页面上选择，或在 Action 的两级菜单里选择。

### 4.4 本机 RKP 签发页

**目标**：把现在只存在于 CLI 文本输出的前置识别、失败原因，做成可读的页面。

**页面结构**（参考 Duck 的 RkpDevicePage 信息层级，字段为本项目实有项）：

1. **限制提示**（顶部常驻）：仅高通 + arm64 + 已注册 RKP 的设备；签发成功不等于通过任何检测。
2. **前置识别**：
   - 结论徽章：`条件满足` / `缺少条件`
   - 阻断项列表（翻译后的句子，不直接抛英文 id）
   - 键值表：平台、Root、SoC 厂商 / 型号、识别状态、厂商库路径、`/dev/smcinvoke`、fingerprint、结论
   - 「识别证据」折叠区：每条 prop / sysfs 的原始值与来源（现有 `status_lines` 已经在收集，改成字段化）
3. **签发按钮** → 确认弹窗（说明会联网、耗时 10–30 秒）→ 执行并展示分阶段进度。
4. **结果**：协议（CSR_V2）、challenge、cert 数量、device id、产物路径（可复制）、耗时。
5. **失败**：错误卡片，见 4.4.1。

#### 4.4.1 报错透传与原因说明

三层信息，前端不改写后端事实：

```
[RKP_TA_STATUS] KeyMint 返回 -18（INCOMPATIBLE_KEY_FORMAT）
阶段：TA 调用
原始：status=-18, len=0
可能原因：厂商 TA 未实现 CSR_V2 所需的密钥格式，或该固件只接受硬件密钥
建议：换用远程公共池；如为软件密钥场景可尝试导入硬件密钥后重试
```

**错误码表**（`ErrorCode.aidl` 为 AOSP 定义，已核对原文；解释与建议为本项目补充）：

| 码 | 名称 | 常见原因 | 建议 |
|---|---|---|---|
| −18 | INCOMPATIBLE_KEY_FORMAT | TA 不接受当前密钥格式（软件密钥走 CSR_V2 时最常见） | 改用远程池；或换硬件密钥 |
| −38 | INVALID_ARGUMENT | 请求参数被 TA 拒绝，多为固件实现差异 | 上报日志 |
| −39 | UNSUPPORTED_TAG | TA 不支持请求中的 tag | 上报；换设备/固件 |
| −40 | INVALID_TAG | tag 值非法 | 同上 |
| −63/−65 | ATTESTATION_CHALLENGE/APPLICATION_ID_MISSING | 请求缺少必要字段 | 检查 challenge 获取是否成功 |
| −64 | KEYMINT_NOT_CONFIGURED | TA 尚未完成配置 | 重启后重试 |
| −74/−75 | ATTESTATION_KEYS/IDS_NOT_PROVISIONED | 设备未预置证明密钥 | 该设备无法本机签发 |
| −100 | UNIMPLEMENTED | 固件未实现该命令 | 该机型不支持 |
| −101 | VERSION_MISMATCH | 命令版本与 TA 不匹配 | 上报 |
| −1000 | UNKNOWN_ERROR | TA 内部错误 | 重启后重试 |

非 TA 层的失败也单独归类：

| 类别 | 透传内容 | 说明 |
|---|---|---|
| 前置条件 | 阻断项 id + 证据 | 不联网、不调 TA，直接给出结论 |
| 传输 | 工具名、退出码、HTTP 状态、stderr 摘要 | 444 = 设备未在 Google RKP 注册 |
| 帧格式 | `status`、`len`、缓冲区大小、前 32 字节 hex | 用于区分「TA 拒绝」与「返回结构不符合假设」——当前日志里的 `invalid payload length` 属于这一类 |
| 证书链 | 证书数量、主体/签发者不匹配的位置 | 例如叶子公钥与私钥不匹配 |

实现：`crates/teeforge/src/rkp/error.rs` 新增 `RkpError { stage, code, raw, hint_key }`，序列化成 JSON；文案在 i18n 里按 `rkp.error.<stage>.<code>` 查，缺键时回落为「原始码 + 通用建议」。

**注册判断**：HTTP 444 表示该 fingerprint 未在 Google 侧登记，这个要单独成条，不然用户会以为是网络问题。

### 4.5 本机 RKP 产物与删除

**目录**：`/data/adb/teeforge/RKP/`（全大写，按需求）

| 文件 | 权限 | 内容 |
|---|---|---|
| `keybox.xml` | 0600 | Tricky Store 兼容的 keybox，含私钥 |
| `cert-chain.pem` | 0600 | leaf → root，便于排查 |
| `meta.json` | 0600 | `created_at`、`fingerprint`、`device_id`、`cert_count`、`ta_status`、`tool_version`、`sha256` |

**页面**：产物卡片显示生成时间、证书数量、fingerprint 尾 8 位、文件大小；`查看路径`（复制）、`删除`。

**删除语义**：

1. 弹二次确认，明确「会删除私钥，不可恢复；已安装到后端的 keybox.xml 不受影响」。
2. 一次性删除整个 `RKP/` 目录，不逐文件删。
3. 删除后 Keybox 配置页若正指向 RKP，自动回退到远程池并提示。
4. 删除不触碰 `/data/adb/tricky_store/keybox.xml` 与 `/data/adb/teesim/keybox.xml`。

**二进制**：`--rkp-artifacts --json`（列目录）、`--rkp-delete --json`（删目录）。

### 4.6 二进制侧拆分

原则：RKP 只负责「产出产物」，安装由 Keybox 侧负责。

```text
--rkp-status   → 只读探测，JSON
--rkp-issue    → 生成并写入 RKP/（不安装到任何后端）
--rkp-artifacts→ 列出产物
--rkp-delete   → 删除产物

--keybox                    → 远程池
--keybox --source rkp       → 安装 RKP/ 产物
```

改造点：

- `keybox::acquire()` 拆掉对 `rkp::issue` 的调用，删除 `Source::Device|Cached` 分支与 `.rkp-issued` 复用标记。
- `rkp::issue()` 改为返回产物路径而非 XML 字节；写入统一走 `atomic_file::write_private`。
- `AppPaths` 增加 `rkp_dir`；`config.keybox_dir` 仍是远程池缓存目录。
- 兼容：`.rkp-issued` 存在时视为历史遗留，读取一次后忽略并删除，不做迁移。

### 4.7 模块与更新

**页面**：

```
模块
TeeForge-CD v0.6.0 (118)          [有新版本]
通道  [稳定版] [开发版]
[检查更新]

更新内容（折叠 Markdown）
v0.7.0
- ...
[下载并安装]
```

**CDN 策略**（中国大陆优先）：

1. 首选自建域名 `https://teeforge.mcxiaochen.top/files/update/release.json`（现有 updateJson 已指向它，且 zip 也在该域）。
2. 备选 GitHub Release / raw（仅在自建域名失败时使用，避免默认走被墙线路）。
3. 两个端点并发 HEAD 探测（超时 3 秒），取先返回者；结果缓存 5 分钟。
4. 清单需扩展字段：

```json
{
  "versionCode": 129,
  "version": "v0.7.0",
  "zipUrl": "https://teeforge.mcxiaochen.top/files/teeforge_cd-Release.zip",
  "changelog": "https://teeforge.mcxiaochen.top/files/CHANGELOG.md",
  "sha256": "…",
  "size": 1596390,
  "minVersionCode": 100,
  "channel": "stable"
}
```

**安装**：`--self-update --install --json`

- 下载到 `/data/adb/teeforge/update/pkg.zip`，校验 `sha256`，不一致直接失败且不安装。
- 清单没有 `sha256` 时不自动安装，只保存到 `/sdcard/Download/` 并提示手动安装（安全优先）。
- 安装器按顺序探测：`/data/adb/ksud module install` → `apd module install` → `magisk --install-module`，都没有则落到保存文件。
- 成功返回 `needs_reboot: true`，页面显示「重启生效」。

**文件**：`crates/teeforge/src/update.rs`（新增）、`pages/UpdatePage.vue`。

### 4.8 Action 脚本交互

**按键约定**（默认值，后续可配置）：

- **音量下 = 确认 / 执行**（主操作）
- **音量上 = 返回 / 取消**
- 15 秒无输入 = 取消，不写任何文件，退出码 0

**流程**：

```text
action.sh
  └─ 询问：是否执行？          音量下=是  音量上=否
       ├─ 否 → 打印「已取消」→ exit 0
       └─ 是 → 主菜单（高亮第 1 项，音量下选当前项，音量上移到下一项）
            ├─ 1. 更新 Keybox
            │    └─ 二级菜单：1. 本机 RKP 签发并应用  2. 远程公共池
            │         ├─ RKP：--rkp-issue && --keybox --source rkp
            │         └─ 远程：--keybox
            └─ 2. 更新作用域
                 └─ 直接执行 --generate
  └─ 收尾：更新描述、打印结果横幅、exit
```

实现要点：

- 复用 `teeforge --volume 15`（返回 1/0/−1），逐级调用，不新增按键解析代码。
- 每级菜单先打印完整选项再等待，避免「只有一行提示、用户不知道在等什么」。
- `--volume` 返回错误（输入设备打不开）时，打印「无法读取音量键，请使用 WebUI 或 CLI」并退出 1，而不是默默执行全量。
- 不确认就不写文件：所有写操作都在用户确认之后。
- Action 与 WebUI 走同一组命令，保证行为一致。

**风险**：部分设备在 Action 上下文读 input 事件可能被 SELinux 拦截。需要在真机验证；失败时给出明确提示而不是静默。

### 4.9 日志截断修复

针对 1.3 的两类原因，四项措施一起上：

1. **落盘日志（始终开启）**
   - `/data/adb/teeforge/logs/action-<YYYYmmdd-HHMMSS>.log`，保留最近 10 份。
   - 启动时打印路径，结束时打印结果；即使 UI 截断，也有完整副本。
   - 实现：`tee -a`（优先 `set -o pipefail` 拿真实退出码；不支持的 shell 退回「先重定向到临时文件再回放」）。

2. **静默期心跳**
   - `process::run_bounded` 增加可选心跳：子进程运行超过 3 秒时，每 3 秒输出一行 `…仍在执行（已 3s/30s）`。
   - 用户能看出「还在跑」，不会误以为卡死而关闭面板。

3. **缩短单步静默期**
   - `PACKAGE_LIST_TIMEOUT` 30s → 12s；超时后再用 `pm list packages -U --user 0` 兜底一次。
   - 两次都失败才报错，错误信息包含两种命令各自的退出原因。

4. **收尾兜底**
   - `trap 'on_exit' EXIT INT TERM HUP`：无论正常结束还是被杀，都写一行 `ACTION_END ret=…` 到日志文件，并尽量 echo 到 stdout。
   - 若进程被 SIGKILL（trap 无法执行），只能靠落盘日志的最后一行动手排查——这也正是第 1 条的价值。

5. **顺带加固 `run_bounded`（条件性）**
   - 读线程 `join` 改为带超时（`mpsc::recv_timeout`，子进程退出后再等 500 ms），避免孙进程持有写端导致永久阻塞。
   - 先加日志探针确认是否真的发生；没有复现就不改这段并发逻辑，避免引入新风险。

## 5. 命令与 JSON 契约

统一外壳：

```json
{ "ok": true, "data": { } }
{ "ok": false, "error": { "stage": "ta", "code": "-18", "message": "…", "hint": "rkp.error.ta.-18", "raw": { } } }
```

失败时 JSON 仍写 stdout，退出码非零；`--json` 未指定时保持现有中文/英文双语文本。

| 命令 | 关键字段 |
|---|---|
| `--bootstrap --json` | device{model,android,sdk,patch,abi,soc}、root{method,version}、module{version,code}、capabilities{rkp,keybox_source,target_mode} |
| `--target-list --json` | packages[]、mode、backends[] |
| `--target-apply --json --file <json>` | results[]{backend,written,skipped} |
| `--rkp-status --json` | supported、blockers[]、evidence[]{source,value}、library、device、fingerprint、soc{state,model,manufacturer} |
| `--rkp-issue --json` | artifacts{keybox,cert_chain,meta}、cert_count、device_id、duration_ms |
| `--rkp-artifacts --json` | exists、files[]{path,size,mtime}、meta{} |
| `--rkp-delete --json` | deleted、freed_bytes |
| `--keybox --json [--source rkp]` | source、targets[]{path,ok}、sha256 |
| `--self-update --check --json` | current、latest{...}、available、endpoint |
| `--self-update --install --json` | installed、installer、path、needs_reboot |

## 6. 配置与文件布局

```ini
# 新增
keybox_source=remote          # remote | rkp
target_mode=auto              # auto | manual
update_channel=stable         # stable | dev
update_base=https://teeforge.mcxiaochen.top/files/update

# 废弃但兼容一版
rkp_mode=auto                 # 解析后映射到 keybox_source
```

```text
/data/adb/teeforge/
├─ RKP/                     # 本机签发产物（0600）
│   ├─ keybox.xml
│   ├─ cert-chain.pem
│   └─ meta.json
├─ target-selection.json    # manual 模式的选择集合
├─ update/pkg.zip           # 下载中转，安装后删除
├─ logs/action-*.log        # Action 运行日志，保留 10 份
└─ keybox/                  # 远程池缓存（不变）
```

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Vue/Tailwind 重写引入回归 | 现有两条命令入口不可用 | 先做路由骨架与 Home，功能入口逐条替换；每步都能构建验收 |
| WebView 版本低 | 白屏 | `chrome87` 目标；不依赖 `:has()`、`color-mix()`；首屏无 JS 时显示静态提示 |
| 老管理器无 `listPackages`/`ksu://icon` | 列表无名称/图标 | 全部能力探测后降级为包名 + 首字母块 |
| 300+ 应用渲染卡顿 | 列表滚动掉帧 | `content-visibility` + 搜索过滤 + 分页追加（每页 80） |
| 手动集合与开机覆盖冲突 | 用户选择丢失 | `target_mode` 持久化；`--generate` 分流；升级时默认 auto 保持旧行为 |
| RKP 真机仍失败 | 功能不可用 | 错误透传 + 原因表；保留远程池；不阻塞其他功能 |
| 更新包被篡改 | 安全问题 | 强制 sha256；清单缺失则拒绝自动安装 |
| 删除 RKP 目录误操作 | 私钥丢失 | 二次确认 + 明确说明「不影响已安装的 keybox」 |
| Action 音量键被 SELinux 拦 | 无法交互 | 明确报错并指向 WebUI/CLI，不静默执行 |
| 管道读取被孙进程占住 | Action 卡死 | 条件性加固 `run_bounded`，先探针后改 |

## 8. 验证方式

**主机侧（每阶段）**

- `cargo test -p teeforge`、`cargo clippy --all-targets`、`cargo fmt --check`
- 新增单测：`target_mode=manual` 的写入与剔除、JSON 外壳、错误码映射、manifest 校验、删除目录的幂等
- `npm run build` + `npm run preview`，本地 mock 宿主下走完五个页面
- 截图核对：360 / 412 宽度、明暗两套、空态/错误态/加载态
- `cargo run -p xtask -- build` 四 ABI + `package` + `verify`

**真机（小米 17 Pro，KernelSU）**

| 场景 | 预期 |
|---|---|
| 首屏加载 | 信息栏 3 秒内出数据，无横向溢出 |
| 目标作用域：全量 → 切手动 → 取消勾选 3 个 → 应用 | target.txt 受管区块只少这 3 项，区块外内容不变 |
| 重启后 | manual 集合仍生效，不被全量覆盖 |
| Keybox 配置：选远程 → 应用 | 后端 keybox 更新，页面状态同步 |
| RKP 页：前置识别 | 阻断项与证据与 `--rkp-status` 文本一致 |
| RKP 签发 | 成功则产物落 `RKP/`；失败则错误卡片显示阶段与码（预计仍可能是 −18） |
| 删除产物 | 目录消失，页面回退到远程池 |
| 更新检查 | 能读到 129/更高版本，显示更新日志 |
| Action：音量下确认 → 更新作用域 | 完整日志 + 收尾横幅 + 落盘日志存在 |
| Action：音量上取消 | 不写任何文件，退出码 0 |
| Action：15 秒无操作 | 自动取消 |
| 中途关闭面板 | `logs/action-*.log` 里仍有完整记录（验证 4.9） |

**回归**：升级安装保留 `config.conf`；旧 `rkp_mode` 正确映射；`--generate` 全量行为与旧版逐字节一致（除时间戳）。

## 9. 分阶段实施

| 阶段 | 内容 | 产出 |
|---|---|---|
| M0 | 二进制契约：`--json` 外壳、`--rkp-*` 拆分、`--target-*`、`--self-update`、错误码表 | 可编译 + 单测通过，CLI 可用 |
| M1 | 前端骨架：Vite+Vue+Router+Tailwind、令牌迁移、AppBar、DeviceHeader | Home 可跑，信息栏数据正确 |
| M2 | 目标作用域页 + manual 持久化 | 全量/手动两条路径可切换并生效 |
| M3 | Keybox 配置页 + RKP 页（含产物与删除） | 两条来源可切换，错误可读 |
| M4 | 模块与更新页 + 清单 sha256 | 检查/下载/安装闭环 |
| M5 | Action 两级菜单 + 日志加固 | 真机走通三条分支 |
| M6 | 文档、真机验收、截图归档 | README/ARCHITECTURE/ROADMAP 更新 |

顺序理由：先定二进制契约，前端才有稳定接口；Action 放最后，因为它依赖前四项命令。

## 10. 待确认

1. **按键映射**：计划采用「音量下 = 确认、音量上 = 返回」，与现有安装脚本「音量+ = 保留、音量− = 清除」方向一致。是否改成 TWRP 式「音量下切换、音量上确认」？
2. **默认 Keybox 来源**：建议 `remote`（保持现有行为），已有 RKP 产物的设备在页面上高亮提示切换。是否改为「有产物则默认 RKP」？
3. **更新安装**：是否允许 `ksud module install` 这类静默安装，还是只下载到 Download 目录由用户在管理器里手动装？前者体验好，后者更保守。
4. **手动集合的粒度**：确认三后端共用一份集合可以接受（本项目既有语义），还是需要按后端分别维护。
