# Spare Research: everything to build

*Stage: Mapping. This list is written from [features.json](features.json), which holds the same map as data. The architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

Spare Research replaces journal paywalls and the fees researchers pay to publish openly. Research that is free to read and free to publish, with open peer review and permanent links for citing. Papers are posted, screened, reviewed in the open and gathered into journals that editors run on top. It hosts only papers the authors have the right to share, keeps every version, and shows corrections and retractions openly. People, scripts and AI agents all use the same commands. This map works top-down so nothing is missed: 22 areas a full replacement needs, 193 features a person can use, and 290 buildable parts under them. Each feature is marked **core** (needed for a first usable release, 139 features), **next** (soon after, 35) or **later** (19). Spare Research keeps only its own rules: where [Spare Kit](../kit/FEATURES.md) already maps a part, the research part depends on that kit part by its ID (KIT-...) and adds only what a research archive needs. Parts also name other research parts by ID. Every action is a command schema, so the command API and MCP server are core: anything a person can do, a script or an agent can do through the same commands, except the people-only actions (declarations, posting, reviews and votes). Where law or institutions differ by country, a part says so and the rules live in each country's rules pack; New Zealand is the first worked example, not the only country. No work-board pieces exist yet, so the Piece column is empty. IDs are permanent: a gap in the numbering will be a part that moved to the kit or merged into another part, never something missing. Other organisations' names (publishers, preprint servers, DOI agencies) appear only to say what we replace or work with; Spare Research has its own name, look and icons.

## Areas

