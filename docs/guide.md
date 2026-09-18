# del — User Guide

[English](guide.md) | [简体中文](guide.zh-CN.md) · [← README](../README.md)

The README is the 30-second pitch. This guide is the full manual: what every flag
does, how ids are parsed, what actually lands on disk, and what to do when
something goes wrong.

> This guide describes the behavior of the code in this repository (v0.1.0). The
> [source map](#11-source-map) at the end points at the file that implements each
> area, so you can check whether the docs have drifted.

## 1 Install and first run

You need **Rust 1.88 or newer**: the code uses edition 2024 features, including
let-chains.

```bash
cargo install --path .   # build and install the `del` binary from this checkout

del -h                   # one line per option
del --help               # full descriptions, examples and the id syntax
```

A first run:

```bash
mkdir -p ~/scratch && echo hello > ~/scratch/note.txt

del ~/scratch/note.txt   # packed into ~/.trash, the original is removed
del -l                   # id, name, original directory, size, age
del -R 1                 # restored to ~/scratch/note.txt
```

`-l`, `-w` and `-x` send their table through a pager (`q` quits, `/` searches).
An empty result prints `Nothing.` instead of opening the pager.

## 2 What a delete does

For `del <path>...` (the default, no `--force`):

1. Each path is canonicalized, so a symlink resolves to its target. A path that
   does not exist is dropped without a message and without a failing exit code.
2. A path that contains — or equals — an entry of the disable list is dropped the
   same way (see [§6.3](#63-the-disable-list)).
3. The path is packed with `tar` + `zstd` into a temporary file in the trash
   directory, then renamed to `<blake3 of the archive>.bak`.
4. A row is inserted into the SQLite database, and — unless `--save` was given —
   the original is removed. Directories are packed recursively by default, so
   `del some/dir` stores the whole tree.

With `--force` **and** safe mode off, steps 3–4 are skipped: the path is removed
directly, and a directory needs `--recursive` as well.

Failures are per path and quiet. If packing one path fails (permission denied,
I/O error), the remaining paths are still processed, the failure is only visible
under `--verbose` (`Pack failed for "…": Permission denied (os error 13)`), and
the exit code stays 0. A failed pack can leave a `.tmp-*` file behind in the
trash directory; `--autoclean` removes it.

## 3 Command reference

### 3.1 Deleting

- **`del <path>...`** — pack each path into the trash and remove the original.
  Nonexistent and protected paths are skipped silently.
- **`-S`, `--save`** — pack without removing the originals; the entry can be
  restored later while the file stays in place. `--save` also overrides
  `--force`, so nothing is deleted when both are given.
- **`-f`, `--force`** — delete permanently instead of packing. Ignored while safe
  mode is on, and ignored together with `--save`.
- **`-r`, `--recursive`** — remove directories recursively. Only meaningful
  together with `--force`; without `--force` directories are packed recursively
  anyway.
- **`-t`, `--tui [PATH]`** — browse and act on files in a terminal UI. `PATH`
  defaults to the current directory. Keys: arrows move, `Enter` / `→` opens a
  directory, `←` goes back up, `d` deletes the selection into the trash, `s`
  packs it like `--save`, `q` / `Esc` quits. The UI needs a real terminal: with
  stdin redirected it does not exit on its own (it waits for key events), and the
  child processes it spawns do not inherit `--trash-dir`, `--level`, `--disable`
  or `--cover`.

### 3.2 Trash records

- **`-l`, `--list`** — list every record: id, name, original directory, size, age.
- **`-w`, `--show <ID>...`** — show the given records in detail.
- **`-x`, `--select <PATTERN>...`** — list records whose original path contains a
  pattern. The match is `original_path LIKE '%pattern%'`, so it is a substring
  search (ASCII case-insensitive) and `%` / `_` act as wildcards.
- **`-R`, `--restore <ID>...`** — restore records. Each one is unpacked into the
  original directory, or into the matching `--output` directory. A record is
  deleted only after a complete restore.
- **`-d`, `--delete <ID>...`** — drop records, keeping the packed files. Run
  `--autoclean` afterwards to reclaim the space.

### 3.3 Maintenance

- **`-a`, `--autoclean`** — clean the trash in two passes: first every file in
  the trash directory that no record points at is deleted (the database and its
  side files are never touched), then records older than `--save-time` are
  dropped. Because the age check happens after the sweep, the
  archives of expired records are collected by the *next* `--autoclean` run.
  See [§8.4](#84-lifecycle-of-a-record) for the full sequence and the side effects.
- **`-c`, `--clear`** — drop every record after a confirmation prompt. Only `y`
  or `Y` proceeds; any other input, including a bare Enter, cancels. The archives
  stay, so run `--autoclean` afterwards to reclaim the space. Note that the clear
  drops the table itself: combining `-c` with `-a` in one run fails with
  `Error: no such table: trash`.
- **`--level <N>`** — zstd compression level for this run, 1 (fastest) to 22
  (smallest). Defaults to `compression_level` from the config file, which is 3.
- **`--save-time <DAYS>`** — retention window used by `--autoclean` only.
  Defaults to `save_time` from the config file, which is 30.
- **`--trash-dir <DIR>`** — use another trash directory. It holds both the
  archives and `database.db`, and is created if missing. Defaults to
  `trash_dir` from the config file, which is `~/.trash`.
- **`--disable <PATH>,...`** — protect extra paths for this run. Appended to the
  configured disable list, not a replacement for it.

### 3.4 Output and behaviour

- **`-C`, `--cover <MODE>`** — what to do when a restore target already exists:
  `always` overwrites, `never` skips the file, `ask` prompts per file. A skipped
  file keeps its record so you can retry after moving the conflicting file away.
  Defaults to `cover_mode` from the config file, which is `ask`.
- **`-o`, `--output <PATH>,...`** — restore directories for `-R`. Values are
  paired with the ids in the order they appear: `del -R 1,2 -o a,b` restores
  record 1 into `a` and record 2 into `b`. Ids without a matching value fall back
  to their original directory.
- **`-s`, `--safe`** — force safe mode on. There is no flag to turn it off.
- **`-v`, `--verbose`** — print debug logs to stderr, each line prefixed with the
  source `file:line` it came from.
- **`-h` / `--help`** — short summary / full descriptions.
- **`-V`, `--version`** — print the version.

## 4 Id syntax

`--show`, `--restore` and `--delete` all accept the same id syntax:

- `3` — one id
- `2,5,9` — a comma-separated list
- `2-5` or `2~5` — an inclusive range
- `-5` — shorthand for `0-5`
- Repeat the flag to add more: `del -R 1,2 -R 7`

Every character that is not a digit, `,`, `-` or `~` is ignored, so a list pasted
from elsewhere still parses (`1 , 2` gives ids 1 and 2, and `5_000_090` reads as
`5000090`).

A value that starts with a dash must be passed in the `=` form — `del --show=-5`,
not `del --show -5` — because clap would otherwise read `-5` as a flag.

## 5 Configuration

### 5.1 Config file

`~/.config/del/config.toml`, all keys optional:

```toml
trash_dir = "/data/.trash"       # where archives and database.db live (default ~/.trash)
safe_mode = true                 # default true
compression_level = 6            # zstd level, default 3
save_time = 30                   # days kept by --autoclean, default 30
cover_mode = "ask"               # always / ask / never, default ask
disable_list = ["/boot", "/etc"] # REPLACES the built-in protected list
```

`disable_list` in the config file **replaces** the built-in list. The built-in
list protects the trash itself, the home directory, and `/`, `/boot`, `/etc`,
`/usr`, `/var`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/opt`, `/root`, `/home`,
`/proc`, `/sys`, `/dev`, `/tmp`.

### 5.2 Environment variables

Every key above can be set through an environment variable with the `DEL_` prefix;
keys are lowercased. List values use TOML array syntax.

```bash
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_SAFE_MODE=false del -f ~/junk.log          # really delete, no trash
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

### 5.3 Precedence

**CLI flags > `DEL_*` environment variables > config file > built-in defaults.**

CLI and environment values win per key. The one asymmetric case: `-s` only sets
`safe_mode = true`, and `--disable` only appends — turning safe mode off, or
replacing the protected list, has to happen in the config file or the
environment.

## 6 Safety model

### 6.1 The trash

Nothing is removed before it is stored. Deletion packs the path, writes a record,
and only then unlinks the original, so a `del` that fails mid-way leaves you with
either the original or an archive, not neither.

### 6.2 Safe mode

`safe_mode` is `true` by default. While it is on, `--force` is ignored and paths
are packed instead. The command line can only turn it on (`-s`); to bypass the
trash for a single run, override the config:

```bash
DEL_SAFE_MODE=false del -f ~/junk.log
```

### 6.3 The disable list

The matching rule is a prefix test on canonicalized paths: a delete target `p` is
skipped when some protected path `d` satisfies `d.strip_prefix(p)`, that is, when
**`p` is `d` itself or a parent directory of `d`**. So `del /etc` and `del /` are
skipped, while `del /etc/hosts` is not — protection points *upward*, it is not an
"this subtree is read-only" rule. Keep that in mind when you add entries with
`--disable` or `disable_list`.

### 6.4 Deferred deletion

Deleting frees no disk space. Space is reclaimed only by `--autoclean`, and
`--delete` / `--clear` drop records without touching the archives.

## 7 Recipes

### Recover a file

```bash
del -l                 # find the id
del -w 4               # check it is the right one (name, original directory, age)
del -R 4               # restore it
```

### Restore to a different directory

```bash
del -R 4 -o ~/Downloads          # this record goes to ~/Downloads
del -R 3,4 -o /tmp/a,/tmp/b      # 3 → /tmp/a, 4 → /tmp/b
```

### Restore over an existing file

```bash
del -C never -R 4                # skip files that already exist
del -C always -R 4               # overwrite them
del -C ask -R 4                  # prompt per file
```

Entries that were skipped keep their record, so you can move the conflicting file
away and run the restore again.

### Keep a copy without deleting

```bash
del -S ~/draft.md                # archive it, keep the original in place
```

### Reclaim disk space

```bash
del -d 1,2,3                     # drop records you no longer need (files stay)
del -a                           # sweep unreferenced archives + old records
del -a                           # run again: the archives of the expired records go now
```

### Use a separate trash

```bash
DEL_TRASH_DIR=/mnt/backup/.trash del ~/big.iso
DEL_TRASH_DIR=/mnt/backup/.trash del -l
```

### Search the trash

```bash
del -x report                    # every record whose original path contains "report"
del -x 2025-06,2025-07           # several patterns at once
```

## 8 Storage layout and record lifecycle

### 8.1 The trash directory

```
~/.trash/
├── database.db                  # SQLite (WAL mode)
├── database.db-wal, -shm        # SQLite side files, present while the DB is open
├── <blake3>.bak                 # one archive per packed path, tar + zstd
└── .tmp-<ms>-<seq>              # in-flight pack; left behind if a pack fails
```

### 8.2 Deduplication: what actually happens

An archive is named after the blake3 hash of **the archive bytes** — the tar
stream (entry name, mode, mtime) followed by the zstd frame — not the hash of the
original file contents. `del` then renames the temporary file into place, so two
packs that produce byte-identical archives share one file, and the later write
wins.

In practice this means:

- Deleting the same path twice with the same content, entry name and timestamp
  (tar stores whole seconds) reuses one `.bak`.
- Two files with identical contents but different names get **two** archives.
- Packing the same file with `--level 3` and `--level 9` gets **two** archives.

So do not read this as content deduplication: it keeps repeated deletes of the
same thing from piling up copies, and nothing more.

### 8.3 Database schema

```sql
CREATE TABLE trash (
    id            INTEGER PRIMARY KEY,
    original_path TEXT NOT NULL,   -- where the path was deleted from
    present_path  TEXT NOT NULL,   -- the .bak that holds it
    size          TEXT NOT NULL,   -- human-readable, e.g. 12.34KiB
    time          INTEGER NOT NULL -- deletion time, milliseconds since the epoch
);
```

### 8.4 Lifecycle of a record

```
delete   pack → <hash>.bak → INSERT row → remove the original (unless --save)
restore  unpack → (drop the row only if nothing was skipped)
delete   -d drops the row; the .bak stays
clear    -c drops every row (and the table); the .bak files stay
autoclean -a
   pass 1  delete every file in the trash directory that no row points at
   pass 2  DELETE FROM trash WHERE time < now - save_time days
           (their archives are now orphaned and go in the next -a)
```

Two consequences worth knowing: `-a` deletes *every* unreferenced file in the
trash directory except the database and its side files (`database.db`,
`database.db-wal`, `database.db-shm`, `database.db-journal`), so `.tmp-*`
leftovers are collected — and so is anything else you leave in that directory;
and `-a` aborts on a subdirectory in the trash directory with
`Error: Is a directory (os error 21)` (a directory whose name starts with the
database file name, such as `database.db-backup`, is skipped instead).

## 9 Exit codes

- `0` — the run completed. Skipped paths (missing, protected, failed pack) are
  not errors.
- `1` — a runtime error stopped the run, for example `Error: Is a directory
  (os error 21)` when `--force` is used on a directory without `--recursive`, or
  `Error: no such table: trash` when `-c` and `-a` are combined.
- `2` — bad command line (unknown flag, invalid value).

## 10 Known limitations

- **TUI needs a terminal.** With stdin redirected, `del -t` waits for key events
  and never exits on its own. Its `d` / `s` actions spawn a fresh `del` that does
  not inherit `--trash-dir`, `--level`, `--disable` or `--cover`, and the list
  removes an entry optimistically even if that child process fails.
- **`-c` and `-a` cannot share a run** (the clear drops the table the clean then
  reads).
- **Quiet failures.** A path that fails to pack leaves no trace unless you pass
  `-v`.
- **Protection points upward** (see [§6.3](#63-the-disable-list)); a child of a
  protected directory is not protected.
- **`-x` patterns are LIKE patterns**, so `%` and `_` are wildcards rather than
  literal characters.

## 11 Source map

| Area | File |
|------|------|
| CLI definition, help text, id parser | `src/cli.rs` |
| Config loading, precedence, disable-list matching, todo assembly | `src/config.rs` |
| Command dispatch: delete, restore, delete-records, clear, autoclean | `src/main.rs` |
| Packing, unpacking, hashing, cover modes | `src/compress.rs` |
| SQLite schema and queries | `src/sqlite.rs` |
| Terminal UI | `src/tui.rs` |

## License

MIT — see [LICENSE](../LICENSE).
