//! The one window: every document kind in the same tab strip.

use std::collections::HashMap;
use std::path::PathBuf;

use ely_engine::{DocId, EngineDyn, EngineKind, LokEngine, PdfEngine, Registry, Source, TextEngine};
use ely_gpui_component::{
    buttons::Button,
    navigation::{EditorTab, EditorTabs},
    shell::{StatusBar, StatusBarItem, Toolbar, ToolbarGroup},
};
use gpui::{
    Action, AnyView, App, AppContext, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, Menu, MenuItem, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div,
};

use crate::home::{Home, HomeEvent};
use crate::kinds;
use crate::placeholder::Placeholder;

/// One open document.
struct Tab {
    id: u64,
    name: SharedString,
    kind: EngineKind,
    /// The document inside its engine; none when the engine is not in the window.
    doc: Option<DocId>,
    view: AnyView,
    path: Option<PathBuf>,
}

/// The unified window.
pub struct Office {
   focus: FocusHandle,
   registry: Registry,
   engines: HashMap<&'static str, Box<dyn EngineDyn>>,
    /// The start page, when it is there.
    home: Option<Entity<Home>>,
    /// Keeps the start page's requests listened to.
    home_watch: Option<Subscription>,
    /// The worker of the document engine, when it started.
    lok: Option<std::sync::Arc<std::sync::Mutex<ely_lok::Worker>>>,
    /// Why the document engine did not start.
    lok_error: Option<String>,
    tabs: Vec<Tab>,
    active: Option<usize>,
    next: u64,
    untitled: u32,
    recent: Vec<PathBuf>,
    /// A message about what the window just did or refused to do.
    notice: Option<SharedString>,
    /// The window was asked to close while a document was unsaved.
    pending_close: bool,
    /// A tab was asked to close while it was unsaved.
    pending_tab: Option<u64>,
}

/// The menus of the one window: the commands it has, and only those.
fn menus(app: ely_platform::AppId, cx: &App) -> Vec<Menu> {
    let item = |name: SharedString, action: Box<dyn Action>| MenuItem::Action {
        name,
        action,
        os_action: None,
        checked: false,
        disabled: false,
    };
    let text = |key: &str| ely_ui::i18n::text(cx, key, &[]);
    vec![
        Menu {
            name: app.name().into(),
            items: vec![item(
                text("mpelyoffice.ribbon.new"),
                Box::new(ely_ui::commands::New),
            )],
            disabled: false,
        },
        Menu {
            name: text("mpelyoffice.menu.file"),
            items: vec![
                item(text("mpelyoffice.ribbon.new"), Box::new(ely_ui::commands::New)),
                item(
                    text("mpelyoffice.ribbon.open"),
                    Box::new(ely_ui::commands::Open),
                ),
                item(
                    text("mpelyoffice.ribbon.save"),
                    Box::new(ely_ui::commands::Save),
                ),
                item(
                    text("mpelyoffice.ribbon.save_as"),
                    Box::new(ely_ui::commands::SaveAs),
                ),
            ],
            disabled: false,
        },
    ]
}

/// Builds the window and opens what the shell handed over.
pub fn open(app: ely_platform::AppId, window: &mut Window, cx: &mut App) -> Entity<Office> {
    cx.set_menus(menus(app, cx));
    ely_ui::commands::bind_keys(cx);
    // The document engine starts with the window, as the other applications
    // do; when it cannot start, the window says so instead of failing
    // later on a file.
    let (lok, lok_error) = match ely_office::start(app, cx) {
        Ok((worker, _work)) => (Some(worker), None),
        Err(error) => {
            log::warn!("office: the document engine did not start: {error}");
            (None, Some(error.to_string()))
        }
    };
    let files = ely_ui::launch_files(cx);
    let office = cx.new(|cx| Office::new(lok, lok_error, cx));
    // The start page, with the files of late.
    let data =
        ely_platform::data_dir(app).unwrap_or_else(|_| std::env::temp_dir().join("MPelyoffice"));
    let home = cx.new(|cx| Home::new(data, window, cx));
    office.update(cx, |office, cx| {
        office.home = Some(home.clone());
        office.home_watch = Some(cx.subscribe(&home, |office, _, event, cx| match event {
            HomeEvent::Open(path) => office.open_path(path.clone(), cx),
            HomeEvent::Pick => office.pick(cx),
            HomeEvent::New(kind) => office.new_document(*kind, cx),
            }));
    });
    ely_ui::on_open(cx, {
        let office = office.clone();
        move |path, cx| {
            office.update(cx, |office, cx| office.open_path(path, cx));
        }
    });
    for path in files {
        office.update(cx, |office, cx| office.open_path(path, cx));
    }
    let closing = office.clone();
    window.on_window_should_close(cx, move |_, cx| {
        closing.update(cx, |office, cx| office.should_close(cx))
    });
    office
}

