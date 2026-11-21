//! A field's short form: what a person types in braces in a note's text, and how a note editor shows
//! a field (`{mat}`, `{p.Length}`). SolidCraft's own, short and lowercase, a dot for "of". Every
//! field a note may hold has exactly one short name, and each short name names exactly one field, so
//! a note's content goes to text and back unchanged ([`to_text`], [`from_text`]). `{{` and `}}` are a
//! brace each, as in a dimension's text template. A `row` field (a parts list's cell) has none.
//! A person's own aliases (`{THK}` for `{p.Thickness}`) are theirs, kept by the app: [`expand`]
//! writes each as the short name it stands for before the text is read, so a note saves the field,
//! never the alias.
//!
//! | Short name | Field |
//! |---|---|
//! | `{part}`, `{pn}`, `{desc}`, `{rev}`, `{mat}`, `{mass}` | the component's `name`, `part_number`, `description`, `revision`, `material`, `mass` |
//! | `{part.<name>}` | the component's `properties.<name>` |
//! | `{p.<Name>}` | `parameter.<Name>`: a design parameter, its name as written |
//! | `{dwg}`, `{dwg.no}`, `{org}`, `{author}`, `{project}` | the drawing's `name`, `number`, `organization`, `author`, `project` |
//! | `{dwg.<key>}` | the drawing's `properties.<key>` |
//! | `{sheet}`, `{sheet.no}`, `{sheet.of}`, `{scale}` | the sheet's `name`, `number`, `count`, `scale` |
//! | `{r.rev}`, `{r.desc}`, `{r.by}`, `{r.chk}`, `{r.date}` | the newest revision's `revision`, `description`, `designer`, `checker`, `date` |
//! | `{ask.<key>}` | `prompt.<key>` |

use solidcraft_geom::cell::{Content, Format, Part, Source};

/// A field with a fixed short name: the short name and the field's source and key. What it reads, in
/// words, is the app's to say, in the user's language.
pub struct Fixed {
    pub short: &'static str,
    pub source: Source,
    pub key: &'static str,
}

/// Every field with a fixed short name, in the order a list shows them.
pub const FIXED: &[Fixed] = &[
    Fixed { short: "part", source: Source::Component, key: "name" },
    Fixed { short: "pn", source: Source::Component, key: "part_number" },
    Fixed { short: "desc", source: Source::Component, key: "description" },
    Fixed { short: "rev", source: Source::Component, key: "revision" },
    Fixed { short: "mat", source: Source::Component, key: "material" },
    Fixed { short: "mass", source: Source::Component, key: "mass" },
    Fixed { short: "dwg", source: Source::Drawing, key: "name" },
    Fixed { short: "dwg.no", source: Source::Drawing, key: "number" },
    Fixed { short: "org", source: Source::Drawing, key: "organization" },
    Fixed { short: "author", source: Source::Drawing, key: "author" },
    Fixed { short: "project", source: Source::Drawing, key: "project" },
    Fixed { short: "sheet", source: Source::Sheet, key: "name" },
    Fixed { short: "sheet.no", source: Source::Sheet, key: "number" },
    Fixed { short: "sheet.of", source: Source::Sheet, key: "count" },
    Fixed { short: "scale", source: Source::Sheet, key: "scale" },
    Fixed { short: "r.rev", source: Source::Revision, key: "revision" },
    Fixed { short: "r.desc", source: Source::Revision, key: "description" },
    Fixed { short: "r.by", source: Source::Revision, key: "designer" },
    Fixed { short: "r.chk", source: Source::Revision, key: "checker" },
    Fixed { short: "r.date", source: Source::Revision, key: "date" },
];

/// Before a name: a short name that reads a named thing of its source.
const PARAMETER: &str = "p.";
const PART_PROPERTY: &str = "part.";
const DRAWING_PROPERTY: &str = "dwg.";
const PROMPT: &str = "ask.";
/// Before a property's name in a field's key.
const PROPERTIES: &str = "properties.";

/// The short name of `source`'s field `key`, without its braces; none for a `row` field or a key
/// with no short form (an empty name).
pub fn short_name(source: Source, key: &str) -> Option<String> {
    if let Some(fixed) = FIXED.iter().find(|fixed| fixed.source == source && fixed.key == key) {
        return Some(fixed.short.to_string());
    }
    let named = |prefix: &str, name: &str| (!name.is_empty()).then(|| format!("{prefix}{name}"));
    match source {
        Source::Parameter => named(PARAMETER, key),
        Source::Component => named(PART_PROPERTY, key.strip_prefix(PROPERTIES)?),
        Source::Drawing => named(DRAWING_PROPERTY, key.strip_prefix(PROPERTIES)?),
        Source::Prompt => named(PROMPT, key),
        Source::Sheet | Source::Revision | Source::Row => None,
    }
}

