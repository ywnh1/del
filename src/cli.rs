use crate::config::CoverMode;
use clap::Parser;
use std::path::PathBuf;

/// Delete files and directories safely and securely.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Path to the file or directory to delete.
    pub path: Vec<PathBuf>,
    /// Remove the file forever.
    /// This will not be worked in the safe mode.
    #[arg(short, long)]
    pub force: bool,
    /// Remove the file recursively.
    /// Only used with force.
    /// Without force, it's automatical.
    /// This will not be worked in the safe mode.
    #[arg(short, long)]
    pub recursive: bool,
    /// List all files in your trash.
    #[arg(short, long)]
    pub list: bool,
    /// Show more details by id.
    #[arg(short = 'w', long, value_name = "SHORT_HASH")]
    pub show: Vec<String>,
    /// Restore the files in trash to where they come from.
    /// Input its id.
    #[arg(short = 'R', long, value_name = "HASH")]
    pub restore: Vec<String>,
    /// Delete the files from trash.
    /// Input its id.
    #[arg(short, long, value_name = "HASH")]
    pub delete: Vec<String>,
    /// Set zstd compression level once.
    #[arg(long)]
    pub level: Option<i32>,
    /// Set how long files are allowed to be saved in trash once.
    #[arg(long, value_name = "DAYS")]
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
    /// Clear your trash.
    #[arg(long, short)]
    pub clear: bool,
    /// Set what will happen if there's a file already there which may be covered
    #[arg(long, short = 'C', value_name = "COVER_MODE")]
    pub cover: Option<CoverMode>,
    /// Restore files to another directory.
    /// It's associated with the restore's input.
    #[arg(long, short, value_name = "PATH")]
    pub output: Vec<PathBuf>,
    /// Show detail logs.
    #[arg(long, short)]
    pub verbose: bool,
}
