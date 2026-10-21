//! The chrome of the window: the menus, the tab strip, the status bar and the ribbon.

use ely_gpui_component::{
    buttons::Button,
    navigation::{EditorTab, EditorTabs},
    shell::{StatusBar, StatusBarItem, Toolbar, ToolbarGroup},
};
use gpui::{Action, App, Context, Menu, MenuItem, ParentElement, SharedString};

use super::{Office, office_kind};
use crate::kinds;

/// The menus of the one window: the commands it has, and only those.
pub(super) fn menus(app: ely_platform::AppId, cx: &App) -> Vec<Menu> {
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

impl Office {
    /// The tabs for the strip.
    pub(super) fn tab_bar(&self, cx: &mut Context<Self>) -> EditorTabs {
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
    pub(super) fn status_bar(&self, cx: &mut Context<Self>) -> StatusBar {
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
    pub(super) fn ribbon(&self, cx: &mut Context<Self>) -> Toolbar {
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
}
