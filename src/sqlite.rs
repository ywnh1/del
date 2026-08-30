use crate::{verbose_dbg, verbose_println};
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, named_params, params};
use std::{
    fs::File,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Database {
    conn: Connection,
}
#[derive(Debug, Clone, Default)]
pub struct DatabaseRow {
    /// 自增主键
    id: i64,
    /// 原路径
    original_path: String,
    present_path: String,
    size: String,
    time: i64,
}

impl Database {
    // 路径应该在 trash_dir/database.db
    pub fn new(path: &Path) -> Result<Self> {
        let conn = if path.is_file() {
            Connection::open(path)?
        } else {
            File::create(path)?;
            Connection::open(path)?
        };
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        conn.execute_batch(
            r#"CREATE TABLE IF NOT EXISTS trash (
                         id INTEGER PRIMARY KEY,
                         original_path TEXT NOT NULL,
                         present_path TEXT NOT NULL,
                         size TEXT NOT NULL,
                         time INTEGER NOT NULL,
                     );"#,
        )?;
        Ok(Self { conn })
    }
    pub fn insert_many(&mut self, lines: &[DatabaseRow]) -> Result<()> {
        let tx = self.conn.transaction()?;
        {
            let mut stmt = tx.prepare("
                INSERT INTO trash (hash,original_path,present_path,size,time) VALUES (?1,?2,?3,?4,?5)
            ")?;
            for dbl in lines {
                stmt.execute(params![
                    dbl.original_path,
                    dbl.present_path,
                    dbl.size,
                    dbl.time
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn select_all(&mut self) -> Result<Vec<DatabaseRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare("SELECT * FROM trash")?;
        stmt.query_map([], |row| {
            Ok(DatabaseRow {
                id: row.get(0)?,
                original_path: row.get(1)?,
                present_path: row.get(2)?,
                size: row.get(3)?,
                time: row.get(4)?,
            })
        })?
        .collect()
    }
    pub fn select_by_id(&mut self, ids: &[i64]) -> Result<Vec<DatabaseRow>, rusqlite::Error> {
        let mut stmt = self.conn.prepare("SELECT * FROM trash WHERE id = ?1")?;
        ids.iter()
            .map(|id| {
                stmt.query_one(params![id], |row| {
                    Ok(DatabaseRow {
                        id: row.get(0)?,
                        original_path: row.get(1)?,
                        present_path: row.get(2)?,
                        size: row.get(3)?,
                        time: row.get(4)?,
                    })
                })
            })
            .collect()
    }
    pub fn delete_by_id(&mut self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let mut stmt = tx.prepare("DELETE FROM trash WHERE id = ?1")?;
        for id in ids {
            stmt.execute(params![id])?;
        }
        Ok(())
    }
}
