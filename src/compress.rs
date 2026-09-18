// SPDX-License-Identifier: MIT
// Copyright (c) 2026 ywnh1

use crate::config::CoverMode;
use crate::{input, verbose_dbg, verbose_println};
use anyhow::{Result, anyhow};
use blake3::Hasher;
use std::fs;
use std::io::{BufReader, Read, Seek};
use std::time::SystemTime;
use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};
use tar::{Archive, Builder};
use zstd::{Decoder, Encoder};

// Per-process counter so parallel pack() calls never collide on tmp names.
static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Current time as a millisecond UNIX timestamp (u64).
#[inline]
pub fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        // The system clock cannot predate 1970; unwrap is safe.
        .unwrap()
        .as_millis() as u64
}

/// Format a byte count as a human-readable size.
#[inline]
pub fn humanized_size(size: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;
    if size < KIB {
        format!("{size}B")
    } else if size < MIB {
        format!("{:.2}KiB", size as f64 / KIB as f64)
    } else if size < GIB {
        format!("{:.2}MiB", size as f64 / MIB as f64)
    } else if size < TIB {
        format!("{:.2}GiB", size as f64 / GIB as f64)
    } else {
        format!("{:.2}TiB", size as f64 / TIB as f64)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PackResult {
    pub original_path: PathBuf,
    pub present_path: PathBuf,
    pub size: String,
    pub time: u64,
}

/// 打包到指定目录下，返回PackResult
pub fn pack(src: &Path, output_dir: &Path, level: i32) -> Result<PackResult> {
    // 1. Timestamp + unique sequence number for the temp file
    let time = timestamp_ms();
    let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    // 2. Pack and compress
    let mut tmp_path = output_dir.join(format!(".tmp-{time}-{seq}"));
    let mut n = 0;
    // Make sure the temp name is free
    while tmp_path.exists() {
        n += 1;
        tmp_path = output_dir.join(format!(".tmp-{time}-{seq}-{n}"))
    }
    // Open with read+write: the fd is used again below to hash the file.
    let mut file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(verbose_dbg!(&tmp_path))?;
    let buf_writer = BufWriter::new(&file);
    let encoder = Encoder::new(buf_writer, level)?;
    let mut tar_builder = Builder::new(encoder);
    // tar rejects absolute paths, so archive under a relative entry name:
    // a file becomes "<name>", a directory becomes "<name>/..." recursively.
    let entry_name = src
        .file_name()
        .ok_or_else(|| anyhow!("invalid path for packing: {src:?}"))?;
    if src.is_dir() {
        tar_builder.append_dir_all(entry_name, src)?;
    } else {
        tar_builder.append_path_with_name(src, entry_name)?;
    }
    let encoder = tar_builder.into_inner()?;
    let buf_writer = encoder.finish()?;
    drop(buf_writer);

    // 3. 求hash
    let mut hasher = Hasher::new();
    let mut buf = [0_u8; 1024 * 4];
    file.seek(std::io::SeekFrom::Start(0))?;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let hash = verbose_dbg!(hasher.finalize().to_string());

    // 4. Rename to the content hash.
    // Same content hashes to the same name, so identical files overwrite
    // each other: each unique file is stored only once.
    let path = output_dir.join(format!("{hash}.bak"));
    fs::rename(tmp_path, &path)?;
    verbose_println!("Stored as {:#?}", path);

    // 5. 构建返回值
    let size = humanized_size(file.metadata()?.len());
    verbose_println!("Packed {src:?} into {output_dir:?}");
    Ok(PackResult {
        original_path: src.into(),
        present_path: path,
        size,
        time,
    })
}

/// Unpack results: how many entries were restored vs skipped.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnpackStats {
    /// Entries successfully restored.
    pub restored: usize,
    /// Entries skipped because the cover mode refused to overwrite.
    pub skipped: usize,
}

/// Unpack an archive into `output_dir`, returning restore/skip statistics.
pub fn unpack(src: &Path, output_dir: &Path, cover: CoverMode) -> Result<UnpackStats> {
    let file = File::open(src)?;
    let buf_reader = BufReader::new(file);
    let decoder = Decoder::new(buf_reader)?;
    let mut archive = Archive::new(decoder);
    fs::create_dir_all(output_dir)?;
    let mut stats = UnpackStats::default();
    match cover {
        CoverMode::Always => {
            archive.unpack(output_dir)?;
        }
        _ => {
            for entry_result in archive.entries()? {
                let mut entry = entry_result?;
                let path = entry.path()?.to_path_buf();
                let dest = output_dir.join(&path);
                if dest.exists() {
                    match cover {
                        CoverMode::Always => {
                            unreachable!();
                        }
                        CoverMode::Never => {
                            verbose_println!("Skip {:#?}: {:#?} already exists", dest, output_dir);
                            stats.skipped += 1;
                        }
                        CoverMode::Ask => {
                            match input!("{dest:?} already exists. Overwrite it? [y/N] ").as_str() {
                                "Y" | "y" => {
                                    entry.unpack_in(output_dir)?;
                                    stats.restored += 1;
                                }
                                _ => {
                                    verbose_println!("User declined to overwrite {dest:?}");
                                    stats.skipped += 1;
                                }
                            }
                        }
                    }
                } else {
                    entry.unpack_in(output_dir)?;
                    stats.restored += 1;
                }
            }
        }
    }
    verbose_println!(
        "Unpack {src:?} to {output_dir:?}: restored {} entries, skipped {} entries",
        stats.restored,
        stats.skipped
    );
    Ok(stats)
}