impl Office {
    fn new(
        lok: Option<std::sync::Arc<std::sync::Mutex<ely_lok::Worker>>>,
        lok_error: Option<String>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut engines: HashMap<&'static str, Box<dyn EngineDyn>> = HashMap::new();
        engines.insert("text", Box::new(TextEngine::text()));
        engines.insert("markdown", Box::new(TextEngine::markdown()));
        engines.insert("pdf", Box::new(PdfEngine::new()));
        Self {
            focus: cx.focus_handle(),
            registry: Registry::new(),
            home: None,
        home_watch: None,
                engines,
            lok,
            lok_error,
            tabs: Vec::new(),
            active: None,
            next: 0,
            untitled: 0,
            recent: Vec::new(),
            notice: None,
            pending_close: false,
            pending_tab: None,
        }
    }

    /// The engine for a kind, made when the document engine is there.
    fn engine_for(&mut self, kind: EngineKind) -> Option<&mut Box<dyn EngineDyn>> {
        if self.engines.contains_key(kind.id) {
            return self.engines.get_mut(kind.id);
        }
        let worker = self.lok.clone()?;
        let engine = LokEngine::new(kind, worker);
        self.engines.insert(kind.id, Box::new(engine));
        self.engines.get_mut(kind.id)
    }

    /// The engine for a kind, if the window has it.
    fn engine(&mut self, kind: EngineKind) -> Option<&mut Box<dyn EngineDyn>> {
        self.engines.get_mut(kind.id)
    }

    /// Opens a file in a tab of its own.
    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let Some(kind) = self.registry.kind_for(&path) else {
            self.notice = Some(format!("{} 暂不支持。", path.display()).into());
            cx.notify();
            return;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        let source = Source::File(path.clone());
        self.push(kind, name, Some(path), source, cx);
    }

    /// Opens an empty document of `kind`.
    fn new_document(&mut self, kind: EngineKind, cx: &mut Context<Self>) {
        self.untitled += 1;
        let name = format!("{} {}", kinds::label(kind, cx), self.untitled);
        self.push(kind, name, None, Source::Blank, cx);
    }

    fn push(
        &mut self,
        kind: EngineKind,
        name: String,
        path: Option<PathBuf>,
        source: Source,
        cx: &mut Context<Self>,
    ) {
        let id = self.next;
        self.next += 1;
        let (doc, view) = match self.engine_for(kind) {
            Some(engine) => match engine.open(source, cx) {
                Ok(opened) => (Some(opened.id), opened.view),
                Err(error) => {
                    log::error!("office: {} did not open: {error:#}", kind.id);
                    let reason = format!("{error:#}");
                    self.notice = Some(reason.clone().into());
                    let placeholder = cx.new(|cx| {
                        Placeholder::new(
                            kinds::label(kind, cx),
                            path.clone(),
                            kinds::module(kind),
                            Some(reason),
                            cx,
                        )
                    });
                    (None, placeholder.into())
                }
            },
            None => {
                let reason = self.lok_error.clone();
                let placeholder = cx.new(|cx| {
                    Placeholder::new(
                        kinds::label(kind, cx),
                        path.clone(),
                        kinds::module(kind),
                        reason,
                        cx,
                    )
                });
                (None, placeholder.into())
            }
        };
        self.tabs.push(Tab {
            id,
            name: name.into(),
            kind,
            doc,
            view,
            path,
        });
        self.active = Some(self.tabs.len() - 1);
        cx.notify();
    }

