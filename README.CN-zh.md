# del — 安全删除工具

> 一个"后悔药"式的删除工具：删除的文件不会消失，而是压缩进回收站，随时可以恢复。

## ✨ 亮点

**四重安全**，让 `rm` 的误删恐惧成为过去式：

| 层 | 说明 |
|----|------|
| ① | **回收站**：所有删除先进 `~/.trash`，随时 `-l` 查看、`-R` 恢复 |
| ② | **Safe Mode（默认开启）**：`--force` 被忽略，无法绕过回收站直接删除 |
| ③ | **Disable List**：系统关键路径（home、`/`、`/etc`、`/usr`、`/boot` 等）永不删除 |
| ④ | **延迟物理删除**：删除后数据仍安全存档，直到显式运行 `--autoclean` 才真正释放磁盘空间 |

**按内容去重**：文件按内容哈希命名，内容相同的文件**只存一份**，后存的自动覆盖先存的，回收站空间占用极小。

---

## 为什么需要

`rm` 删除即消失，误删、覆盖、脚本翻车都不可逆。`del` 把"删除"变成两步：

1. **软删除**：文件压缩后移入回收站（立即可恢复）；
2. **硬释放**：只有你运行 `--autoclean`，过期/无记录的文件才真正从磁盘消失。

## 快速开始

```bash
# 安装
cargo install --path .

# 删除一个文件（进回收站，原文件消失）
del ~/tmp/old-report.txt

# 查看回收站
del -l

# 恢复指定记录（回到原位置）
del -R 1

# 恢复并输出到其他目录
del -R 1 -o ~/Downloads

# 清理回收站：删除过期记录 + 无记录文件（真正释放空间）
del -a
```

## 命令一览

| 命令 | 说明 |
|------|------|
| `del <path>...` | 删除文件/目录（进回收站） |
| `del -f <path>` | 强制永久删除（Safe Mode 下无效） |
| `del -r` | 配合 `-f` 递归删除目录 |
| `del -l` | 列出回收站全部记录 |
| `del -w <id>` | 按 id 查看记录详情（逗号分隔多值） |
| `del -R <id>` | 按 id 恢复（逗号分隔多值） |
| `del -d <id>` | 删除记录（归档文件保留到 autoclean） |
| `del -a` | 自动清理：过期记录 + 无记录文件 |
| `del -S <path>` | 只打包进回收站，不删除原文件 |
| `del -C <mode>` | 恢复遇同名文件的处理：`always` / `ask` / `never` |
| `del -o <path>` | 恢复输出目录，与 `-R` 一一对应 |
| `del --disable <path>` | 追加受保护路径（逗号分隔多值） |
| `del --trash-dir <dir>` | 指定回收站目录（默认 `~/.trash`） |
| `del --save-time <days>` | 设置保留天数（默认 30，供 autoclean 用） |
| `del -v` | 打印详细调试日志 |

## 配置

配置文件 `~/.config/del/config.toml`（可选）：

```toml
trash_dir = "/data/.trash"       # 回收站位置
safe_mode = true                 # 默认开启
compression_level = 6            # zstd 压缩级别（默认 3）
save_time = 30                   # autoclean 保留天数
cover_mode = "ask"               # always / ask / never
disable_list = ["/boot", "/etc"] # 覆盖默认保护列表
```

所有配置均可写入 `~/.config/del/config.toml`，也可用环境变量覆盖（前缀 `DEL_`，键名自动转小写）。列表类配置需用 TOML 数组语法。优先级：**CLI 参数 > 环境变量 > 配置文件 > 默认值**。

```bash
# 环境变量示例
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

## 工作原理

```
删除 del <path>
  ├─ 1. zstd + tar 压缩打包
  ├─ 2. 按内容哈希命名 → ~/.trash/<hash>.bak
  │        （相同内容 → 相同哈希 → 自动覆盖，只存一份）
  ├─ 3. 记录进 SQLite（id / 原路径 / 归档路径 / 大小 / 时间）
  └─ 4. 移除原文件（-S 保留原文件）

恢复 del -R <id>
  ├─ 按记录解包回原位置（或 -o 指定目录）
  └─ 全部恢复成功后删除记录；被覆盖策略跳过的记录保留，可重试

清理 del -a
  ├─ 删除超过保留天数的记录
  └─ 删除回收站中无数据库记录的文件（真正释放空间）
```

## 四重安全详解

1. **回收站（Trash）**：所有删除先进 `~/.trash`，`-l` 可查、`-R` 可恢复，删除不再是单向操作。
2. **Safe Mode（Safe Mode）**：默认开启，`--force` 直接失效。想真正删除？先显式关闭保护。
3. **Disable List（Disable List）**：默认保护 `~/.trash`、home 目录与系统关键目录（`/`、`/boot`、`/etc`、`/usr`、`/var`、`/bin`、`/sbin`、`/lib`、`/lib64`、`/opt`、`/root`、`/home`、`/proc`、`/sys`、`/dev`、`/tmp`）；任何"包含或等于受保护路径"的删除请求都会被跳过并保留原样。
4. **延迟物理删除（Deferred deletion）**：删除动作只是把数据搬进回收站，磁盘空间直到 `--autoclean` 才释放。`--delete` / `--clear` 只动记录，不动归档文件。

## 按内容去重

归档文件按内容的 **blake3 哈希** 命名。删除同一个文件两次、或删除两个内容相同的文件，都会得到相同的哈希，后存的自动覆盖先存的。因此**每份唯一内容只存一次**，回收站空间占用极小。
