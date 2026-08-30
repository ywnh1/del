use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::{compress::PackResult, sqlite::DatabaseRow};
mod cli;
mod compress;
mod config;
mod sqlite;

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
    let db_path = verbose_dbg!(todo.trash_dir.join("database.db"));
    let mut db = sqlite::Database::new(&db_path)?;
    if todo.list {
        db.list_all()?;
    }
    if todo.clear {
        match input!("It'll delete all your data forever. Do you really want to continue?[Y/n] ")
            .as_str()
        {
            "Y" | "y" => {
                db.clear()?;
            }
            _ => {}
        }
    }
    if todo.autoclearn {
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
            verbose_println!("Checking {:#?}", file);
            if let Some(name) = file.file_name() {
                if name.to_str() == Some("database.db") {
                    verbose_println!("It's the database.");
                    continue;
                }
            }
            let file = file.canonicalize().unwrap();
            if set.contains(&file) {
                verbose_println!("It's recorded.");
                continue;
            }
            verbose_println!("It'll be cleaned.");
            fs::remove_file(file)?;
        }
        db.delete_by_time(todo.save_time)?;
    }
    // 删除主逻辑
    if todo.force {
        if todo.recursive {
            for p in todo.to_remove {
                remove_any(&p)?;
            }
        } else {
            for p in todo.to_remove {
                fs::remove_file(&p)?;
            }
        }
    } else {
        let lines: Vec<DatabaseRow> = todo
            .to_remove
            .par_iter()
            .filter_map(|p| compress::pack(&p, &todo.trash_dir, todo.level).ok())
            .map(|x| x.into())
            .collect();
        db.insert_many(&lines)?;
        if !todo.save {
            for r in lines {
                remove_any(Path::new(&r.original_path))?;
            }
        }
    }
    // 恢复主逻辑
    if !todo.to_restore.is_empty() {
        let mut ids: Vec<i64> = todo.to_restore.keys().map(|id| *id).collect();
        let rows = db.select_by_id(&ids)?;
        ids = rows
            .par_iter()
            .filter_map(|row| {
                Some((
                    Path::new(&row.present_path),
                    Path::new(&row.original_path).parent()?,
                    row.id,
                ))
            })
            .filter_map(|(src, output_dir, id)| {
                compress::unpack(src, output_dir, todo.cover).ok()?;
                Some(id)
            })
            .collect();
        db.delete_by_id(&ids)?;
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
