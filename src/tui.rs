use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, poll},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, List, ListItem, ListState, Paragraph, Shadow},
};
use std::{
    io::{Stdout, stdout},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex, Weak,
        mpsc::{self, Sender},
    },
    time::Duration,
};

struct Guard;

impl Guard {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        disable_raw_mode().unwrap();
        execute!(stdout(), LeaveAlternateScreen).unwrap();
    }
}

#[derive(Debug, Clone)]
struct File {
    path: PathBuf,
    file_type: FileType,
    children: Option<Vec<Arc<Mutex<File>>>>,
    parent: Option<Weak<Mutex<Self>>>,
    size: u64,
}

impl File {
    /// 新建一整个目录树，耗时极长
    fn new(
        path: &Path,
        parent: Option<Weak<Mutex<Self>>>,
        sender: Sender<SendMsg>,
    ) -> Result<Arc<Mutex<Self>>> {
        let _ = sender.send(SendMsg::Working(path.display().to_string()));
        let metadata = path.metadata()?;
        let file_type = if path.symlink_metadata()?.is_symlink() {
            let inner = if metadata.is_file() {
                Box::new(FileType::File(metadata.len()))
            } else if metadata.is_dir() {
                Box::new(FileType::Dir)
            } else {
                Box::new(FileType::Others)
            };
            FileType::Symlink(inner)
        } else {
            if metadata.is_file() {
                FileType::File(metadata.len())
            } else if metadata.is_dir() {
                FileType::Dir
            } else {
                FileType::Others
            }
        };
        let children: Option<Vec<Arc<Mutex<File>>>> = if file_type.is_dir() {
            Some(
                path.read_dir()?
                    .filter_map(|entry| {
                        let entry = entry.ok()?;
                        File::new(&entry.path(), None, sender.clone()).ok()
                    })
                    .collect(),
            )
        } else {
            None
        };
        let size = if let Some(size) = file_type.get_size() {
            size
        } else if let Some(children) = &children {
            children
                .iter()
                .map(|child| child.lock().unwrap().size)
                .sum()
        } else {
            0
        };
        let res = Arc::new(Mutex::new(Self {
            path: path.to_path_buf(),
            file_type,
            children: None,
            size,
            parent,
        }));

        let parent = Arc::downgrade(&res);

        res.lock().unwrap().children = children.map(|children| {
            children
                .iter()
                .map(|child| {
                    child.lock().unwrap().parent = Some(parent.clone());
                    child.clone()
                })
                .collect()
        });

        Ok(res)
    }
    fn size(&self) -> String {
        let size = self.size as f64;
        const KIB: f64 = 1024.;
        const MIB: f64 = KIB * KIB;
        const GIB: f64 = MIB * KIB;
        const TIB: f64 = GIB * KIB;
        const PIB: f64 = TIB * KIB;
        let (size, format) = if size <= KIB {
            (size, "B")
        } else if size <= MIB {
            (size / KIB, "KiB")
        } else if size <= GIB {
            (size / MIB, "MiB")
        } else if size <= TIB {
            (size / GIB, "GiB")
        } else if size <= PIB {
            (size / TIB, "TiB")
        } else {
            (size / PIB, "PiB")
        };
        format!("{size:.2}{format}")
    }
    fn name(&self) -> &str {
        self.path
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(self.path.to_str().unwrap_or("."))
    }
}
#[derive(Debug, Clone)]
enum FileType {
    File(u64),
    Dir,
    Symlink(Box<Self>),
    Others,
}
impl std::fmt::Display for FileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            Self::File(_size) => "",
            Self::Dir => "/",
            Self::Symlink(_type) => "&",
            Self::Others => "",
        };
        f.write_str(data)
    }
}
impl FileType {
    fn is_dir(&self) -> bool {
        match self {
            Self::Dir => true,
            Self::Symlink(inner) => inner.is_dir(),
            _ => false,
        }
    }
    fn get_size(&self) -> Option<u64> {
        match self {
            Self::File(size) => Some(*size),
            Self::Symlink(inner) => inner.get_size(),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum SendMsg {
    Working(String),
    Finished(Arc<Mutex<File>>),
    Error(anyhow::Error),
}

#[derive(Debug)]
struct Tui<'a> {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    weights: UI<'a>,
    list_state: ListState,
}

impl<'a> Tui<'a> {
    fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(stdout());
        let terminal = Terminal::new(backend)?;
        let list_state = ListState::default().with_selected(Some(1));
        Ok(Self {
            terminal,
            weights: UI::new(),
            list_state,
        })
    }
    fn scanning_draw(&mut self, path: &str) -> Result<()> {
        self.terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::vertical([
                Constraint::Fill(1),
                Constraint::Length(4),
                Constraint::Fill(1),
            ])
            .split(area);
            let pop_up_area = Layout::horizontal([
                Constraint::Percentage(15),
                Constraint::Fill(1),
                Constraint::Percentage(15),
            ])
            .split(chunks[1])[1];

            let msg_area = pop_up_area.inner(Margin {
                horizontal: 1,
                vertical: 1,
            });

            let msg = Paragraph::new(format!("Scanning:\n {path}")).centered();

            f.render_widget(&self.weights.scanning_block, pop_up_area);
            f.render_widget(msg, msg_area);
            f.render_widget(&self.weights.main_block, area);
        })?;
        Ok(())
    }
    fn main_draw(&mut self, current_dir: Arc<Mutex<File>>) -> Result<()> {
        self.terminal.draw(|f| {
            // 一定要确保传入的是一个 dir
            // 构建 list
            let guard = current_dir.lock().unwrap();
            let children = guard.children.clone().unwrap();
            let items = children
                .iter()
                .map(|child| {
                    let child = child.lock().unwrap();
                    ListItem::new(vec![
                        Line::raw(child.name().to_string()).left_aligned(),
                        Line::raw(child.size()).right_aligned(),
                    ])
                })
                .collect::<Vec<ListItem>>();
            let list = List::new(items)
                .block(
                    UI::MAIN_BLOCK
                        .title_top(Line::from(guard.name()).right_aligned())
                        .title_top(Line::from("Del TUI").left_aligned()),
                )
                .style(Style::new().on_dark_gray())
                .highlight_style(Style::new().bold().underlined().black().on_cyan())
                .highlight_symbol(">>");
            let area = f.area();
            f.render_stateful_widget(list, area, &mut self.list_state);
        })?;
        Ok(())
    }
}
#[derive(Debug, Clone)]
struct UI<'a> {
    main_block: Block<'a>,
    scanning_block: Block<'a>,
}
impl<'a> UI<'a> {
    const MAIN_BLOCK: Block<'static> =
        Block::bordered().border_type(ratatui::widgets::BorderType::Rounded);
    fn new() -> Self {
        let main_block = Self::MAIN_BLOCK.title_top(Line::from("Del TUI").left_aligned());
        let scanning_block = Self::MAIN_BLOCK
            .title_top(Line::from("Scanning...").left_aligned())
            .shadow(Shadow::dark_shade())
            .style(Style::new().bg(Color::DarkGray));
        Self {
            main_block,
            scanning_block,
        }
    }
}

