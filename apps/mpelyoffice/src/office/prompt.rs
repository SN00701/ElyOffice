//! The bars that tell and ask: what the window just did, and what to do
//! about the changes that are not saved.

use ely_gpui_component::buttons::Button;
use gpui::{Context, IntoElement, ParentElement, Styled, div};

use super::Office;

impl Office {
    /// The bar that says what the window just did.
    pub(super) fn notice_bar(&self, _cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
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
    pub(super) fn close_notice(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
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

}