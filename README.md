# del — Safe & Secure Deletion

**English** | [简体中文](README.zh-CN.md) · 📖 [User Guide](docs/guide.md)

> A delete tool with a safety net: files are packed into a trash before they
> disappear, and can be restored at any time.

`del` turns deletion into two deliberate steps — soft delete into the trash, and
an explicit `--autoclean` that actually frees disk space — and backs them with a
protected-path list plus a safe mode that is on by default.

## ✨ Highlights

**Four layers of safety** turn the fear of an `rm` accident into a thing of the past:

| # | Layer |
|---|-------|
| ① | **Trash**: deletions go to `~/.trash` first; list with `-l`, restore with `-R` |
| ② | **Safe mode (on by default)**: `--force` is ignored, so nothing bypasses the trash |
| ③ | **Disable list**: critical paths (home, `/`, `/etc`, `/usr`, `/boot`, ...) are never deleted |
| ④ | **Deferred deletion**: data stays archived until you run `--autoclean` to free disk space |

**Content-addressed archives**: each archive is named by the blake3 hash of its own bytes, so re-deleting an unchanged path reuses one file instead of piling up copies. The hash covers the packed archive (tar header + zstd frame), not the file contents — see [deduplication](docs/guide.md#82-deduplication-what-actually-happens).

---

## Why

`rm` is irreversible: one typo, one wrong wildcard, one script that ran in the wrong directory, and the data is gone. `del` makes deletion two deliberate steps:

1. **Soft delete** — the file is packed (tar + zstd) into the trash and a record is written to SQLite; restore it whenever you want.
2. **Hard release** — disk space is freed only when you run `--autoclean`.

## Install

Rust 1.88 or newer is required (the crate uses edition 2024, including let-chains).

```bash
cargo install --path .
```

## Quick Start

```bash
# Delete a file: it moves into the trash, the original disappears
del ~/tmp/old-report.txt

# See what is in the trash
del -l

# Restore record 1 to its original location
del -R 1

# Restore it somewhere else instead
del -R 1 -o ~/Downloads

# Free disk space: drop expired records and unreferenced archives
del -a

# Browse and delete in a terminal UI
del -t ~/Downloads
```

## Commands

Run `del -h` for a one-line summary and `del --help` for the full description of
every option.

### Deleting

| Command | Description |
|---------|-------------|
| `del <path>...` | Pack files/directories into the trash and remove the originals |
| `del -S <path>...` | Pack into the trash but keep the originals (`--save`) |
| `del -f <path>...` | Delete permanently (ignored while safe mode is on) |
| `del -f -r <dir>` | Delete a directory tree permanently |
| `del -t [path]` | Browse and delete in a TUI (`path` defaults to `.`) |

TUI keys: arrows move, `Enter` / `→` opens a directory, `←` goes back up, `d`
deletes the selection into the trash, `s` packs it like `-S`, `q` / `Esc` quits.

### Trash records

| Command | Description |
|---------|-------------|
| `del -l` | List every record |
| `del -w <id>...` | Show the given records in detail |
| `del -x <pattern>...` | List records whose original path contains a pattern |
| `del -R <id>...` | Restore records |
| `del -d <id>...` | Drop records, keeping the packed files |

`-l`, `-w` and `-x` show a table in a pager: press `q` to quit, `/` to search.

### Maintenance

| Command | Description |
|---------|-------------|
| `del -a` | Clean the trash: expired records plus archives no record points at |
| `del -c` | Drop every record after a confirmation prompt (archives stay) |
| `del --level <n>` | zstd compression level for this run (default 3) |
| `del --save-time <days>` | Retention window used by `-a` (default 30) |
| `del --trash-dir <dir>` | Use another trash directory (default `~/.trash`) |
| `del --disable <path>,...` | Protect extra paths for this run |
| `del -s` | Force safe mode on |
| `del -v` | Print debug logs, prefixed with `file:line` |

### Ids

Every id argument accepts the same syntax:

- `3` — one id
- `2,5,9` — a comma-separated list
- `2-5` or `2~5` — an inclusive range
- `-5` — shorthand for `0-5`; it starts with a dash, so pass it in the `=`
  form: `del --show=-5` or `del -R=-5`
- Repeat the flag to add more: `del -R 1,2 -R 7`

Any character other than a digit, `,`, `-` or `~` is ignored, so a list pasted
from elsewhere still works.

## Configuration

Optional config file `~/.config/del/config.toml`:

```toml
trash_dir = "/data/.trash"       # trash location
safe_mode = true                 # on by default
compression_level = 6            # zstd level (default 3)
save_time = 30                   # days of retention used by --autoclean
cover_mode = "ask"               # always / ask / never
disable_list = ["/boot", "/etc"] # REPLACES the built-in protected list
```

Every key can also be set through environment variables with the `DEL_` prefix; keys are lowercased (`DEL_SAFE_MODE`, `DEL_TRASH_DIR`, ...). List values use TOML array syntax.

Precedence: **CLI flags > environment variables > config file > built-in defaults**.

```bash
# Environment variable examples
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

`safe_mode` can only be turned **on** from the command line (`-s`), so skipping
the trash for one run means overriding the config:

```bash
DEL_SAFE_MODE=false del -f ~/junk.log   # really delete, straight past the trash
```

## How It Works

```
Delete   del <path>
  ├─ 1. Pack with tar + zstd
  ├─ 2. Name by content hash → ~/.trash/<hash>.bak
  │        (same content → same name → stored once)
  ├─ 3. Insert a record into SQLite (id / original path / archive path / size / time)
  └─ 4. Remove the original (-S keeps it)

Restore  del -R <id>
  ├─ Unpack into the original directory (or the matching -o target)
  └─ Drop the record only after a complete restore; entries skipped by the
     cover mode keep their record so the restore can be retried

Clean    del -a
  ├─ Pass 1: delete archives that no record points at
  └─ Pass 2: drop records older than --save-time
     (their archives are collected by the next -a run)
```

## The Four Layers of Safety

1. **Trash** — every deletion lands in `~/.trash` first; list with `-l`, restore with `-R`. Deletion is never one-way.
2. **Safe mode (on by default)** — `--force` is ignored, so files cannot bypass the trash. No command-line flag switches it off: set `safe_mode = false` in the config file, or run with `DEL_SAFE_MODE=false`.
3. **Disable list** — `~/.trash`, your home directory and the system directories `/`, `/boot`, `/etc`, `/usr`, `/var`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/opt`, `/root`, `/home`, `/proc`, `/sys`, `/dev` and `/tmp` are protected by default. Any delete target that contains (or equals) a protected path is skipped and left untouched. `--disable a,b` **appends** to the list for that run; `disable_list` in the config file **replaces** the built-in defaults.
4. **Deferred deletion** — deleting only moves data into the trash; disk space is freed by `--autoclean`. `--delete` and `--clear` touch records only, never the archives.

## Content-Addressed Storage

An archive is named `<blake3 of the archive>.bak`. The hash is taken over the packed bytes — the tar stream (entry name, mode, mtime) plus the zstd frame — not over the original file contents. Two identical files with different names therefore get two archives, while deleting one unchanged path twice reuses a single one. The guide's [deduplication section](docs/guide.md#82-deduplication-what-actually-happens) spells out exactly what this does and does not collapse.

## License

MIT — see [LICENSE](LICENSE).
