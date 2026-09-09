use crate::config::CoverMode;
use clap::Parser;
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
    /// Remove directories recursively with --force.
    /// Without --force, directories are packed recursively automatically.
    #[arg(short, long)]
    pub recursive: bool,
    /// List all entries currently in the trash.
    #[arg(short, long)]
    pub list: bool,
    /// Show details of trash entries by id (comma-separated).
    #[arg(short = 'w', long, value_name = "ID", value_delimiter = ',')]
    pub show: Vec<u64>,
    /// Restore trash entries by id (comma-separated).
    /// Restored to the original location, or to --output if given.
    #[arg(short = 'R', long, value_name = "ID", value_delimiter = ',')]
    pub restore: Vec<u64>,
    /// Delete trash records by id (comma-separated).
    /// The packed files stay on disk until --autoclean collects them.
    #[arg(short, long, value_name = "ID", value_delimiter = ',')]
    pub delete: Vec<u64>,
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
