//! Placement and editing of sheet notes. The Text box is plain text: a field shows as its short name
//! in braces (`{mat}`, `{p.Length}`, `solidcraft_engine::doc::drawing::short_form`) and is saved back
//! as the field it names, so a note that holds fields opens and saves like any other.
use serde_json::json;
use solidcore_record::Vec2;
use solidcraft_engine::{
    doc::{drawing::short_form, ids},
    geom::cell::{MAX_TEXT_BYTES, Part},
};
use solidcraft_theme::Tokens;

use super::close_panel;
use crate::{
    SolveApp,
    ui_kit::{self, Button},
};

#[derive(Default)]
pub(super) struct Draft {
    text: String,
    /// The Text box's id, for its caret: where a field picked in the smart callout dialog goes.
    text_id: Option<egui::Id>,
    /// The smart callout dialog, while it is open (the Fields… button, or an unclosed `{`).
    fields: Option<super::fields_dialog::Dialog>,
    /// The preview last drawn: for this text, with the pointer here, on this sheet. Drawn again only
    /// when one of them changes.
    shown: Option<(String, Vec2, u64, Option<crate::drawing_sheet_page::Marks>)>,
}

/// How finely a preview's letters are flattened, on paper.
const PREVIEW_TOLERANCE_MM: f64 = 0.01;

fn tr(key: &'static str) -> &'static str {
    solidcraft_ui_text::tr(key)
}

fn ready(text: &str) -> bool {
    !text.trim().is_empty() && text.len() <= MAX_TEXT_BYTES && short_form::from_text(text).is_ok()
}

/// What `text`, typed in the Text box, gives a note's `content`: plain text when it holds no field
/// (as a note always was), else its parts, each `{short name}` the field it names.
fn content_of(text: &str) -> Result<serde_json::Value, String> {
    let content = short_form::from_text(text)?;
    let plain: Option<String> = content.0.iter().map(|part| if let Part::Text(text) = part { Some(text.as_str()) } else { None }).collect();
    if let Some(plain) = plain {
        return Ok(json!(plain));
    }
    serde_json::to_value(&content).map_err(|e| e.to_string())
}

pub(super) fn editor(app: &mut SolveApp, ui: &mut egui::Ui, selected: Option<u64>) {
    if app.drawing_workspace.note.is_none() {
        let text = match selected {
            None => Some(String::new()),
            Some(id) => app.session.drawing("drawing.inspect").ok().and_then(|drawing| Some(short_form::to_text(&drawing.notes.by_id(id)?.content))),
        };
        app.drawing_workspace.note = text.map(|text| Draft { text, ..Draft::default() });
    }
    let (cancel, save) = super::inset(ui, |ui| {
        let mut valid = false;
        if let Some(draft) = app.drawing_workspace.note.as_mut() {
            let id = ui.scope_id().with("note_text");
            let field = ui_kit::multiline_text_field(ui, tr("ui.drawing_panel.text"), id, &mut draft.text);
            crate::scenario::publish_rect("drawing:note:text", field.rect);
            draft.text_id = Some(id);
            valid = ready(&draft.text);
            // The one control beside the box: the smart callout dialog.
            let fields = ui_kit::row(ui, "", |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui_kit::button(ui, tr("ui.smart_callout.open"), Button::Normal))
                    .inner
            });
            crate::scenario::publish_rect("drawing:note:fields", fields.rect);
            if fields.clicked() {
                draft.fields.get_or_insert_with(Default::default);
            }
            if field.has_focus() && completing(&draft.text, caret(ui.ctx(), id)).is_some() {
                draft.fields.get_or_insert_with(Default::default);
            }
            if let Err(why) = short_form::from_text(&draft.text) {
                let message = solidcraft_ui_text::format("ui.drawing_note.unreadable", &[("why", &why)]);
                ui_kit::status_line(ui, &message, ui_kit::StatusTone::Warning);
            } else if selected.is_none() {
                ui_kit::text(ui, tr("ui.drawing_note.place"), ui_kit::Text::Note("drawing_note_place"));
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            let cancel = ui_kit::button(ui, tr("ui.cancel"), Button::Normal).clicked();
            let save = selected.is_some() && ui.add_enabled_ui(valid, |ui| ui_kit::button(ui, tr("ui.heading_ok"), Button::Normal)).inner.clicked();
            (cancel, save)
        })
        .inner
    });
    fields_dialog(app, ui.ctx(), selected);
    if cancel {
        close_panel(app);
    } else if save
        && let Some(id) = selected
        && let Some(draft) = app.drawing_workspace.note.as_ref()
    {
        let content = match content_of(&draft.text) {
            Ok(content) => content,
            Err(why) => return app.set_status(why, true),
        };
        let params = json!({"note": ids::encode(id), "set": {"content": content}});
        match app.run("drawing.note.edit", params) {
            Ok(_) => {
                close_panel(app);
            }
            Err(error) => app.set_status(error, true),
        }
    }
}

