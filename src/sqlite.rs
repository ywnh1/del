use crate::{
    compress::{PackResult, timestamp_ms},
    verbose_dbg, verbose_println,
};
use anyhow::{Result, anyhow};
use humantime::{Duration, format_duration};
use minus::{Pager, page_all};
use rusqlite::{Connection, params};
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use tabled::{Table, Tabled, settings::Style};

#[derive(Debug)]
pub struct Database {
    conn: Connection,
}
#[derive(Debug, Clone, Default)]
pub struct DatabaseRow {
    /// 自增主键
    pub id: i64,
    /// 原路径
    pub original_path: String,
    pub present_path: String,
    pub size: String,
    pub time: i64,
}

impl From<PackResult> for DatabaseRow {
    fn from(value: PackResult) -> Self {
        Self {
            id: 0,
            original_path: value.original_path.display().to_string(),
            present_path: value.present_path.display().to_string(),
            size: value.size,
            time: value.time as i64,
        }
    }
}

#[derive(Debug, Clone, Default, Tabled)]
pub struct TableRow {
    id: i64,
    name: String,
    path: String,
    size: String,
    time: String,
}

impl TryFrom<&DatabaseRow> for TableRow {
    type Error = anyhow::Error;
    fn try_from(x: &DatabaseRow) -> std::prelude::v1::Result<Self, Self::Error> {
        Ok(Self {
            id: x.id,
            name: Path::new(&x.original_path)
                .file_name()
                .ok_or(anyhow!("fali to get {}'s name", x.original_path))?
                .display()
                .to_string(),
            path: Path::new(&x.original_path)
                .parent()
                .ok_or(anyhow!("fali to get {}'s parents dir", x.original_path))?
                .display()
                .to_string(),
            size: x.size.to_string(),
            time: n_days_ago_humanlize(x.time),
        })
    }
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
                         time INTEGER NOT NULL
                     );"#,
        )?;
        verbose_println!("Connect to {:#?}", path);
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
                verbose_println!("Insert {:#?}", dbl);
            }
        }
        tx.commit()?;
        Ok(())
    }
    pub fn select_all(&mut self) -> Result<Vec<DatabaseRow>, rusqlite::Error> {
        verbose_println!("Select all");
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
                verbose_println!("Selecting by id: {id}");
                verbose_dbg!(stmt.query_one(params![id], |row| {
                    Ok(DatabaseRow {
                        id: row.get(0)?,
                        original_path: row.get(1)?,
                        present_path: row.get(2)?,
                        size: row.get(3)?,
                        time: row.get(4)?,
                    })
                }))
            })
            .collect()
    }
    pub fn delete_by_id(&mut self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.transaction()?;
        let mut stmt = tx.prepare("DELETE FROM trash WHERE id = ?1")?;
        for id in ids {
            stmt.execute(params![id])?;
            verbose_println!("The rows with a id of {id} is deleted");
        }
        Ok(())
    }
    pub fn delete_by_time(&mut self, days: u32) -> Result<()> {
        let timestamp = n_days_ago(days) as i64;
        let tx = self.conn.transaction()?;
        let mut stmt = tx.prepare("DELETE FROM trash WHERE time < ?1")?;
        stmt.execute(params![timestamp])?;
        verbose_println!("The rows {days} days ago is deleted");
        Ok(())
    }
    pub fn list_all(&mut self) -> Result<()> {
        let all = self.select_all()?;
        list(&all)?;
        Ok(())
    }
    pub fn clear(&mut self) -> Result<()> {
        self.conn.execute_batch("DROP TABLE IF EXISTS trash;")?;
        Ok(())
    }
}

#[inline]
pub fn n_days_ago(n: u32) -> u64 {
    timestamp_ms().saturating_sub(n as u64 * 86400_000)
}

pub fn n_days_ago_humanlize(then: i64) -> String {
    let ms = timestamp_ms().saturating_sub(then as u64);
    let d = core::time::Duration::from_millis(ms);
    format!("{} ago", format_duration(d))
}

pub fn list(rows: &[DatabaseRow]) -> Result<()> {
    let pager = Pager::new();
    let table = Table::new(
        rows.iter()
            .filter_map(|x| TryInto::<TableRow>::try_into(x).ok()),
    )
    .with(Style::modern_rounded())
    .to_string();
    pager.set_text(table)?;
    pager.set_prompt("Press 'q' to exit | Press '/' to search")?;
    page_all(pager)?;
    Ok(())
}
