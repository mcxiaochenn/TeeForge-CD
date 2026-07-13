# TeeForge-CD 任务清单

> 最后更新：2026-07-14

---

## Phase 1：MVP — 自动填充 target.txt

### 1.1 项目基础设施
- [ ] 初始化 Git 仓库
- [ ] 创建 `.gitignore`（忽略 `out/`、`*.o`、`*.so`）
- [ ] 编写 `build.sh` 一键构建脚本
- [ ] 配置 Android NDK 交叉编译（aarch64-linux-android）

### 1.2 核心二进制（C）
- [ ] `native/src/main.c` — 程序入口，参数解析
- [ ] `native/src/target.c` — 解析 `/data/system/packages.xml`
  - [ ] XML 解析（轻量状态机，不依赖 libxml）
  - [ ] 过滤用户安装 app（codePath 以 `/data/app/` 开头）
  - [ ] 排除系统 app（`/system/`, `/vendor/`, `/product/`）
  - [ ] 输出到 `/data/adb/tricky_store/target.txt`
- [ ] `native/src/utils.c` — 文件读写、字符串处理、日志
- [ ] `native/include/teeforge.h` — 公共头文件

### 1.3 模块框架
- [ ] `module.prop` — 模块元信息
- [ ] `META-INF/com/google/android/update-binary`
- [ ] `META-INF/com/google/android/updater-script`
- [ ] `customize.sh` — 安装脚本
- [ ] `service.sh` — 开机执行

### 1.4 验证
- [ ] 本地 NDK 编译通过
- [ ] 推送到设备测试
- [ ] 确认 target.txt 内容正确
- [ ] Tricky Store 识别生效

---

## Phase 2：Keybox 管理

- [ ] `native/src/keybox.c` — HTTP GET 请求实现
- [ ] 支持配置多个 keybox 源 URL
- [ ] 解析响应，提取 keybox 内容
- [ ] 写入对应路径
- [ ] `config/sources.conf` — 源列表格式定义
- [ ] 失败回退 / 多源轮询

---

## Phase 3：弱隐 BL

- [ ] 调研主流 BL 检测方案
- [ ] 实现弱隐藏逻辑
- [ ] 与 Tricky Store 机制配合

---

## Phase 4：扩展

- [ ] 日志系统（分级、轮转）
- [ ] 自动更新模块
- [ ] 用户配置文件（排除列表等）
- [ ] WebUI / CLI（可选）

---

## 环境依赖

| 依赖 | 版本 | 状态 |
|------|------|------|
| Android NDK | r26+ | ❌ 待安装 |
| Git | 任意 | ❓ 待确认 |
| adb | 任意 | ❓ 待确认 |

---

## 决策记录

| 日期 | 决策 | 原因 |
|------|------|------|
| 2026-07-14 | 核心语言选择 C | 二进制体积小、NDK 原生支持、底层操作直接 |
| 2026-07-14 | 模块名 TeeForge-CD | TEE + Forge 双关，CD 为作者标识 |
| 2026-07-14 | MVP 聚焦 target.txt | 最小可用，快速验证模块框架 |