fn listen_keyboard() -> Option<KeyCode> {
    if let Ok(Event::Key(key)) = event::read() {
        Some(key.code)
    } else {
        None
    }
}

pub fn main_loop(path: PathBuf) -> Result<()> {
    use KeyCode::*;
    let _g = Guard::new()?;

    let (tx, rx) = mpsc::channel();
    let _handle = std::thread::spawn(move || {
        let root = match File::new(&path, None, tx.clone()) {
            Ok(root) => SendMsg::Finished(root),
            Err(e) => SendMsg::Error(e),
        };
        let _ = tx.send(root);
    });

    let mut tui = Tui::new()?;
    let root = loop {
        let msg = match rx.recv()? {
            SendMsg::Working(s) => s,
            SendMsg::Finished(root) => break root,
            SendMsg::Error(e) => return Err(e),
        };
        tui.scanning_draw(&msg)?;
        if poll(Duration::ZERO)?
            && let Some(code) = listen_keyboard()
        {
            match code {
                Char('q') | Esc => return Ok(()),
                _ => {}
            }
        }
    };

    let mut current_dir = root.clone();

    loop {
        tui.main_draw(Arc::clone(&current_dir))?;
        if let Some(code) = listen_keyboard() {
            match code {
                Char('q') | Esc => return Ok(()),
                Up => tui.list_state.select_previous(),
                Down => tui.list_state.select_next(),
                Char('d') => {
                    let mut guard = current_dir.lock().unwrap();
                    let mut child =
                        Command::new(std::env::args().next().unwrap_or("del".to_string()))
                            .arg(
                                &guard.children.clone().unwrap()
                                    [tui.list_state.selected().unwrap()]
                                .lock()
                                .unwrap()
                                .path,
                            )
                            .spawn()?;
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    if let Some(children) = guard.children.as_mut() {
                        children.remove(tui.list_state.selected().unwrap());
                    }
                }
                Char('s') => {
                    let guard = current_dir.lock().unwrap();
                    let mut child =
                        Command::new(std::env::args().next().unwrap_or("del".to_string()))
                            .arg("-S")
                            .arg(
                                &guard.children.clone().unwrap()
                                    [tui.list_state.selected().unwrap()]
                                .lock()
                                .unwrap()
                                .path,
                            )
                            .spawn()?;
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Enter | Right => {
                    let togo = current_dir.lock().unwrap().children.clone().unwrap()
                        [tui.list_state.selected().unwrap()]
                    .clone();
                    if togo.lock().unwrap().file_type.is_dir() {
                        current_dir = togo;
                        tui.list_state.select(Some(0));
                    }
                }
                Left => {
                    if let Some(togo) = {
                        let guard = current_dir.lock().unwrap();
                        guard.parent.clone()
                    } && let Some(togo) = togo.upgrade()
                    {
                        current_dir = togo;
                        tui.list_state.select(Some(0));
                    }
                }
                _ => {}
            }
        }
    }
}
