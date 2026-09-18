# del — 使用手册

[English](guide.md) | **简体中文** · [← 返回 README](../README.zh-CN.md)

README 是 30 秒速览，这份手册讲清楚完整细节：每个参数到底做什么、id 怎么解析、
磁盘上究竟存了什么、以及出问题时怎么办。

> 本手册描述的是本仓库当前代码的行为（v0.1.0）。最后一节给出源码映射，方便你
> 核对文档与代码有没有漂移。

## 1 安装与第一次运行

需要 **Rust 1.88 或更新版本**：代码使用了 edition 2024 的特性，包括 let-chains。

```bash
cargo install --path .   # 从当前仓库构建并安装 del

del -h                   # 每个选项一行摘要
del --help               # 完整说明、示例与 id 语法
```

第一次运行：

```bash
mkdir -p ~/scratch && echo hello > ~/scratch/note.txt

del ~/scratch/note.txt   # 打包进 ~/.trash，原文件被移除
del -l                   # id、文件名、原目录、大小、距今时间
del -R 1                 # 恢复到 ~/scratch/note.txt
```

`-l`、`-w`、`-x` 的表格通过分页器显示（`q` 退出、`/` 搜索）。结果为空时不会打开
分页器，直接打印 `Nothing.`。

## 2 一次删除到底做了什么

对于默认的 `del <path>...`（不带 `--force`）：

1. 每个路径先做 canonicalize，符号链接会解析到目标。不存在的路径被静默丢弃 ——
   没有提示，退出码也不是失败。
