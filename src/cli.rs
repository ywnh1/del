// SPDX-License-Identifier: MIT
// Copyright (c) 2026 ywnh1

use crate::config::CoverMode;
use clap::{ArgAction::Append, Parser};
use std::path::PathBuf;

/// Delete files and directories safely and securely
///
/// Deleting a path packs it with tar and zstd into a content-addressed trash,
/// records it in SQLite, and only then removes the original. Every record can be
/// listed and restored later, so a typo is no longer the end of a file.
///
/// Safe mode is on by default: --force is ignored and protected paths are left
/// untouched. Disk space is reclaimed only when you ask for it with --autoclean.
///
/// Use `del -h` for a one-line summary of every option, `del --help` for the
/// details, or `del --tui` to browse and delete from a terminal UI.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about, after_long_help = AFTER_LONG_HELP)]
pub struct Cli {
    /// Files or directories to delete
    ///
    /// Each path is canonicalized first, so a symlink resolves to its target.
    /// Paths that do not exist are skipped silently, and so is any path that
    /// contains (or equals) an entry of the disable list.
    pub path: Vec<PathBuf>,
    /// Delete permanently instead of moving to the trash
    ///
    /// A directory also needs --recursive, otherwise its removal fails. This flag
    /// is ignored while safe mode is on (the default) and ignored together with
    /// --save. Turn safe mode off in the config file or with DEL_SAFE_MODE=false
    /// if you really want to bypass the trash.
    #[arg(short, long)]
    pub force: bool,
    /// List records whose original path matches a pattern
    ///
    /// A substring search (SQLite LIKE '%pattern%', case-insensitive for ASCII),
    /// not a glob. Repeatable, and each value may list several patterns separated
    /// by commas. Results are shown in a pager: press q to quit, / to search.
    #[arg(short = 'x', long, value_delimiter = ',', value_name = "PATTERN")]
    pub select: Vec<String>,
    /// Remove directories recursively, together with --force
    ///
    /// Only meaningful for a permanent delete. Without --force a directory is
    /// packed into the trash recursively anyway.
    #[arg(short, long)]
    pub recursive: bool,
    /// List every record in the trash
    ///
    /// Columns: id, name, original directory, size, and how long ago the entry
    /// was trashed. Shown in a pager: press q to quit, / to search.
    #[arg(short, long)]
    pub list: bool,
    /// Show the records with these ids
    ///
    /// One id (`3`), a comma-separated list (`2,5,9`), a range (`2-5` or `2~5`),
    /// or an open range (`-5` means 0 through 5). Repeatable. A value that starts
    /// with a dash needs the `=` form (`--show=-5`), otherwise clap reads it as a
    /// flag.
    #[arg(short = 'w', long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub show: Vec<Vec<u64>>,
    /// Restore the records with these ids
    ///
    /// Accepts the same id syntax as --show. Each entry is unpacked to its
    /// original location, or into the matching --output directory when one is
    /// given. A record is dropped only after a complete restore; entries skipped
    /// by the cover mode stay in the trash so the restore can be retried.
    #[arg(short = 'R', long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub restore: Vec<Vec<u64>>,
    /// Drop the records with these ids, keeping the packed files
    ///
    /// Accepts the same id syntax as --show. The archives stay in the trash
    /// directory until --autoclean collects them, which is what actually frees
    /// disk space.
    #[arg(short, long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub delete: Vec<Vec<u64>>,
    /// zstd compression level for this run
    ///
    /// Higher is smaller and slower; 1 is fastest, 19 is a practical maximum.
    /// Defaults to compression_level from the config file (3).
    #[arg(long)]
    pub level: Option<i32>,
    /// Keep trash entries for this many days
    ///
    /// Only used by --autoclean, which drops records older than this window.
    /// Defaults to save_time from the config file (30).
    #[arg(long, value_name = "DAYS")]
    pub save_time: Option<u32>,
    /// Use this directory as the trash
    ///
    /// Holds both the archives and database.db, and is created if it does not
    /// exist. Defaults to trash_dir from the config file (~/.trash).
    #[arg(long)]
    pub trash_dir: Option<PathBuf>,
    /// Paths that must never be deleted
    ///
    /// Any delete target that contains (or equals) one of these paths is skipped
    /// and kept where it is. Comma-separated, and appended to the configured
    /// disable list. The built-in defaults protect the trash itself, the home
    /// directory, and /, /boot, /etc, /usr, /var, /bin, /sbin, /lib, /lib64,
    /// /opt, /root, /home, /proc, /sys, /dev and /tmp.
    #[arg(long, value_delimiter = ',')]
    pub disable: Vec<PathBuf>,
    /// Enable safe mode for this run (on by default)
    ///
    /// While safe mode is on, --force is ignored and protected paths are skipped.
    /// There is no flag to turn it off for a single run: set safe_mode = false in
    /// the config file, or export DEL_SAFE_MODE=false.
    #[arg(long, short)]
    pub safe: bool,
    /// Free disk space: drop expired records and unreferenced archives
    ///
    /// Runs in two passes. First every file in the trash directory that no record
    /// points at is deleted, except the SQLite database and its side files
    /// (database.db, database.db-wal, database.db-shm, database.db-journal). Then
    /// records older than --save-time are dropped, so their archives are
    /// collected by the next --autoclean run.
    #[arg(long, short)]
    pub autoclean: bool,
    /// Pack paths into the trash without removing the originals
    ///
    /// Useful as a snapshot: the entry can be restored later while the original
    /// file stays in place. --force is ignored when --save is given.
    #[arg(long, short = 'S')]
    pub save: bool,
    /// Drop every trash record after a confirmation prompt
    ///
    /// Only "y" or "Y" proceeds; any other input, including Enter, cancels. The
    /// packed files are not deleted, so run --autoclean afterwards to reclaim the
    /// space.
    #[arg(long, short)]
    pub clear: bool,
    /// What to do when a restored file would overwrite an existing one
    ///
    /// always overwrites, never skips the file, and ask prompts per file. A
    /// skipped file keeps its trash record, so the restore can be retried after
    /// you move the conflicting file away. Defaults to cover_mode from the config
    /// file (ask).
    #[arg(long, short = 'C', value_name = "COVER_MODE")]
    pub cover: Option<CoverMode>,
    /// Restore into this directory instead of the original one
    ///
    /// Values are paired with --restore ids in the order they appear: with
    /// `-R 1,2 -o a,b`, record 1 goes to a and record 2 to b. Ids without a
    /// matching value fall back to their original directory.
    #[arg(long, short, value_name = "PATH", value_delimiter = ',')]
    pub output: Vec<PathBuf>,
    /// Browse and act on files in a terminal UI
    ///
    /// PATH defaults to the current directory. In the UI, arrows move, Enter or
    /// Right opens a directory, Left goes back up, d deletes the selection into
    /// the trash, s packs it like --save, and q or Esc quits.
    #[arg(long, short, value_name = "PATH", default_missing_value = ".",num_args=0..=1)]
    pub tui: Option<PathBuf>,
    /// Print debug logs to stderr
    ///
    /// Each line is prefixed with the source file and line it came from.
    #[arg(long, short)]
    pub verbose: bool,
}

