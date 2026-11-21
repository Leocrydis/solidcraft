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
        app.drawing_workspace.note = text.map(|text| Draft { text, shown: None });
    }
    let (cancel, save) = super::inset(ui, |ui| {
        let mut valid = false;
        if let Some(draft) = app.drawing_workspace.note.as_mut() {
            let field = ui_kit::multiline_text_field(ui, tr("ui.drawing_panel.text"), ui.scope_id().with("note_text"), &mut draft.text);
            crate::scenario::publish_rect("drawing:note:text", field.rect);
            valid = ready(&draft.text);
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
