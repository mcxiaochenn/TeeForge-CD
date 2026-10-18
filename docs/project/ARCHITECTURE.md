# TeeForge-CD 当前架构

## 整体数据流

```text
Android boot
  → module/service.sh
  → selected ABI teeforge
  → /data/adb/teeforge/ 配置和日志
  → 一次用户应用扫描
      ├─ Tricky Store / TEESimulator-RS：target.txt
      ├─ TEESimulator：config.json profiles.teeforge.apps
      └─ OMK（安装时启用）：injector.toml scoop
```

WebUI 通过 `ksu.spawn()` 流式读取进度、`ksu.exec()` 读取一次性结果；所有命令都带 `--config /data/adb/teeforge/config.conf` 与 `--json`。

## WebUI 层

```text
index.html            主题在首帧前确定；挂载点
main.ts               i18n → createApp → router → 首屏数据
App.vue               应用栏 + router-view（无页面级过渡，避免低端 WebView 空白帧）
router.ts             hash 路由：/ /target /keybox /rkp /update
lib/
  ksu.ts              宿主探测、exec/spawn、应用名与图标
  api.ts              唯一的命令出口：拼参数、解析 {ok,data} / {ok,error}
  i18n.ts             字典加载与 t()；缺键回落键名
state/session.ts      首屏数据单例（--bootstrap 只调一次）
components/ui/        按钮、浮层、分段控件、徽章、图标、空态
components/layout/    应用栏、设备信息栏、分组卡、入口行、偏好浮层
pages/                五个页面
styles/tokens.css     色彩、圆角、间距、动效、安全区令牌（明/暗两套）
styles/tailwind.css   Tailwind 入口与全局基样式
```

技术栈为 Vite + Vue 3 + vue-router（hash）+ Tailwind 3.4，构建目标 `chrome87`，产物使用相对路径。放弃 Astro 的原因是它只承担静态壳，路由与跨页状态仍需自建。

数据与契约：

- 首屏只发一条 `--bootstrap --json`，一次拿到设备、Root、模块版本、配置与能力位；`getprop` 全量输出在 Rust 侧解析。
- 页面动作分别调用 `--target-list`、`--target-apply`、`--target-mode`、`--keybox --source`、`--rkp-status`、`--rkp-issue`、`--rkp-artifacts`、`--rkp-delete`、`--self-update`。
- `--json` 成功返回 `{"ok":true,"data":...}`，失败返回 `{"ok":false,"error":{stage,code,message,hint,raw}}`；日志固定走 stderr，不污染 JSON。
- 错误按阶段透传：`prerequisite` / `transport` / `frame` / `ta` / `server` / `keybox` / `target` / `update`。TA 状态码保留原始值，页面按 `rkp.code.<code>` 查解释，未收录的码只显示原值。
- 主题与语言持久化键仍为 `tf-theme` / `tf-lang`。
- 版本展示统一走 `formatVersion()`：`module.prop` 不带 `v`、CDN 清单带 `v`，调用点不再各自拼前缀。
- 视口约定：`body` 只用 `min-height: 100%`（不锁 `vh`/`dvh`），横向用 `overflow-x: clip`（`@supports` 兜底 `hidden`，避免破坏 sticky），viewport 禁止缩放。导航栏不使用相对容器的负外边距——那会让块级自动宽度增加 32px 并撑出横向滚动。

