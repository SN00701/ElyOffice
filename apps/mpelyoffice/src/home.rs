//! The start page: what is new, what is open, and the files of late.
//!
//! The page follows the shape of an office suite's home: the name and a
//! search box at the top, a panel with new and open on the left, and the
//! files of late in the middle with their place, author, time and size.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ely_gpui_component::{
    buttons::Button,
    forms::{Input, InputEvent, TextInput},
    layout::ScrollArea,
    primitives::Icon,
    theme::ActiveTheme,
};
use gpui::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement, Render, StatefulInteractiveElement as _, Styled, Subscription, Window, div, px,
};

use crate::kinds;
use ely_engine::{EngineKind, Registry};

/// What the left panel shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Recent,
    Starred,
}

/// What the page asks the window to do.
pub enum HomeEvent {
    /// Open this file.
    Open(PathBuf),
    /// Ask for a file to open.
    Pick,
    /// Make a new document of this kind.
    New(EngineKind),
}

/// A file the page shows.
#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub kind: EngineKind,
    /// Where the file is.
    pub place: String,
    /// Who made it; the local user, since there is no cloud here.
    pub author: String,
    /// When it was last changed.
    pub changed: String,
    /// How large it is.
    pub size: String,
}

/// The start page.
pub struct Home {
    registry: Registry,
    /// The files of late, newest first.
    recent: Vec<Entry>,
    /// The files the user marked.
    starred: Vec<Entry>,
    /// What is searched for.
    query: String,
    /// What the left panel shows.
    section: Section,
    /// Whether the new-document sheet is open.
    newing: bool,
    /// Where the lists are kept.
    data: PathBuf,
    /// The search box.
    search: Entity<TextInput>,
    /// Keeps the search box's changes listened to.
    search_watch: Option<Subscription>,
}

impl Home {
    /// The page, with its lists read from the data folder.
    pub fn new(data: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new(window, cx).placeholder("搜索文件和技巧…"));
        let mut home = Self {
            registry: Registry::new(),
            recent: Vec::new(),
            starred: Vec::new(),
            query: String::new(),
            section: Section::Recent,
            newing: false,
            data,
            search,
            search_watch: None,
        };
        home.reload();
        // Every keystroke in the search box narrows the list.
        home.search_watch = Some(cx.subscribe(&home.search, |home, _, event, cx| {
            if matches!(event, InputEvent::Changed) {
                home.query = home.search.read(cx).text().to_string();
                cx.notify();
            }
        }));
        home
    }

    /// The list of the files of late.
    fn recent_file(&self) -> PathBuf {
        self.data.join("recent.json")
    }

    /// The list of the files the user marked.
    fn starred_file(&self) -> PathBuf {
        self.data.join("starred.json")
    }

    /// Reads the lists again.
    pub fn reload(&mut self) {
        self.recent = read_list(&self.recent_file(), &self.registry);
        self.starred = read_list(&self.starred_file(), &self.registry);
    }

    /// The files the page shows, after the search.
    fn shown(&self) -> Vec<Entry> {
        let list = match self.section {
            Section::Recent => &self.recent,
            Section::Starred => &self.starred,
        };
        if self.query.is_empty() {
            return list.clone();
        }
        let query = self.query.to_lowercase();
        list.iter()
            .filter(|entry| entry.name.to_lowercase().contains(&query))
            .cloned()
            .collect()
    }
}

impl EventEmitter<HomeEvent> for Home {}

/// Reads a list of files and describes them.
fn read_list(path: &Path, registry: &Registry) -> Vec<Entry> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let Ok(paths) = serde_json::from_slice::<Vec<PathBuf>>(&bytes) else {
        return Vec::new();
    };
    paths.iter().map(|path| describe(path, registry)).collect()
}

/// What the page shows of one file.
fn describe(path: &Path, registry: &Registry) -> Entry {
    let metadata = std::fs::metadata(path).ok();
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let kind = registry.kind_for(path).unwrap_or(EngineKind {
        id: "unknown",
        extensions: &[],
        default_extension: "",
    });
    let place = path
        .parent()
        .map(|parent| parent.display().to_string())
        .unwrap_or_default();
    let changed = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .map(show_time)
        .unwrap_or_else(|| "—".to_string());
    let size = metadata
        .as_ref()
        .map(|metadata| show_size(metadata.len()))
        .unwrap_or_else(|| "—".to_string());
    Entry {
        path: path.to_path_buf(),
        name,
        kind,
        place,
        author: ely_platform::user::named("").unwrap_or_default(),
        changed,
        size,
    }
}

