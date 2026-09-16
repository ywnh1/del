use crate::config::CoverMode;
use clap::{ArgAction::Append, Parser};
use std::path::PathBuf;

/// Delete files and directories safely and securely.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// File or directory to delete.
    pub path: Vec<PathBuf>,
    /// Remove the file forever instead of moving it to the trash.
    /// Ignored while safe mode is on.
    #[arg(short, long)]
    pub force: bool,
    /// Select entries like the patterns.
    #[arg(short = 'x', long, value_delimiter = ',', value_name = "PATTERN")]
    pub select: Vec<String>,
    /// Remove directories recursively with --force.
    /// Without --force, directories are packed recursively automatically.
    #[arg(short, long)]
    pub recursive: bool,
    /// List all entries currently in the trash.
    #[arg(short, long)]
    pub list: bool,
    /// Show details of trash entries by id (comma-separated).
    #[arg(short = 'w', long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub show: Vec<Vec<u64>>,
    /// Restore trash entries by id (comma-separated).
    /// Restored to the original location, or to --output if given.
    #[arg(short = 'R', long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub restore: Vec<Vec<u64>>,
    /// Delete trash records by id (comma-separated).
    /// The packed files stay on disk until --autoclean collects them.
    #[arg(short, long, value_name = "ID", value_parser = parse_ids,action=Append)]
    pub delete: Vec<Vec<u64>>,
    /// Set the zstd compression level for this run (default: 3).
    #[arg(long)]
    pub level: Option<i32>,
    /// Keep trash entries for this many days before --autoclean removes them.
    #[arg(long, value_name = "DAYS")]
    pub save_time: Option<u32>,
    /// Use this directory as the trash (default: ~/.trash).
    #[arg(long)]
    pub trash_dir: Option<PathBuf>,
    /// Paths that must never be deleted.
    /// Any path to delete that contains (or equals) one of these paths
    /// will be skipped and kept where it is.
    #[arg(long, value_delimiter = ',')]
    pub disable: Vec<PathBuf>,
    /// Enable safe mode for this run (default: on).
    #[arg(long, short)]
    pub safe: bool,
    /// Clean the trash: drop expired records and delete files with no record.
    #[arg(long, short)]
    pub autoclean: bool,
    /// Pack paths into the trash without removing the originals.
    #[arg(long, short = 'S')]
    pub save: bool,
    /// Drop all trash records. Packed files stay until --autoclean.
    #[arg(long, short)]
    pub clear: bool,
    /// What to do when a restore target already exists: always, ask, never.
    #[arg(long, short = 'C', value_name = "COVER_MODE")]
    pub cover: Option<CoverMode>,
    /// Restore to custom directories, one per --restore id (comma-separated).
    #[arg(long, short, value_name = "PATH", value_delimiter = ',')]
    pub output: Vec<PathBuf>,
    /// Use TUI mode to select paths.
    #[arg(long, short, value_name = "PATH", default_missing_value = ".",num_args=0..=1)]
    pub tui: Option<PathBuf>,
    /// Print verbose debug logs.
    #[arg(long, short)]
    pub verbose: bool,
}
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
