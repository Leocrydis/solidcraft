//! The smart callout dialog: the fields a note can read, to pick from. A search box, each field's
//! name in words with its short name (`{mat}`, `{p.Length}`, `short_form`), and what the note will
//! read. Picking a field puts it into the Note panel's text at its caret; the Note panel stays a
//! plain text box (its Fields… button opens this, and typing an unclosed `{` opens it narrowed to
//! what follows the brace).
use solidcraft_engine::doc::drawing::short_form::{self, FIXED};
use solidcraft_engine::geom::cell::Source;

use crate::{SolveApp, ui_kit};

/// The dialog's own choices while it is open.
#[derive(Default)]
pub(super) struct Dialog {
    pub(super) search: String,
    /// Every parameter of the design, not only its user parameters and favourites.
    pub(super) all: bool,
    /// An alias being made: its text, and the short name of the field it will stand for.
    pub(super) alias: String,
    pub(super) target: String,
}

/// One field the list offers: its section, its name in words and its short name.
struct Entry {
    section: Section,
    label: String,
    short: String,
}

/// The list's sections, in order. The feature a note points at goes first once feature fields exist.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Part,
    Parameters,
    Drawing,
    Sheet,
    Revision,
    /// The user's own aliases.
    Aliases,
}

impl Section {
    const ALL: [Section; 6] = [Section::Part, Section::Parameters, Section::Drawing, Section::Sheet, Section::Revision, Section::Aliases];

    fn of(source: Source) -> Option<Section> {
        match source {
            Source::Component => Some(Section::Part),
            Source::Parameter => Some(Section::Parameters),
            Source::Drawing => Some(Section::Drawing),
            Source::Sheet => Some(Section::Sheet),
            Source::Revision => Some(Section::Revision),
            Source::Prompt | Source::Row => None,
        }
    }

    fn heading(self) -> &'static str {
        solidcraft_ui_text::tr(match self {
            Section::Part => "ui.smart_callout.part",
            Section::Parameters => "ui.smart_callout.parameters",
            Section::Drawing => "ui.smart_callout.drawing",
            Section::Sheet => "ui.smart_callout.sheet",
            Section::Revision => "ui.smart_callout.revision",
            Section::Aliases => "ui.smart_callout.aliases",
        })
    }
}

/// A fixed field's name in words, in the user's language.
fn label(short: &str) -> &'static str {
    solidcraft_ui_text::tr(match short {
        "part" => "ui.smart_callout.field.part",
        "pn" => "ui.smart_callout.field.pn",
        "desc" => "ui.smart_callout.field.desc",
        "rev" => "ui.smart_callout.field.rev",
        "mat" => "ui.smart_callout.field.mat",
        "mass" => "ui.smart_callout.field.mass",
        "dwg" => "ui.smart_callout.field.dwg",
        "dwg.no" => "ui.smart_callout.field.dwg_no",
        "org" => "ui.smart_callout.field.org",
        "author" => "ui.smart_callout.field.author",
        "project" => "ui.smart_callout.field.project",
        "sheet" => "ui.smart_callout.field.sheet",
        "sheet.no" => "ui.smart_callout.field.sheet_no",
        "sheet.of" => "ui.smart_callout.field.sheet_of",
        "scale" => "ui.smart_callout.field.scale",
        "r.rev" => "ui.smart_callout.field.r_rev",
        "r.desc" => "ui.smart_callout.field.r_desc",
        "r.by" => "ui.smart_callout.field.r_by",
        "r.chk" => "ui.smart_callout.field.r_chk",
        _ => "ui.smart_callout.field.r_date",
    })
}

/// The sheet a note in the panel is on, or will be placed on: its index in the open drawing.
fn sheet_at(app: &SolveApp, note: Option<u64>) -> Option<usize> {
    let drawing = app.session.drawing("drawing.inspect").ok()?;
    match note.and_then(|id| drawing.notes.by_id(id)) {
        Some(note) => drawing.sheet_of(&note.anchor),
        None => {
            let active = crate::drawing_sheet_page::active_sheet(app)?;
            drawing.sheets.iter().position(|sheet| sheet.id == active)
        }
    }
}

/// Every field the list offers for the note's sheet: the fixed ones, then the design's parameters
/// (its user parameters and favourites, or with `all` every one), each with its short name.
fn entries(app: &SolveApp, at: Option<usize>, all: bool) -> Vec<Entry> {
    let mut out: Vec<Entry> = FIXED
        .iter()
        .filter_map(|fixed| {
            Some(Entry { section: Section::of(fixed.source)?, label: label(fixed.short).to_string(), short: fixed.short.to_string() })
        })
        .collect();
    if let (Ok(drawing), Some(at)) = (app.session.drawing("drawing.inspect"), at) {
        for listed in solidcraft_engine::title_block_text::parameters_listed(&app.session, drawing, at, all) {
            let Some(short) = short_form::short_name(Source::Parameter, &listed.name) else { continue };
            let label = if listed.favourite { format!("{} \u{2605}", listed.name) } else { listed.name };
            out.push(Entry { section: Section::Parameters, label, short });
        }
    }
    for (alias, target) in &app.preferences.smart_callout_aliases {
        out.push(Entry { section: Section::Aliases, label: format!("{alias} \u{2192}"), short: target.clone() });
    }
    out
}

/// What the alias row says is wrong with the alias being made, in the user's language.
fn refusal(why: short_form::AliasRefused) -> &'static str {
    solidcraft_ui_text::tr(match why {
        short_form::AliasRefused::Written => "ui.smart_callout.alias_written",
        short_form::AliasRefused::BuiltIn => "ui.smart_callout.alias_builtin",
        short_form::AliasRefused::NotAField => "ui.smart_callout.alias_not_a_field",
    })
}