    /// The tab a command acts on.
    fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|index| self.tabs.get(index))
    }

    /// Brings a tab to the front.
    fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(index) = self.tabs.iter().position(|tab| tab.id.to_string() == id) {
            self.active = Some(index);
            cx.notify();
        }
    }

    /// Closes a tab; a tab with changes asks first.
    fn close(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id.to_string() == id) else {
            return;
        };
        if self.changed(index, cx) {
            self.pending_tab = Some(self.tabs[index].id);
            cx.notify();
            return;
        }
        self.drop_tab(index, cx);
    }

    /// Removes a tab without asking.
    fn drop_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(doc) = self.tabs[index].doc {
            if let Some(engine) = self.engine(self.tabs[index].kind) {
                engine.close(doc, cx);
            }
        }
        self.tabs.remove(index);
        if self.active == Some(index) {
            self.active = if self.tabs.is_empty() {
                None
            } else {
                Some(index.min(self.tabs.len() - 1))
            };
        }
        cx.notify();
    }

    /// Whether a tab has changes that are not saved.
    fn changed(&self, index: usize, cx: &gpui::App) -> bool {
        let Some(tab) = self.tabs.get(index) else {
            return false;
        };
        let Some(doc) = tab.doc else {
            return false;
        };
        self.engines
            .get(tab.kind.id)
            .map(|engine| engine.modified(doc, cx))
            .unwrap_or(false)
    }

    /// Whether the window may close; a document with changes stops it.
    ///
    /// A second request to close does not let the window through either:
    /// only the buttons in the notice do, and they say what happens to the
    /// changes.
    fn should_close(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.tabs.iter().enumerate().any(|(index, _)| self.changed(index, cx)) {
            return true;
        }
        self.pending_close = true;
        self.notice = Some(ely_ui::i18n::text(cx, "mpelyoffice.close.window", &[]));
        cx.notify();
        false
    }

    /// Saves every changed document; returns how many were written.
    fn save_all(&mut self, cx: &mut Context<Self>) -> usize {
        let mut written = 0;
        for index in 0..self.tabs.len() {
            if self.save_at(index, cx) {
                written += 1;
            }
        }
        written
    }

    /// Writes the active document back to its file.
    fn save(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.active else {
            return;
        };
        self.save_at(index, cx);
    }

    /// Asks for another name, then writes the active document there.
    fn save_as(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.active else {
            return;
        };
        let Some(tab) = self.tabs.get(index).map(|tab| (tab.kind, tab.name.to_string())) else {
            return;
        };
        let (kind, name) = tab;
        let label = kinds::label(kind, cx).to_string();
        cx.spawn(
            async move |this, cx| {
                let picked = cx
                    .background_executor()
                    .spawn(async move {
                        rfd::FileDialog::new()
                            .set_file_name(&name)
                            .add_filter(
                                &label,
                                &kind.extensions.iter().copied().collect::<Vec<_>>(),
                            )
                            .save_file()
                    })
                    .await;
                let Some(path) = picked else {
                    return;
                };
                let _ = this.update(cx, |office, cx| office.save_as_to(index, path, cx));
            },
        )
        .detach();
    }

    /// Writes one document to another file, and points the tab at it.
    fn save_as_to(&mut self, index: usize, path: PathBuf, cx: &mut Context<Self>) {
        if self.tabs.get(index).is_none() {
            return;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.name = name.into();
            tab.path = Some(path.clone());
        }
        self.save_at(index, cx);
    }

    /// Writes one document back; false when it was not written.
    fn save_at(&mut self, index: usize, cx: &mut Context<Self>) -> bool {
        let Some(tab) = self.tabs.get(index) else {
            return false;
        };
        let (kind, doc, path) = (tab.kind, tab.doc, tab.path.clone());
        let Some(doc) = doc else {
            return false;
        };
        let Some(path) = path else {
            self.notice = Some(ely_ui::i18n::text(cx, "mpelyoffice.notice.no_name", &[]));
            cx.notify();
            return false;
        };
        let Some(engine) = self.engines.get_mut(kind.id) else {
            return false;
        };
        match engine.write(doc, &path, cx) {
            Ok(()) => {
                self.remember(&path);
                self.notice = Some(ely_ui::i18n::text(
                    cx,
                    "mpelyoffice.notice.saved",
                    &[("name", &path.display().to_string())],
                ));
                cx.notify();
                true
            }
            Err(error) => {
                log::error!("office: {} was not saved: {error}", path.display());
                let reason = error.to_string();
                let message = if reason.contains("changed on disk") {
                    ely_ui::i18n::text(
                        cx,
                        "mpelyoffice.notice.external",
                        &[("name", &path.display().to_string())],
                    )
                } else {
                    ely_ui::i18n::text(cx, "mpelyoffice.notice.failed", &[("error", &reason)])
                };
                self.notice = Some(message);
                cx.notify();
                false
            }
        }
    }

    /// Remembers a file for the start page.
    fn remember(&mut self, path: &std::path::Path) {
        self.recent.retain(|known| known != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(10);
    }

    /// The tabs for the strip.
    fn tab_bar(&self, cx: &mut Context<Self>) -> EditorTabs {
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                EditorTab::new(tab.id.to_string(), tab.name.clone())
                    .icon(kinds::mark(tab.kind))
                    .dirty(self.changed(index, cx))
            })
            .collect::<Vec<_>>();
        let selected = self
            .active_tab()
            .map(|tab| tab.id.to_string())
            .unwrap_or_default();
        let select = cx.entity();
        let close = select.clone();
        EditorTabs::new("mpelyoffice-tabs", tabs)
            .selected(selected)
            .on_select(move |id, _, cx| {
                select.update(cx, |office, cx| office.select(id, cx));
            })
            .on_close(move |id, _, cx| {
                close.update(cx, |office, cx| office.close(id, cx));
            })
    }

    /// The bar under the document.
    fn status_bar(&self, cx: &mut Context<Self>) -> StatusBar {
        let state = match self.active_tab() {
            None => ely_ui::i18n::text(cx, "mpelyoffice.status.none", &[]),
            Some(tab) => {
                let index = self.active.unwrap_or_default();
                // A document that is still opening says so.
                let loading = tab
                    .doc
                    .and_then(|document| {
                        self.engines
                            .get(tab.kind.id)
                            .map(|engine| engine.is_loading(document, cx))
                    })
                    .unwrap_or(false);
                if loading {
                    ely_ui::i18n::text(cx, "mpelyoffice.status.loading", &[])
                } else {
                let changed = if self.changed(index, cx) {
                    ely_ui::i18n::text(cx, "mpelyoffice.status.modified", &[]).to_string()
                } else {
                    ely_ui::i18n::text(cx, "mpelyoffice.status.saved", &[]).to_string()
                };
                let where_ = tab
                    .path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| {
                        ely_ui::i18n::text(cx, "mpelyoffice.status.unsaved", &[]).to_string()
                    });
                let kind_label = kinds::label(tab.kind, cx).to_string();
                let encoding = match (tab.doc, self.engines.get(tab.kind.id)) {
                    (Some(document), Some(engine)) => engine.encoding(document),
                    _ => None,
                }
                .map(|encoding| encoding.name().to_string())
                .unwrap_or_default();
                ely_ui::i18n::text(
                    cx,
                    "mpelyoffice.status.state",
                    &[
                        ("kind", &kind_label),
                        ("state", &changed),
                        ("encoding", &encoding),
                        ("where", &where_),
                    ],
                )
                }
            }
        };
        let count = ely_ui::i18n::text(
            cx,
            "mpelyoffice.status.count",
            &[("count", &self.tabs.len().to_string())],
        );
        StatusBar::new()
            .left(StatusBarItem::new("mpelyoffice-state").label(state))
            .right(StatusBarItem::new("mpelyoffice-count").label(count))
    }

    /// The row of commands, in the manner of an office suite.
    fn ribbon(&self, cx: &mut Context<Self>) -> Toolbar {
        let new = cx.entity();
        let open = new.clone();
        let save = new.clone();
        let save_as = new.clone();
        let text_kind = self.registry.kind("text").unwrap_or_else(|| office_kind());
        let label = |key: &str| ely_ui::i18n::text(cx, key, &[]);
        let new_group = ToolbarGroup::new().child(
            Button::new("mpelyoffice-new", label("mpelyoffice.ribbon.new")).on_click(move |_, _, cx| {
                new.update(cx, |office, cx| office.new_document(text_kind, cx));
            }),
        );
        let open_group = ToolbarGroup::new().child(
            Button::new("mpelyoffice-open", label("mpelyoffice.ribbon.open")).on_click(move |_, _, cx| {
                open.update(cx, |office, cx| office.pick(cx));
            }),
        );
        let save_group = ToolbarGroup::new().child(
            Button::new("mpelyoffice-save", label("mpelyoffice.ribbon.save")).on_click(move |_, _, cx| {
                save.update(cx, |office, cx| office.save(cx));
            }),
        );
        let save_as_group = ToolbarGroup::new().child(
            Button::new("mpelyoffice-save-as", label("mpelyoffice.ribbon.save_as"))
                .on_click(move |_, _, cx| {
                    save_as.update(cx, |office, cx| office.save_as(cx));
                }),
        );
        Toolbar::new()
            .child(new_group)
            .child(open_group)
            .child(save_group)
            .child(save_as_group)
    }

    /// The area under the ribbon.
    fn body(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        match self.active_tab() {
            Some(tab) => tab.view.clone().into_any_element(),
            None => match &self.home {
                Some(home) => home.clone().into_any_element(),
                None => self.start_page(cx).into_any_element(),
            },
        }
    }

    /// The page with no document: what to open, and what was open.
    fn start_page(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity();
        let mut new = div().flex().flex_wrap().gap_2();
        for kind in self.registry.kinds() {
            let this = this.clone();
            let kind = *kind;
            let label = kinds::label(kind, cx);
            new = new.child(
                Button::new(format!("mpelyoffice-new-{}", kind.id), label).on_click(
                    move |_, _, cx| {
                        this.update(cx, |office, cx| office.new_document(kind, cx));
                    },
                ),
            );
        }
        let mut recent = div().flex().flex_col().gap_1();
        for path in &self.recent {
            let this = this.clone();
            let path = path.clone();
            recent = recent.child(
                Button::new(
                    format!("mpelyoffice-recent-{}", path.display()),
                    path.display().to_string(),
                )
                .on_click(move |_, _, cx| {
                    this.update(cx, |office, cx| office.open_path(path.clone(), cx));
                }),
            );
        }
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .child(
                div()
                    .text_lg()
                    .child(ely_ui::i18n::text(cx, "mpelyoffice.start.title", &[])),
            )
            .child(
                div()
                    .text_sm()
                    .child(ely_ui::i18n::text(cx, "mpelyoffice.start.new", &[])),
            )
            .child(new)
            .child(
                div()
                    .text_sm()
                    .child(ely_ui::i18n::text(cx, "mpelyoffice.start.recent", &[])),
            )
            .child(recent)
    }

    /// The bar that says what the window just did.
    fn notice_bar(&self, _cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let notice = self.notice.clone()?;
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .child(div().text_sm().child(notice))
                .into_any_element(),
        )
    }

    /// The bar that asks what to do about unsaved changes.
    fn close_notice(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let this = cx.entity();
        let label = |key: &str| ely_ui::i18n::text(cx, key, &[]);
        if let Some(id) = self.pending_tab {
            let save = this.clone();
            let discard = this.clone();
            let cancel = this.clone();
            return Some(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2()
                    .child(div().text_sm().child(label("mpelyoffice.close.tab")))
                    .child(
                        Button::new("mpelyoffice-save-tab", label("mpelyoffice.close.save"))
                            .on_click(move |_, _, cx| {
                                save.update(cx, |office, cx| {
                                    if let Some(index) =
                                        office.tabs.iter().position(|tab| tab.id == id)
                                    {
                                        office.save_at(index, cx);
                                    }
                                    office.pending_tab = None;
                                    if let Some(index) =
                                        office.tabs.iter().position(|tab| tab.id == id)
                                    {
                                        office.drop_tab(index, cx);
                                    }
                                });
                            }),
                    )
                    .child(
                        Button::new("mpelyoffice-discard-tab", label("mpelyoffice.close.discard"))
                            .on_click(move |_, _, cx| {
                                discard.update(cx, |office, cx| {
                                    office.pending_tab = None;
                                    if let Some(index) =
                                        office.tabs.iter().position(|tab| tab.id == id)
                                    {
                                        office.drop_tab(index, cx);
                                    }
                                });
                            }),
                    )
                    .child(
                        Button::new("mpelyoffice-cancel-tab", label("mpelyoffice.close.cancel"))
                            .on_click(move |_, _, cx| {
                                cancel.update(cx, |office, cx| {
                                    office.pending_tab = None;
                                    cx.notify();
                                });
                            }),
                    )
                    .into_any_element(),
            );
        }
        if !self.pending_close {
            return None;
        }
        let save = this.clone();
        let discard = this.clone();
        let cancel = this.clone();
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_2()
                .child(div().text_sm().child(label("mpelyoffice.close.window")))
                .child(
                    Button::new("mpelyoffice-save-close", label("mpelyoffice.close.save"))
                        .on_click(move |_, _, cx| {
                            save.update(cx, |office, cx| {
                                office.save_all(cx);
                                office.pending_close = false;
                                cx.quit();
                            });
                        }),
                )
                .child(
                    Button::new("mpelyoffice-discard-close", label("mpelyoffice.close.discard"))
                        .on_click(move |_, _, cx| {
                            discard.update(cx, |office, cx| {
                                office.pending_close = false;
                                cx.quit();
                            });
                        }),
                )
                .child(
                    Button::new("mpelyoffice-cancel-close", label("mpelyoffice.close.cancel"))
                        .on_click(move |_, _, cx| {
                            cancel.update(cx, |office, cx| {
                                office.pending_close = false;
                                cx.notify();
                            });
                        }),
                )
                .into_any_element(),
        )
    }

    /// Asks the system for a file and opens it.
    fn pick(&mut self, cx: &mut Context<Self>) {
        cx.spawn(
            async move |this, cx| {
                let picked = cx
                    .background_executor()
                    .spawn(async move {
                        rfd::FileDialog::new()
                            .add_filter(
                                "文档",
                                &[
                                    "txt", "md", "markdown", "docx", "odt", "xlsx", "ods", "csv",
                                    "pptx", "odp", "pdf",
                                ],
                            )
                            .pick_file()
                    })
                    .await;
                let Some(path) = picked else {
                    return;
                };
                let _ = this.update(cx, |office, cx| office.open_path(path, cx));
            },
        )
        .detach();
    }
}

