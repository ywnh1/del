// SPDX-License-Identifier: MIT
// Copyright (c) 2026 ywnh1

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::sqlite::DatabaseRow;
mod cli;
mod compress;
mod config;
mod sqlite;
mod tui;

static VERBOSE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[macro_export]
macro_rules! input {

    ($($arg:tt)*) => {{
        use ::std::io::Write as _;
        let _guard = $crate::LOCK.lock().unwrap_or_else(|e| e.into_inner());
        print!($($arg)*);
        ::std::io::stdout().flush().expect("stdout flush failed");
        let mut buf = String::new();
        ::std::io::stdin().read_line(&mut buf).expect("input read failed");

        if buf.ends_with('\n') {
            buf.pop();
            if buf.ends_with('\r') {
                buf.pop();
            }
        }
        buf
    }};
}

#[macro_export]
macro_rules! verbose_println {
    ($($arg:tt)*) => {
        if *$crate::VERBOSE.get().unwrap_or(&false){
            let info = format!("[{}:{}]",file!(),line!());
            let msg = format!($($arg)*);
            eprintln!("{info}{msg}");
        }
    };
}
#[macro_export]
macro_rules! verbose_dbg {
    ($arg:expr) => {{
        if *$crate::VERBOSE.get().unwrap_or(&false) {
            dbg!($arg)
        } else {
            $arg
        }
    }};
}

fn main() -> Result<()> {
    let todo = verbose_dbg!(config::init())?;
    verbose_println!(
        "Todo loaded: remove={}, restore={}, delete={}, show={}, force={}, save={}, trash_dir={:#?}",
        todo.to_remove.len(),
        todo.to_restore.len(),
        todo.to_delete.len(),
        todo.show.len(),
        todo.force,
        todo.save,
        todo.trash_dir
    );
    // 确保回收站目录存在
    fs::create_dir_all(&todo.trash_dir)?;
    verbose_println!("Trash dir ready: {:#?}", todo.trash_dir);
    let db_path = verbose_dbg!(todo.trash_dir.join("database.db"));
    let mut db = sqlite::Database::new(&db_path)?;
    if todo.list {
        verbose_println!("Listing all entries");
        db.list_all()?;
    }
    if todo.clear {
        match input!(
            "Permanently remove all trash records? Packed files stay until autoclean. [y/N] "
        )
        .as_str()
        {
            "Y" | "y" => {
                verbose_println!("User confirmed clearing the trash");
                db.clear()?;
            }
            _ => {
                verbose_println!("User declined clearing the trash");
            }
        }
    }
    if todo.autoclearn {
        // SQLite keeps side files next to the database (-wal, -shm, -journal).
        // They belong to the database, so an autoclean sweep must not collect
        // them as if they were orphaned archives.
        let db_name = db_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("database.db");
        let set: HashSet<PathBuf> = db
            .select_all()?
            .par_iter()
            .filter_map(|x| {
                let p = PathBuf::from(&x.present_path);
                if p.exists() {
                    Some(p.canonicalize().unwrap())
                } else {
                    None
                }
            })
            .collect();
        for file in fs::read_dir(&todo.trash_dir)? {
            let file = file?.path();
            verbose_println!("Checking {file:?}");
            if let Some(name) = file.file_name()
                && name.to_str().is_some_and(|n| n.starts_with(db_name))
            {
                verbose_println!("Skipping the database file");
                continue;
            }
            let file = file.canonicalize().unwrap();
            if set.contains(&file) {
                verbose_println!("Still referenced by the database, keeping");
                continue;
            }
            verbose_println!("Not referenced, will be removed");
            fs::remove_file(file)?;
        }
        db.delete_by_time(todo.save_time)?;
    }
    // 删除主逻辑
    if todo.force {
        verbose_println!(
            "Force removing {} path(s), recursive={}",
            todo.to_remove.len(),
            todo.recursive
        );
        if todo.recursive {
            for p in todo.to_remove {
                verbose_println!("Force remove {:#?}", p);
                remove_any(&p)?;
            }
        } else {
            for p in todo.to_remove {
                verbose_println!("Force remove file {:#?}", p);
                fs::remove_file(&p)?;
            }
        }
    } else {
        verbose_println!(
            "Packing {} path(s) into {:#?} (level {})",
            todo.to_remove.len(),
            todo.trash_dir,
            todo.level
        );
        let lines: Vec<DatabaseRow> = todo
            .to_remove
            .par_iter()
            .filter_map(|p| match compress::pack(p, &todo.trash_dir, todo.level) {
                Ok(x) => Some(x),
                Err(e) => {
                    verbose_println!("Pack failed for {:#?}: {e:#}", p);
                    None
                }
            })
            .map(|x| x.into())
            .collect();
        verbose_println!("Inserting {} row(s) into database", lines.len());
        db.insert_many(&lines)?;
        if !todo.save {
            verbose_println!("Removing {} original path(s)", lines.len());
            for r in lines {
                verbose_println!("Remove original {:#?}", r.original_path);
                remove_any(Path::new(&r.original_path))?;
            }
        }
    }
    // 恢复主逻辑
    if !todo.to_restore.is_empty() {
        verbose_println!("Restoring {} record(s)", todo.to_restore.len());
        let mut ids: Vec<i64> = todo.to_restore.keys().copied().collect();
        let rows = db.select_by_id(&ids)?;
        ids = rows
            .par_iter()
            .filter_map(|row| {
                Some((
                    Path::new(&row.present_path),
                    if let Some(p) = todo.to_restore.get(&row.id)? {
                        p
                    } else {
                        Path::new(&row.original_path).parent()?
                    },
                    row.id,
                ))
            })
            .filter_map(|(src, output_dir, id)| {
                let stats = compress::unpack(src, output_dir, todo.cover).ok()?;
                if stats.skipped > 0 {
                    // 有条目因覆盖策略未恢复，保留数据库记录以便重试
                    verbose_println!(
                        "Keep record {id}: {skipped} entry(s) skipped, restored {restored}",
                        skipped = stats.skipped,
                        restored = stats.restored
                    );
                    return None;
                }
                verbose_println!("Record {id} fully restored");
                Some(id)
            })
            .collect();
        verbose_println!(
            "Deleting {} fully-restored record(s) from database",
            ids.len()
        );
        db.delete_by_id(&ids)?;
    }
    // 删除主逻辑
    if !todo.to_delete.is_empty() {
        verbose_println!("Deleting {} record(s) by id", todo.to_delete.len());
        db.delete_by_id(&todo.to_delete)?;
    }
    if !todo.show.is_empty() {
        verbose_println!("Showing {} record(s) by id", todo.show.len());
        db.list_by_id(&todo.show)?;
    }
    if !todo.select.is_empty() {
        verbose_println!("Selecting {} patterns", todo.select.len());
        db.list_by_path(&todo.select)?;
    }

    Ok(())
}

fn remove_any(path: &Path) -> std::io::Result<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else {
        fs::remove_file(path)?;
    }
    Ok(())
}