样式为本项目自行实现；视觉模型（色调分层、分组列表、底部浮层）参考 [Duck-ToolBox](https://github.com/eltavine/Duck-ToolBox)（MIT）与 Tricky-Addon 的列表交互，未复制其代码、资源或样式表。

## Rust 核心模块

| 模块 | 职责 |
|---|---|
| `cli` | 兼容参数解析、动作顺序和统一退出码 |
| `config` | 默认值、系统/用户配置优先级和兼容回退 |
| `target` | 调用 `cmd package list packages -f -U --user 0`，筛选用户应用/UID，并适配三种配置格式后原子事务写入 |
| `omk` | 用锁定的 toml_edit 定位 scoop，保留手动项、注释及扩展表头，更新受管区块 |
| `keybox` | 按 `keybox_source` 取远程池或本机 RKP 产物、严格解码和按实际后端多目标回滚 |
| `rkp` | 设备端 RKP：前置检查、CSR_V2 帧、TA 隔离子进程、服务交互、证书链校验、产物落盘与删除、错误码解释 |
| `update` | 更新清单解析、单一 CDN 端点、sha256/大小/ZIP 头校验、按 root 管理器调用安装器 |
| `info` | `--bootstrap` 单次采集（getprop 全量、内核、SELinux、模块版本、能力位） |
| `blhide` | 参数数组调用 resetprop，逐项收集失败 |
| `rootdetect` | 环境变量和 `/data/adb/*` 路径检测 |
| `volume` | 动态扫描 input event，封装设备输入 unsafe 适配 |
| `atomic_file` | 同目录临时文件、同步写入和原子替换；OMK 专用路径保留 UID/GID、mode 和 Android SELinux 标签 |
| `logging` | 分级日志、debug 文件和最近 15 份清理 |

所有设备端操作返回 `Result`；外部命令必须检查真实退出状态。target、配置、Keybox 和模块描述更新均不直接覆盖旧内容。

## 配置与路径

系统规范路径：

```text
/data/adb/teeforge/sys.conf
/data/adb/teeforge/config.conf
/data/adb/teeforge/logs/
/data/adb/teeforge/keybox/
/data/adb/teeforge/keybox/.rkp-issued
/data/adb/teeforge/keybox/tmp/
/data/adb/tricky_store/target.txt
/data/adb/tricky_store/keybox.xml
/data/adb/teesim/config.json
/data/adb/teesim/keybox.xml
```

加载顺序为：默认值 → `sys.conf` → 用户配置；用户配置覆盖同名系统键。缺少规范文件时，开发环境允许回退到当前目录的旧配置文件。

## Keybox 数据流

```text
keybox_source=remote
  自建 CDN → wget / curl / 管理器 busybox 后备 → 有界下载
  → 严格解码和 AndroidAttestation 标记校验 → 本地 keybox.xml

keybox_source=rkp
  /data/adb/teeforge/RKP/keybox.xml（--rkp-issue 产出）→ XML 结构检查

两条来源汇合：
  → 已存在的 Tricky Store/TEESimulator-RS 目录和/或 TEESimulator 目录中的 keybox.xml
```

两条来源互不回退：远程下载失败不会改用本机产物，本机产物缺失也不会联网。每个已存在的目标采用原子写入；任一目标失败时恢复本轮已写目标和本地旧文件。具体私密维护细节只存在于被 Git 忽略的 `docs/private/KEYBOX_CRYPTO.md`，不得复制到公开文档。

`target.txt` 只属于 Tricky Store/TEESimulator-RS 兼容接口；当前 TEESimulator 使用独立的 JSON profile。目标适配器只处理真实存在的配置，不创建目标目录。

## 设备端 RKP 签发

```text
--rkp-status（只读）
  → 前置检查：arm64 / 高通 SoC / libkeymasterdeviceutils.so / /dev/smcinvoke / fingerprint
  → 输出 blockers[] 与 evidence[]，不联网

--rkp-issue（独立功能，不碰后端）
  → /dev/urandom 生成 P-256 密钥
  → POST :fetchEekChain（curl / wget）→ challenge（≤64 字节）
  → CBOR {4: challenge, 57: bstr([COSE_Key])} + 小端命令头 0x3122
  → 子进程 dlopen 厂商库 → send_cmd（回执只回原始缓冲区，不解析帧）
  → decode_frame：按 5 种候选布局切片，用 CSR 结构校验判定命中
  → 本地校验 CSR 结构、challenge 与 keysToSign 绑定
  → POST :signCertificates?challenge=<base64url> → shared + unique DER
  → 校验唯一 leaf→root 路径、叶子公钥 == 本机私钥公钥
  → 写入 /data/adb/teeforge/RKP/{keybox.xml, cert-chain.pem, meta.json}（0600）

--keybox --source rkp
  → 读取 RKP/keybox.xml → 安装事务（与远程池共用）
```

设计边界：

- 仅实现 V2（`CSR_V2`）。V1 AIDL 需要 JDK/D8 构建链，原生 V1 只服务 PureSoft 零 HBK 固件，均未纳入。
- HTTP 使用设备已有 `curl` / `wget` / 管理器 busybox，不引入 reqwest 与异步运行时。
- 本地只做结构与绑定校验。DICE 链和 COSE 签名的密码学校验由 Google RKP 服务端完成，非法 CSR 不会签发证书。
- TA 调用在隔离子进程中执行，父进程超时后 kill 并回收；厂商库崩溃不会影响主进程。
- **帧只解析一次**：`ta` 层只调用、只返回原始缓冲区；`protocol::decode_frame` 是唯一的帧解析入口，按 `status+len` → `status+total` → `len` → `status` → `bare` 顺序尝试，并要求业务解析（`unwrap_csr`）成功才采纳。曾经在 `ta` 层与编排层各剥一次帧，导致合法的 CBOR payload 被当成帧头，报出误导性的“长度无效”。
- **帧布局由业务结构判定，不由长度字段判定**：长度字段可以自洽但语义不同，能解出合法 `AuthenticatedRequest` 才是真的；命中布局写入 `meta.json.frame_layout`，便于回溯固件差异。
- 错误按阶段结构化，且**跨 worker 进程保留** `stage` / `code` / `hint` / `raw`。帧类失败额外带 `head_hex`（前 64 字节）、`written`（最后一个非零字节位置）与 `status_le`，用于区分“TA 拒绝”“空成功”“封装不同”“ABI 不匹配”。
- TA 状态码走 `explain` 映射（AOSP `ErrorCode.aidl`）；未收录的码保留原始数值。
- `--rkp-probe` 是诊断入口：取一次 challenge 后，把缓冲区初始化（哨兵 / 全 0 / 容量）、`security_level`（1 / 0）、请求封装（裸 / 长度前缀）拆成 6 个变体，**每个变体一个子进程**，因此某个变体触发崩溃只丢失该组。输出 `status`、`written`、`request_changed`、`response_head`，用于区分「固件不认命令」「参数顺序相反」「缓冲区约定不同」三类原因。
- `rkp` 只在 `aarch64-linux-android` 编译出厂商调用路径，其余 ABI 的 `--rkp-status` 直接给出 `arch` 阻断项。

上游来源：Duck-ToolBox `fb6b1e427f2773ac6139d2eba27068fbdfd089de`，MIT，见 `module/licenses/duck-toolbox.txt`。

## 目标配置事务

`target` 模块先完成一次稳定的主用户 0 应用快照，再分别渲染所有已发现后端。`target.txt` 使用受管区块，区块外的注释、模式后缀和 Keybox 分段保持原顺序；TEESimulator 只更新带 `_teeforgeManaged: true` 标记的 `teeforge.apps`，首次创建时深复制 `default` 的非 `apps` 设置。所有渲染和源文件一致性检查通过后才写入，任一写入失败回滚本轮已写文件。

## OMK 作用域适配

OMK 启用状态只在安装时检测并写入 sys.conf 的 `omk_enabled`；默认关闭，用户配置可覆盖。安装器按模块 ID `oh_my_keymint` 检查待更新目录与已安装目录，待更新目录优先；disable/remove 标记使该目录不参与启用。运行时只根据固化状态和 `omk_injector_config` 工作，不重新发现模块。活动文件缺失时 warning 跳过，不创建目录；读取或解析失败时中止本轮目标事务。

适配基线为上游 `a1f3241f614948a0c17d9d127474878600d596f1`，兼容 Issue #1 附件对应 `67dc5e7`。接受缺失版本号、version=0、version=1，但不迁移版本；其他版本拒绝写入。只处理顶层字符串数组 scoop，保留其他字段。历史 [scoop.<package>] 扩展表头只在解析副本中等长映射，不写回原文件。

现有条目首次视为手动项；仅注释标记 BEGIN/END TeeForge-CD managed scoop 之间的自动项随主用户应用扫描更新。自动项排序去重、排除手动项，重复执行字节不变。非法语法、类型和标记拒绝写入，日志不包含 TOML 原文。

PendingUpdate 保存渲染使用的原始字节。OMK 替换前检查原始内容与元数据，并在限制权限的临时文件中准备原 UID/GID、mode 和 Android SELinux 标签；失败不提交。专用备份为 injector.toml.teeforge.bak，不占用 OMK 的备份名；回滚也保留元数据。回滚发现目标已被其他进程修改时拒绝覆盖并报告失败。跨文件事务无法保证多个热加载服务同时切换；写入完成不代表 OMK 已加载或应用路由已生效。

不接管 OMK config.toml、Keybox、filter、intercept 或服务重启。旧 C 不新增 OMK 支持。

## BL 隐藏

全架构的 Rust 与旧 C 均使用 standard resetprop，删除预置 rs 二进制和安装选择。Rust 使用参数数组逐条调用工具；失败项聚合到最终错误。旧 prop_tool 和 blhide_compact 键按未知键忽略，保留其他 BL 分类开关。

## 构建产物和 ABI

| Android ABI | Rust target | NDK linker target | ELF machine |
|---|---|---|---:|
| arm64-v8a | `aarch64-linux-android` | `aarch64-linux-android24` | 183 |
| armeabi-v7a | `armv7-linux-androideabi` | `armv7a-linux-androideabi24` | 40 |
| x86 | `i686-linux-android` | `i686-linux-android24` | 3 |
| x86_64 | `x86_64-linux-android` | `x86_64-linux-android24` | 62 |

ZIP 内置四个 ABI，`customize.sh` 根据 `ro.product.cpu.abi` 选择一个并校验 ELF 后删除其余版本。

## 安全边界

- `.sha256` 用于发现损坏或不完整，不等同于签名，也不能阻止重新制作 ZIP。
- 下载内容必须检查真实工具退出码、大小和格式，不能只检查非空。
- Keybox 内嵌私钥，本地副本与目标文件统一按 `0600` 写入（含备份与回滚路径）。
- 设备端 RKP 的临时请求/回执目录为 `0700`，文件为 `0600`，退出时清理；日志不输出私钥、challenge 原文或服务端响应体。
- 日志不得输出派生密钥、解码内容或私密维护信息。
- Android 设备不依赖 `openssl`；Rust 使用锁定的 `sha2`，安装校验使用 toybox `sha256sum`。
