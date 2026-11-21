//! A field in a note's or a title block cell's content may read a design parameter:
//! `{"field": {"source": "parameter", "key": "Length"}}`, the parameter's value in its own unit.
//! An older drawing has no such field and reads as before; nothing is rewritten as it opens. The
//! last release meets a field source it doesn't know: it warns that the drawing is from a newer
//! format, opens what it can read and keeps the rest (`crates/doc/src/newer_file.rs`).
