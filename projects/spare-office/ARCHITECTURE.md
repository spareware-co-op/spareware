# Spare Office: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

Documents, spreadsheets, slides, email and calendar, chat and video calls, notes, forms and file storage, replacing Microsoft 365 with apps that are free to use and owned by their members. Everything saves in open, published formats and reads and writes Microsoft's formats from their published standards, so nobody is locked in. Several people can edit the same file at once through the shared kit's sync.

## The apps

Our products use their own names. Microsoft's names appear only to say what each replaces.

| Spare app | Replaces | What it does |
| --- | --- | --- |
| Spare Write | Word | Documents, styles, track changes, comments |
| Spare Sheets | Excel | Spreadsheets, formulas, charts, pivot tables |
| Spare Slides | PowerPoint | Presentations, themes, presenter view |
| Spare Mail | Outlook | Email, calendar and contacts on open protocols |
| Spare Chat | Teams | Chat, channels, video calls and meetings |
| Spare Notes | OneNote | Notebooks, sections and pages, handwriting |
| Spare Drive | OneDrive, SharePoint | File storage, sharing, version history |
| Spare Forms | Forms | Forms and surveys feeding into Sheets |

## What it uses from the shared kit

Accounts (one login for every app), sync and history (live co-editing and version history), storage, messaging and notifications, plugins and the MCP server (so AI agents can read and edit documents), and the shared engines: text, PDF, 2D drawing and rendering. Spare Mail and Spare Chat also use the open protocols below.

## Its own parts

### Documents, sheets and slides

**High level:** one document core shared by Write, Sheets and Slides: styled text, tables, images, comments, track changes and live co-editing.

**Low level:** every edit is an operation (kit-ops), so co-editing, undo and history share one path; text co-editing uses a text CRDT where the server-ordered log isn't enough (see Spare Arch's multi-user section); Sheets needs a formula engine compatible with common spreadsheet functions (piece OF06).

### File formats

**High level:** our own files use the OpenDocument standard; Word, Excel and PowerPoint files are read and written faithfully.

**Low level:** OpenDocument is ISO/IEC 26300; Office Open XML (.docx, .xlsx, .pptx) is ECMA-376 and ISO/IEC 29500. Both are published standards, so reading and writing them meets our legal rules. Which patent promises cover them is part of piece OF02.

### Mail and calendar

**High level:** works with any email provider and with the co-op's own mail service.

**Low level:** IMAP and SMTP for mail, JMAP where servers support it, CalDAV and CardDAV for calendars and contacts: all open protocols.

### Chat and meetings

**High level:** chat, channels, and video calls with screen sharing.

**Low level:** the Matrix protocol (an open standard) for chat and WebRTC for calls are the proven options (piece OF05).

### Drive

**High level:** files stored and shared at cost, with version history and offline sync.

## Learn from (licences to confirm in piece OF01)

| Project | What it is | How we can use it |
| --- | --- | --- |
| ArtCraft's WordCraft, GridCraft and DeckCraft | Rust clones of Word, Excel and PowerPoint (Apache-2.0) | Partner: their author has been invited to the Spareware umbrella |
| LibreOffice, Collabora Online | Mature office suites (MPL-2.0) | Learn from and possibly reuse under MPL terms |
| OnlyOffice, Nextcloud, CryptPad | Office and file storage (AGPL) | Learn from only |
| Thunderbird | Mail and calendar (MPL-2.0) | Learn from and possibly reuse |
| Jitsi | Video meetings (Apache-2.0) | Possible reuse |

## The hard parts

- Opening complex Word and Excel files exactly as Microsoft does (layout, macros, edge-case formulas).
- Live co-editing that never loses work.
- Businesses moving their whole email and calendar across.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Partner or build | Partner with ArtCraft's office apps; build on LibreOffice's engine; build our own | Research first (piece OF01) |
| Our file format | OpenDocument; our own | OpenDocument |
| Chat protocol | Matrix; our own on the kit | Research first (piece OF05) |
| First app | Write; Sheets; Mail | Research first (piece OF07) |

The work board is [STATUS.md](STATUS.md).
