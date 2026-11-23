//! Spare Construct: text notes, text styles and leaders (CON-A9-F3-P1; the data half).
//!
//! A text note is a record in a document that belongs to one view: its lines of text, its anchor in
//! the view's own coordinates, its wrap width, alignment and rotation, a list style, the text
//! style it reads and its leaders. A text style is a record too: a name, a font by name, a size on
//! paper, bold, italic, underline, a colour and a line spacing. A note holds only its style's ID,
//! so changing a style changes every note that reads it. Map: projects/spare-construct/
//! ARCHITECTURE.md; the stored form is in FORMAT.md beside this crate and the choices are in
//! notes/CON-A9-F3-P1-text-notes.md.
//!
//! - [`TextStyle`]: a style, its name unique in the document, its size on paper above zero.
//! - [`TextNote`], [`Align`], [`ListKind`], [`Leader`], [`Arrow`]: a note and what it carries.
//! - [`Texts`]: a document's styles and notes, and the rules a command checks: a note in a view
//!   that is there reading a style that is there, a style in use never deleted, a view that holds
//!   notes never deleted.
//! - [`lay_out`], [`Laid`], [`LaidLine`], [`FontMetrics`], [`Face`]: a note's lines, their sizes
//!   and its box, worked from the record, the style and font metrics the caller supplies. Nothing
//!   about the layout is stored.
//! - [`TextError`]: every refusal, as a value.
//!
//! Promises: records are the truth (main's ruling 7), and this crate is the types and pure
//! functions over them; wiring into the edit log and the server is construct-api's `cloud::text`.
//! The same data gives the same bytes and the same answer on every machine and run; reading
//! refuses anything writing would not give; no length is held as a float; nothing panics. Depends
//! on construct-model (the record), construct-categories (the category), construct-params
//! (lengths, angles, stored values), kit-core (IDs) and unicode-linebreak (Apache-2.0, where a
//! line may break).
//!
//! Interim: the shared text engine `engine-text` has no code yet (font loading, shaping and
//! paragraph layout are its stated purpose), so the line breaking here is the Unicode algorithm's
//! and the font numbers come from a [`FontMetrics`] the caller supplies, never from a table here.
//! The piece that gives `engine-text` its code replaces [`FontMetrics`]' callers' sources with one
//! implementation over it; [`lay_out`] and every test written against it stay.
//!
//! Not built here, with their places kept: drawing text in a view or on a page, editing it in
//! the page, text on a sheet, dimensions, tags (job 8.3), keynotes (A9-F3-P2), spelling (A9-F3-P3).

mod error;
mod layout;
mod metrics;
mod note;
mod stored;
mod style;
mod texts;

pub use error::TextError;
pub use layout::{Laid, LaidLine, lay_out};
pub use metrics::{Face, FixedWidth, FontMetrics};
pub use note::{
    ALIGN_KEY, Align, Arrow, CATEGORY_KEY, LEADER_PREFIX, LIST_KEY, Leader, ListKind, MAX_CHARS,
    MAX_LEADER_POINTS, MAX_LEADERS, NOTE_KIND, NOTE_NAME, ROTATION_KEY, STYLE_KEY, TEXT_KEY, TURN,
    TextNote, VIEW_KEY, WIDTH_KEY, X_KEY, Y_KEY, note_recipe,
};
pub use style::{
    BOLD_KEY, COLOUR_KEY, DEFAULT_SPACING, FONT_KEY, ITALIC_KEY, MAX_NAME_LEN, MAX_SIZE, SIZE_KEY,
    SPACING_KEY, SPACING_RANGE, STYLE_KIND, TextStyle, UNDERLINE_KEY, style_recipe,
};
pub use texts::Texts;