/// A time, as a date and a clock.
fn show_time(time: SystemTime) -> String {
    let since = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = since / 86_400;
    let seconds = since % 86_400;
    format!(
        "{}-{:02}-{:02} {:02}:{:02}",
        days / 365 + 1970,
        (days % 365) / 30 + 1,
        days % 30 + 1,
        seconds / 3600,
        (seconds % 3600) / 60
    )
}

/// A size, as a reader reads it.
fn show_size(bytes: u64) -> String {
    const UNIT: u64 = 1024;
    if bytes < UNIT {
        return format!("{bytes} B");
    }
    let mut size = bytes as f64;
    let mut unit = "KB";
    for next in ["MB", "GB", "TB"] {
        size /= UNIT as f64;
        unit = next;
        if size < UNIT as f64 {
            break;
        }
    }
    format!("{size:.1} {unit}")
}

/// The folders of the local places.
fn local_places() -> Vec<(String, PathBuf)> {
    let mut places = Vec::new();
    if let Ok(home) = std::env::var("USERPROFILE") {
        let home = PathBuf::from(home);
        for (name, folder) in [
            ("桌面", home.join("Desktop")),
            ("文档", home.join("Documents")),
            ("下载", home.join("Downloads")),
        ] {
            if folder.is_dir() {
                places.push((name.to_string(), folder));
            }
        }
    }
    places
}

impl Render for Home {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let shown = self.shown();
        let (recent, earlier): (Vec<Entry>, Vec<Entry>) = shown
            .iter()
            .cloned()
            .partition(|entry| is_recent(&entry.path));

        let rail = div()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(48.0))
            .items_center()
            .child(
                div()
                    .size(px(32.0))
                    .rounded_md()
                    .bg(theme.colors.accent)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(kinds::mark(EngineKind {
                            id: "document",
                            extensions: &[],
                            default_extension: "",
                        }))
                        .color(gpui::white()),
                    ),
            );

        let this = cx.entity();
        let new_this = this.clone();
        let open_this = this.clone();
        let panel = div()
            .w(px(220.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .child(
                Button::new("home-new", "＋ 新建")
                    .primary()
                    .full_width()
                    .on_click(move |_, _, cx| {
                        // The sheet asks what kind of document to make.
                        let _ = new_this.update(cx, |home, _| home.newing = true);
                    }),
            )
            .child(
                Button::new("home-open", "打开").full_width().on_click(move |_, _, cx| {
                    let _ = open_this.update(cx, |_, cx| cx.emit(HomeEvent::Pick));
                }),
            )
            .child(self.nav_item(Section::Recent, "最近", this.clone()))
            .child(self.nav_item(Section::Starred, "星标", this.clone()))
            .child(
                div()
                    .text_sm()
                    .mt_2()
                    .child("本地")
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .mt_1()
                            .children(local_places().into_iter().map(|(name, path)| {
                                let this = this.clone();
                                let search = self.search.clone();
                                let place = path.clone();
                                div()
                                    .id(format!("home-place-{name}"))
                                    .text_sm()
                                    .cursor_pointer()
                                    .on_click(move |_, _, cx| {
                                        let text = place.display().to_string();
                                        let _ = this.update(cx, |home, _| home.query = text.clone());
                                        search.update(cx, |input, cx| input.set_text(text, cx));
                                    })
                                    .child(name)
                            })),
                    ),
            );

        let mut list = div().flex().flex_col();
        if !recent.is_empty() {
            list = list.child(
                div()
                    .text_lg()
                    .mt_4()
                    .mb_2()
                    .child("最近")
                    .child(self.columns()),
            );
            for entry in recent.iter() {
                list = list.child(self.row(entry, cx));
            }
        }
        if !earlier.is_empty() {
            list = list.child(
                div()
                    .text_lg()
                    .mt_4()
                    .mb_2()
                    .child("更早")
                    .child(self.columns()),
            );
            for entry in earlier.iter() {
                list = list.child(self.row(entry, cx));
            }
        }
        if shown.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .mt_4()
                    .child(if self.query.is_empty() {
                        "还没有打开过文件。"
                    } else {
                        "没有找到这个文件。"
                    }),
            );
        }

        let body = ScrollArea::new("home-files")
            .vertical()
            .child(div().flex().flex_col().p_4().child(list));

        let page = div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().text_xl().child("MPelyoffice"))
                    .child(
                        // A column, so the field takes the width it is given.
                        div()
                            .w(px(520.0))
                            .flex()
                            .flex_col()
                            .child(Input::new(&self.search)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(rail)
                    .child(panel)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(body),
                    ),
            );

        if self.newing {
            return div().size_full().child(page).child(self.new_sheet(cx));
        }
        page
    }
}

