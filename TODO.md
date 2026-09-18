# TODO — del 代码问题清单

> 生成日期：2026-09-18
> 来源：README / README.zh-CN / docs/guide{,.zh-CN} / LICENSE 与代码行为的一致性核查
> 环境：rustc 1.98.1，`cargo build`（debug），结论均基于实机运行
> 标记：**[实测]** 有可复现的运行证据 · **[读代码]** 仅从源码判断，未实机复现
> 优先级：**P0** 用户可见的破损 · **P1** 行为与文档不符或有实际影响 · **P2** 健壮性/可维护性

---

## 一、已修复

### ✅ A-1 `del -c` 单独运行确认后报错并退出 1

- **现象**：默认配置（safe mode 开启）下 `del -c` 确认 `y` 后，打印 `Error: no such table: trash` 并以退出码 1 结束。
- **根因**：`clear()` 用 `DROP TABLE` 删掉整张表（`src/sqlite.rs:224`），之后 `src/main.rs:178` 无条件调用 `db.insert_many(&lines)`；而 `insert_many` 在循环**之前**就 `prepare("INSERT INTO trash …")`，空数组也照样 prepare，于是撞上已删除的表。
- **修复**：`src/sqlite.rs:103-105` 在 `insert_many` 开头对空数组直接返回；新增回归测试 `empty_insert_many_is_a_noop_after_clear`（`src/sqlite.rs:287`）。
- **验证**：`cargo test` 3 passed；把 guard 临时改成 `if false && …` 后该测试立即 `FAILED`（报错正是 `no such table: trash`），证明测试确实锁住了缺陷；端到端 `del -c` → exit 0，记录清空、归档保留。
- **注意**：本次修复只覆盖"单独 `-c`"。同一根因的其余表现见 P0-1。

---

## 二、待修复 — 代码缺陷

### P0-1 `clear()` 用 DROP TABLE，使 `-c` 与任何其它数据库操作组合都失败 **[实测]**

- **现象**：`-c` 确认后整张表被删除，同一次运行里后续任何数据库访问都会撞上 `no such table: trash`。
- **实测退出码矩阵**（每次用独立的干净回收站，`echo y |` 确认）：

  - `del -c` → **0**（A-1 已修）
  - `del -c -a` → **1**
  - `del -c -d 1` → **1**
  - `del -c -w 1` → **1**
  - `del -c -R 1` → **1**

- **影响**：五种组合里只有 `-c -a` 被写进了文档（guide §3.3 / §9 / §10），另外三种（`-d`、`-w`、`-R`）完全没有记载。文档把 `-c -a` 的失败描述成"设计使然"，但对使用者来说更像是实现副作用。
- **修复方向**：让 `clear()` 执行 `DELETE FROM trash` 而不是 `DROP TABLE`。表结构得以保留，`-c` 就能与 `-a`/`-d`/`-w`/`-R` 自由组合；`Database::new` 里的 `CREATE TABLE IF NOT EXISTS` 仍然可以兜底。
- **注意**：这会**推翻**文档里三处明确的"已知限制"，必须同步改文档（见 D-1）。属于行为契约变更，动手前先确认。
- **验证方式**：改写后重跑上面的退出码矩阵，应全部为 0；并确认 `-c` 之后 `del -l` 仍显示 `Nothing.`

### P1-2 `--level` 没有任何取值校验，帮助文本与手册互相矛盾 **[实测]**

- **现象**：
  - `del --help`（`src/cli.rs:80`）："1 is fastest, **19** is a practical maximum"
  - `docs/guide.md:109` 与 `docs/guide.zh-CN.md:96`：**1 到 22**
  - 代码 `src/cli.rs:83` 声明为 `Option<i32>`，直接透传给 zstd，**不做任何范围校验**
- **实测**：`--level 0`、`--level 22`、`--level 30` 全部 exit 0 且打包成功（`-v` 可见 `Stored as …bak`）。
- **影响**：两处文档给出不同的上界；用户以为超范围会报错，实际不会。
- **修复方向**：给 clap 加 `value_parser = clap::value_parser!(i32).range(1..=22)`（或自定义校验函数），并把帮助文本与手册的说法统一。
- **风险**：会拒绝目前能跑的 `--level 0` 与 `--level 30`，属行为收紧，需确认没有依赖。
- **验证方式**：`--level 0/30` 应报错并 exit 2；`--level 1/19/22` 正常。

### P1-3 恢复时解包失败被静默吞掉 **[实测]**