/// The caret in the Text box `id`, as a character index, while the box keeps one.
fn caret(ctx: &egui::Context, id: egui::Id) -> Option<usize> {
    egui::TextEdit::load_state(ctx, id).and_then(|state| state.cursor.char_range()).map(|range| range.primary.index.0)
}

/// What follows an unclosed `{` just before the caret (`{ma|` gives `ma`), with the brace's character
/// index: a short name being typed. A doubled `{{` is a brace, not a field.
fn completing(text: &str, caret: Option<usize>) -> Option<(usize, String)> {
    let before: Vec<char> = text.chars().take(caret?).collect();
    let open = before.iter().rposition(|c| *c == '{')?;
    let typed: String = before.get(open + 1..)?.iter().collect();
    let doubled = open > 0 && before.get(open - 1) == Some(&'{');
    (!doubled && !typed.contains(['}', '\n', ' '])).then_some((open, typed))
}

/// The smart callout dialog over the Note panel, while it is open: a field picked goes into the text
/// at the caret, replacing a short name being typed after an unclosed `{`, and the caret moves past it.
fn fields_dialog(app: &mut SolveApp, ctx: &egui::Context, selected: Option<u64>) {
    let Some(mut draft) = app.drawing_workspace.note.take() else { return };
    if let Some(mut dialog) = draft.fields.take() {
        let at = draft.text_id.and_then(|id| caret(ctx, id));
        let typing = completing(&draft.text, at);
        let (closed, picked) =
            super::fields_dialog::show(app, ctx, &mut dialog, &draft.text, selected, typing.as_ref().map(|(_, typed)| typed.as_str()));
        if let Some(short) = picked {
            let token = format!("{{{short}}}");
            let chars: Vec<char> = draft.text.chars().collect();
            let end = at.unwrap_or(chars.len()).min(chars.len());
            let start = typing.map_or(end, |(open, _)| open);
            let text: String = chars
                .get(..start)
                .into_iter()
                .flatten()
                .chain(token.chars().collect::<Vec<_>>().iter())
                .chain(chars.get(end..).into_iter().flatten())
                .collect();
            draft.text = text;
            if let Some(id) = draft.text_id {
                let after = start + token.chars().count();
                let mut state = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
                state.cursor.set_char_range(Some(egui::text::CCursorRange::one(egui::text::CCursor::new(after))));
                state.store(ctx, id);
                ctx.memory_mut(|memory| memory.request_focus(id));
            }
        }
        if !closed {
            draft.fields = Some(dialog);
        }
    }
    app.drawing_workspace.note = Some(draft);
}

pub(crate) fn on_sheet(app: &mut SolveApp, painter: &egui::Painter, at: crate::drawing_sheet_page::OnScreen, point: Vec2, sheet: u64, clicked: bool) {
    if !app.drawing_workspace.note.as_ref().is_some_and(|draft| ready(&draft.text)) {
        return;
    }
    let Some(mut draft) = app.drawing_workspace.note.take() else { return };
    let Ok(content) = content_of(&draft.text) else {
        app.drawing_workspace.note = Some(draft);
        return;
    };
    let params = json!({"anchor":{"sheet":ids::encode(sheet)}, "at":[point.x, point.y], "content":content});
    // The note as the sheet will draw it once placed: its own group, in the drawing's lettering, at
    // its paper size, in the accent colour as every preview on the sheet is.
    let fresh = draft.shown.as_ref().is_some_and(|(text, at, on, _)| *text == draft.text && *at == point && *on == sheet);
    if !fresh {
        let marks = solidcraft_engine::drawing_scene::note_preview_scene(&app.session, &params)
            .ok()
            .map(|scene| crate::drawing_sheet_page::Marks::of(&scene, 0, PREVIEW_TOLERANCE_MM));
        draft.shown = Some((draft.text.clone(), point, sheet, marks));
    }
    if let Some((.., Some(marks))) = &draft.shown {
        at.paint_preview(painter, marks, Vec2::ZERO, Tokens::get().accent);
    }
    app.drawing_workspace.note = Some(draft);
    if clicked {
        match app.run("drawing.note.add", params) {
            Ok(_) => {
                close_panel(app);
            }
            Err(error) => app.set_status(error, true),
        }
    }
}