| ID | Area | Features | Core | Parts |
| --- | --- | --- | --- | --- |
| RES-A1 | [Getting started: accounts, researcher profiles and ORCID](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) | 10 | 8 | 18 |
| RES-A2 | [Search, browse and following](#res-a2-search-browse-and-following) | 10 | 6 | 18 |
| RES-A3 | [Reading a paper](#res-a3-reading-a-paper) | 11 | 6 | 20 |
| RES-A4 | [Submitting a paper](#res-a4-submitting-a-paper) | 12 | 10 | 26 |
| RES-A5 | [Rights, licences and publisher agreements](#res-a5-rights-licences-and-publisher-agreements) | 10 | 9 | 18 |
| RES-A6 | [Screening before posting](#res-a6-screening-before-posting) | 7 | 6 | 12 |
| RES-A7 | [Open peer review](#res-a7-open-peer-review) | 14 | 10 | 21 |
| RES-A8 | [Journals on top and editorial work](#res-a8-journals-on-top-and-editorial-work) | 11 | 5 | 17 |
| RES-A9 | [Versions, corrections and retractions](#res-a9-versions-corrections-and-retractions) | 10 | 9 | 15 |
| RES-A10 | [Citing, DOIs and permanent links](#res-a10-citing-dois-and-permanent-links) | 9 | 7 | 17 |
| RES-A11 | [Data, code and supplementary files](#res-a11-data-code-and-supplementary-files) | 10 | 7 | 15 |
| RES-A12 | [Author, reviewer and editor dashboards](#res-a12-author-reviewer-and-editor-dashboards) | 8 | 8 | 8 |
| RES-A13 | [Research integrity and fraud](#res-a13-research-integrity-and-fraud) | 10 | 7 | 12 |
| RES-A14 | [Long-term preservation](#res-a14-long-term-preservation) | 8 | 5 | 11 |
| RES-A15 | [Indexing and being found](#res-a15-indexing-and-being-found) | 7 | 3 | 8 |
| RES-A16 | [Commands, AI agents and integrations](#res-a16-commands-ai-agents-and-integrations) | 8 | 5 | 10 |
| RES-A17 | [Accessibility, languages and countries](#res-a17-accessibility-languages-and-countries) | 6 | 5 | 8 |
| RES-A18 | [Apps on every device](#res-a18-apps-on-every-device) | 5 | 5 | 5 |
| RES-A19 | [Running costs, the cost ledger and members' say](#res-a19-running-costs-the-cost-ledger-and-members-say) | 8 | 7 | 10 |
| RES-A20 | [Safety, moderation and legal duties](#res-a20-safety-moderation-and-legal-duties) | 10 | 8 | 12 |
| RES-A21 | [Moving in and taking data out](#res-a21-moving-in-and-taking-data-out) | 4 | 3 | 4 |
| RES-A22 | [Conference and workshop review](#res-a22-conference-and-workshop-review) | 5 | 0 | 5 |

## RES-A1 Getting started: accounts, researcher profiles and ORCID

One Spareware login, a researcher profile linked to ORCID and ROR, reading with no account at all, and the first steps for readers, authors, reviewers and editors.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A1-F1** Sign up and sign in | Create one Spareware login and use it to submit, review and edit on any device. | core | Kit sign-in in Spare Research; Research roles |
| **RES-A1-F2** Read without an account | Read, download, search and cite any paper with no account, no sign-in wall and no tracking. | core | Open reading with no account; Reading settings without an account |
| **RES-A1-F3** Link my ORCID iD | Prove my ORCID iD once, so my papers and reviews are credited to me everywhere. | core | ORCID sign-in link; ORCID terms and membership; ORCID record sync |
| **RES-A1-F4** Researcher profile | Show my name, affiliations, fields, papers and reviews on one public page. | core | Researcher profile model; Profile page |
| **RES-A1-F5** Affiliations from an open list | Pick my university or institute from an open list, so my affiliation is written the same way everywhere. | core | ROR registry data; Affiliation picker |
| **RES-A1-F6** Change my name with care | Change my name on every paper and review I wrote, without the old name staying on show. | core | Name change across the record |
| **RES-A1-F7** First steps for each role | Get a short guide the first time I read, submit, review or run a journal. | core | Role guides |
| **RES-A1-F8** Help and support | Find plain help on submitting, rights, reviewing and citing, and reach support from any screen. | core | Research help articles; Support for authors and editors |
| **RES-A1-F9** Institution accounts | Let a library or department set up an organisation, see its researchers' papers and help cover running costs. | next | Institutions on the kit's organisation model; Institution sign-in |
| **RES-A1-F10** Let a helper prepare my submissions | Let a co-author or research assistant fill in a submission for me, while I approve and post it myself. | later | Submission helpers |

**Parts in RES-A1**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A1-F1-P1 | **Kit sign-in in Spare Research.** Uses the kit's sign-up, passkeys and sign-in screens, and asks for the second sign-in step from editors, screeners and moderators. | integration | small | [KIT-A1-F1-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A1-F2-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A1-F2-P4](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A1-F3-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A1-F1-P2 | **Research roles.** Reader, author, reviewer, editor, screener and moderator roles on the kit's roles per app, each with its own checks. | library | small | [KIT-A1-F5-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A1-F2-P1 | **Open reading with no account.** Every paper page, file and published review is public and served without tracking cookies or a sign-in; an account is needed only to submit, review, follow or comment. | library | small | [KIT-A28-F2-P1](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds), [KIT-A22-F8-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RES-A1-F2-P2 | **Reading settings without an account.** Keeps reading settings and a saved list on the device with no account, and moves them in if the reader signs up later. | interface | small | [KIT-A1-F15-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A1-F3-P1 | **ORCID sign-in link.** Proves a person holds an ORCID iD through ORCID's own sign-in (OAuth), so nobody can claim someone else's iD by typing it in. | integration | small | [KIT-A1-F5-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A1-F3-P2 | **ORCID terms and membership.** ORCID's free public API is for non-commercial use with daily read limits; adding works and reviews to records needs the member API, about USD 4,775 a year for a non-profit in 2026. Which fits a co-op running at cost. | research | small | none |  |
| RES-A1-F3-P3 | **ORCID record sync.** With the person's permission, adds their papers and reviews to their ORCID record, and reads their affiliations and past works in. | integration | medium | [RES-A1-F3-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A1-F3-P2](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F4-P1 | **Researcher profile model.** Adds name variants, the ORCID iD, affiliations with dates, fields and languages to the kit's profile, with who can see each set by the person. | data | small | [KIT-A1-F5-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [RES-A1-F3-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F4-P2 | **Profile page.** A public page with the person's papers, the reviews they chose to sign, the journals they edit and a way to contact them through the app. | interface | small | [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F5-P1 | **ROR registry data.** Loads the open Research Organization Registry (ROR, CC0 data) and keeps it current, with names in several languages. | data | small | none |  |
| RES-A1-F5-P2 | **Affiliation picker.** Type-ahead search over ROR, with past affiliations kept by date and a free-text option for places not yet listed, offered to ROR as a suggestion. | interface | small | [RES-A1-F5-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F6-P1 | **Name change across the record.** Updates the author name on every version, review and metadata record, sends corrected metadata to the DOI agency, and leaves no public trace of the old name unless the person asks. | service | medium | [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [KIT-A15-F1-P6](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A1-F7-P1 | **Role guides.** Short first-run screens and helpful empty pages for readers, authors, reviewers and editors, on the kit's first-run guide. | interface | small | [KIT-A20-F6-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A1-F8-P1 | **Research help articles.** Short how-to pages for every feature, translated, in the kit's help centre, with glossary entries for terms such as preprint, accepted manuscript and DOI. | data | medium | [KIT-A21-F1-P1](../kit/FEATURES.md#kit-a21-help-and-member-support), [KIT-A21-F1-P2](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RES-A1-F8-P2 | **Support for authors and editors.** Support cases linked to a submission or a journal, on the kit's support cases. | integration | small | [KIT-A21-F2-P1](../kit/FEATURES.md#kit-a21-help-and-member-support) |  |
| RES-A1-F9-P1 | **Institutions on the kit's organisation model.** Adds institution types (university, institute, library, department, funder) with their ROR ID to the kit's organisations. | data | small | [KIT-A1-F7-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [RES-A1-F5-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F9-P2 | **Institution sign-in.** Lets researchers sign in through their institution's own identity provider, through the kit's organisation sign-in. | integration | small | [KIT-A1-F7-P4](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [RES-A1-F9-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A1-F10-P1 | **Submission helpers.** Scoped access for a helper to fill in a submission; the author still makes the rights declaration and posts. | library | small | [KIT-A1-F8-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [RES-A5-F2-P3](#res-a5-rights-licences-and-publisher-agreements) |  |

## RES-A2 Search, browse and following

Finding research: full-text search with plain filters, browsing by field, following people, fields and journals, feeds for other tools, and separate human and AI sections.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A2-F1** Search papers | Search titles, abstracts, full text, authors and references, and filter by field, date, status, licence and language. | core | Research search index; Search screen and filters; Published ranking rules; Search by ID |
| **RES-A2-F2** Browse by field | Browse new and reviewed papers in a field, a subfield or a journal. | core | Field list; Field pages |
| **RES-A2-F3** Follow people, fields and journals | Follow people, fields and journals and get their new papers by email, in the app or in a feed. | core | Follows and alerts |
| **RES-A2-F4** Save a search | Save a search and be told when new papers match it. | core | Saved research searches |
| **RES-A2-F5** Feeds for other tools | Get any field, journal, author or search as a feed, and let other archives harvest our records. | core | Atom and RSS feeds; OAI-PMH endpoint |
| **RES-A2-F6** Human and AI sections | Choose to see papers written by people, papers generated by AI, or both. | core | Human and AI sections; Wrong-section reports |
| **RES-A2-F7** Find related work | See what a paper cites, what cites it, and papers on similar topics with the reason shown. | next | Citation links; Similar papers |
| **RES-A2-F8** Reading lists | Keep private or shared reading lists and export them to a reference manager. | next | Reading lists; List export |
| **RES-A2-F9** Search in my language | Search in my language and find papers whose titles and abstracts were written or translated into it. | next | Search across languages |
| **RES-A2-F10** Search other open archives | Search papers held elsewhere from the same box, each result linking to where it lives. | later | Search beyond Spare Research |

**Parts in RES-A2**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A2-F1-P1 | **Research search index.** Declares papers, versions, reviews, authors and journals to the kit's search service, with full text from the reading view and fields for status, licence, data and code. | service | medium | [KIT-A6-F1-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [KIT-A6-F1-P2](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A2-F1-P2 | **Search screen and filters.** A search box with filters for field, date, reviewed or not, has data, has code, licence and language, and results that show each paper's status in words, on the kit's search screens. | interface | medium | [KIT-A6-F1-P5](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F1-P3 | **Published ranking rules.** Results ranked by match and date under rules anyone can read, with no paid placement and no boost for any journal or institution. | library | small | [KIT-A6-F1-P3](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch) |  |
| RES-A2-F1-P4 | **Search by ID.** Finds a paper straight from a pasted DOI, permanent link, preprint server ID or ORCID iD, and supports author: and title: terms. | library | small | [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F2-P1 | **Field list.** An open list of research fields and subfields under a licence that allows reuse, mapped to the fields open catalogues use, so papers line up across services. | data | medium | none |  |
| RES-A2-F2-P2 | **Field pages.** New this week, recently reviewed, journals in the field and recent updates, for each field. | interface | small | [RES-A2-F2-P1](#res-a2-search-browse-and-following), [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F3-P1 | **Follows and alerts.** Follows kept for each person, with new papers sent through the kit's notification router as single alerts or daily and weekly digests. | service | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A8-F5-P3](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A2-F4-P1 | **Saved research searches.** Uses the kit's saved searches with the research filters and the person's chosen channels. | integration | small | [KIT-A6-F8-P1](../kit/FEATURES.md#kit-a6-search-matching-and-dispatch), [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F5-P1 | **Atom and RSS feeds.** A feed for every field, journal, author and saved search, with no tracking. | service | small | [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F5-P2 | **OAI-PMH endpoint.** Serves all metadata over OAI-PMH, the open protocol archives and catalogues use to harvest each other, in Dublin Core and JATS. | service | medium | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A2-F6-P1 | **Human and AI sections.** Every paper declares its AI use; papers whose text or results were generated by an AI system go in the AI section, and each reader picks which sections they see. | library | small | [RES-A4-F6-P2](#res-a4-submitting-a-paper), [KIT-A10-F13-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F13-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A2-F6-P2 | **Wrong-section reports.** Anyone can flag a paper in the wrong section; moderators move it with the reason in the public log, and the authors can appeal. | integration | small | [KIT-A10-F3-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F7-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A11-F5-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals) |  |
| RES-A2-F7-P1 | **Citation links.** Shows each paper's references and cited-by list from open citation data and our own extracted references. | service | medium | [RES-A10-F5-P1](#res-a10-citing-dois-and-permanent-links), [RES-A10-F8-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A2-F7-P2 | **Similar papers.** Suggests papers with similar text and references, says why, and runs only when the reader turns suggestions on. | library | medium | [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A2-F8-P1 | **Reading lists.** Saved papers in named lists, private or shared by link, on the kit's favourites and sharing. | interface | small | [KIT-A5-F12-P1](../kit/FEATURES.md#kit-a5-marketplace-engine), [KIT-A15-F8-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A2-F8-P2 | **List export.** Exports a list as BibTeX, RIS or CSL-JSON. | library | small | [RES-A10-F3-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A2-F9-P1 | **Search across languages.** Indexes every language a title and abstract is given in, with accent- and macron-aware matching. | library | small | [KIT-A19-F1-P4](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A17-F3-P1](#res-a17-accessibility-languages-and-countries) |  |
| RES-A2-F10-P1 | **Search beyond Spare Research.** Adds results from open catalogues whose terms allow it, labelled with their source, and links out to the paper where it lives. | integration | medium | [RES-A10-F6-P2](#res-a10-citing-dois-and-permanent-links) |  |

## RES-A3 Reading a paper

The paper page and the reading experience: a clear landing page, a reflowing web view built from open XML, the PDF, figures, maths and references, readable on any screen and counted without tracking.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A3-F1** Paper page | Open one page with the title, authors, abstract, status, licence, versions, files, reviews and how to cite. | core | Paper landing page; Machine-readable metadata; Share cards |
| **RES-A3-F2** Read in the browser | Read the full paper as a web page that reflows on a phone, with sections, figures, tables and references. | core | Full text as JATS XML; Web reading view; Maths in the reading view; Linked references |
| **RES-A3-F3** Read and download the PDF | Read the PDF in the page or download it, with the paper's permanent link on it. | core | PDF viewer; Download with a stamp |
| **RES-A3-F4** Accessible papers | Read papers with a screen reader, a keyboard, large text or high contrast. | core | Accessible reading view; Figure descriptions at submission; Accessible PDF from the reading view |
| **RES-A3-F5** Read on a phone or a slow connection | Read comfortably on an old phone or a slow link, with figures loading only when wanted. | core | Light reading mode |
| **RES-A3-F6** Reads and downloads without tracking | See how often a paper is read and downloaded, counted without tracking anyone. | core | Private usage counts; Usage counting rules |
| **RES-A3-F7** Plain-language summary | Read a short summary in plain words written by the authors. | next | Plain-language summary |
| **RES-A3-F8** Notes on a paper | Highlight and note a paper privately or share the notes with a group. | next | Notes on papers |
| **RES-A3-F9** Listen to a paper | Have a paper read aloud, skipping references and reading equations sensibly. | next | Read aloud |
| **RES-A3-F10** Print and read offline | Print a clean copy or save papers to read offline. | next | Offline papers |
| **RES-A3-F11** Read a translation | Read a machine translation of a paper, clearly labelled, with the original one tap away. | later | Paper translation |

**Parts in RES-A3**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A3-F1-P1 | **Paper landing page.** A fast, server-rendered public page for every paper and version on the kit's public pages, showing its status (preprint, under review, reviewed, corrected, retracted) in words, not only colour. | interface | medium | [KIT-A28-F2-P1](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds), [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A3-F1-P2 | **Machine-readable metadata.** Adds the citation meta tags scholar search engines read, schema.org ScholarlyArticle data and Signposting links to every paper page, so tools find the PDF, licence and authors. | library | small | [KIT-A28-F2-P2](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds), [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A3-F1-P3 | **Share cards.** Preview cards with the title, authors and status when a paper link is shared, built without tracking. | integration | small | [KIT-A28-F2-P3](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds) |  |
| RES-A3-F2-P1 | **Full text as JATS XML.** Keeps each version's full text as JATS XML, the open NISO standard most journals use, as the one source for the reading view, search and archive copies. | data | medium | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A3-F2-P2 | **Web reading view.** Renders JATS as a clean, reflowing page with a contents list, figure and table views, reference popups and links to each section. | interface | large | [RES-A3-F2-P1](#res-a3-reading-a-paper), [KIT-A27-F5-P1](../kit/FEATURES.md#kit-a27-shared-engines), [KIT-A20-F1-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A3-F2-P3 | **Maths in the reading view.** Shows equations as MathML with the TeX source copyable, using an openly licensed renderer whose licence suits the core. | library | medium | [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F2-P4 | **Linked references.** Each reference shows its DOI link, an open copy where one exists, and whether it was corrected or retracted. | interface | small | [RES-A3-F2-P2](#res-a3-reading-a-paper), [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A3-F3-P1 | **PDF viewer.** Shows the PDF in the page through the shared PDF engine, with page thumbnails and search. | integration | small | [KIT-A27-F6-P1](../kit/FEATURES.md#kit-a27-shared-engines) |  |
| RES-A3-F3-P2 | **Download with a stamp.** Adds a small line on each page with the permanent link, version and licence; the authors' original file stays available unchanged. | library | small | [KIT-A27-F6-P1](../kit/FEATURES.md#kit-a27-shared-engines), [RES-A10-F1-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A3-F4-P1 | **Accessible reading view.** The reading view built and tested to WCAG 2.2 AA on the kit's components, with MathML that screen readers can speak and figures that carry descriptions. | library | medium | [KIT-A19-F2-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A19-F2-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F4-P2 | **Figure descriptions at submission.** Asks authors for a short description of each figure and flags missing ones, on the kit's image descriptions. | interface | small | [KIT-A19-F2-P3](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A3-F4-P3 | **Accessible PDF from the reading view.** Makes a tagged, accessible PDF from the JATS when the authors' PDF is not accessible. | integration | medium | [KIT-A27-F6-P2](../kit/FEATURES.md#kit-a27-shared-engines), [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A3-F5-P1 | **Light reading mode.** Text first, figures on tap and small pages, on the kit's light mode. | integration | small | [KIT-A19-F5-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F6-P1 | **Private usage counts.** Views and downloads counted in totals only, with robots removed by a published rule, on the kit's private usage counts. | integration | small | [KIT-A22-F8-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RES-A3-F6-P2 | **Usage counting rules.** Whether to follow the COUNTER Code of Practice so counts compare with other publishers, and how to do it without personal data (*unverified* fit). | research | small | none |  |
| RES-A3-F7-P1 | **Plain-language summary.** An optional summary shown above the abstract, which can be translated. | data | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A3-F8-P1 | **Notes on papers.** Private and group notes anchored to text, kept on the W3C Web Annotation model so they can be exported. | library | medium | [KIT-A8-F12-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F9-P1 | **Read aloud.** Reads the web view aloud with the kit's open voices. | integration | small | [KIT-A27-F12-P5](../kit/FEATURES.md#kit-a27-shared-engines), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F10-P1 | **Offline papers.** Saves chosen papers on the device for reading offline, with a print layout. | interface | small | [KIT-A20-F2-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A3-F11-P1 | **Paper translation.** Machine translation of the web view through the kit's translation service, never replacing the original. | integration | medium | [KIT-A19-F10-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |

## RES-A4 Submitting a paper

From files to a posted paper: upload in common formats, details and authors with ORCID and ROR, a licence, declarations and the rights check, by a person or through the same commands a script uses.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A4-F1** Start a submission | Start a submission, save it and come back later. | core | Submission model; Submission steps |
| **RES-A4-F2** Upload the manuscript | Upload a PDF, a Word or OpenDocument file, or LaTeX source. | core | Manuscript upload; PDF checks; LaTeX build service; Word and OpenDocument upload |
| **RES-A4-F3** A web version made for me | Get a readable web version of my paper made automatically, and fix it before posting. | core | Conversion method; Conversion to JATS; Check the web version |
| **RES-A4-F4** Details and authors | Enter the title, abstract, field, keywords and funding, and list every author with their part in the work. | core | Paper metadata; Author list; Co-author confirmation; Details read from the file |
| **RES-A4-F5** Choose a licence | Pick how others may reuse my paper, with each choice explained in plain words. | core | Licence records; Licence chooser |
| **RES-A4-F6** Declarations | Declare funding, competing interests, ethics approval, data availability and AI use. | core | Declaration forms; AI use statement |
| **RES-A4-F7** Check and post | See everything in one place, fix anything missing, and post. | core | Final check; Post a paper |
| **RES-A4-F8** Who can publish | Know the rule for who can post a paper, and ask for an endorsement if I need one. | core | Who-can-publish rule; Endorsements |
| **RES-A4-F9** Submit from a script or agent | Prepare a submission from a script or an AI agent, with a person making the declarations and posting. | core | Submission commands |
| **RES-A4-F10** Send to a journal | Ask a journal run on Spare Research to review my posted paper. | core | Send to a journal |
| **RES-A4-F11** Bring a paper from another archive | Bring in a paper I already posted elsewhere, linked to the original. | next | Import terms; Import from other preprint servers |
| **RES-A4-F12** Notebook papers | Post a paper with live figures or notebooks readers can open. | later | Notebook papers |

**Parts in RES-A4**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A4-F1-P1 | **Submission model.** A submission with its files, details, authors, licence, declarations and rights answers, every change an operation. | data | medium | [KIT-A15-F1-P2](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [KIT-A15-F1-P3](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A4-F1-P2 | **Submission steps.** A short flow (files, details, licence, rights, declarations, check and post) that saves as it goes and works on a phone. | interface | medium | [RES-A4-F1-P1](#res-a4-submitting-a-paper), [KIT-A20-F1-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A4-F2-P1 | **Manuscript upload.** Resumable uploads with safety scanning and checksums, on the kit's file uploads. | integration | small | [KIT-A15-F7-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A4-F2-P2 | **PDF checks.** Checks a PDF opens, has real text, embeds its fonts and is not locked, and says in plain words how to fix it. | library | small | [KIT-A27-F6-P1](../kit/FEATURES.md#kit-a27-shared-engines) |  |
| RES-A4-F2-P3 | **LaTeX build service.** Builds LaTeX source in a locked-down sandbox with no network, as a separate service, so TeX's many licences stay out of the core. | service | large | none |  |
| RES-A4-F2-P4 | **Word and OpenDocument upload.** Reads .docx and .odt files through the shared office formats engine. | integration | small | [KIT-A27-F10-P1](../kit/FEATURES.md#kit-a27-shared-engines) |  |
| RES-A4-F3-P1 | **Conversion method.** How to turn LaTeX, Word and PDF into JATS: which open converters exist, their licences and the quality each reaches. Pandoc is GPL, so it can only run as a separate tool or be learned from. | research | medium | none |  |
| RES-A4-F3-P2 | **Conversion to JATS.** Converts each upload to JATS with the chosen method, keeping figures, tables, maths and references. | service | large | [RES-A4-F3-P1](#res-a4-submitting-a-paper), [RES-A3-F2-P1](#res-a3-reading-a-paper), [KIT-A27-F6-P6](../kit/FEATURES.md#kit-a27-shared-engines) |  |
| RES-A4-F3-P3 | **Check the web version.** Shows the author the web version beside the PDF before posting, lists any problems, and lets them fix headings, figures and references. | interface | medium | [RES-A4-F3-P2](#res-a4-submitting-a-paper), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A4-F4-P1 | **Paper metadata.** Title, abstract, keywords, field, language, funders and grant numbers, and dates, in one model that maps to Crossref, DataCite, JATS and Dublin Core. | data | medium | [RES-A2-F2-P1](#res-a2-search-browse-and-following) |  |
| RES-A4-F4-P2 | **Author list.** Authors in order with ORCID iDs, ROR affiliations, the corresponding author and each person's contribution from the CRediT roles (an ANSI/NISO standard). | interface | medium | [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A1-F5-P2](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A4-F4-P3 | **Co-author confirmation.** Tells every co-author and asks them to confirm they agree to posting; the paper waits until all confirm or a set time passes with no objection. | service | small | [KIT-A8-F4-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A15-F14-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A4-F4-P4 | **Details read from the file.** Fills in the title, authors, abstract and references from the upload for the author to check. | library | medium | [RES-A10-F5-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A4-F5-P1 | **Licence records.** The chosen licence kept as a machine-readable link in every metadata record and on every file. | data | small | none |  |
| RES-A4-F5-P2 | **Licence chooser.** Creative Commons licences explained plainly, with CC BY 4.0 suggested because many funders ask for it, and any funder's requirement shown. | interface | small | [RES-A4-F5-P1](#res-a4-submitting-a-paper) |  |
| RES-A4-F6-P1 | **Declaration forms.** Funding, competing interests, ethics approval, trial registration, and data and code availability, on the kit's form model and shown on the paper page. | interface | medium | [KIT-A25-F3-P1](../kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training) |  |
| RES-A4-F6-P2 | **AI use statement.** Every paper states whether and how AI tools were used: none, help with language or code, or text or results generated by AI. This also sets its section. | data | small | none |  |
| RES-A4-F7-P1 | **Final check.** One page listing anything missing or failing, with links back to fix it. | interface | small | [RES-A4-F1-P2](#res-a4-submitting-a-paper) |  |
| RES-A4-F7-P2 | **Post a paper.** Posts version 1: gives it a permanent ID, queues its DOI, sends it to screening, and tells the authors what happens next. | service | medium | [RES-A10-F1-P1](#res-a10-citing-dois-and-permanent-links), [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links), [RES-A6-F1-P1](#res-a6-screening-before-posting), [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A4-F8-P1 | **Who-can-publish rule.** The rule members choose (any account, a linked ORCID iD, or endorsement by someone already publishing in the field), kept as a setting changed only by vote. | library | small | [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A4-F8-P2 | **Endorsements.** A new author asks someone who has published in the field to endorse them; the endorser sees the paper first and is named in the record. | service | medium | [RES-A4-F8-P1](#res-a4-submitting-a-paper), [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A4-F9-P1 | **Submission commands.** Every submission step as a command in the kit's command API and MCP server; declarations and posting are people-only actions. | integration | small | [KIT-A16-F1-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), [KIT-A17-F1-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F9-P2](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RES-A4-F10-P1 | **Send to a journal.** Sends a posted paper to a chosen journal, showing its scope, review model and typical times first. | interface | small | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work), [RES-A7-F1-P1](#res-a7-open-peer-review) |  |
| RES-A4-F11-P1 | **Import terms.** What each preprint server's terms and licences let us copy, and when only a link is allowed. | research | small | none |  |
| RES-A4-F11-P2 | **Import from other preprint servers.** Brings in an author's own paper by its ID through the other server's published API, once the author proves it is theirs, and links back to the original. | integration | medium | [RES-A4-F1-P1](#res-a4-submitting-a-paper), [RES-A4-F11-P1](#res-a4-submitting-a-paper) |  |
| RES-A4-F12-P1 | **Notebook papers.** Accepts Jupyter notebooks and shows them as static pages, with running them at cost as a later step. | library | large | [RES-A11-F2-P1](#res-a11-data-code-and-supplementary-files) |  |

## RES-A5 Rights, licences and publisher agreements

Hosting only what authors have the right to share: plain guidance on each version, publisher policies looked up, each country's rules, a recorded declaration, embargoes, rights retention, and takedowns that leave a clear notice.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A5-F1** Know what I may share | See in plain words which version of my paper I may post, and on what conditions. | core | Versions explained; Policy data terms; Publisher policy lookup |
| **RES-A5-F2** Rights check before posting | Answer a few questions so the app can tell me whether I can post, and what I must add. | core | Rights questions; Rights declaration wording; Rights declaration |
| **RES-A5-F3** Country rules for rights | See what my country's law adds, such as a right to share publicly funded work after a set time. | core | Secondary publication rights research; Rights rules packs |
| **RES-A5-F4** Funder and institution policies | See what my funder or institution asks, such as immediate open access or a set licence. | core | Funder policy data; Rights retention helper |
| **RES-A5-F5** Embargoes | Post now and have a version go public on a set date. | core | Embargo timer |
| **RES-A5-F6** No paywalled copies | Trust that the archive holds only papers their authors may share. | core | Publisher-version check; Already-published check |
| **RES-A5-F7** Keep my copyright | Read a short agreement that leaves my copyright with me. | core | Author agreement |
| **RES-A5-F8** Licence shown everywhere | See how I may reuse a paper on its page, in its files and in its metadata. | core | Reuse guide |
| **RES-A5-F9** Copyright complaints and takedowns | Have a rights complaint about a paper handled fairly, with a notice left in its place. | core | Copyright notice flow; Tombstone pages |
| **RES-A5-F10** Material reused from others | Record permission for figures or text reused from others. | next | Reused material log |

**Parts in RES-A5**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A5-F1-P1 | **Versions explained.** Explains the preprint, the accepted manuscript and the published version, and that usually only the first two are the author's to share. | data | small | none |  |
| RES-A5-F1-P2 | **Policy data terms.** Jisc's Open Policy Finder (formerly Sherpa Romeo) needs an API key and licenses its data under CC BY-NC-ND 4.0: get Jisc's written confirmation that a non-profit may show it, and decide what to do when a journal is missing. | research | small | none |  |
| RES-A5-F1-P3 | **Publisher policy lookup.** Looks up the journal a paper went to in Open Policy Finder where its terms allow, and shows the version, embargo, licence and conditions it lists. | integration | medium | [RES-A5-F1-P2](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A5-F2-P1 | **Rights questions.** Who wrote it, was it accepted or published, which version is this, was an agreement signed, is any figure someone else's; the answers lead to post now, post on a date, or do not post. | interface | medium | [RES-A5-F1-P1](#res-a5-rights-licences-and-publisher-agreements), [RES-A5-F1-P3](#res-a5-rights-licences-and-publisher-agreements), [RES-A5-F3-P2](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A5-F2-P2 | **Rights declaration wording.** Plain wording for what authors confirm and what the co-op relies on, checked by a lawyer. | legal | small | none |  |
| RES-A5-F2-P3 | **Rights declaration.** The submitting author confirms they have the right to share this version under the chosen licence; the statement and its wording version are kept with the paper. A person must make it, never an agent. | legal | small | [RES-A5-F2-P2](#res-a5-rights-licences-and-publisher-agreements), [KIT-A18-F4-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A17-F9-P2](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RES-A5-F3-P1 | **Secondary publication rights research.** Which countries give authors a legal right to share their work after a period, which contracts cannot override, and the conditions in each: Germany and Austria (accepted manuscript after 12 months), France and Belgium (6 months, 12 in the humanities), the Netherlands (a reasonable period), and countries with none found, such as New Zealand (*to confirm*). | legal | medium | none |  |
| RES-A5-F3-P2 | **Rights rules packs.** Each country's rights rules as data in its country pack, so the rights check can say what the law allows there. | data | small | [KIT-A19-F6-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A5-F3-P1](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A5-F4-P1 | **Funder policy data.** Open access rules of funders and institutions as data, each with its source and date, starting with policies published openly. | data | medium | [RES-A5-F3-P2](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A5-F4-P2 | **Rights retention helper.** Adds a rights retention statement to a manuscript before it goes to an outside journal, so the author keeps the right to post the accepted version, as some funders ask. | library | small | [RES-A5-F4-P1](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A5-F5-P1 | **Embargo timer.** Holds a version private until a date and then publishes it on the kit's durable timers, with the date shown on the page. | service | small | [KIT-A15-F14-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [RES-A4-F7-P2](#res-a4-submitting-a-paper) |  |
| RES-A5-F6-P1 | **Publisher-version check.** Flags uploads that look like a publisher's typeset version (layout, running heads, publisher marks) for a screener to check against the rights answers. | library | medium | [RES-A6-F1-P1](#res-a6-screening-before-posting) |  |
| RES-A5-F6-P2 | **Already-published check.** Matches the title and authors against open DOI metadata and warns when a published version exists, linking to it. | integration | small | [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A5-F7-P1 | **Author agreement.** A short non-exclusive licence: authors keep their copyright, and the co-op may host, convert formats and copy to preservation archives. | legal | small | [KIT-A18-F4-P2](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RES-A5-F8-P1 | **Reuse guide.** Shows the licence with what it allows in plain words and how to credit the authors. | interface | small | [RES-A4-F5-P1](#res-a4-submitting-a-paper) |  |
| RES-A5-F9-P1 | **Copyright notice flow.** Notices and counter-notices about papers on the kit's copyright notice handling, with the authors told and heard. | integration | small | [KIT-A10-F21-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F9-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A5-F9-P2 | **Tombstone pages.** When files must come down, the page and its permanent link stay with the metadata and a plain reason, so citations never break. | interface | small | [RES-A3-F1-P1](#res-a3-reading-a-paper) |  |
| RES-A5-F10-P1 | **Reused material log.** Lists reused figures with their source and permission or licence, shown in the caption. | data | small | [RES-A4-F1-P1](#res-a4-submitting-a-paper) |  |

## RES-A6 Screening before posting

Quick checks by trained members before a paper goes live: in scope, complete, not copied, safe, lawful and careful with people in the data. Screening is not peer review, and every decision has a reason.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A6-F1** Screening queue | Have my paper checked by a trained member within a published time. | core | Screening queue; Screening rules; Screeners and their training |
| **RES-A6-F2** Copied text check | Know my paper was checked for copied text. | core | Overlap check method; Overlap check |
| **RES-A6-F3** Safety and ethics checks | Know papers that could put people at risk are held for a closer look. | core | Ethics and trial checks; Personal details check; Dangerous research rules |
| **RES-A6-F4** Health research posted with care | See a clear notice that unreviewed health research is not yet checked by experts. | core | Health preprint notice |
| **RES-A6-F5** Holds and appeals | Know why my paper was held or declined and ask for another look. | core | Screening appeals |
| **RES-A6-F6** No junk in the archive | Not see adverts, junk or nonsense papers in the archive. | core | Junk submission limits |
| **RES-A6-F7** Screening figures in public | See how many papers were screened, held and declined, and how fast. | next | Screening figures |

**Parts in RES-A6**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A6-F1-P1 | **Screening queue.** Papers wait in the kit's review queue for screening, with published target times and a reason for every hold or decline. | integration | medium | [KIT-A10-F6-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F8-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A6-F1-P2 | **Screening rules.** A published, member-voted checklist: research, not advertising; in a listed field; complete, with authors and an abstract; readable; lawful. | data | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A6-F1-P3 | **Screeners and their training.** Member screeners for each field, trained and certified on the kit's training module, with conflicts checked. | integration | small | [KIT-A14-F6-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training) |  |
| RES-A6-F2-P1 | **Overlap check method.** Compares open methods for finding copied text with paid services, such as Crossref's Similarity Check, which costs extra on top of membership, on accuracy, cost and what text each can compare against. | research | medium | none |  |
| RES-A6-F2-P2 | **Overlap check.** Checks new papers against our own archive and openly licensed full text, showing matches to the screener as hints, never automatic verdicts. | service | large | [RES-A6-F2-P1](#res-a6-screening-before-posting), [RES-A2-F1-P1](#res-a2-search-browse-and-following) |  |
| RES-A6-F3-P1 | **Ethics and trial checks.** Checks that studies on people or animals state ethics approval, and that clinical trials give a registry number, before posting. | library | small | [RES-A4-F6-P1](#res-a4-submitting-a-paper) |  |
| RES-A6-F3-P2 | **Personal details check.** Flags papers and files that may identify patients or participants, such as names, faces or exact dates, for the screener. | library | medium | [KIT-A15-F7-P4](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A6-F3-P3 | **Dangerous research rules.** What to do with papers that could help cause serious harm, such as weapons or dangerous pathogen work: who reviews them and which laws apply (*needs a lawyer*). | legal | medium | none |  |
| RES-A6-F4-P1 | **Health preprint notice.** A plain notice on unreviewed health research saying it should not guide treatment, as health preprint servers do. | interface | small | [RES-A3-F1-P1](#res-a3-reading-a-paper) |  |
| RES-A6-F5-P1 | **Screening appeals.** The author gets the reason and can appeal to different members through the kit's appeals. | integration | small | [KIT-A11-F5-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals) |  |
| RES-A6-F6-P1 | **Junk submission limits.** Limits on new accounts' submissions and signals for generated nonsense, on the kit's spam limits and fraud signals. | integration | small | [KIT-A10-F11-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A6-F7-P1 | **Screening figures.** Counts and times through the kit's public reporting pipeline. | integration | small | [KIT-A13-F9-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |

## RES-A7 Open peer review

Review in the open: authors or journals ask for review, conflicts are checked, reviewers comment on the paper itself, and reviews, author replies and assessments are published with their own DOIs. Whether reviewers are named is an open decision.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A7-F1** Ask for review | Ask for my posted paper to be reviewed, by a journal or by the community. | core | Review requests |
| **RES-A7-F2** Find and invite reviewers | Invite people with the right knowledge and no conflict with the authors. | core | Reviewer pool; Conflict of interest checks; Reviewer suggestions; Invitations and replies |
| **RES-A7-F3** Write a review | Write a clear review, with comments on the exact lines they refer to. | core | Review form; Comments on the paper; Reviewer guide |
| **RES-A7-F4** Named or anonymous reviews | Know whether my name will be shown, and choose where the journal allows. | core | Review identity modes; Anonymity protection |
| **RES-A7-F5** Published reviews | Read the reviews, the authors' reply and the editor's assessment next to the paper. | core | Publish reviews; Review DOIs |
| **RES-A7-F6** Reply and revise | Answer each review point and post a revised version. | core | Author reply |
| **RES-A7-F7** Credit for reviewing | Get credit for reviewing on my profile and my ORCID record. | core | Review records |
| **RES-A7-F8** Due dates and reminders | Know when a review is due and be reminded. | core | Review due dates |
| **RES-A7-F9** Reviews by people | Trust that reviews are written by people, with any AI help declared. | core | People-only reviews |
| **RES-A7-F10** Respectful reviews | Know reviews are checked for personal attacks before they are published. | core | Review conduct check |
| **RES-A7-F11** Reviews from other services | See reviews of the paper from other open review services. | next | Review links with other services |
| **RES-A7-F12** Comments from readers | Comment on a paper and read others' comments, with moderation. | next | Comments on papers |
| **RES-A7-F13** Reviewer workload | Set how many reviews I take on, and let editors see who is overloaded. | next | Review load limits |
| **RES-A7-F14** Learn to review | Learn to review with a short course and examples. | next | Reviewer course |

**Parts in RES-A7**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A7-F1-P1 | **Review requests.** A request for review linked to one version, from the authors or from a journal's editor. | service | small | [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A7-F2-P1 | **Reviewer pool.** People who offer to review, with their fields, languages and how many reviews they will take. | data | small | [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A7-F2-P2 | **Conflict of interest checks.** Flags recent co-authorship, a shared institution and declared conflicts, from ORCID and our own records, and asks each reviewer to confirm. | library | medium | [RES-A1-F3-P3](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A1-F5-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A7-F2-P3 | **Reviewer suggestions.** Suggests reviewers from fields and past papers, explains each suggestion, and leaves out anyone with a conflict. | library | medium | [RES-A7-F2-P1](#res-a7-open-peer-review), [RES-A7-F2-P2](#res-a7-open-peer-review) |  |
| RES-A7-F2-P4 | **Invitations and replies.** Invites with the abstract and due date, accept or decline in one step, and reminders. | service | small | [KIT-A8-F4-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A15-F14-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A7-F3-P1 | **Review form.** Summary, strengths, concerns, questions for the authors, and an assessment of how well the evidence supports the claims, set by each journal or the default. | interface | medium | [KIT-A25-F3-P1](../kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training) |  |
| RES-A7-F3-P2 | **Comments on the paper.** Comments pinned to text, figures and equations in the web view, on the kit's anchored comments. | integration | medium | [KIT-A8-F12-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [RES-A3-F2-P2](#res-a3-reading-a-paper) |  |
| RES-A7-F3-P3 | **Reviewer guide.** Short guidance on fair, specific and kind reviews, and what not to do. | data | small | none |  |
| RES-A7-F4-P1 | **Review identity modes.** Named; anonymous to the public but known to the editor; or kept private: as each journal or the members decide, shown on every review. | library | medium | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A7-F4-P2 | **Anonymity protection.** Strips names and hidden data from review files and keeps anonymous reviewers out of public metadata and exports. | library | small | [KIT-A15-F7-P3](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A7-F5-P1 | **Publish reviews.** Reviews, author replies and assessments published with the version they refer to once the round closes. | service | medium | [RES-A7-F3-P1](#res-a7-open-peer-review), [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A7-F5-P2 | **Review DOIs.** Registers each published review as peer review content with the DOI agency, linked to the paper, so reviews can be cited. | integration | small | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links), [RES-A7-F5-P1](#res-a7-open-peer-review) |  |
| RES-A7-F6-P1 | **Author reply.** A reply linked to each review point, published with the reviews. | interface | medium | [RES-A7-F5-P1](#res-a7-open-peer-review) |  |
| RES-A7-F7-P1 | **Review records.** Reviews listed on the reviewer's profile, signed or counted only, and added to their ORCID record with consent. | integration | small | [RES-A1-F3-P3](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A1-F4-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A7-F8-P1 | **Review due dates.** Due dates, reminders and gentle chasers on the kit's durable timers, with extensions on request. | service | small | [KIT-A15-F14-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A7-F9-P1 | **People-only reviews.** Sending a review is a people-only action; reviewers declare any AI help and agree never to paste unpublished papers into outside AI tools. | library | small | [KIT-A17-F9-P2](../kit/FEATURES.md#kit-a17-ai-access), [RES-A4-F6-P2](#res-a4-submitting-a-paper) |  |
| RES-A7-F10-P1 | **Review conduct check.** The editor sees each review before it is published and can send back one that attacks people, under published conduct rules. | library | small | [KIT-A10-F5-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A7-F11-P1 | **Review links with other services.** Sends and receives COAR Notify messages, so reviews from other open review services show on the paper and ours show there. | integration | medium | [RES-A7-F5-P1](#res-a7-open-peer-review) |  |
| RES-A7-F12-P1 | **Comments on papers.** Signed comments on the kit's threads, with reports and member moderation. | integration | small | [KIT-A8-F12-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A10-F3-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A7-F13-P1 | **Review load limits.** Each reviewer sets a limit, invites stop when it is reached, and editors see the load across their journal. | library | small | [RES-A7-F2-P1](#res-a7-open-peer-review) |  |
| RES-A7-F14-P1 | **Reviewer course.** A short course with sample papers and published reviews, on the kit's training module. | data | small | [KIT-A25-F2-P1](../kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training) |  |

## RES-A8 Journals on top and editorial work

Editors run journals as lists of papers they reviewed and accepted, on top of papers already posted: set-up, scope and policies, an editorial queue, decisions and assessments, issues where wanted, and protected journal names.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A8-F1** Start a journal | Set up a journal with its name, scope, editors, review model and policies. | core | Journal model; Journal set-up; Protected journal names |
| **RES-A8-F2** Editors and roles | Invite editors and give each a role, with conflicts declared. | core | Editorial roles |
| **RES-A8-F3** Editorial queue | See every paper sent to the journal, its stage and what it waits on. | core | Editorial queue; Assign a handling editor |
| **RES-A8-F4** Decisions and assessments | Make a decision or write an assessment, and tell the authors. | core | Decision types; Decision letters |
| **RES-A8-F5** Journal pages | Read a journal's scope, editors, policies and accepted papers on one page. | core | Journal home page; Policy pages |
| **RES-A8-F6** A journal's own web address | Run a journal under its own web address. | next | Journal domains |
| **RES-A8-F7** Issues and volumes | Group accepted papers into volumes and issues where the field expects them. | next | Issues and volumes |
| **RES-A8-F8** An ISSN for the journal | Get an ISSN, with my country's steps shown. | next | ISSN steps |
| **RES-A8-F9** Themed collections | Gather papers on a theme with guest editors. | next | Collections |
| **RES-A8-F10** Move a journal in | Bring an existing journal with its papers, DOIs and history. | next | Import from other journal systems; DOI transfer |
| **RES-A8-F11** Journal figures | See open figures such as time to decision and reviews per paper, never one ranking number. | later | Journal figures |

**Parts in RES-A8**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A8-F1-P1 | **Journal model.** A journal with its scope, fields, editors, review model, review identity mode, policies and accepted papers, as an organisation on the kit's model. | data | medium | [KIT-A1-F7-P1](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A8-F1-P2 | **Journal set-up.** A guided set-up with plain choices and the policies a journal must publish. | interface | medium | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A8-F1-P3 | **Protected journal names.** Checks new journal names against existing journals' titles and ISSNs, and holds lookalikes for a person to review, so nobody passes off a journal as another. | library | small | [KIT-A10-F6-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A8-F2-P1 | **Editorial roles.** Editor-in-chief, handling editor, managing editor and screener roles on the kit's roles and team management, each with conflict declarations. | library | small | [KIT-A1-F5-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in), [KIT-A1-F7-P2](../kit/FEATURES.md#kit-a1-accounts-and-sign-in) |  |
| RES-A8-F3-P1 | **Editorial queue.** Papers by stage with due dates, reviewer status and the next step, filtered by editor. | interface | medium | [RES-A7-F1-P1](#res-a7-open-peer-review) |  |
| RES-A8-F3-P2 | **Assign a handling editor.** Gives a paper to an editor after checking conflicts. | library | small | [RES-A7-F2-P2](#res-a7-open-peer-review) |  |
| RES-A8-F4-P1 | **Decision types.** Accept into the journal, ask for changes, or decline; or, for journals that do not accept or reject, a published assessment of how important the work is and how strong its evidence. | library | medium | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A8-F4-P2 | **Decision letters.** Templates for decision letters that link to the reviews, sent through the kit's messages. | interface | small | [KIT-A8-F4-P3](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A8-F5-P1 | **Journal home page.** Scope, editors, review model, policies and papers, with feeds, on the kit's public pages. | interface | medium | [KIT-A28-F2-P1](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds), [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A8-F5-P2 | **Policy pages.** Peer review, ethics, corrections, conflicts, licence and preservation policies written once from templates and shown on the journal page, as directories of open journals ask. | data | small | [KIT-A15-F18-P2](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A8-F6-P1 | **Journal domains.** Connects a journal's own domain on the kit's custom domains. | integration | small | [KIT-A28-F1-P2](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds) |  |
| RES-A8-F7-P1 | **Issues and volumes.** Optional volumes, issues and article numbers, kept in the metadata. | data | small | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A8-F8-P1 | **ISSN steps.** How to apply to each country's national ISSN centre, kept in its country pack, and where the number is recorded. | data | small | [RES-A17-F4-P1](#res-a17-accessibility-languages-and-countries) |  |
| RES-A8-F9-P1 | **Collections.** Themed collections with guest editors and their own page. | interface | small | [RES-A8-F5-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A8-F10-P1 | **Import from other journal systems.** Imports papers, public reviews and metadata from other open journal systems through their published export formats and OAI-PMH. | integration | large | [RES-A3-F2-P1](#res-a3-reading-a-paper), [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A8-F10-P2 | **DOI transfer.** Points a journal's existing DOIs at Spare Research through the DOI agency's own transfer process. | integration | medium | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A8-F11-P1 | **Journal figures.** Time to first review and to decision, reviews per paper and corrections, through the public reporting pipeline. | integration | small | [KIT-A13-F9-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |

## RES-A9 Versions, corrections and retractions

The record stays honest: every version kept and compared, corrections and expressions of concern linked to what they correct, retractions that leave the paper readable with a clear notice, and one public log of every change.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A9-F1** Post a new version | Post a new version with a short note on what changed. | core | Version model; New version flow |
| **RES-A9-F2** See and compare versions | See every version and what changed between them. | core | Version list; Compare versions |
| **RES-A9-F3** Corrections | Read a correction linked to the paper it corrects. | core | Correction notices; Update metadata |
| **RES-A9-F4** Expressions of concern | See when editors have a concern under investigation. | core | Expression of concern |
| **RES-A9-F5** Retractions | Know a retracted paper at once, with the reason and who decided. | core | Retraction process; Retraction notices; Honest error label |
| **RES-A9-F6** Withdraw a preprint | Withdraw my preprint, leaving a notice and the earlier versions on record. | core | Withdrawal |
| **RES-A9-F7** Removal for legal reasons | Know when content was removed for a legal reason, and why. | core | Legal removals |
| **RES-A9-F8** One log of every change | Browse every correction, concern, retraction and withdrawal across the archive. | core | Public updates log |
| **RES-A9-F9** Updates reach readers | Be told when a paper I saved, cited or followed is corrected or retracted. | core | Update alerts |
| **RES-A9-F10** Retractions elsewhere | See when a paper I cite was retracted by another publisher. | next | Outside retraction data |

**Parts in RES-A9**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A9-F1-P1 | **Version model.** Versions v1, v2 and on, each with its own files, date and note, kept for good; the permanent link leads to the latest and each version has its own link. | data | medium | [KIT-A15-F3-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [RES-A10-F1-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A9-F1-P2 | **New version flow.** The submission flow for a new version, carrying the details over and asking what changed. | interface | small | [RES-A4-F1-P2](#res-a4-submitting-a-paper), [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F2-P1 | **Version list.** Every version with its date and note on the paper page, and a clear notice when reading an older one. | interface | small | [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F2-P2 | **Compare versions.** Shows the text changes between two versions from their JATS. | library | medium | [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A9-F3-P1 | **Correction notices.** A correction with its own page and DOI, linked both ways to the version it corrects, issued by the authors or editors with a reason. | service | medium | [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions), [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A9-F3-P2 | **Update metadata.** Sends corrections, retractions and new versions to the DOI agency as updates, so other services show them too. | integration | small | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A9-F4-P1 | **Expression of concern.** A notice that an investigation is open, shown on the paper and in its metadata, and closed by a later notice. | service | small | [RES-A9-F3-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F5-P1 | **Retraction process.** A published process following COPE's retraction guidelines (updated in 2025) and NISO's recommended practice for retractions: who can decide, how the authors are heard, and what the notice must explain. | legal | small | none |  |
| RES-A9-F5-P2 | **Retraction notices.** A notice with the reason, who decided and when; the paper stays readable with a clear banner and a mark on every PDF page. | service | medium | [RES-A9-F5-P1](#res-a9-versions-corrections-and-retractions), [RES-A9-F3-P1](#res-a9-versions-corrections-and-retractions), [RES-A3-F3-P2](#res-a3-reading-a-paper) |  |
| RES-A9-F5-P3 | **Honest error label.** Says plainly when authors retracted their own paper over an honest error, so correcting the record is not treated as misconduct. | library | small | [RES-A9-F5-P2](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F6-P1 | **Withdrawal.** Authors withdraw a preprint with a reason; a withdrawal notice becomes the latest version and earlier versions stay, as preprint servers do. | service | small | [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F7-P1 | **Legal removals.** Files removed only for a legal reason or a serious risk to people, leaving a tombstone, the rule and an entry in the public log. | integration | small | [RES-A5-F9-P2](#res-a5-rights-licences-and-publisher-agreements), [KIT-A10-F7-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A9-F8-P1 | **Public updates log.** Every notice by date, field and journal with its reason, through the kit's public reporting pipeline. | interface | small | [KIT-A13-F9-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger), [RES-A9-F3-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A9-F9-P1 | **Update alerts.** Alerts to people who saved or followed a paper, and a status check on every reference in new submissions. | service | small | [KIT-A8-F3-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A9-F10-P1 | **Outside retraction data.** Uses the Retraction Watch data, which Crossref makes openly available under CC0, to flag retracted references. | integration | small | [RES-A10-F6-P2](#res-a10-citing-dois-and-permanent-links) |  |

## RES-A10 Citing, DOIs and permanent links

Every paper, version, review and dataset gets a link that never breaks: our own permanent IDs, DOIs from a registration agency, ORCID and ROR in the metadata, references linked and shared openly, and citations in any style.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A10-F1** Permanent links | Cite a link that will always lead to the paper. | core | Permanent IDs; Link resolver |
| **RES-A10-F2** DOIs | Get a DOI for my paper so it can be cited and found anywhere. | core | DOI agency choice; Which things get DOIs; DOI deposits |
| **RES-A10-F3** How to cite | Copy a citation in my style and export it to my reference manager. | core | Citation export; Citation styles; Cite box |
| **RES-A10-F4** Author and organisation IDs | See ORCID iDs and ROR IDs in every citation record. | core | IDs in metadata |
| **RES-A10-F5** References linked and shared | See each reference linked, and have my references shared openly. | core | Reference extraction; Reference matching; Open references |
| **RES-A10-F6** Open metadata | Reuse all our metadata freely. | core | Metadata under CC0; Open metadata sources |
| **RES-A10-F7** Cite data and code | Cite the exact dataset and code version behind a paper. | core | Data and code citations |
| **RES-A10-F8** Who cited a paper | See who cited a paper, from open data. | next | Cited-by from open data |
| **RES-A10-F9** Backup identifiers | Know links still work if a DOI agency fails. | later | Backup identifiers |

**Parts in RES-A10**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A10-F1-P1 | **Permanent IDs.** Our own short, permanent ID for every paper, version and review, built on the kit's permanent IDs and never reused or changed. | library | small | [KIT-A15-F1-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A10-F1-P2 | **Link resolver.** Resolves permanent links after pages or servers move, and keeps working from a simple static copy if the app is down. | service | medium | [RES-A10-F1-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F2-P1 | **DOI agency choice.** Crossref (yearly membership from USD 200, plus a fee for each DOI such as USD 0.25 for a preprint and USD 1 for a journal article in 2026) or DataCite (EUR 2,000 a year plus an infrastructure fee, with no fee for each DOI), or both, and what fits papers, reviews and data. | research | medium | none |  |
| RES-A10-F2-P2 | **Which things get DOIs.** Whether each version gets its own DOI or one DOI leads to the latest, following how preprint servers and data archives do it. | library | small | [RES-A10-F2-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F2-P3 | **DOI deposits.** Builds and sends metadata deposits for papers, versions, reviews and updates, retries failures and checks every DOI resolves. | service | large | [RES-A10-F2-P1](#res-a10-citing-dois-and-permanent-links), [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A10-F3-P1 | **Citation export.** BibTeX, RIS and CSL-JSON for any paper, list or search. | library | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A10-F3-P2 | **Citation styles.** Formats citations in common styles from the open Citation Style Language styles (CC BY-SA 3.0, credited as they ask), with a processor whose licence suits the core, such as hayagriva (MIT or Apache-2.0). | library | medium | [RES-A10-F3-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F3-P3 | **Cite box.** A cite box on every paper with the permanent link, the DOI, the version and a copy button. | interface | small | [RES-A10-F3-P2](#res-a10-citing-dois-and-permanent-links), [RES-A10-F1-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F4-P1 | **IDs in metadata.** ORCID iDs, ROR IDs, funder IDs and grant numbers carried in every deposit and export. | library | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper), [RES-A1-F3-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A1-F5-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid) |  |
| RES-A10-F5-P1 | **Reference extraction.** Pulls references and structure from uploads with an openly licensed extractor, such as GROBID (Apache-2.0), run as a separate service. | service | medium | none |  |
| RES-A10-F5-P2 | **Reference matching.** Matches references to DOIs through open metadata APIs and lets authors fix mismatches. | service | medium | [RES-A10-F5-P1](#res-a10-citing-dois-and-permanent-links), [RES-A10-F6-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F5-P3 | **Open references.** Deposits every paper's references openly with its DOI metadata, so citation data stays free. | integration | small | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links), [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F6-P1 | **Metadata under CC0.** All metadata (titles, authors, abstracts, references) published under CC0, apart from each paper's own licence. | legal | small | none |  |
| RES-A10-F6-P2 | **Open metadata sources.** Which open sources we read and on what terms: DOI agency metadata, ORCID, ROR (CC0), OpenAlex (CC0 data, an API with a free daily allowance and charges for heavy use), OpenCitations and the Retraction Watch data (CC0). | research | small | none |  |
| RES-A10-F7-P1 | **Data and code citations.** Dataset DOIs and code archive IDs (SWHIDs) shown in the cite box and in the paper's metadata as related items. | library | small | [RES-A11-F1-P2](#res-a11-data-code-and-supplementary-files), [RES-A11-F2-P2](#res-a11-data-code-and-supplementary-files) |  |
| RES-A10-F8-P1 | **Cited-by from open data.** Cited-by lists from open citation data and our own references. | integration | small | [RES-A10-F6-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A10-F9-P1 | **Backup identifiers.** Whether to also make free ARK identifiers as a backup to DOIs, and how to keep both pointing at the same place. | research | small | none |  |

## RES-A11 Data, code and supplementary files

The evidence behind a paper: datasets with their own DOIs, code archived for good, previews of common files, care with personal and Indigenous data, and links to the specialist archives researchers already use.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A11-F1** Attach data | Upload my data with the paper or link to where it already lives. | core | Data uploads; Dataset DOIs; Links to data elsewhere; Data availability statement |
| **RES-A11-F2** Attach code | Link my code and have the exact version archived for good. | core | Code links; Code archiving |
| **RES-A11-F3** Preview files | Look at tables, images and notebooks in the browser before downloading. | core | File previews |
| **RES-A11-F4** Supplementary files | Add supplementary files that stay with each version. | core | Supplementary files |
| **RES-A11-F5** Data about people | Share data about people only in ways that protect them. | core | Data about people; Controlled access |
| **RES-A11-F6** Indigenous data | Have Indigenous data cared for as its communities ask. | core | Indigenous data principles |
| **RES-A11-F7** Describe data well | Describe data so others can find and reuse it. | core | Dataset metadata |
| **RES-A11-F8** Large datasets | Share large datasets without running out of space. | next | Large data at cost |
| **RES-A11-F9** Open data and code badges | See whether data and code were checked by a reviewer. | next | Open data and code badges |
| **RES-A11-F10** Run the code | Run a paper's code in the browser. | later | Run code at cost |

**Parts in RES-A11**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A11-F1-P1 | **Data uploads.** Data files with checksums and a free allowance per paper, larger sets at cost, on the kit's file uploads and storage. | integration | medium | [KIT-A15-F7-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [KIT-A15-F7-P2](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A11-F1-P2 | **Dataset DOIs.** A DOI for each dataset with its own metadata and licence, through the chosen agency. | integration | small | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links), [RES-A11-F7-P1](#res-a11-data-code-and-supplementary-files) |  |
| RES-A11-F1-P3 | **Links to data elsewhere.** Links to datasets in other archives by DOI, checked to resolve. | library | small | [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A11-F1-P4 | **Data availability statement.** A required statement saying where the data are, or why they cannot be shared. | data | small | [RES-A4-F6-P1](#res-a4-submitting-a-paper) |  |
| RES-A11-F2-P1 | **Code links.** Links a public code repository at a tagged version, with its licence read from the repository. | integration | small | none |  |
| RES-A11-F2-P2 | **Code archiving.** Asks Software Heritage to archive the repository and records its permanent SWHID. | integration | small | [RES-A11-F2-P1](#res-a11-data-code-and-supplementary-files) |  |
| RES-A11-F3-P1 | **File previews.** Previews for CSV, spreadsheets, images, notebooks and plain text on the kit's preview service. | integration | small | [KIT-A15-F16-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A11-F4-P1 | **Supplementary files.** Extra files tied to a version, each with a description, size and checksum. | data | small | [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A11-F5-P1 | **Data about people.** Rules for data from participants and patients: remove identifying details before sharing, record consent, and never post raw personal data, checked by a lawyer in each country. | legal | medium | [KIT-A18-F18-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A18-F18-P2](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RES-A11-F5-P2 | **Controlled access.** Data described openly but released only to approved people under a signed agreement, decided by the authors or a data committee. | service | large | [KIT-A25-F1-P2](../kit/FEATURES.md#kit-a25-signatures-forms-notices-and-training), [RES-A11-F5-P1](#res-a11-data-code-and-supplementary-files) |  |
| RES-A11-F6-P1 | **Indigenous data principles.** Applies the CARE principles and each country's own principles, with the kit's Māori data principles in the New Zealand example, and shows Local Contexts labels on the data. | legal | small | [KIT-A18-F16-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RES-A11-F7-P1 | **Dataset metadata.** Dataset details mapped to the DataCite schema: creators, dates, formats, sizes and methods. | data | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A11-F8-P1 | **Large data at cost.** Chunked uploads and storage at cost for very large sets, or a link to a specialist archive. | integration | medium | [KIT-A13-F3-P6](../kit/FEATURES.md#kit-a13-open-cost-ledger), [RES-A11-F1-P1](#res-a11-data-code-and-supplementary-files) |  |
| RES-A11-F9-P1 | **Open data and code badges.** Badges for open data, open code and results reproduced, given by reviewers under published criteria. | library | small | [RES-A7-F3-P1](#res-a7-open-peer-review) |  |
| RES-A11-F10-P1 | **Run code at cost.** Runs archived code in a sandboxed container, charged at cost. | service | large | [RES-A11-F2-P2](#res-a11-data-code-and-supplementary-files) |  |

## RES-A12 Author, reviewer and editor dashboards

One place for my work: my papers and their stage, what is waiting on me, reviews to write, messages with editors and plain counts, on a phone or a desk.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A12-F1** My papers | See every paper I wrote or submitted and its stage. | core | My papers |
| **RES-A12-F2** What is waiting on me | See every task in one list: confirm authorship, answer reviews, fix a check. | core | To-do list |
| **RES-A12-F3** A paper's progress | See each step a paper went through and what comes next. | core | Paper timeline |
| **RES-A12-F4** Messages with editors | Talk to the editor or screener about my paper in one thread. | core | Paper messages |
| **RES-A12-F5** My reviews | See reviews I was asked for, am writing and have finished. | core | My reviews |
| **RES-A12-F6** Reads and citations of my papers | See reads, downloads and citations of my papers, with no tracking. | core | My figures |
| **RES-A12-F7** Notifications | Choose which alerts I get and how. | core | Research notifications |
| **RES-A12-F8** Editor dashboard | As an editor, see my journal's queue, late reviews and decisions to make. | core | Editor dashboard |

**Parts in RES-A12**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A12-F1-P1 | **My papers.** Drafts, in screening, posted, under review, revised, accepted, corrected; each with its next step. | interface | medium | [RES-A4-F1-P1](#res-a4-submitting-a-paper), [RES-A9-F1-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A12-F2-P1 | **To-do list.** Tasks across papers, reviews and journals, with due dates. | interface | small | [RES-A7-F8-P1](#res-a7-open-peer-review), [RES-A4-F4-P3](#res-a4-submitting-a-paper) |  |
| RES-A12-F3-P1 | **Paper timeline.** Every step with its date and who acted, drawn from the operation log. | interface | small | [KIT-A15-F3-P1](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A12-F4-P1 | **Paper messages.** A message thread for each paper with its authors and editors, on the kit's chat service. | integration | small | [KIT-A8-F1-P2](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A12-F5-P1 | **My reviews.** Invites, drafts, due dates and published reviews. | interface | small | [RES-A7-F2-P4](#res-a7-open-peer-review), [RES-A7-F3-P1](#res-a7-open-peer-review) |  |
| RES-A12-F6-P1 | **My figures.** Reads, downloads and open citations for each paper and version. | interface | small | [RES-A3-F6-P1](#res-a3-reading-a-paper) |  |
| RES-A12-F7-P1 | **Research notifications.** Alerts for screening, reviews, decisions and updates on the kit's notification settings and inbox. | integration | small | [KIT-A8-F5-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications), [KIT-A8-F3-P3](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A12-F8-P1 | **Editor dashboard.** The journal's queue, load and due dates at a glance. | interface | small | [RES-A8-F3-P1](#res-a8-journals-on-top-and-editorial-work) |  |

## RES-A13 Research integrity and fraud

Keeping the record trustworthy: signals for paper mills, fake reviewers, image and citation tricks, all treated as hints for trained people, a way for anyone to raise a concern, and an integrity panel that decides in the open.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A13-F1** Raise a concern | Report a possible problem with any paper, even without an account. | core | Integrity reports |
| **RES-A13-F2** Integrity panel | Know concerns are looked into by trained members, fairly and on time. | core | Integrity panel; Published outcomes |
| **RES-A13-F3** Paper mill signals | Know papers are checked for signs of mass-produced fake research. | core | Paper mill methods; Paper mill signals |
| **RES-A13-F4** Real reviewers | Trust that reviewers are who they say they are. | core | Reviewer identity checks |
| **RES-A13-F5** Author list changes | Know late changes to the author list are checked. | core | Author list changes |
| **RES-A13-F6** AI-generated text | Know AI use is declared, and that undeclared generated text can be flagged. | core | AI text hints |
| **RES-A13-F7** Integrity figures | See how many concerns were raised and what happened. | core | Integrity figures |
| **RES-A13-F8** Image checks | Know figures are checked for duplication. | next | Image duplication hints |
| **RES-A13-F9** Citation tricks | Know citation rings and forced citations are watched for. | next | Citation pattern hints |
| **RES-A13-F10** Share signals with other publishers | Know fraud found here can be shared with other publishers in a lawful way. | later | Sharing integrity signals |

**Parts in RES-A13**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A13-F1-P1 | **Integrity reports.** A report form on every paper and review, open to anyone, on the kit's reports, with the reporter's name kept from the authors on request. | integration | small | [KIT-A10-F3-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F3-P3](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A13-F2-P1 | **Integrity panel.** Trained members drawn with conflict checks, following COPE's published guidance and hearing the authors before deciding. | integration | medium | [KIT-A11-F4-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals), [KIT-A11-F4-P2](../kit/FEATURES.md#kit-a11-disputes-and-appeals), [RES-A9-F5-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A13-F2-P2 | **Published outcomes.** Each outcome becomes a correction, a concern, a retraction or no action, with reasons in the public log. | library | small | [RES-A9-F8-P1](#res-a9-versions-corrections-and-retractions) |  |
| RES-A13-F3-P1 | **Paper mill methods.** How published, open methods spot paper mills (odd stock phrases, template text, reused images, sudden author changes) and which suit us. | research | medium | none |  |
| RES-A13-F3-P2 | **Paper mill signals.** Signals from the chosen methods shown to screeners and editors as hints with their evidence, never automatic verdicts. | library | large | [RES-A13-F3-P1](#res-a13-research-integrity-and-fraud), [KIT-A10-F12-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A13-F4-P1 | **Reviewer identity checks.** Reviewers link a proven ORCID iD and, for journal reviews, an institutional email or a record of past papers; reviewers suggested by authors are checked before they are invited. | library | medium | [RES-A1-F3-P1](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A7-F2-P1](#res-a7-open-peer-review) |  |
| RES-A13-F5-P1 | **Author list changes.** Adding or removing authors after posting needs every author's agreement and a reason, shown in the history. | library | small | [RES-A4-F4-P3](#res-a4-submitting-a-paper) |  |
| RES-A13-F6-P1 | **AI text hints.** Signals for undeclared generated text shown as hints only, because detectors are often wrong, with the authors always heard. | library | medium | [RES-A4-F6-P2](#res-a4-submitting-a-paper) |  |
| RES-A13-F7-P1 | **Integrity figures.** Counts and outcomes in the kit's transparency reports. | integration | small | [KIT-A10-F15-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A13-F8-P1 | **Image duplication hints.** Finds reused or duplicated images within and across papers, starting from the kit's reused photo checks. | integration | large | [KIT-A10-F12-P3](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A13-F9-P1 | **Citation pattern hints.** Flags unusual self-citation and citation rings for editors to look at. | library | medium | [RES-A10-F5-P2](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A13-F10-P1 | **Sharing integrity signals.** Which shared integrity services exist, who may join them, and what privacy law lets us share. | research | small | none |  |

## RES-A14 Long-term preservation

Papers that outlive any server: open archive packages, copies held by outside preservation archives and national libraries, checked copies, bulk exports anyone can mirror, and a plan if Spare Research ever closes.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A14-F1** Copies kept outside | Know copies of every paper are held by archives outside the co-op. | core | Preservation archive choice; Archive packages; Archive deposits |
| **RES-A14-F2** Legal deposit | Know papers are deposited with national libraries where the law asks. | core | Legal deposit rules; Deposits to national libraries |
| **RES-A14-F3** Checked copies | Know stored files are checked and never silently damaged. | core | Fixity checks |
| **RES-A14-F4** Formats that last | Know papers are kept in formats that will still open in fifty years. | core | Preservation formats |
| **RES-A14-F5** Whole-archive export | Download the whole archive, so anyone can keep a copy. | core | Bulk exports |
| **RES-A14-F6** If Spare Research closes | Know what happens to papers if the service ever ends. | next | Closing plan |
| **RES-A14-F7** Mirrors | Run a mirror of the archive. | next | Mirrors |
| **RES-A14-F8** Format migration | Know old files move to newer formats when needed. | later | Format watch |

**Parts in RES-A14**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A14-F1-P1 | **Preservation archive choice.** Compares outside preservation archives, such as CLOCKSS, Portico, LOCKSS networks and the Internet Archive, on cost, terms and what they take (*costs to confirm*). | research | medium | none |  |
| RES-A14-F1-P2 | **Archive packages.** Each version packed with its files, JATS, metadata and checksums in an open package format any archive can take. | library | medium | [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A14-F1-P3 | **Archive deposits.** Sends packages to the chosen archives and records each receipt. | service | medium | [RES-A14-F1-P1](#res-a14-long-term-preservation), [RES-A14-F1-P2](#res-a14-long-term-preservation) |  |
| RES-A14-F2-P1 | **Legal deposit rules.** Each country's legal deposit duties for online publications, as data in its country pack. In the New Zealand example the National Library harvests online documents under a 2006 notice and publishers help when asked (*to confirm against the Act*). | legal | small | [RES-A17-F4-P1](#res-a17-accessibility-languages-and-countries) |  |
| RES-A14-F2-P2 | **Deposits to national libraries.** Sends papers to national libraries in the way each asks. | integration | medium | [RES-A14-F2-P1](#res-a14-long-term-preservation), [RES-A14-F1-P2](#res-a14-long-term-preservation) |  |
| RES-A14-F3-P1 | **Fixity checks.** Regular checksum checks on every file and backup, with repair from another copy. | service | small | [KIT-A22-F3-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RES-A14-F4-P1 | **Preservation formats.** Keeps the authors' files plus JATS and archival PDF, and records each file's format. | library | small | [KIT-A27-F6-P2](../kit/FEATURES.md#kit-a27-shared-engines), [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A14-F5-P1 | **Bulk exports.** Regular full exports of metadata (CC0) and openly licensed papers, with checksums, so others can mirror the archive. | service | medium | [RES-A10-F6-P1](#res-a10-citing-dois-and-permanent-links), [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A14-F6-P1 | **Closing plan.** Links keep working through archives and mirrors, DOIs are pointed at the archive copies, and the steward holds the data, on the kit's wind-down plan. | legal | small | [KIT-A14-F19-P1](../kit/FEATURES.md#kit-a14-co-op-membership-and-member-portal), [RES-A14-F1-P3](#res-a14-long-term-preservation) |  |
| RES-A14-F7-P1 | **Mirrors.** A read-only mirror package for member groups and libraries, on the kit's self-host package. | integration | medium | [KIT-A22-F7-P1](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin), [RES-A14-F5-P1](#res-a14-long-term-preservation) |  |
| RES-A14-F8-P1 | **Format watch.** Watches for formats at risk and converts copies, keeping the originals. | service | medium | [RES-A14-F4-P1](#res-a14-long-term-preservation) |  |

## RES-A15 Indexing and being found

Getting papers into the places researchers look: web and scholar search engines, open catalogues, directories of open journals, subject indexes and library systems, with metadata checked before it leaves.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A15-F1** Found in web search | Find our papers in web and scholar search engines. | core | Scholar search readiness; Site maps |
| **RES-A15-F2** In open catalogues | Find our papers in open research catalogues. | core | Open catalogue feeds |
| **RES-A15-F3** Metadata checks | Know our metadata is complete before it goes out. | core | Metadata checks |
| **RES-A15-F4** Directories of open journals | Get a journal listed in directories of open journals. | next | Directory application helper |
| **RES-A15-F5** Subject indexes | Get papers into the indexes my field uses. | next | Subject index research |
| **RES-A15-F6** Library systems | Find our papers through my library's search and link tools. | next | Library link support |
| **RES-A15-F7** Commercial citation indexes | Know whether our journals can be listed in commercial citation indexes. | later | Commercial index research |

**Parts in RES-A15**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A15-F1-P1 | **Scholar search readiness.** Pages, meta tags, site maps and PDF links laid out as scholar search engines' published inclusion guidelines ask. | library | small | [RES-A3-F1-P2](#res-a3-reading-a-paper) |  |
| RES-A15-F1-P2 | **Site maps.** Site maps of every paper and version, updated as papers are posted. | service | small | [KIT-A28-F2-P2](../kit/FEATURES.md#kit-a28-own-sites-public-pages-and-feeds) |  |
| RES-A15-F2-P1 | **Open catalogue feeds.** DOI deposits and OAI-PMH that open catalogues and repository aggregators harvest, with a check that papers appear. | integration | small | [RES-A2-F5-P2](#res-a2-search-browse-and-following), [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A15-F3-P1 | **Metadata checks.** Checks each deposit for an abstract, ORCID iDs, ROR IDs, references, licence and funders, and shows gaps to the authors. | library | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A15-F4-P1 | **Directory application helper.** Checks a journal against the published criteria of open journal directories and gathers what the application needs. | interface | small | [RES-A8-F5-P2](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A15-F5-P1 | **Subject index research.** What subject indexes in life sciences, physics, computing, economics and other fields ask for, such as full-text XML or a publishing record. | research | medium | none |  |
| RES-A15-F6-P1 | **Library link support.** OpenURL support, title lists for libraries, and links that send readers to the open copy. | integration | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper) |  |
| RES-A15-F7-P1 | **Commercial index research.** What commercial citation indexes ask of journals and whether their terms fit us. | research | small | none |  |

## RES-A16 Commands, AI agents and integrations

Everything a person can do, a script or AI agent can do through the same commands, with people-only actions kept for people; plus webhooks, fair bulk access, reference manager support and deposits to institutional repositories.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A16-F1** Use Spare Research from a script | Search, read, submit and review from a script through the same commands as the app. | core | Research command schemas; Headless check |
| **RES-A16-F2** Connect an AI agent | Let my AI agent search, read and draft for me, asking me before anything is posted or sent. | core | Research tools for agents; What agents may not do |
| **RES-A16-F3** Reference managers | Save any paper straight into my reference manager. | core | Reference manager support |
| **RES-A16-F4** Webhooks | Be told when papers, reviews or updates are posted. | core | Research webhooks |
| **RES-A16-F5** Fair bulk access | Download metadata and papers in bulk, fairly. | core | Fair bulk access |
| **RES-A16-F6** Institutional repositories | Have my papers copied to my institution's repository automatically. | next | Repository deposits |
| **RES-A16-F7** Add-ins | Add field-specific checks and review forms as add-ins. | later | Research add-ins |
| **RES-A16-F8** Writing tools | Send a paper straight from my writing tool. | later | Writing tool links |

**Parts in RES-A16**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A16-F1-P1 | **Research command schemas.** Every action described once as a command for the kit's command API, SDKs and command-line tool. | library | medium | [KIT-A15-F1-P3](../kit/FEATURES.md#kit-a15-sync-history-and-storage), [KIT-A16-F1-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), [KIT-A16-F3-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RES-A16-F1-P2 | **Headless check.** The kit's release check that every action on every screen maps to a command. | integration | small | [KIT-A16-F1-P5](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RES-A16-F2-P1 | **Research tools for agents.** The MCP server for Spare Research, built from the command schemas, with paper text marked as outside content. | integration | small | [KIT-A17-F1-P1](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F5-P1](../kit/FEATURES.md#kit-a17-ai-access), [RES-A16-F1-P1](#res-a16-commands-ai-agents-and-integrations) |  |
| RES-A16-F2-P2 | **What agents may not do.** Agents cannot be authors or reviewers, make declarations, post, vote or decide; other actions ask the person where it matters. | library | small | [KIT-A17-F9-P2](../kit/FEATURES.md#kit-a17-ai-access), [KIT-A17-F2-P1](../kit/FEATURES.md#kit-a17-ai-access) |  |
| RES-A16-F3-P1 | **Reference manager support.** Meta tags, embedded citation data and export links that common reference managers already read, with nothing to install. | library | small | [RES-A3-F1-P2](#res-a3-reading-a-paper), [RES-A10-F3-P1](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A16-F4-P1 | **Research webhooks.** Signed webhooks for new papers, versions, reviews and updates. | integration | small | [KIT-A16-F2-P1](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RES-A16-F5-P1 | **Fair bulk access.** Published rate limits and bulk exports instead of scraping, with personal details limited by the kit's bulk copy limits. | integration | small | [KIT-A16-F1-P3](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), [KIT-A16-F1-P4](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations), [RES-A14-F5-P1](#res-a14-long-term-preservation) |  |
| RES-A16-F6-P1 | **Repository deposits.** Sends papers to institutional repositories through the SWORD protocol and COAR Notify, with the author's consent. | integration | medium | [RES-A14-F1-P2](#res-a14-long-term-preservation) |  |
| RES-A16-F7-P1 | **Research add-ins.** Sandboxed add-ins for checks, review forms and viewers on the kit's plugin host. | integration | medium | [KIT-A16-F5-P2](../kit/FEATURES.md#kit-a16-plugins-open-api-and-integrations) |  |
| RES-A16-F8-P1 | **Writing tool links.** Submits from open writing tools through the command API. | integration | small | [RES-A4-F9-P1](#res-a4-submitting-a-paper) |  |

## RES-A17 Accessibility, languages and countries

For researchers everywhere: every screen accessible, the interface and papers in any language and script, titles and abstracts in several languages, and a rules pack for each country's law and institutions.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A17-F1** Accessible screens | Use every screen with a screen reader or a keyboard. | core | Accessible components; Accessibility statement |
| **RES-A17-F2** Use it in my language | Use the app in my language, with local dates and numbers. | core | Research interface translations |
| **RES-A17-F3** Papers in any language | Post a paper in any language, with its title and abstract in more than one. | core | Titles and abstracts in several languages; Every script |
| **RES-A17-F4** Country rules packs | Use Spare Research with my country's law and institutions. | core | Research rules in country packs |
| **RES-A17-F5** Display settings | Change text size, contrast and motion. | core | Display settings |
| **RES-A17-F6** Easy read | Read the submission and rights steps in easy-read form. | next | Easy-read guides |

**Parts in RES-A17**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A17-F1-P1 | **Accessible components.** Every screen on the kit's accessible components, tested with disabled researchers before release. | integration | small | [KIT-A19-F2-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A19-F2-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A17-F1-P2 | **Accessibility statement.** An accessibility statement and barrier reports for Spare Research. | integration | small | [KIT-A19-F2-P4](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A17-F2-P1 | **Research interface translations.** All words on screen on the kit's translation framework, with research terms in the glossary for translators. | integration | medium | [KIT-A19-F1-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A19-F1-P2](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A17-F3-P1 | **Titles and abstracts in several languages.** Authors give titles and abstracts in as many languages as they like, each marked with its language. | data | small | [RES-A4-F4-P1](#res-a4-submitting-a-paper), [KIT-A19-F1-P6](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A17-F3-P2 | **Every script.** Right-to-left and non-Latin scripts in the web view and PDFs, on the kit's text engine. | integration | small | [KIT-A19-F7-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A27-F5-P1](../kit/FEATURES.md#kit-a27-shared-engines) |  |
| RES-A17-F4-P1 | **Research rules in country packs.** Adds rights, legal deposit, ethics, ISSN centre and funder rules to each kit country pack, with New Zealand as the first worked example and others added as members ask. | data | medium | [KIT-A19-F6-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [KIT-A19-F9-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries), [RES-A5-F3-P2](#res-a5-rights-licences-and-publisher-agreements) |  |
| RES-A17-F5-P1 | **Display settings.** The kit's display settings applied to the web view and every screen. | integration | small | [KIT-A19-F3-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |
| RES-A17-F6-P1 | **Easy-read guides.** Easy-read versions of the submission and rights steps, on the kit's easy-read pages. | data | small | [KIT-A19-F8-P1](../kit/FEATURES.md#kit-a19-languages-accessibility-and-countries) |  |

## RES-A18 Apps on every device

The shared Spareware frame in the research colour: in any browser, installable on phones and computers, with links that open the right screen and a command search.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A18-F1** Use it in any browser | Use Spare Research in any modern browser, on a phone or a desk. | core | Research app shell |
| **RES-A18-F2** Install the app | Install Spare Research on a phone or computer. | core | Installable app |
| **RES-A18-F3** Links that open the right screen | Open a paper, review or journal link in the app or the browser. | core | Deep links |
| **RES-A18-F4** Find any command | Type to find and run any action. | core | Command search |
| **RES-A18-F5** Licences and credits | See every open library, style and data set the app uses, with its licence. | core | Licences and credits screen |

**Parts in RES-A18**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A18-F1-P1 | **Research app shell.** The header, navigation, account menu and research colour on the kit's app frame, laid out for phones first. | integration | small | [KIT-A20-F1-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A18-F2-P1 | **Installable app.** The kit's installable app wrappers for Spare Research. | integration | small | [KIT-A20-F2-P2](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A18-F3-P1 | **Deep links.** Paper, review and journal links open the right screen, on the kit's deep links. | integration | small | [KIT-A20-F4-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |
| RES-A18-F4-P1 | **Command search.** Every research action in the kit's command search. | integration | small | [KIT-A20-F10-P1](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps), [RES-A16-F1-P1](#res-a16-commands-ai-agents-and-integrations) |  |
| RES-A18-F5-P1 | **Licences and credits screen.** Lists the open libraries, citation styles, fonts and data sets Spare Research uses, with each licence, on the kit's credits screen. | integration | small | [KIT-A20-F1-P3](../kit/FEATURES.md#kit-a20-app-shell-and-phone-apps) |  |

## RES-A19 Running costs, the cost ledger and members' say

Free to read and free to publish: the only costs are running costs (servers, storage, DOIs, preservation), shown on the open cost ledger, with members voting on the rules and on how costs are covered.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A19-F1** Free to read and to publish | Read and publish without paying anything. | core | No reading or publishing fees; Research costs on the ledger |
| **RES-A19-F2** What a paper costs to run | See the running cost of one paper, from storage to its DOI. | core | Cost per paper |
| **RES-A19-F3** Who pays for DOIs | Know who pays for DOIs, and vote on it. | core | DOI fee rule |
| **RES-A19-F4** Free storage allowance | Know how much data I can attach for free. | core | Free storage allowance |
| **RES-A19-F5** Help from institutions and funders | Let libraries, institutions and funders help cover running costs, in the open. | core | Running cost support |
| **RES-A19-F6** Members decide the rules | Vote on who can publish, how reviews work and how costs are covered. | core | Research proposals and votes; Who votes on research rules |
| **RES-A19-F7** Elected screeners and panels | Know screeners and integrity panels are chosen by members. | core | Elected screeners and panels |
| **RES-A19-F8** Journal staff paid at cost | Let a journal pay a managing editor, with the cost shown openly. | later | Journal staff costs |

**Parts in RES-A19**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A19-F1-P1 | **No reading or publishing fees.** A rule, changed only by member vote, that reading and posting are free; running costs are covered in the way members decide. | library | small | [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A19-F1-P2 | **Research costs on the ledger.** Servers, storage, DOI fees, preservation archives and screening support as monthly lines on the kit's ledger. | integration | small | [KIT-A13-F1-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A13-F1-P3](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |
| RES-A19-F2-P1 | **Cost per paper.** Works out the cost of a paper from the ledger and usage metering, and shows it on the ledger page. | library | small | [KIT-A13-F2-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A13-F3-P6](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |
| RES-A19-F3-P1 | **DOI fee rule.** The members' choice on DOI fees, a shared running cost or paid by each journal; until they decide, shown as a running cost. | library | small | [RES-A10-F2-P1](#res-a10-citing-dois-and-permanent-links), [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A19-F4-P1 | **Free storage allowance.** A free allowance per paper set by member vote, storage at cost above it, and a hardship rule. | integration | small | [KIT-A13-F3-P7](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A15-F7-P5](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A19-F5-P1 | **Running cost support.** Donations, grants and institutional support taken through the kit's funding records, each shown on the ledger and giving no say over what is published. | integration | small | [KIT-A13-F12-P1](../kit/FEATURES.md#kit-a13-open-cost-ledger), [KIT-A13-F12-P2](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |
| RES-A19-F6-P1 | **Research proposals and votes.** Settings for who can publish, review identity, screening rules and fees, changed only through the kit's proposals and votes. | integration | small | [KIT-A12-F1-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), [KIT-A12-F3-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings), [KIT-A12-F4-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A19-F6-P2 | **Who votes on research rules.** Which members vote on Spare Research's rules: authors, reviewers, editors, readers, or all members. | legal | small | [KIT-A12-F5-P2](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A19-F7-P1 | **Elected screeners and panels.** Screeners and integrity panel members elected on the kit's elections. | integration | small | [KIT-A12-F6-P1](../kit/FEATURES.md#kit-a12-member-voting-and-meetings) |  |
| RES-A19-F8-P1 | **Journal staff costs.** Whether and how journals pay staff, recorded as their own ledger lines. | legal | small | [KIT-A13-F11-P3](../kit/FEATURES.md#kit-a13-open-cost-ledger) |  |

## RES-A20 Safety, moderation and legal duties

Reports, moderation with a public log, care with personal details in a permanent record, defamation in reviews, legal requests, sanctions, and every part of Spare Research built legally.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A20-F1** Report anything | Report a paper, review, comment or profile. | core | Report from every page |
| **RES-A20-F2** Moderation in the open | Know every removal is logged with its rule, and appeal it. | core | Removals and appeals |
| **RES-A20-F3** Privacy in a permanent record | Have my personal data handled lawfully, even in a record meant to last. | core | Erasure and the research record; Research privacy notices |
| **RES-A20-F4** No attacks in reviews | Know reviews and comments cannot be used to attack people. | core | Defamation in reviews and comments |
| **RES-A20-F5** Legal requests | Know court and police requests are handled by a published process. | core | Legal requests |
| **RES-A20-F6** Illegal content | Know illegal material is removed as the law requires. | core | Illegal content duties |
| **RES-A20-F7** Terms in plain words | Read short terms and conduct rules before submitting or reviewing. | core | Research terms |
| **RES-A20-F8** Built legally | Know every part of Spare Research is our own work or used under a licence that allows it, and that it copies no publisher's site. | core | Research licence register; Name and interface review |
| **RES-A20-F9** Sanctions and export rules | Know researchers in every country can publish wherever the law allows. | next | Sanctions and publishing |
| **RES-A20-F10** Duties in other countries | Use Spare Research with each country's platform duties met. | later | Platform duties by country |

**Parts in RES-A20**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A20-F1-P1 | **Report from every page.** The kit's report forms on every paper, review, comment and profile. | integration | small | [KIT-A10-F3-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A20-F2-P1 | **Removals and appeals.** Every removal in the kit's public log with its rule, the reason sent to the person, and appeals. | integration | small | [KIT-A10-F7-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F8-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A11-F5-P1](../kit/FEATURES.md#kit-a11-disputes-and-appeals) |  |
| RES-A20-F3-P1 | **Erasure and the research record.** How deletion requests sit beside a permanent record: what research and archiving exemptions allow, what is removed (contact details, accounts) and what stays (authorship), in each country (*needs a lawyer*). | legal | medium | [KIT-A18-F12-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A15-F1-P5](../kit/FEATURES.md#kit-a15-sync-history-and-storage) |  |
| RES-A20-F3-P2 | **Research privacy notices.** Collection notices for authors, reviewers and people named in papers. | legal | small | [KIT-A18-F5-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RES-A20-F4-P1 | **Defamation in reviews and comments.** What published reviews and comments may say about people, and how complaints are handled under each country's law. | legal | small | [KIT-A9-F2-P2](../kit/FEATURES.md#kit-a9-ratings-and-reputation) |  |
| RES-A20-F5-P1 | **Legal requests.** Court and police requests about papers and accounts on the kit's published process, counted in reports. | integration | small | [KIT-A10-F9-P3](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A20-F6-P1 | **Illegal content duties.** What the kit's research on hosts' duties means for papers and data, with known-content matching on uploaded images. | integration | small | [KIT-A10-F9-P1](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation), [KIT-A10-F10-P2](../kit/FEATURES.md#kit-a10-safety-reports-and-moderation) |  |
| RES-A20-F7-P1 | **Research terms.** Plain terms and conduct rules for authors, reviewers and editors on the kit's versioned terms. | legal | small | [KIT-A18-F4-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [KIT-A18-F4-P2](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |
| RES-A20-F8-P1 | **Research licence register.** Every library, style, font and data set in the kit's dependency and licence checks, with no GPL or AGPL code in the core. | legal | small | [KIT-A22-F5-P2](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RES-A20-F8-P2 | **Name and interface review.** Checks that names, icons and pages are our own and that no publisher's or preprint server's interface is imitated. | legal | small | [KIT-A22-F2-P6](../kit/FEATURES.md#kit-a22-running-it-hosting-security-and-admin) |  |
| RES-A20-F9-P1 | **Sanctions and publishing.** How sanctions laws treat publishing and reviewing with people in sanctioned countries, and export rules on some research (*needs a lawyer*). | legal | medium | none |  |
| RES-A20-F10-P1 | **Platform duties by country.** What the kit's overseas privacy and platform duties mean for a research archive. | legal | medium | [KIT-A18-F13-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties) |  |

## RES-A21 Moving in and taking data out

No lock-in: authors take every paper, review and history with them, journals leave with all their content and DOIs, and past papers come in from ORCID and other archives.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A21-F1** Take my work out | Download every paper, review and message I have, in open formats. | core | Author export |
| **RES-A21-F2** Move a journal out | Move a journal elsewhere with all its papers, reviews and DOIs. | core | Journal export |
| **RES-A21-F3** Close my account | Close my account and know what stays in the record. | core | Closing an account |
| **RES-A21-F4** Bring my past papers | List my past papers from my ORCID record and link open copies. | next | Past papers from ORCID |

**Parts in RES-A21**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A21-F1-P1 | **Author export.** Every paper, version, review and message in open formats, on the kit's data export. | integration | small | [KIT-A18-F1-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [RES-A3-F2-P1](#res-a3-reading-a-paper) |  |
| RES-A21-F2-P1 | **Journal export.** The whole journal in JATS and open formats, with help pointing its DOIs at the new host. | service | medium | [RES-A3-F2-P1](#res-a3-reading-a-paper), [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |
| RES-A21-F3-P1 | **Closing an account.** Closes the account on the kit's account deletion, keeping authorship on posted papers as the terms say. | integration | small | [KIT-A18-F2-P1](../kit/FEATURES.md#kit-a18-privacy-data-rights-and-legal-duties), [RES-A20-F3-P1](#res-a20-safety-moderation-and-legal-duties) |  |
| RES-A21-F4-P1 | **Past papers from ORCID.** Reads the person's works from ORCID, links open copies, and hosts only those they may share. | integration | small | [RES-A1-F3-P3](#res-a1-getting-started-accounts-researcher-profiles-and-orcid), [RES-A5-F2-P1](#res-a5-rights-licences-and-publisher-agreements) |  |

## RES-A22 Conference and workshop review

Later: reviewing many papers at once for a conference or workshop, with calls for papers, bidding, deadlines, discussion and decisions, as open review platforms for conferences do.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| **RES-A22-F1** Call for papers | Run a call for papers with deadlines and tracks. | later | Calls for papers |
| **RES-A22-F2** Reviewer bidding | Let reviewers bid on papers and get a fair assignment. | later | Bidding and assignment |
| **RES-A22-F3** Discussion among reviewers | Let reviewers and chairs discuss each paper. | later | Reviewer discussion |
| **RES-A22-F4** Decisions for a whole round | Make and send decisions for a whole round at once. | later | Decisions in bulk |
| **RES-A22-F5** Proceedings | Publish accepted papers as proceedings. | later | Proceedings |

**Parts in RES-A22**

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| RES-A22-F1-P1 | **Calls for papers.** A conference or workshop as a venue like a journal, with deadlines and tracks. | data | medium | [RES-A8-F1-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A22-F2-P1 | **Bidding and assignment.** Reviewers bid on papers; assignment balances bids, expertise, load and conflicts. | library | large | [RES-A7-F2-P3](#res-a7-open-peer-review), [RES-A7-F13-P1](#res-a7-open-peer-review) |  |
| RES-A22-F3-P1 | **Reviewer discussion.** A private or public discussion for each paper among reviewers and chairs, on the kit's threads. | integration | small | [KIT-A8-F12-P1](../kit/FEATURES.md#kit-a8-messaging-and-notifications) |  |
| RES-A22-F4-P1 | **Decisions in bulk.** Accept and decline lists for a whole round, with letters. | interface | medium | [RES-A8-F4-P1](#res-a8-journals-on-top-and-editorial-work) |  |
| RES-A22-F5-P1 | **Proceedings.** Accepted papers as a proceedings volume with its own page and DOIs. | library | small | [RES-A10-F2-P3](#res-a10-citing-dois-and-permanent-links) |  |

## Questions for assessing this map and its demo

- Walk through a researcher's month (post a preprint, get it screened, be invited to review, review on a phone, reply to reviews, cite a paper, read a retraction notice): does every step land on a core feature, or is something missing?
- Are the core features the smallest set for a first usable release? Should the first release be a preprint server with open reviews, and journals on top come next?
- Peer review (open decision): reviews published with reviewers' names, published anonymously, or kept private as most journals do now. Should each journal choose, or members set one rule? Does the demo make each choice and its effect clear?
- Who can publish (open decision): anyone with an account, researchers with an ORCID iD, or endorsement by someone already publishing, as arXiv does. What does each mean for junk, paper mills and new researchers with no network, and does the endorsement step in the demo feel fair?
- DOIs (open decision): Crossref, DataCite or both; one DOI for each version or one for the paper; covered as a running cost or paid by each journal. What would the first year cost at 1,000, 10,000 and 100,000 papers?
- Rights: does the rights check in the demo keep paywalled copies out without blocking authors who may legally post? Who keeps each country's rules pack (secondary publication rights, funder policies) up to date?
- Existing publisher agreements: what happens when an author signed their rights away, or the journal is missing from Open Policy Finder?
- Screening: who screens, how fast, and with what checklist? Will readers ever mistake a screened preprint for a reviewed paper in the demo?
- Review quality and fraud: are the reviewer identity checks, conflict checks and paper mill signals enough, and are signals always hints for a person rather than automatic verdicts?
- Corrections and retractions: does the demo show updates openly and fairly, including the difference between an honest error and misconduct, and do updates reach other indexes?
- Preservation: which outside archives and national libraries will hold copies, what do they cost, and do links keep working if Spare Research closes?
- Indexing: what does it take for papers to show in scholar search engines, open catalogues, directories of open journals and subject indexes from the first release?
- Health research: is the 'not yet reviewed' notice strong enough for unreviewed clinical work, and should health papers get extra screening?
- Data about people and Indigenous data: are the rules for participants' data, controlled access and Indigenous data (CARE, and in the New Zealand example Māori data principles) right, and who are our partners in each country?
- AI: does the demo make clear that every paper declares its AI use, AI-generated papers sit in their own section, agents can never be authors or reviewers, and unpublished papers never go into outside AI tools?
- Legal: in each country, which parts need a lawyer before launch (the rights declaration, erasure beside a permanent record, defamation in published reviews, dangerous research, sanctions, legal deposit)?
- Licences: can the tools we plan to use (reference extractor, citation processor and styles, maths renderer, LaTeX build, format converters) be used under our rules, with GPL and AGPL kept out of the core?
- Does the demo use only our own names, look and icons, imitate no publisher's or preprint server's interface, and name other organisations only to say what we replace or work with?
- Headless: can a script and an agent do everything in the demo through the same commands, with declarations, posting, reviews and votes kept for people?
- Kit dependencies: Spare Research's first release needs the kit's search service, anchored comments, durable timers, public pages, review queue, preview service and PDF engine. Are they all core in the kit, and should anything here move down into the kit, such as the JATS store, reference extraction or citation styles?
- Are the part sizes believable? Which large parts (conversion to JATS, the LaTeX build, DOI deposits, overlap checks, paper mill signals, the web reading view) should become work-board pieces first?
