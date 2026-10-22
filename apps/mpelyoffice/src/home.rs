//! The start page: what is new, what is open, and the files of late.
//!
//! The page follows the shape of an office suite's home: the name and a
//! search box at the top, a panel with new and open on the left, and the
//! files of late in the middle with their place, author, time and size.

mod sheet;

use std::path::PathBuf;

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

use crate::files::{self, Entry};
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
    pub(super) newing: bool,
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
        let search = cx.new(|cx| {
            TextInput::new(window, cx).placeholder(ely_ui::i18n::text(cx, "mpelyoffice.home.search", &[]).to_string())
        });
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
        self.recent = files::read_list(&self.recent_file(), &self.registry);
        self.starred = files::read_list(&self.starred_file(), &self.registry);
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

impl Render for Home {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let text = |key: &str| ely_ui::i18n::text(cx, key, &[]).to_string();
        let shown = self.shown();
        let (recent, earlier): (Vec<Entry>, Vec<Entry>) = shown
            .iter()
            .cloned()
            .partition(|entry| files::is_recent(&entry.path));

        // The narrow column with the mark of the program in it.
        let rail = div()
            .flex()
            .flex_col()
            .w(px(56.0))
            .items_center()
            .py_4()
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
                        .color(theme.colors.on_accent),
                    ),
            );

        // The panel: what to make, and the lists of the files.
        let this = cx.entity();
        let new_this = this.clone();
        let open_this = this.clone();
        let panel = div()
            .w(px(200.0))
            .flex()
            .flex_col()
            .gap_1()
            .py_4()
            .pr_3()
            .child(
                Button::new("home-new", text("mpelyoffice.home.new"))
                    .primary()
                    .full_width()
                    .on_click(move |_, _, cx| {
                        // The sheet asks what kind of document to make.
                        let _ = new_this.update(cx, |home, _| home.newing = true);
                    }),
            )
            .child(
                Button::new("home-open", text("mpelyoffice.home.open"))
                    .full_width()
                    .on_click(move |_, _, cx| {
                        let _ = open_this.update(cx, |_, cx| cx.emit(HomeEvent::Pick));
                    }),
            )
            .child(div().h(px(12.0)))
            .child(self.nav_item(Section::Recent, text("mpelyoffice.home.recent"), this.clone(), &theme))
            .child(self.nav_item(Section::Starred, text("mpelyoffice.home.starred"), this.clone(), &theme))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.fg_subtle)
                    .mt_3()
                    .mb_1()
                    .px_2()
                    .child(text("mpelyoffice.home.local")),
            )
            .child(self.places(cx, &theme));

        // The name of the program and the field to search in.
        let head = div()
            .flex()
            .items_center()
            .gap_6()
            .child(div().text_xl().child(text("mpelyoffice.start.title")))
            .child(
                // A column, so the field takes the width it is given.
                div()
                    .w(px(480.0))
                    .flex()
                    .flex_col()
                    .child(Input::new(&self.search)),
            );

        // The files of late, in two groups: the ones of this week and the
        // ones before.
        let mut list = div().flex().flex_col();
        if !recent.is_empty() {
            list = list.child(
                div()
                    .text_lg()
                    .mt_6()
                    .mb_2()
                    .child(text("mpelyoffice.home.group.recent"))
                    .child(self.columns(cx, &theme)),
            );
            for entry in recent.iter() {
                list = list.child(self.row(entry, cx, &theme));
            }
        }
        if !earlier.is_empty() {
            list = list.child(
                div()
                    .text_lg()
                    .mt_6()
                    .mb_2()
                    .child(text("mpelyoffice.home.group.earlier"))
                    .child(self.columns(cx, &theme)),
            );
            for entry in earlier.iter() {
                list = list.child(self.row(entry, cx, &theme));
            }
        }
        if shown.is_empty() {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(theme.colors.fg_subtle)
                    .mt_6()
                    .child(if self.query.is_empty() {
                        text("mpelyoffice.home.empty")
                    } else {
                        text("mpelyoffice.home.not_found")
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
            .gap_6()
            .child(head)
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
            return div().size_full().child(page).child(self.new_sheet(cx, &theme));
        }
        page
    }
}

