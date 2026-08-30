# del — Safe & Secure Deletion

> A delete tool with a safety net: removed files are compressed into a trash and can be restored anytime.

## ✨ Highlights

**Four layers of safety** turn the fear of `rm` accidents into a thing of the past:

| # | Layer |
|---|-------|
| ① | **Trash**: deletions go to `~/.trash` first; list with `-l`, restore with `-R` |
| ② | **Safe Mode (on by default)**: `--force` is ignored, so files can't bypass the trash |
| ③ | **Disable List**: critical system paths (home, `/`, `/etc`, `/usr`, `/boot`, ...) are never deleted |
| ④ | **Deferred deletion**: data stays archived until you explicitly run `--autoclean` to free disk space |

**Content-addressed storage**: archives are named by their content hash, so identical files are stored **only once** — later copies overwrite earlier ones and the trash stays tiny.

---

## Why

`rm` is irreversible: one typo and your data is gone. `del` turns deletion into two deliberate steps:

1. **Soft delete** — files are compressed into the trash (instantly restorable);
2. **Hard release** — disk space is only freed when you explicitly run `--autoclean`.

## Quick Start

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

## Commands

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

## Configuration

Optional config file `~/.config/del/config.toml`:

```toml
trash_dir = "/data/.trash"       # trash location
safe_mode = true                 # on by default
compression_level = 6            # zstd level (default 3)
save_time = 30                   # days autoclean keeps entries
cover_mode = "ask"               # always / ask / never
disable_list = ["/boot", "/etc"] # replaces the default protected list
```

Every option can also be overridden via environment variables with the `DEL_` prefix (keys are lowercased automatically; list values use TOML array syntax). Precedence: **CLI flags > environment variables > config file > defaults**.

```bash
# Environment variable examples
DEL_TRASH_DIR=/data/.trash DEL_COVER_MODE=never del -l
DEL_DISABLE_LIST='["/boot","/etc"]' del /some/path
```

## How It Works

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

## The Four Layers of Safety

1. **Trash** — every deletion lands in `~/.trash` first; list with `-l`, restore with `-R`. Deletion is never one-way.
2. **Safe Mode (on by default)** — `--force` is ignored, so files cannot bypass the trash unless you explicitly opt out.
3. **Disable List** — `~/.trash`, your home directory and critical system directories (`/`, `/boot`, `/etc`, `/usr`, `/var`, `/bin`, `/sbin`, `/lib`, `/lib64`, `/opt`, `/root`, `/home`, `/proc`, `/sys`, `/dev`, `/tmp`) are protected by default; any delete request that contains (or equals) a protected path is skipped and left untouched.
4. **Deferred deletion** — "deleting" only moves data into the trash; disk space is not freed until you run `--autoclean`. `--delete` / `--clear` touch records only, never the archived files.

## Content-Addressed Storage

Archives are named by the **blake3 hash of their content**. Deleting the same file twice — or two identical files — produces the same hash, so the second copy simply overwrites the first. Each unique file is stored exactly once, keeping the trash tiny.