/// The field short name `name` (without its braces) names; none when it names none.
pub fn field_of(name: &str) -> Option<(Source, String)> {
    if let Some(fixed) = FIXED.iter().find(|fixed| fixed.short == name) {
        return Some((fixed.source, fixed.key.to_string()));
    }
    let named = |prefix: &str| name.strip_prefix(prefix).filter(|rest| !rest.is_empty());
    if let Some(parameter) = named(PARAMETER) {
        return Some((Source::Parameter, parameter.to_string()));
    }
    if let Some(property) = named(PART_PROPERTY) {
        return Some((Source::Component, format!("{PROPERTIES}{property}")));
    }
    // `dwg.no` is fixed and was found above; any other `dwg.` name is a property.
    if let Some(property) = named(DRAWING_PROPERTY) {
        return Some((Source::Drawing, format!("{PROPERTIES}{property}")));
    }
    named(PROMPT).map(|key| (Source::Prompt, key.to_string()))
}

/// `content` as a note editor shows it: its text with each brace doubled, each field as its short
/// name in braces. A field with no short name (a `row` field) is shown as its source and key, which
/// [`from_text`] then refuses, so it can't be saved back changed by accident.
pub fn to_text(content: &Content) -> String {
    let mut out = String::new();
    for part in &content.0 {
        match part {
            Part::Text(text) => out.push_str(&text.replace('{', "{{").replace('}', "}}")),
            Part::Field { source, key, .. } => match short_name(*source, key) {
                Some(short) => out.push_str(&format!("{{{short}}}")),
                None => out.push_str(&format!("{{{}.{key}}}", source_word(*source))),
            },
        }
    }
    out
}

/// `text` as a note's content: `{{` and `}}` a brace each, `{name}` the field `name` is the short name
/// of, every other character text. Text next to text is one part. An unknown short name, a `}`
/// alone or a `{` left open is refused, naming it.
pub fn from_text(text: &str) -> Result<Content, String> {
    let mut parts = Vec::new();
    let mut run = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                run.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                run.push('}');
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some('{') | None => return Err(format!("`{{{name}` has no closing `}}`")),
                        Some(c) => name.push(c),
                    }
                }
                let (source, key) = field_of(name.trim()).ok_or_else(|| format!("`{{{name}}}` is not a field"))?;
                if !run.is_empty() {
                    parts.push(Part::Text(std::mem::take(&mut run)));
                }
                parts.push(Part::Field { source, key, format: Format::default() });
            }
            '}' => return Err("a `}` has no opening `{` (type `}}` for a brace)".to_string()),
            c => run.push(c),
        }
    }
    if !run.is_empty() {
        parts.push(Part::Text(run));
    }
    Ok(Content(parts))
}

/// The most aliases one person keeps.
pub const MAX_ALIASES: usize = 200;
/// The longest an alias may be, in characters.
pub const MAX_ALIAS_CHARS: usize = 32;

/// Why an alias can't stand for a field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasRefused {
    /// Empty, longer than [`MAX_ALIAS_CHARS`], or with a character other than a letter, a digit,
    /// `.`, `_` or `-`.
    Written,
    /// A built-in short name: it keeps its own meaning.
    BuiltIn,
    /// What it stands for is no field's short name.
    NotAField,
}

/// Why `alias` can't stand for the field short name `target` names, when it can't.
pub fn alias_refused(alias: &str, target: &str) -> Option<AliasRefused> {
    let alias = alias.trim();
    let written =
        !alias.is_empty() && alias.chars().count() <= MAX_ALIAS_CHARS && alias.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !written {
        return Some(AliasRefused::Written);
    }
    if field_of(alias).is_some() || field_of(&alias.to_lowercase()).is_some() {
        return Some(AliasRefused::BuiltIn);
    }
    field_of(target.trim().trim_start_matches('{').trim_end_matches('}')).is_none().then_some(AliasRefused::NotAField)
}

/// `text` with each `{alias}` of `aliases` (alias to the short name it stands for, matched ignoring
/// case) written as the short name it stands for, so [`from_text`] reads it as that field. A built-in
/// short name, a doubled brace and anything else stay as typed.
pub fn expand(text: &str, aliases: &std::collections::BTreeMap<String, String>) -> String {
    if aliases.is_empty() || !text.contains('{') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(brace) = after.strip_prefix('{') {
            out.push_str("{{");
            rest = brace;
            continue;
        }
        let Some(close) = after.find(['}', '{']).filter(|at| after[*at..].starts_with('}')) else {
            out.push('{');
            rest = after;
            continue;
        };
        let name = after[..close].trim();
        let target = (field_of(name).is_none())
            .then(|| aliases.iter().find(|(alias, _)| alias.eq_ignore_ascii_case(name) || alias.to_lowercase() == name.to_lowercase()))
            .flatten()
            .map(|(_, target)| target.as_str());
        match target {
            Some(target) => out.push_str(&format!("{{{target}}}")),
            None => out.push_str(&rest[open..open + 1 + close + 1]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// A source as a field names it: `component`, `row` and so on.
fn source_word(source: Source) -> String {
    serde_json::to_value(source).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}