/// Whether `entry` matches what was searched: its name in words or its short name, ignoring case.
fn matches(entry: &Entry, search: &str) -> bool {
    let search = search.trim().to_lowercase();
    search.is_empty() || entry.label.to_lowercase().contains(&search) || entry.short.to_lowercase().contains(&search)
}

/// Draw the dialog for the Note panel's `text` (note `note`, or one being placed). `completing` is
/// what follows an unclosed `{` at the caret, when there is one: the list narrows to it and the
/// search box shows it. Returns whether to close, the short name of the field picked, and the
/// user's aliases when they changed. A field clicked while an alias is being typed is what the alias
/// will stand for, not a pick.
pub(super) fn show(
    app: &SolveApp,
    ctx: &egui::Context,
    dialog: &mut Dialog,
    text: &str,
    note: Option<u64>,
    completing: Option<&str>,
) -> (bool, Option<String>, Option<std::collections::BTreeMap<String, String>>) {
    let mut aliases = None;
    let tr = solidcraft_ui_text::tr;
    let at = sheet_at(app, note);
    let entries = entries(app, at, dialog.all);
    let search = completing.map_or(dialog.search.clone(), str::to_string);
    let reads = match short_form::from_text(&short_form::expand(text, &app.preferences.smart_callout_aliases)) {
        Ok(content) => match (app.session.drawing("drawing.inspect"), at) {
            (Ok(drawing), Some(at)) => Some(solidcraft_engine::title_block_text::note_preview(&app.session, drawing, at, &content)),
            _ => None,
        },
        Err(why) => Some(solidcraft_ui_text::format("ui.drawing_note.unreadable", &[("why", &why)])),
    };
    let mut picked = None;
    let closed = ui_kit::Window::new(egui::Id::unique("sc_smart_callout"), "drawing_note", tr("ui.smart_callout.title")).show_body(ctx, |ui| {
        ui_kit::scope(ui, |ui| {
            let mut typed = search.clone();
            let field = ui_kit::search_field(ui, egui::Id::unique("sc_smart_callout_search"), &mut typed, tr("ui.smart_callout.search"));
            crate::scenario::publish_rect("smart_callout:search", field.rect);
            if completing.is_none() {
                dialog.search = typed;
            }
            let shown: Vec<&Entry> = entries.iter().filter(|entry| matches(entry, &search)).collect();
            egui::ScrollArea::vertical().id_salt("sc_smart_callout_list").max_height(Tokens::get().control_height * 12.0).show(ui, |ui| {
                for section in Section::ALL {
                    let rows: Vec<&&Entry> = shown.iter().filter(|entry| entry.section == section).collect();
                    if section == Section::Parameters {
                        ui.horizontal(|ui| {
                            ui_kit::section_heading(ui, section.heading());
                        });
                        ui_kit::checkbox(ui, tr("ui.smart_callout.all"), &mut dialog.all);
                    } else if rows.is_empty() {
                        continue;
                    } else {
                        ui_kit::section_heading(ui, section.heading());
                    }
                    for entry in rows {
                        let row = format!("{}    {{{}}}", entry.label, entry.short);
                        if entry.section == Section::Aliases {
                            let alias = entry.label.trim_end_matches(" \u{2192}").to_string();
                            if ui_kit::list_row(ui, &format!("smart_callout:alias:{alias}"), &row).clicked() {
                                let mut kept = app.preferences.smart_callout_aliases.clone();
                                kept.remove(&alias);
                                aliases = Some(kept);
                            }
                        } else if ui_kit::list_picker(ui, &format!("smart_callout:{}", entry.short), &row, false).clicked() {
                            if dialog.alias.trim().is_empty() {
                                picked = Some(entry.short.clone());
                            } else {
                                dialog.target = entry.short.clone();
                            }
                        }
                    }
                }
            });
            // The one row that makes an alias: its text, the field it stands for, Add.
            let why = short_form::alias_refused(&dialog.alias, &dialog.target);
            let full = app.preferences.smart_callout_aliases.len() >= short_form::MAX_ALIASES;
            for (key, label, value, hint) in [
                ("smart_callout:alias", tr("ui.smart_callout.alias"), &mut dialog.alias, "THK"),
                ("smart_callout:alias_for", tr("ui.smart_callout.alias_for"), &mut dialog.target, "p.Thickness"),
            ] {
                let top = ui.cursor().top();
                ui_kit::text_row(ui, label, value, hint);
                crate::scenario::publish_rect(key, egui::Rect::from_x_y_ranges(ui.max_rect().x_range(), top..=ui.cursor().top()));
            }
            if ui_kit::action_row(ui, "smart_callout_add_alias", tr("ui.smart_callout.alias_add")) && why.is_none() && !full {
                let mut kept = app.preferences.smart_callout_aliases.clone();
                kept.insert(dialog.alias.trim().to_string(), dialog.target.trim().trim_start_matches('{').trim_end_matches('}').to_string());
                aliases = Some(kept);
                dialog.alias.clear();
                dialog.target.clear();
            }
            if let Some(why) = why.filter(|_| !dialog.alias.trim().is_empty() || !dialog.target.trim().is_empty()) {
                ui_kit::status_line(ui, refusal(why), ui_kit::StatusTone::Dim);
            }
            ui.separator();
            if let Some(reads) = &reads {
                ui_kit::value_row(ui, tr("ui.smart_callout.reads"), reads);
            }
        });
    });
    (closed, picked, aliases)
}

use solidcraft_theme::Tokens;