/// The kind a new document gets when the shell asks for one.
fn office_kind() -> EngineKind {
    EngineKind {
        id: "text",
        extensions: &["txt", "log", "ini", "conf"],
        default_extension: "txt",
    }
}

impl Render for Office {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tabs = self.tab_bar(cx);
        let status = self.status_bar(cx);
        let ribbon = self.ribbon(cx);
        let body = self.body(cx);
        let notice = self.close_notice(cx);
        let message = self.notice_bar(cx);
        let mut shell = ely_ui::Shell::new("MPelyoffice", &self.focus)
            .tabs(tabs)
            .status(status)
            .child(ribbon)
            .child(body);
        if let Some(message) = message {
            shell = shell.child(message);
        }
        if let Some(notice) = notice {
            shell = shell.child(notice);
        }
        // A file that is dropped in opens by its own kind.
        let dropped = cx.entity();
        div()
            .size_full()
            .on_drop::<Vec<PathBuf>>(move |paths, _, cx| {
                for path in paths {
                    let _ = dropped.update(cx, |office, cx| office.open_path(path.clone(), cx));
                }
            })
            .on_action(cx.listener(|office, _: &ely_ui::commands::New, _, cx| {
                let kind = office
                    .registry
                    .kind("text")
                    .unwrap_or_else(|| office_kind());
                office.new_document(kind, cx);
            }))
            .on_action(cx.listener(|office, _: &ely_ui::commands::Open, _, cx| office.pick(cx)))
            .on_action(cx.listener(|office, _: &ely_ui::commands::Save, _, cx| office.save(cx)))
            .on_action(cx.listener(|office, _: &ely_ui::commands::SaveAs, _, cx| {
                office.save_as(cx)
            }))
            .child(shell)
    }
}
