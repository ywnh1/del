use crate::config::CoverMode;
use crate::{input, verbose_dbg, verbose_println};
use anyhow::Result;
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

/// 获取 当前毫秒时间戳 u64
#[inline]
pub fn timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        // 系统时钟不可能早于1970，unwrap安全
        .unwrap()
        .as_millis() as u64
}

/// 计算 人类可读的大小
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
    // 1. 获取时间戳
    let time = timestamp_ms();
    // 2. 打包压缩
    let mut tmp_path = output_dir.join(format!(".tmp-{time}"));
    let mut n = 0;
    // 确保不存在
    while tmp_path.exists() {
        n += 1;
        tmp_path = output_dir.join(format!(".tmp-{time}-{n}"))
    }
    let mut file = File::create(verbose_dbg!(&tmp_path))?;
    let buf_writer = BufWriter::new(&file);
    let encoder = Encoder::new(buf_writer, level)?;
    let mut tar_builder = Builder::new(encoder);
    tar_builder.append_path(src)?;
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

    // 4. 改名
    // 如果哈希一样，说明是同一份文件，允许覆盖，节省储存空间
    let path = output_dir.join(format!("{hash}.bak"));
    fs::rename(tmp_path, &path)?;

    // 5. 构建返回值
    let size = humanized_size(file.metadata()?.len());
    verbose_println!("Pack {:#?} to {:#?}", src, output_dir);
    Ok(PackResult {
        original_path: src.into(),
        present_path: path,
        size,
        time,
    })
}

/// 解包到指定目录
pub fn unpack(src: &Path, output_dir: &Path, cover: CoverMode) -> Result<()> {
    let file = File::open(src)?;
    let buf_reader = BufReader::new(file);
    let decoder = Decoder::new(buf_reader)?;
    let mut archive = Archive::new(decoder);
    fs::create_dir_all(output_dir)?;
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
                        CoverMode::Never => {}
                        CoverMode::Ask => {
                            match input!("{:#?} is exists, do you want to cover it?[Y/n] ", dest)
                                .as_str()
                            {
                                "Y" | "y" => {
                                    entry.unpack_in(output_dir)?;
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    entry.unpack_in(output_dir)?;
                }
            }
        }
    }
    verbose_println!("Unpack {:#?} to {:#?}", src, output_dir);
    Ok(())
}