impl Home {
    /// One entry of the left panel.
    fn nav_item(
        &self,
        section: Section,
        label: String,
        this: Entity<Home>,
        theme: &ely_gpui_component::theme::Theme,
    ) -> impl IntoElement {
        let active = self.section == section;
        let mut item = div()
            .id(format!("home-nav-{section:?}"))
            .text_sm()
            .cursor_pointer()
            .px_2()
            .py_1()
            .rounded_md();
        if active {
            item = item.bg(theme.colors.active).font_weight(gpui::FontWeight::SEMIBOLD);
        } else {
            item = item.hover(|style| style.bg(theme.colors.hover));
        }
        item.on_click(move |_, _, cx| {
            let _ = this.update(cx, |home, _| home.section = section);
        })
        .child(label)
    }

    /// The folders of the local places.
    fn places(&self, cx: &Context<Self>, theme: &ely_gpui_component::theme::Theme) -> impl IntoElement {
        let text = |key: &str| ely_ui::i18n::text(cx, key, &[]).to_string();
        let names = [
            ("mpelyoffice.home.desktop", "Desktop"),
            ("mpelyoffice.home.documents", "Documents"),
            ("mpelyoffice.home.downloads", "Downloads"),
        ];
        div()
            .flex()
            .flex_col()
            .gap_1()
            .children(files::local_places().into_iter().map(|(name, path)| {
                let this = cx.entity();
                let folder_name = name.clone();
                let search = self.search.clone();
                let place = path.clone();
                let label = names
                    .iter()
                    .find(|(_, known)| *known == folder_name)
                    .map(|(key, _)| text(key))
                    .unwrap_or_else(|| name.clone());
                div()
                    .id(format!("home-place-{name}"))
                    .text_sm()
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .hover(|style| style.bg(theme.colors.hover))
                    .on_click(move |_, _, cx| {
                        let text = place.display().to_string();
                        let _ = this.update(cx, |home, _| home.query = text.clone());
                        search.update(cx, |input, cx| input.set_text(text, cx));
                    })
                    .child(label)
            }))
    }

    /// The column heads of the list.
    fn columns(&self, cx: &Context<Self>, theme: &ely_gpui_component::theme::Theme) -> impl IntoElement {
        let text = |key: &str| ely_ui::i18n::text(cx, key, &[]).to_string();
        div()
            .flex()
            .gap_2()
            .text_xs()
            .text_color(theme.colors.fg_subtle)
            .px_2()
            .child(div().w(px(220.0)))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(text("mpelyoffice.home.column.place")),
            )
            .child(div().w(px(100.0)).child(text("mpelyoffice.home.column.author")))
            .child(div().w(px(120.0)).child(text("mpelyoffice.home.column.changed")))
            .child(div().w(px(60.0)).child(text("mpelyoffice.home.column.size")))
    }

    /// One file of the list.
    fn row(&self, entry: &Entry, cx: &Context<Self>, theme: &ely_gpui_component::theme::Theme) -> impl IntoElement {
        let path = entry.path.clone();
        let this = cx.entity();
        let name = entry.name.clone();
        let place = entry.place.clone();
        let author = entry.author.clone();
        let changed = entry.changed.clone();
        let size = entry.size.clone();
        let mark = kinds::mark(entry.kind);
        div()
            .id(format!("home-row-{name}"))
            .flex()
            .items_center()
            .gap_2()
            .py_1()
            .px_2()
            .rounded_md()
            .cursor_pointer()
            .hover(|style| style.bg(theme.colors.hover))
            .on_click(move |_, _, cx| {
                let _ = this.update(cx, |_, cx| cx.emit(HomeEvent::Open(path.clone())));
            })
            .child(Icon::new(mark))
            .child(div().w(px(220.0)).truncate().child(name))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(theme.colors.fg_subtle)
                    .child(place),
            )
            .child(div().w(px(100.0)).truncate().text_xs().child(author))
            .child(div().w(px(120.0)).truncate().text_xs().child(changed))
            .child(div().w(px(60.0)).text_xs().child(size))
    }
}
