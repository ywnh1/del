use crate::config::CoverMode;
use clap::Parser;
use std::path::PathBuf;

/// Delete files and directories safely and securely.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the file or directory to delete.
    pub path: Vec<PathBuf>,
    /// Remove the file using system `rm -f`.
    /// This will not be worked in the safe mode.
    #[arg(short, long)]
    pub force: bool,
    /// Remove the file using system `rm -r`.
    /// This will not be worked in the safe mode.
    #[arg(short, long)]
    pub recursive: bool,
    /// List all files in your trash.
    #[arg(short, long)]
    pub list: bool,
    /// Show more details by short hash.
    #[arg(short = 'w', long)]
    pub show: Vec<String>,
    /// Restore the files in trash to where they come from.
    /// Input it's long or short hash.
    #[arg(short = 'R', long)]
    pub restore: Vec<String>,
    /// Delete the files from trash.
    /// Input it's long or short hash.
    #[arg(short, long)]
    pub delete: Vec<String>,
    /// Set zstd compression level once.
    #[arg(long)]
    pub level: Option<i32>,
    /// Set how long files are allowed to be saved in trash once.
    #[arg(long)]
    pub save_time: Option<u32>,
    /// Set trash once.
    #[arg(long)]
    pub trash_dir: Option<PathBuf>,
    /// Add file won't be moved into trash.
    #[arg(long)]
    pub disable: Vec<PathBuf>,
    /// Use safe mode once.
    #[arg(long, short)]
    pub safe: bool,
    /// Clean trash automatically.
    /// It will remove files out of date and files with no record in the database forever.
    #[arg(long, short)]
    pub autoclean: bool,
    /// Save them to trash without deleting them.
    #[arg(long, short = 'S')]
    pub save: bool,
    /// Set what will happen if there's a file already there which may be covered
    #[arg(long, short = 'C')]
    pub cover: Option<CoverMode>,
    /// Restore files to another directory.
    /// It's associated with the restore's input.
    #[arg(long, short)]
    pub output: Vec<PathBuf>,
    /// Show detail logs.
    #[arg(long, short)]
    pub verbose: bool,
}