impl Home {
    /// One entry of the left panel.
    fn nav_item(&self, section: Section, label: &'static str, this: Entity<Home>) -> impl IntoElement {
        let active = self.section == section;
        let mut item = div()
            .id(format!("home-nav-{section:?}"))
            .text_sm()
            .cursor_pointer();
        if active {
            item = item.font_weight(gpui::FontWeight::SEMIBOLD);
        }
        item.on_click(move |_, _, cx| {
            let _ = this.update(cx, |home, _| home.section = section);
        })
        .child(label)
    }

    /// The column heads of the list.
    fn columns(&self) -> impl IntoElement {
        div()
            .flex()
            .gap_4()
            .text_xs()
            .child(div().flex_1().child("文件位置"))
            .child(div().w(px(120.0)).child("创建者"))
            .child(div().w(px(140.0)).child("最近修改"))
            .child(div().w(px(70.0)).child("大小"))
    }

    /// One file of the list.
    fn row(&self, entry: &Entry, cx: &mut Context<Self>) -> impl IntoElement {
        let path = entry.path.clone();
        let this = cx.entity();
        let name = entry.name.clone();
        div()
            .id(format!("home-row-{name}"))
            .flex()
            .items_center()
            .gap_2()
            .py_1()
            .cursor_pointer()
            .on_click(move |_, _, cx| {
                let _ = this.update(cx, |_, cx| cx.emit(HomeEvent::Open(path.clone())));
            })
            .child(Icon::new(kinds::mark(entry.kind)))
            .child(div().w(px(280.0)).truncate().child(entry.name.clone()))
            .child(div().text_xs().child("本地"))
            .child(div().flex_1().truncate().text_xs().child(entry.place.clone()))
            .child(div().w(px(120.0)).truncate().text_xs().child(entry.author.clone()))
            .child(div().w(px(140.0)).truncate().text_xs().child(entry.changed.clone()))
            .child(div().w(px(70.0)).text_xs().child(entry.size.clone()))
    }

    /// The sheet that asks what kind of document to make.
    fn new_sheet(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let asked: [(&'static str, EngineKind); 6] = [
            (
                "空白文档",
                EngineKind {
                    id: "document",
                    extensions: &["docx", "odt", "doc", "rtf"],
                    default_extension: "docx",
                },
            ),
            (
                "空白表格",
                EngineKind {
                    id: "spreadsheet",
                    extensions: &["xlsx", "ods", "xls", "csv", "tsv"],
                    default_extension: "xlsx",
                },
            ),
            (
                "空白演示",
                EngineKind {
                    id: "presentation",
                    extensions: &["pptx", "odp", "ppt"],
                    default_extension: "pptx",
                },
            ),
            (
                "PDF 文档",
                EngineKind {
                    id: "pdf",
                    extensions: &["pdf"],
                    default_extension: "pdf",
                },
            ),
            (
                "Markdown 文档",
                EngineKind {
                    id: "markdown",
                    extensions: &["md", "markdown"],
                    default_extension: "md",
                },
            ),
            (
                "文本文件",
                EngineKind {
                    id: "text",
                    extensions: &["txt", "log", "ini", "conf"],
                    default_extension: "txt",
                },
            ),
        ];
        let this = cx.entity();
        let mut grid = div().flex().flex_wrap().gap_3();
        for (label, kind) in asked {
            let this = this.clone();
            grid = grid.child(
                Button::new(format!("home-new-{}", kind.id), label).on_click(move |_, _, cx| {
                    let _ = this.update(cx, |home, cx| {
                        home.newing = false;
                        cx.emit(HomeEvent::New(kind));
                    });
                }),
            );
        }
        div()
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black().opacity(0.35))
            .child(
                div()
                    .w(px(520.0))
                    .rounded_lg()
                    .bg(gpui::white())
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(div().text_lg().child("新建"))
                    .child(grid)
                    .child(
                        Button::new("home-new-cancel", "取消").on_click(move |_, _, cx| {
                            let _ = this.update(cx, |home, _| home.newing = false);
                        }),
                    ),
            )
    }
}

/// Whether a file was changed in the last week.
fn is_recent(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(changed) = metadata.modified() else {
        return false;
    };
    let week = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs().saturating_sub(7 * 86_400))
        .unwrap_or(0);
    changed
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs() >= week)
        .unwrap_or(false)
}