- **位置**：`src/main.rs:206` —— `compress::unpack(src, output_dir, todo.cover).ok()?`
- **现象**：归档文件丢失或损坏时，该记录既不恢复也不报错，退出码仍是 0。
- **实测**：删掉回收站里的 `.bak` 后执行 `del -R 1 -v` → exit 0，全程零错误提示，`-v` 下也只有 `Deleting 0 fully-restored record(s) from database`；记录仍留在数据库里（下次还会再失败一次）。
- **影响**：用户会以为恢复成功，实际数据没回来。文档只描述了"被 cover 模式跳过会保留记录"，没有提归档缺失这种情况。
- **修复方向**：把"跳过"（cover 拒绝覆盖，属正常）与"失败"（归档读不到，属异常）区分开；失败至少用 `verbose_println!` 留痕，更彻底的做法是计入退出码或在非 verbose 下也警告。
- **验证方式**：删掉 `.bak` 后 `del -R <id>`，应能看到明确提示，而不是静默 exit 0。

### P1-4 `-t` 与其它参数组合时，TUI 退出后继续执行后续动作 **[读代码]**

- **位置**：`src/config.rs:100` —— `main_loop(path).unwrap();`
- **现象**：`init()` 调用 TUI 后丢弃返回值继续往下执行，所以 `del -t <dir> -l` 这类组合会在 TUI 退出之后**再跑一遍** `-l` / `-R` / `-a` 等后续逻辑。
- **附带问题**：`main_loop` 返回 `Err` 时 `.unwrap()` 会 panic（退出码 101），游离在文档承诺的 0/1/2 退出码体系之外。
- **未实测原因**：该行为需要真实终端交互，我用 `script -qec` 伪造 pty 的尝试在自动化环境里无法稳定复现，故仅按源码判断。
- **修复方向**：`-t` 走独立分支，进入 TUI 前 `return`（或显式退出），不与其它 todo 逻辑混跑。

---

## 三、健壮性与代码异味（读代码发现）

### P2-1 `unpack()` 里的 `unreachable!()` 是死分支

`src/compress.rs:146` 的外层 `match cover` 已经排除了 `Always`，`:156` 内层却又列了一次 `CoverMode::Always` 并在 `:157` 写 `unreachable!()`。当前是安全的，但逻辑重复，将来重构时容易踩成真正的 panic。建议内层直接用 `Never | Ask` 或调整结构。

### P2-2 `delete_by_id` 吞掉 SQL 错误

`src/sqlite.rs:189` —— `stmt.execute(params![id]).unwrap_or(0)`。执行失败被静默转成 "affected: 0"，只在 `-v` 日志里体现为删了 0 行。建议用 `?` 传播错误。

### P2-3 `select_by_path` 吞掉行读取错误

`src/sqlite.rs:179` —— `all.filter_map(|x| x.ok())`。查询出错的行被无声丢弃，结果不完整也不报错。建议收集错误，或改用 `collect::<Result<Vec<_>, _>>()`。

### P2-4 `config.rs` 里的死代码

`src/config.rs:128-129`：`let p = &p.canonicalize().ok()?;` 之后紧跟 `if !p.exists()`。canonicalize 成功即意味着路径存在，这个分支恒为假。

### P2-5 TUI 多处 `unwrap()` 可能 panic

`src/tui.rs:296`（`guard.children.clone().unwrap()`）、`:410-411`、`:432-433`、`:444-445`（`tui.list_state.selected().unwrap()`）。目录为空、选中索引越界或 `children` 为 `None` 时都会 panic。考虑到 TUI 的 `d` 动作会并发删除条目，"选中索引"与"实际列表长度"不一致是现实可能发生的情况。

### P2-6 TUI 启动时一次性递归扫描整棵树

`src/tui.rs:99` 的注释自己写着"新建一整个目录树，耗时极长"。`File::new` 对每个目录递归 `read_dir`，进入 TUI 之前要把整棵树建完，大目录下等待很久（虽有 "Scanning" 界面兜着）。建议改为按需展开。

---

## 四、改代码时需同步的文档

> 本节不是代码缺陷，而是上面几项动手时必须一起改掉的地方。

- **D-1（与 P0-1 绑定）**：`docs/guide.md` §3.3 / §9 / §10 以及 `docs/guide.zh-CN.md` 对应段落中，关于"`-c` 与 `-a` 组合会失败"的三处描述。
- **D-2（与 P1-2 绑定）**：`del --help` 的级别范围（`src/cli.rs:80`）与 `docs/guide.md:109`、`docs/guide.zh-CN.md:96` 需要统一。
- **D-3**：`-w` / `--show` 各处称"显示详情 / in detail"，实际是与 `-l` **完全相同**的 5 列表格（id/name/path/size/time）按 id 过滤，没有任何更细的字段。涉及 `README.md`、`README.zh-CN.md`、`docs/guide.md` §3.2、`docs/guide.zh-CN.md`。
- **D-4**：符号链接行为没有说明 —— `del link.txt` 会删除**目标**，并让链接本身残留为悬空链接。文档只说了 "symlink resolves to its target"。
- **D-5**：`-c` 与 `-d` / `-w` / `-R` 组合失败（见 P0-1 的矩阵）目前完全没有文档记载。