/// Extra text shown after the long help: examples, config location, precedence.
const AFTER_LONG_HELP: &str = "\
Examples:
  del report.txt            move report.txt into the trash
  del -l                    list every trash record
  del -w 3                  show record 3 in detail
  del -R 2 -o ~/Downloads   restore record 2 into ~/Downloads
  del -R 1-3,7              restore a set of records
  del -S draft.md           keep a copy in the trash, the original stays put
  del -a                    drop expired records and unreferenced archives

Id syntax: 3 | 2,5,9 | 2-5 or 2~5 | -5 (0 through 5)
           a value starting with '-' needs the = form, e.g. --restore=-5
Config:    ~/.config/del/config.toml
Precedence: CLI flags > DEL_* environment variables > config file > defaults";
#[derive(Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
enum ParseState {
    RecordingId,
    WaitingRangeEnd(u64),
    JustComma,
}
fn parse_ids(s: &str) -> anyhow::Result<Vec<u64>> {
    use ParseState::*;
    let mut res = Vec::with_capacity(s.len());
    let mut buf = String::new();
    let mut state = JustComma;
    for c in s.chars() {
        match c {
            '0'..='9' => {
                if state == JustComma {
                    state = RecordingId;
                }
                buf.push(c);
            }
            ',' => {
                if let WaitingRangeEnd(first) = state {
                    // let first = res.last().unwrap_or(&Some(0));
                    let last = buf.parse::<u64>();
                    if let Ok(l) = last {
                        for id in first..=l {
                            res.push(Some(id));
                        }
                    }
                } else if state == JustComma {
                    continue;
                } else {
                    res.push(buf.parse::<u64>().ok());
                }
                state = JustComma;
                buf.clear();
            }
            '-' | '~' => {
                if state == JustComma {
                    state = WaitingRangeEnd(0);
                } else if let WaitingRangeEnd(_) = state {
                    continue;
                } else {
                    state = WaitingRangeEnd(if let Ok(id) = buf.parse::<u64>() {
                        id
                    } else {
                        state = JustComma;
                        continue;
                    });
                    buf.clear();
                }
            }
            _ => {
                continue;
            }
        }
    }
    if !buf.is_empty() {
        if let WaitingRangeEnd(first) = state {
            // let first = res.last().unwrap_or(&Some(0));
            let last = buf.parse::<u64>();
            if let Ok(l) = last {
                for id in first..=l {
                    res.push(Some(id));
                }
            }
        } else {
            res.push(buf.parse::<u64>().ok());
        }
        buf.clear();
    }
    Ok(res.iter().filter_map(|x| *x).collect())
}
#[cfg(test)]
mod test {
    use super::*;
    macro_rules! tpi{
        ($s: expr,$($t: expr),*) => {
            assert_eq!(parse_ids($s).unwrap(),vec![$($t),*]);
        }
    }
    #[test]
    fn test_parse_ids() {
        tpi!("0,5,6,70,", 0, 5, 6, 70_u64);
        tpi!("0,6,7-9", 0, 6, 7, 8, 9_u64);
        tpi!("0,,6-~8,", 0, 6, 7, 8_u64);
        tpi!("5_000_090,7,8", 5000090, 7, 8_u64);
        tpi!("-5", 0, 1, 2, 3, 4, 5_u64);
    }
}
