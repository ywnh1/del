use crate::cli::Cli;
use crate::{VERBOSE, verbose_dbg};
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
        let trash_dir = home_dir().unwrap().join(".trash");
        Self {
            cover_mode: CoverMode::Ask,
            compression_level: 3,
            save_time: 30,
            trash_dir: trash_dir.clone(),
            safe_mode: true,
            disable_list: vec![trash_dir, home_dir().unwrap(), PathBuf::from("/boot")],
        }
    }
}

fn load_config() -> Result<Config, figment::Error> {
    Figment::new()
        .merge(Serialized::defaults(Config::default()))
        .merge(Toml::file(
            home_dir().unwrap().join(".config/del/config.toml"),
        ))
        .merge(Env::prefixed("DEL_").split("_"))
        .extract()
}
/// 同时加载config和cli
pub fn init() -> Result<Todo> {
    let mut config = load_config()?;
    let cli = Cli::parse();

    VERBOSE.set(cli.verbose).unwrap();

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
            if !p.exists() {
                return None;
            } else {
                for disable in &config.disable_list {
                    if disable.exists()
                        && disable
                            .canonicalize()
                            .ok()?
                            .strip_prefix(p.canonicalize().ok()?)
                            .is_ok()
                    {
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
    })
}

#[derive(Debug, Clone)]
pub struct Todo {
    /// 要被删除的路径
    pub to_remove: Vec<PathBuf>,
    /// 是否强制删除
    pub force: bool,
    /// 只有 force 为 true 生效
    pub recursive: bool,
    pub save: bool,
    pub level: i32,

    /// 要被恢复的哈希，和可能有的自定义恢复位置
    pub to_restore: HashMap<i64, Option<PathBuf>>,
    pub cover: CoverMode,

    /// 要被删除的哈希
    pub to_delete: Vec<i64>,

    /// 是否要 Auto clean
    pub autoclearn: bool,
    /// 只有上面true才生效
    pub save_time: u32,

    pub trash_dir: PathBuf,
    pub show: Vec<i64>,
    pub list: bool,
    pub clear: bool,
}