2. 包含（或等于）禁用列表项的路径同样被静默丢弃（见 [§6.3](#63-禁用列表)）。
3. 路径被打包：`tar` + `zstd` 写入回收站目录里的临时文件，随后改名为
   `<归档的 blake3>.bak`。
4. 往 SQLite 写入一条记录，并且 —— 除非给了 `--save` —— 移除原文件。目录默认
   递归打包，所以 `del some/dir` 会存下整棵树。

只有 **`--force` 且安全模式关闭** 时才会跳过第 3、4 步直接删除；此时删目录还必须
加 `--recursive`。

失败是按路径处理且安静的：某个路径打包失败（权限不足、I/O 错误）时，其余路径照常
处理，只有加 `-v` 才能看到
（`Pack failed for "…": Permission denied (os error 13)`），退出码仍是 0。打包失败
可能在回收站目录留下 `.tmp-*` 残留文件，`--autoclean` 会清理它。

## 3 命令参考

### 3.1 删除

- **`del <path>...`** — 把每个路径打包进回收站并移除原文件。不存在的路径与受保护
  路径都会被静默跳过。
- **`-S`、`--save`** — 只打包不删原文件，之后可以恢复，而原文件仍在原处。
  `--save` 同时压过 `--force`，两者同时给出时什么都不会被删除。
- **`-f`、`--force`** — 永久删除而不打包。Safe Mode 下被忽略，与 `--save` 同时给出
  时也被忽略。
- **`-r`、`--recursive`** — 递归删除目录。只对 `--force` 有意义；不带 `--force` 时
  目录本来就会递归打包。
- **`-t`、`--tui [PATH]`** — 在终端 UI 中浏览并操作文件，`PATH` 默认为当前目录。
  键位：方向键移动，`Enter` / `→` 进入目录，`←` 返回上级，`d` 把选中项删除进回收站，
  `s` 以 `--save` 方式打包，`q` / `Esc` 退出。UI 需要真实终端：stdin 被重定向时它
  不会自行退出（一直等待按键事件）；它派生的子进程也不会继承 `--trash-dir`、
  `--level`、`--disable`、`--cover`。

### 3.2 回收站记录

- **`-l`、`--list`** — 列出全部记录：id、文件名、原目录、大小、距今时间。
- **`-w`、`--show <ID>...`** — 查看指定记录的详情。
- **`-x`、`--select <PATTERN>...`** — 按原路径包含的子串筛选记录。匹配是
  `original_path LIKE '%pattern%'`，即子串搜索（ASCII 大小写不敏感），且 `%`、`_`
  会被当作通配符。
- **`-R`、`--restore <ID>...`** — 恢复记录，解包回原目录或 `--output` 指定的目录。
  只有完全恢复成功后才会删除记录。
- **`-d`、`--delete <ID>...`** — 删除记录，归档文件保留。之后用 `--autoclean`
  真正释放空间。

### 3.3 维护

- **`-a`、`--autoclean`** — 分两遍清理：第一遍删除回收站目录中没有任何记录指向的
  文件（数据库及其附属文件从不被触碰）；第二遍删除超过 `--save-time` 的记录。因为
  年龄判断发生在扫描之后，过期记录
  的归档要等**下一次** `--autoclean` 才会被收走。完整时序与副作用见
  [§8.4](#84-记录的生命周期)。
- **`-c`、`--clear`** — 二次确认后清空全部记录。只有 `y` 或 `Y` 会继续，其他输入
  （包括直接回车）都取消。归档文件保留，之后用 `--autoclean` 释放空间。注意它丢弃
  的是整张表：在同一次运行里组合 `-c` 与 `-a` 会报
  `Error: no such table: trash`。
- **`--level <N>`** — 本次运行的 zstd 压缩级别，1（最快）到 22（最小）。默认取配置
  文件里的 `compression_level`，即 3。
- **`--save-time <DAYS>`** — 只被 `--autoclean` 使用的保留天数。默认取配置文件里的
  `save_time`，即 30。
- **`--trash-dir <DIR>`** — 指定回收站目录，归档与 `database.db` 都放在这里，不存在
  时自动创建。默认取配置文件里的 `trash_dir`，即 `~/.trash`。
- **`--disable <PATH>,...`** — 本次运行额外保护的路径。它是在生效列表上**追加**，
  而不是替换。

### 3.4 输出与行为

- **`-C`、`--cover <MODE>`** — 恢复目标已存在时的处理：`always` 覆盖、`never` 跳过、
  `ask` 逐个询问。被跳过的文件会保留记录，等你把冲突文件移开后再重试。默认取配置
  文件里的 `cover_mode`，即 `ask`。
- **`-o`、`--output <PATH>,...`** — 供 `-R` 使用的恢复目录。按出现顺序与 id 一一配对：
  `del -R 1,2 -o a,b` 把记录 1 恢复到 `a`、记录 2 恢复到 `b`。没有对应值的 id 回落到
  原目录。
- **`-s`、`--safe`** — 强制开启安全模式，没有关闭它的参数。
- **`-v`、`--verbose`** — 把调试日志打到 stderr，每行带来源 `文件:行号` 前缀。
- **`-h` / `--help`** — 简短摘要 / 完整说明。
- **`-V`、`--version`** — 打印版本。

## 4 id 语法

`--show`、`--restore`、`--delete` 使用同一套语法：

- `3` — 单个 id
- `2,5,9` — 逗号分隔多个
- `2-5` 或 `2~5` — 闭区间
- `-5` — 等价于 `0-5`
- 同一参数可以重复给：`del -R 1,2 -R 7`

数字、`,`、`-`、`~` 以外的字符会被忽略，所以从别处粘来的列表也能解析（`1 , 2` 得到
id 1 和 2，`5_000_090` 读作 `5000090`）。

以 `-` 开头的值必须用 `=` 形式传入 —— 写 `del --show=-5`，不要写 `del --show -5` ——
否则 clap 会把 `-5` 当成参数标志。

## 5 配置

### 5.1 配置文件

`~/.config/del/config.toml`，全部键可选：

```toml
trash_dir = "/data/.trash"       # 归档与 database.db 的位置（默认 ~/.trash）
safe_mode = true                 # 默认 true
compression_level = 6            # zstd 级别，默认 3
save_time = 30                   # --autoclean 保留天数，默认 30
cover_mode = "ask"               # always / ask / never，默认 ask
disable_list = ["/boot", "/etc"] # 会整体替换内置保护列表
```

配置文件里的 `disable_list` **整体替换**内置列表。内置列表保护回收站本身、home 目录，
以及 `/`、`/boot`、`/etc`、`/usr`、`/var`、`/bin`、`/sbin`、`/lib`、`/lib64`、`/opt`、
`/root`、`/home`、`/proc`、`/sys`、`/dev`、`/tmp`。

### 5.2 环境变量

上面的每个键都可以用带 `DEL_` 前缀的环境变量设置，键名转小写。列表值使用 TOML 数组
语法。

```bash
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_SAFE_MODE=false del -f ~/junk.log          # 真正删除，不进回收站
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

### 5.3 优先级

**CLI 参数 > `DEL_*` 环境变量 > 配置文件 > 内置默认值。**

CLI 与环境变量按「键」覆盖。有一处不对称：`-s` 只能把 `safe_mode` 设成 `true`，
`--disable` 只能追加 —— 关闭安全模式、整体替换保护列表，都必须走配置文件或环境变量。

## 6 安全模型

### 6.1 回收站

任何东西都是先存后删：删除动作先打包、写记录，然后才 unlink 原文件。所以中途失败的
`del` 只会给你留下原文件或者归档，不会两头都空。

### 6.2 安全模式

`safe_mode` 默认为 `true`。开启期间 `--force` 被忽略，改为打包。命令行只能把它打开
（`-s`）；想在某一次运行中绕过回收站，就覆盖配置：

```bash
DEL_SAFE_MODE=false del -f ~/junk.log
```

### 6.3 禁用列表

匹配规则是对 canonicalize 之后的路径做前缀判断：当存在受保护路径 `d` 满足
`d.strip_prefix(p)` 时跳过目标 `p`，也就是说 **`p` 等于 `d`，或 `p` 是 `d` 的父目录**。
所以 `del /etc` 和 `del /` 会被跳过，而 `del /etc/hosts` 不会 —— 保护方向是**向上**的，
它不是「这棵子树只读」的规则。用 `--disable` 或 `disable_list` 添加条目前请记住这一点。

### 6.4 延迟物理删除

删除本身不释放磁盘空间。空间只由 `--autoclean` 释放，而 `--delete` / `--clear` 只动
记录，不碰归档文件。

## 7 常用做法

### 恢复一个文件

```bash
del -l                 # 找到 id
del -w 4               # 确认是这一条（文件名、原目录、距今时间）
del -R 4               # 恢复
```

### 恢复到其他目录

```bash
del -R 4 -o ~/Downloads          # 这条记录恢复到 ~/Downloads
del -R 3,4 -o /tmp/a,/tmp/b      # 3 → /tmp/a，4 → /tmp/b
```

### 恢复时目标已存在

```bash
del -C never -R 4                # 跳过已存在的文件
del -C always -R 4               # 覆盖它们
del -C ask -R 4                  # 逐个询问
```

被跳过的条目会保留记录，把冲突文件移开后可以再次恢复。

### 只留副本、不删原文件

```bash
del -S ~/draft.md                # 存进回收站，原文件留在原处
```

### 释放磁盘空间

```bash
del -d 1,2,3                     # 删掉不再需要的记录（文件保留）
del -a                           # 清理无引用归档 + 过期记录
del -a                           # 再跑一次：过期记录的归档这时才被收走
```

### 使用独立的回收站

```bash
DEL_TRASH_DIR=/mnt/backup/.trash del ~/big.iso
DEL_TRASH_DIR=/mnt/backup/.trash del -l
```

### 搜索回收站

```bash
del -x report                    # 原路径包含 "report" 的全部记录
del -x 2025-06,2025-07           # 一次给多个模式
```

## 8 存储布局与记录生命周期

### 8.1 回收站目录

```
~/.trash/
├── database.db                  # SQLite（WAL 模式）
├── database.db-wal、-shm        # SQLite 附属文件，数据库打开期间存在
├── <blake3>.bak                 # 每个被打包的路径一份归档，tar + zstd
└── .tmp-<ms>-<seq>              # 打包过程中的临时文件；失败时会残留
```

### 8.2 去重到底是怎么工作的

归档的名字取自 **归档字节** 的 blake3 哈希 —— 即 tar 流（条目名、权限、mtime）加上
zstd 帧，而**不是**原文件内容的哈希。之后 `del` 把临时文件改名到位，所以两次打包若
产生逐字节相同的归档，就会共用同一个文件，后写入的覆盖先前的。

实际效果是：

- 同一个路径、同样内容、同样条目名与时间戳（tar 只存秒级）重复删除，只会占用一个
  `.bak`。
- 内容完全相同但文件名不同的两个文件，会得到**两份**归档。
- 同一个文件分别用 `--level 3` 和 `--level 9` 打包，会得到**两份**归档。

所以不要把它理解成内容去重：它只能避免「重复删除同一个东西」堆积副本，仅此而已。

### 8.3 数据库结构

```sql
CREATE TABLE trash (
    id            INTEGER PRIMARY KEY,
    original_path TEXT NOT NULL,   -- 删除前所在的位置
    present_path  TEXT NOT NULL,   -- 承载它的 .bak
    size          TEXT NOT NULL,   -- 人类可读，例如 12.34KiB
    time          INTEGER NOT NULL -- 删除时间，毫秒时间戳
);
```

### 8.4 记录的生命周期

```
删除   打包 → <hash>.bak → INSERT 记录 → 移除原文件（--save 时保留）
恢复   解包 → （只有没有任何条目被跳过时才删除记录）
删记录 -d 只删记录，.bak 保留
清空   -c 删除全部记录（连表一起丢弃），.bak 保留
清理   -a
   第一遍  删除回收站目录中没有任何记录指向的文件
   第二遍  DELETE FROM trash WHERE time < now - save_time 天
           （它们的归档此时变成孤儿，由下一次 -a 收走）
```

两个值得知道的后果：`-a` 会删除回收站目录里**所有**无引用文件，唯独数据库及其附属
文件除外（`database.db`、`database.db-wal`、`database.db-shm`、
`database.db-journal`），所以 `.tmp-*` 残留会被收走 —— 你放在该目录里的其他东西也
一样；另外 `-a` 遇到回收站目录里的子目录会以 `Error: Is a directory (os error 21)`
中止（名字以数据库文件名开头的目录，例如 `database.db-backup`，会被跳过）。

## 9 退出码

- `0` — 运行完成。被跳过的路径（不存在、受保护、打包失败）不算错误。
- `1` — 运行期错误导致中止，例如 `--force` 删目录却漏了 `--recursive` 时的
  `Error: Is a directory (os error 21)`，或 `-c` 与 `-a` 组合时的
  `Error: no such table: trash`。
- `2` — 命令行错误（未知参数、非法取值）。

## 10 已知限制

- **TUI 需要终端。** stdin 被重定向时 `del -t` 会一直等待按键事件，不会自行退出。
  它的 `d` / `s` 会新起一个 `del` 子进程，而该子进程不继承 `--trash-dir`、`--level`、
  `--disable`、`--cover`；即使子进程失败，列表也会乐观地把条目移除。
- **`-c` 与 `-a` 不能放在同一次运行里**（清空会丢弃清理随后要读的表）。
- **失败是安静的。** 打包失败的路径不留痕迹，除非加 `-v`。
- **保护方向向上**（见 [§6.3](#63-禁用列表)）：受保护目录的子路径不受保护。
- **`-x` 的模式是 LIKE 模式**，`%` 和 `_` 是通配符而不是普通字符。

## 11 源码映射

| 区域 | 文件 |
|------|------|
| CLI 定义、帮助文本、id 解析 | `src/cli.rs` |
| 配置加载、优先级、禁用列表匹配、todo 组装 | `src/config.rs` |
| 命令分发：删除、恢复、删记录、清空、autoclean | `src/main.rs` |
| 打包、解包、哈希、覆盖模式 | `src/compress.rs` |
| SQLite 结构与查询 | `src/sqlite.rs` |
| 终端 UI | `src/tui.rs` |

## 许可证

MIT —— 见 [LICENSE](../LICENSE)。
