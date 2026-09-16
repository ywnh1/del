use crate::cli::Cli;
use crate::tui::main_loop;
use crate::{VERBOSE, verbose_dbg, verbose_println};
use anyhow::Result;
use clap::{Parser, ValueEnum};
use dirs_next::home_dir;
use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};
#[derive(
    Debug, Copy, Clone, Hash, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize, ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum CoverMode {
    Always,
    Ask,
    Never,
}
#[derive(Debug, Clone, Hash, PartialEq, PartialOrd, Eq, Ord, Serialize, Deserialize)]
struct Config {
    cover_mode: CoverMode,
    compression_level: i32,
    save_time: u32,
    trash_dir: PathBuf,
    safe_mode: bool,
    disable_list: Vec<PathBuf>,
}
impl Default for Config {
    fn default() -> Self {
        let home = home_dir().expect("could not locate home directory");
        let trash_dir = home.join(".trash");
        Self {
            cover_mode: CoverMode::Ask,
            compression_level: 3,
            save_time: 30,
            trash_dir: trash_dir.clone(),
            safe_mode: true,
            // Protect the trash, the home directory, and critical system
            // directories. Deleting a protected path (or any parent that
            // contains one) is skipped. Unknown/nonexistent entries are
            // ignored at check time, so this list is safe to grow.
            disable_list: vec![
                trash_dir,
                home,
                PathBuf::from("/"),
                PathBuf::from("/boot"),
                PathBuf::from("/etc"),
                PathBuf::from("/usr"),
                PathBuf::from("/var"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/opt"),
                PathBuf::from("/root"),
                PathBuf::from("/home"),
                PathBuf::from("/proc"),
                PathBuf::from("/sys"),
                PathBuf::from("/dev"),
                PathBuf::from("/tmp"),
            ],
        }
    }
}

fn load_config() -> Result<Config, Box<figment::Error>> {
    let res = Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Toml::file(
            home_dir().unwrap().join(".config/del/config.toml"),
        ))
        .merge(Env::prefixed("DEL_"))
        .extract()?;
    Ok(res)
}
/// 同时加载config和cli
pub fn init() -> Result<Todo> {
    let mut config = load_config()?;
    let cli = Cli::parse();

    VERBOSE.set(cli.verbose).unwrap();

    verbose_println!(
        "Config loaded: cover_mode={:?}, compression_level={}, save_time={}, trash_dir={:#?}, safe_mode={}",
        config.cover_mode,
        config.compression_level,
        config.save_time,
        config.trash_dir,
        config.safe_mode
    );

    if let Some(path) = cli.tui {
        main_loop(path).unwrap();
    }

    // cli 优先级高于config
    if let Some(c) = cli.cover {
        config.cover_mode = verbose_dbg!(c);
    }
    if let Some(lv) = cli.level {
        config.compression_level = verbose_dbg!(lv);
    }
    if let Some(t) = cli.save_time {
        config.save_time = verbose_dbg!(t);
    }
    if let Some(d) = cli.trash_dir {
        config.trash_dir = verbose_dbg!(d);
    }
    if cli.safe {
        config.safe_mode = true;
    }
    if !cli.disable.is_empty() {
        config.disable_list.extend(cli.disable);
    }

    // 归一为一个todo对象
    let to_remove = cli
        .path
        .par_iter()
        .filter_map(|p| {
            let p = &p.canonicalize().ok()?;
            if !p.exists() {
                return None;
            } else {
                for disable in &config.disable_list {
                    if disable.exists() && disable.canonicalize().ok()?.strip_prefix(p).is_ok() {
                        verbose_println!("Skip {:#?}: contains disabled path {:#?}", p, disable);
                        return None;
                    }
                }
            }
            Some(p.clone())
        })
        .collect();
    let mut to_restore = HashMap::with_capacity(cli.restore.len());
    for i in 0..cli.restore.len() {
        to_restore.insert(cli.restore[i] as i64, cli.output.get(i).cloned());
    }
    Ok(Todo {
        to_remove,
        force: if config.safe_mode || cli.save {
            false
        } else {
            cli.force
        },
        recursive: cli.recursive,
        save: cli.save,
        level: config.compression_level,
        to_restore,
        cover: config.cover_mode,
        to_delete: cli.delete.iter().map(|&x| x as i64).collect(),
        autoclearn: cli.autoclean,
        save_time: config.save_time,
        trash_dir: config.trash_dir,
        show: cli.show.iter().map(|&x| x as i64).collect(),
        list: cli.list,
        clear: cli.clear,
        select: cli.select,
    })
}

#[derive(Debug, Clone)]
pub struct Todo {
    pub select: Vec<String>,
    /// Paths to delete
    pub to_remove: Vec<PathBuf>,
    /// Whether to delete forever instead of moving to the trash
    pub force: bool,
    /// Only effective when `force` is true
    pub recursive: bool,
    /// Pack into the trash without removing the originals
    pub save: bool,
    pub level: i32,

    /// Trash ids to restore, with an optional custom output directory
    pub to_restore: HashMap<i64, Option<PathBuf>>,
    pub cover: CoverMode,

    /// Trash ids to delete from the records
    pub to_delete: Vec<i64>,

    /// Whether to auto clean the trash
    pub autoclearn: bool,
    /// Only effective when `autoclearn` is true
    pub save_time: u32,

    pub trash_dir: PathBuf,
    pub show: Vec<i64>,
    pub list: bool,
    pub clear: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disable_list_protects_critical_directories() {
        let config = Config::default();
        let list = config
            .disable_list
            .iter()
            .filter_map(|p| p.to_str())
            .collect::<Vec<_>>();
        for critical in [
            "/", "/boot", "/etc", "/usr", "/var", "/bin", "/sbin", "/lib", "/lib64", "/opt",
            "/root", "/home", "/proc", "/sys", "/dev", "/tmp",
        ] {
            assert!(
                list.contains(&critical),
                "default disable_list should protect {critical}"
            );
        }
        // The trash itself and the home directory are also protected.
        assert!(list.iter().any(|p| p.ends_with(".trash")));
        assert!(list.iter().any(|p| p.starts_with("/home/")));
    }
}
