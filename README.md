# del — 安全删除工具 / Safe & Secure Deletion

> 一个"后悔药"式的删除工具：删除的文件不会消失，而是压缩进回收站，随时可以恢复。
> A delete tool with a safety net: removed files are compressed into a trash and can be restored anytime.

## ✨ 亮点 / Highlights

**四重安全（Four layers of safety）**，让 `rm` 的误删恐惧成为过去式：

| 层 | 中文 | English |
|----|------|---------|
| ① | **回收站**：所有删除先进 `~/.trash`，随时 `-l` 查看、`-R` 恢复 | **Trash**: deletions go to `~/.trash` first; list with `-l`, restore with `-R` |
| ② | **Safe Mode（默认开启）**：`--force` 被忽略，无法绕过回收站直接删除 | **Safe Mode (on by default)**: `--force` is ignored, so files can't bypass the trash |
| ③ | **Disable List**：受保护路径（默认含 home 与 `/boot`）永不删除 | **Disable List**: protected paths (home and `/boot` by default) are never deleted |
| ④ | **延迟物理删除**：删除后数据仍安全存档，直到显式运行 `--autoclean` 才真正释放磁盘空间 | **Deferred deletion**: data stays archived until you explicitly run `--autoclean` to free disk space |

**按内容去重（Content-addressed storage）**：文件按内容哈希命名，内容相同的文件**只存一份**，后存的自动覆盖先存的，回收站空间占用极小。

---

## 📖 中文文档

### 为什么需要

`rm` 删除即消失，误删、覆盖、脚本翻车都不可逆。`del` 把"删除"变成两步：

1. **软删除**：文件压缩后移入回收站（立即可恢复）；
2. **硬释放**：只有你运行 `--autoclean`，过期/无记录的文件才真正从磁盘消失。

### 快速开始

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

### 命令一览

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

### 配置

配置文件 `~/.config/del/config.toml`（可选）：

```toml
trash_dir = "/data/.trash"      # 回收站位置
safe_mode = true                # 默认开启
compression_level = 6           # zstd 压缩级别（默认 3）
save_time = 30                  # autoclean 保留天数
cover_mode = "ask"              # always / ask / never
disable_list = ["/boot", "/etc"] # 覆盖默认保护列表
```

所有配置均可写入 `~/.config/del/config.toml`，也可用环境变量覆盖（前缀 `DEL_`，键名自动转小写）。列表类配置需用 TOML 数组语法。优先级：**CLI 参数 > 环境变量 > 配置文件 > 默认值**。

```bash
# 环境变量示例
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

### 工作原理

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

### 四重安全详解

1. **回收站（Trash）**：所有删除先进 `~/.trash`，`-l` 可查、`-R` 可恢复，删除不再是单向操作。
2. **Safe Mode（Safe Mode）**：默认开启，`--force` 直接失效。想真正删除？先显式关闭保护。
3. **Disable List（Disable List）**：默认保护 `~/.trash`、home 目录与 `/boot`；任何"包含或等于受保护路径"的删除请求都会被跳过并保留原样。
4. **延迟物理删除（Deferred deletion）**：删除动作只是把数据搬进回收站，磁盘空间直到 `--autoclean` 才释放。`--delete` / `--clear` 只动记录，不动归档文件。

---

## English Docs

### Why

`rm` is irreversible: one typo and your data is gone. `del` turns deletion into two deliberate steps:

1. **Soft delete** — files are compressed into the trash (instantly restorable);
2. **Hard release** — disk space is only freed when you explicitly run `--autoclean`.

### Quick Start

```bash
# Install
cargo install --path .

# Delete a file (moves it into the trash; the original disappears)
del ~/tmp/old-report.txt

# List the trash
del -l

# Restore entry 1 to its original location
del -R 1

# Restore to a different directory
del -R 1 -o ~/Downloads

# Clean the trash: drop expired records and files with no record (frees space)
del -a
```

### Commands

| Command | Description |
|---------|-------------|
| `del <path>...` | Delete files/directories into the trash |
| `del -f <path>` | Force a permanent delete (no-op in safe mode) |
| `del -r` | Recursively remove directories with `-f` |
| `del -l` | List every trash entry |
| `del -w <id>` | Show details by id (comma-separated) |
| `del -R <id>` | Restore by id (comma-separated) |
| `del -d <id>` | Delete records by id (files stay until autoclean) |
| `del -a` | Autoclean: expired records + files with no record |
| `del -S <path>` | Pack into the trash without removing the originals |
| `del -C <mode>` | Behavior on restore conflict: `always` / `ask` / `never` |
| `del -o <path>` | Custom restore directory, one per `-R` id |
| `del --disable <path>` | Append protected paths (comma-separated) |
| `del --trash-dir <dir>` | Trash directory (default `~/.trash`) |
| `del --save-time <days>` | Retention days for autoclean (default 30) |
| `del -v` | Verbose debug logs |

### Configuration

Optional config file `~/.config/del/config.toml`:

```toml
trash_dir = "/data/.trash"      # trash location
safe_mode = true                # on by default
compression_level = 6           # zstd level (default 3)
save_time = 30                  # days autoclean keeps entries
cover_mode = "ask"              # always / ask / never
disable_list = ["/boot", "/etc"] # replaces the default protected list
```

Every option can also live in `~/.config/del/config.toml`, or be overridden via environment variables with the `DEL_` prefix (keys are lowercased automatically; list values use TOML array syntax). Precedence: **CLI flags > environment variables > config file > defaults**.

```bash
# Environment variable examples
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

### How It Works

```
Delete del <path>
  ├─ 1. Compress with zstd + tar
  ├─ 2. Name by content hash → ~/.trash/<hash>.bak
  │        (same content → same hash → overwritten, stored once)
  ├─ 3. Record into SQLite (id / original path / archive path / size / time)
  └─ 4. Remove the original (-S keeps it)

Restore del -R <id>
  ├─ Unpack to the original location (or -o target)
  └─ Delete the record only when fully restored; skipped entries are kept for retry

Clean del -a
  ├─ Drop records older than the retention period
  └─ Delete trash files with no database record (this actually frees space)
```

### The Four Layers of Safety

1. **Trash** — every deletion lands in `~/.trash` first; list with `-l`, restore with `-R`. Deletion is never one-way.
2. **Safe Mode (on by default)** — `--force` is ignored, so files cannot bypass the trash unless you explicitly opt out.
3. **Disable List** — `~/.trash`, your home directory and `/boot` are protected by default; any delete request that contains (or equals) a protected path is skipped and left untouched.
4. **Deferred deletion** — "deleting" only moves data into the trash; disk space is not freed until you run `--autoclean`. `--delete` / `--clear` touch records only, never the archived files.

### Content-Addressed Storage

Archives are named by the **blake3 hash of their content**. Deleting the same file twice — or two identical files — produces the same hash, so the second copy simply overwrites the first. Each unique file is stored exactly once, keeping the trash tiny.
