use anyhow::Result;
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
        let _guard = crate::LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        if *crate::VERBOSE.get().unwrap(){
            let info = format!("[{}:{}]",file!(),line!());
            let msg = format!($($arg)*);
            eprintln!("{info}{msg}");
        }
    };
}
#[macro_export]
macro_rules! verbose_dbg {
    ($arg:expr) => {{
        if *crate::VERBOSE.get().unwrap() {
            dbg!($arg)
        } else {
            $arg
        }
    }};
}

fn main() -> Result<()> {
    let todo = verbose_dbg!(config::init())?;

    Ok(())
}
