# TeeForge-CD 架构设计

## 整体架构

```
┌─────────────────────────────────────────────┐
│              Android Boot                    │
├─────────────────────────────────────────────┤
│  post-fs-data.sh  →  早期钩子（暂空）        │
│                                             │
│  service.sh       →  调用 teeforge 二进制    │
│         │                                   │
│         ▼                                   │
│  ┌─────────────────────┐                    │
│  │   teeforge (C BIN)  │                    │
│  │                     │                    │
│  │  1. 解析 packages.xml│                   │
│  │  2. 过滤用户 app     │                   │
│  │  3. 写 target.txt   │                   │
│  │  4. (v0.2) 抓 keybox│                   │
│  │  5. (v0.3) 弱隐 BL  │                   │
│  └─────────────────────┘                    │
│         │                                   │
│         ▼                                   │
│  /data/adb/tricky_store/                    │
│    ├── target.txt    ← 生成                 │
│    └── keybox.xml    ← v0.2                 │
└─────────────────────────────────────────────┘
```

## 核心模块设计

### 1. target 生成器（MVP 核心）

**输入**：`/data/system/packages.xml`

**逻辑**：
```
读取 packages.xml
→ 逐行扫描 <package> 节点
→ 提取 name (包名) 和 codePath (安装路径)
→ 过滤条件：
    - codePath 以 /data/app/ 开头 → 用户 app ✓
    - codePath 以 /system/ /vendor/ /product/ 开头 → 跳过
    - name 以 android. / com.android. 开头 → 可选跳过
→ 写入 target.txt（每行一个包名）
```

**输出**：`/data/adb/tricky_store/target.txt`

**XML 解析策略**：
- 不引入 libxml2（体积大）
- 使用简单的状态机逐字符解析
- 只关注 `<package` 标签的 `name` 和 `codePath` 属性
- 参考 Android packages.xml 固定格式，不需完整 XML 解析器

### 2. Keybox 管理器（v0.2）

**数据流**：
```
sources.conf (源列表)
→ HTTP GET 请求
→ 响应校验
→ 写入 keybox.xml
```

**HTTP 实现选型**：
- 方案 A：`libcurl` 静态编译（功能全，体积 ~300KB）
- 方案 B：手写 HTTP/1.1 最小子集（体积最小，仅需 GET）
- 建议先用方案 B，复杂需求再切 A

### 3. 弱隐 BL（v0.3）

待调研后补充。初步方向：
- Hook 相关系统属性读取
- 修改 `/proc/cmdline` 等节点的返回值（如可行）
- 与 Tricky Store 的 PPL 机制配合

## 构建系统

### NDK 交叉编译配置

**目标**：
- 主架构：`aarch64-linux-android`（ARM64）
- 次架构：`armv7a-linux-androideabi`（ARMv7）

**最小 API Level**：Android 7.0 (API 24)

**编译标志**：
```makefile
CC      := $(NDK_TOOLCHAIN)/bin/aarch64-linux-android24-clang
CFLAGS  := -Wall -Wextra -O2 -static -fPIC
LDFLAGS := -static -llog
```

> `-static` 静态链接，二进制自包含，无运行时依赖

### build.sh 流程

```
1. 检测 ANDROID_NDK_HOME
2. 生成 standalone toolchain
3. 编译 native/src/*.c → out/teeforge
4. strip 减小体积
5. 复制到模块目录
6. 打包为 Magisk 模块 zip
```

## 安全设计

### 防拆包修改
- 核心逻辑编译为原生二进制，非脚本
- 可选：编译时嵌入校验 hash，运行时自检
- 可选：`strip` 去除符号表，增加逆向难度

### 运行权限
- 二进制以 root 执行（Magisk/KernelSU 上下文）
- 最小权限原则：只读 packages.xml，只写 target.txt

## 兼容性矩阵

| 环境 | 支持 |
|------|------|
| Magisk 24+ | ✅ |
| KernelSU 0.6+ | ✅ |
| Android 7.0 - 15 | ✅ |
| ARM64 设备 | ✅ |
| ARMv7 设备 | ⚠️ 需额外编译 |
| x86 模拟器 | ❌ 不考虑 |
