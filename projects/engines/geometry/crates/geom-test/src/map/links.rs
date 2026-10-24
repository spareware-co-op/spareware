//! Links in header text: finding them, telling a link to source code from a link to a paper,
//! reading where a link points inside a repository on a code-hosting site, and whether it is pinned
//! to a commit.
//!
//! What it does: [`find`] lists the links in a text: web addresses (`https://…` and `http://…`,
//! bare, in `<…>` or in a Markdown link), DOIs written `doi:10.<registrant>/<suffix>`, and
//! repository paths in backticks under [`REPO_LINK_ROOTS`] that end in `.md` or `/`. [`view`]
//! reads a web address on a code-hosting site into the repository it names, the revision it names
//! and whether it shows code. [`is_code`] says whether an address points at source code,
//! [`is_pinned`] whether it names a full commit, [`repository`] which repository or file it
//! belongs to (so a credit line can be matched with a link elsewhere), and [`source_key`] which
//! source it is (one file at one revision, whatever line range it shows).
//!
//! Guarantees:
//!
//! - **Every link once, in order.** A link that appears twice is listed once, at its first place.
//! - **Trailing punctuation is not part of a link.** A sentence's `.`, `,`, `;` or `:` after an
//!   address, and the `>` or `)` that closes it, are left out.
//! - **A DOI is a whole DOI.** `doi:10.` must be followed by a registrant code (digits, with
//!   sub-divisions after full stops), a `/` and a suffix that is not empty; a `doi.org` address is
//!   a link only when its path is such a DOI.
//! - **Every code host's layout, read by its revision slot.** A link is on a code-hosting site
//!   when its host is one of [`FORGE_HOSTS`], starts with one of [`FORGE_HOST_PREFIXES`] or ends
//!   with one of [`FORGE_HOST_SUFFIXES`], or when its path or query has one of the layouts below,
//!   on any host. It is pinned when the revision slot of its layout, and only that slot, is a
//!   full commit name: 40 hexadecimal digits for SHA-1 or 64 for SHA-256, in lower case.
//!   - GitHub's layout, shared by Gitea, Forgejo, Codeberg, Gitee, Bitbucket, sourcehut and
//!     `SourceForge`: `<owner>/<repository>/<view>/<revision>/<path>`, a view one of [`VIEWS`]
//!     (Gitea's `src/commit/<revision>`, `src/branch/…` and `src/tag/…` included);
//!     `raw.githubusercontent.com/<owner>/<repository>/<revision>/<path>`; and gists,
//!     `gist.github.com/<user>/<id>/<revision>`.
//!   - GitLab's, on any host: `<group>/…/<repository>/-/<view>/<revision>/<path>`.
//!   - Gitiles' (`*.googlesource.com` and any Gerrit host): `<repository>/+/<revision>/<path>`,
//!     and its other `+` commands.
//!   - cgit's (hosts starting `cgit.` or `git.`, or a `cgit` path segment):
//!     `<repository>/<view>/<path>?id=<revision>`; without an `id`, GitHub's revision slot.
//!   - gitweb's, on any host: `?p=<repository>;a=<action>;f=<path>;hb=<revision>`.
//! - **Conservative about code.** An address is code when it shows a file, folder, commit or diff
//!   in a repository on a code-hosting site, when it is a gist, or when its last path segment, or
//!   a value in its query, ends in one of [`CODE_EXTENSIONS`] (`.rs.html` and other source pages
//!   included). A project's home page, a paper or a documentation page is not.
//!
//! Method: URI syntax per RFC 3986 (<https://www.rfc-editor.org/rfc/rfc3986>), reduced to what a
//! header holds: an address runs from its scheme to the first space, quote, backtick, `<`, `>`, `)`
//! or `]`; its query is split at `&` and `;`. The DOI syntax is ISO 26324's, as the DOI Foundation
//! states it (<https://www.doi.org/the-identifier/what-is-a-doi/>): a prefix `10.` and a registrant
//! code, a `/`, and a suffix. The code-hosting layouts are those of npm's `hosted-git-info` (its
//! templates for GitHub, Bitbucket, GitLab, gists and sourcehut, consulted below), Gitiles' view
//! commands (consulted below), gitweb's `p`, `h` and `hb` parameters (gitweb.conf(5),
//! <https://git-scm.com/docs/gitweb.conf>), and cgit's `id` parameter as cgit sites write it. A
//! commit is pinned when it is a full Git object name (Git's hash-function transition,
//! <https://git-scm.com/docs/hash-function-transition>).
//!
//! Cost: linear in the length of the text.
//!
//! Consulted: npm `hosted-git-info` 10.1.1, ISC,
//!   <https://github.com/npm/hosted-git-info/blob/8c3bebb5e42142b7bce06b085b4c1bd1adba7371/lib/hosts.js>,
//!   for the address layouts of GitHub, Bitbucket, GitLab, gists and sourcehut and where each puts
//!   its revision. No code copied.
//! Consulted: Gitiles, Apache-2.0,
//!   <https://github.com/google/gitiles/blob/e54582fdd2a8783a6078f3a25eb08bb622f2e497/java/com/google/gitiles/ViewFilter.java>,
//!   for its `+` view commands and the revision that follows them. No code copied.

/// What kind of link it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// A web address.
    Web,
    /// A DOI written `doi:10.…`.
    Doi,
    /// A path in this repository, in backticks.
    Repo,
}

/// One link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The address, DOI or path, as written.
    pub text: String,
    /// Its kind.
    pub kind: LinkKind,
    /// Its byte offset in the text.
    pub at: usize,
}

/// The folders a repository link may point into: the kernel's own folder (its architecture
/// chapters, evidence and generated notes) and the kit's notes (the evidence behind G01).
pub const REPO_LINK_ROOTS: [&str; 2] = ["projects/engines/geometry/", "projects/kit/notes/"];

/// The source file extensions that make a web address a link to code.
pub const CODE_EXTENSIONS: [&str; 50] = [
    "c", "h", "cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx", "h++", "inl", "ipp", "tcc", "cu",
    "cuh", "cl", "glsl", "hlsl", "wgsl", "metal", "rs", "py", "pyx", "f", "f77", "f90", "f95",
    "f03", "f08", "for", "java", "js", "mjs", "ts", "go", "jl", "m", "mm", "cs", "swift", "kt",
    "scala", "hs", "ml", "lisp", "rb", "lua", "zig", "s", "asm",
];

/// Hosts whose repositories are addressed `<owner>/<repository>/…`: GitHub and the forges that
/// share its layout, and the public GitLab instances whose names do not start `gitlab.`.
pub const FORGE_HOSTS: [&str; 15] = [
    "github.com",
    "gist.github.com",
    "raw.githubusercontent.com",
    "gist.githubusercontent.com",
    "codeberg.org",
    "gitee.com",
    "bitbucket.org",
    "notabug.org",
    "git.sr.ht",
    "sourceforge.net",
    "gitlab.com",
    "salsa.debian.org",
    "invent.kde.org",
    "framagit.org",
    "foss.heptapod.net",
];

/// Host name prefixes of self-hosted forges: GitLab, Gitea and Forgejo instances, and cgit and
/// gitweb installations (`git.kernel.org`, `git.savannah.gnu.org`).
pub const FORGE_HOST_PREFIXES: [&str; 5] = ["gitlab.", "gitea.", "forgejo.", "cgit.", "git."];

/// Host name suffixes of forges: Gitiles on Google's hosts.
pub const FORGE_HOST_SUFFIXES: [&str; 1] = [".googlesource.com"];

/// The path segments that open a view inside a repository, after which the revision (or, on
/// cgit, the path) follows.
pub const VIEWS: [&str; 13] = [
    "blob", "tree", "raw", "blame", "commit", "commits", "src", "plain", "log", "diff", "patch",
    "ci", "snapshot",
];

/// Gitiles' view commands that show a revision's files or changes.
const GITILES_CODE_COMMANDS: [&str; 5] = ["+", "+show", "+blame", "+diff", "+log"];

/// gitweb's actions that show a file, folder, commit or diff.
const GITWEB_CODE_ACTIONS: [&str; 8] = [
    "blob",
    "blob_plain",
    "tree",
    "commit",
    "commitdiff",
    "blobdiff",
    "blame",
    "history",
];

/// Whether `c` ends a web address.
fn ends_address(c: char) -> bool {
    c.is_whitespace() || matches!(c, '<' | '>' | '"' | '`' | ')' | ']' | '\'')
}

/// A link's text without the punctuation a sentence puts after it.
fn trim_trailing(text: &str) -> &str {
    text.trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

/// Whether `text` (after `doi:` or `doi.org/`) is a DOI: `10.`, a registrant code of digits with
/// sub-divisions after full stops, a `/` and a suffix that is not empty.
#[must_use]
pub fn is_doi(text: &str) -> bool {
    let Some(rest) = text.strip_prefix("10.") else {
        return false;
    };
    let Some((registrant, suffix)) = rest.split_once('/') else {
        return false;
    };
    !suffix.is_empty()
        && !suffix.contains(char::is_whitespace)
        && !registrant.is_empty()
        && registrant
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// Every link in `text`, in order, each once.
#[must_use]
pub fn find(text: &str) -> Vec<Link> {
    let mut found: Vec<Link> = Vec::new();
    let mut add = |link: Link| {
        if !link.text.is_empty() && !found.iter().any(|l| l.text == link.text) {
            found.push(link);
        }
    };
    for (at, _) in text.char_indices() {
        let rest = text.get(at..).unwrap_or_default();
        if rest.starts_with("https://") || rest.starts_with("http://") {
            let preceded_by_scheme_char = text
                .get(..at)
                .and_then(|before| before.chars().next_back())
                .is_some_and(|c| c.is_ascii_alphanumeric());
            if !preceded_by_scheme_char {
                let end = rest.find(ends_address).unwrap_or(rest.len());
                let address = trim_trailing(rest.get(..end).unwrap_or_default());
                if web_address_reads(address) {
                    add(Link {
                        text: address.to_owned(),
                        kind: LinkKind::Web,
                        at,
                    });
                }
            }
        } else if let Some(doi) = rest.strip_prefix("doi:") {
            let end = doi.find(ends_address).unwrap_or(doi.len());
            let doi = trim_trailing(doi.get(..end).unwrap_or_default());
            if is_doi(doi) {
                add(Link {
                    text: format!("doi:{doi}"),
                    kind: LinkKind::Doi,
                    at,
                });
            }
        } else if let Some(path) = rest.strip_prefix('`')
            && let Some((inside, _)) = path.split_once('`')
            && REPO_LINK_ROOTS.iter().any(|root| inside.starts_with(root))
            && (inside.ends_with('/')
                || std::path::Path::new(inside)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("md")))
            && !inside.contains(char::is_whitespace)
        {
            add(Link {
                text: inside.to_owned(),
                kind: LinkKind::Repo,
                at,
            });
        }
    }
    found.sort_by_key(|l| l.at);
    found
}

/// Whether a web address names a host, and, on `doi.org`, a whole DOI.
fn web_address_reads(address: &str) -> bool {
    let Some(p) = parts(address) else {
        return false;
    };
    if !p.host.chars().next().is_some_and(char::is_alphanumeric) {
        return false;
    }
    if matches!(p.host.as_str(), "doi.org" | "dx.doi.org") {
        return is_doi(&p.path.join("/"));
    }
    true
}

/// The parts of a web address the checks read.
struct Parts<'a> {
    /// The host, in lower case, without user information or port.
    host: String,
    /// The path's segments that are not empty.
    path: Vec<&'a str>,
    /// The query's `key=value` pairs, split at `&` and `;`; a pair without `=` has an empty value.
    query: Vec<(&'a str, &'a str)>,
}

impl Parts<'_> {
    /// The first value of `key` in the query.
    fn query(&self, key: &str) -> Option<&str> {
        self.query.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }
}

/// The parts of a web address, or `None` when it is not one.
fn parts(url: &str) -> Option<Parts<'_>> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let rest = rest.split('#').next().unwrap_or_default();
    let (before_query, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (authority, path) = before_query.split_once('/').unwrap_or((before_query, ""));
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    Some(Parts {
        host,
        path: path.split('/').filter(|p| !p.is_empty()).collect(),
        query: query
            .split(['&', ';'])
            .filter(|p| !p.is_empty())
            .map(|p| p.split_once('=').unwrap_or((p, "")))
            .collect(),
    })
}

/// Where a link points inside a repository on a code-hosting site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    /// The repository's identity: `host/path-to-repository` (a gitweb repository adds `?p=` and
    /// its name; a raw or gist address is read as the repository it serves).
    pub repository: String,
    /// The revision the link names, if its layout has a revision slot and the slot is filled.
    pub revision: Option<String>,
    /// Whether it shows code: a file, folder, commit or diff, rather than a page about the
    /// repository.
    pub code: bool,
}

/// Whether `host` is a code-hosting site by its name.
fn is_forge_host(host: &str) -> bool {
    FORGE_HOSTS.contains(&host)
        || FORGE_HOST_PREFIXES.iter().any(|p| host.starts_with(p))
        || FORGE_HOST_SUFFIXES.iter().any(|s| host.ends_with(s))
}

/// `host/segments`, joined.
fn identity(host: &str, segments: &[&str]) -> String {
    let mut out = host.to_owned();
    for s in segments {
        out.push('/');
        out.push_str(s);
    }
    out
}

/// A view's fields, as owned values.
fn make_view(repository: String, revision: Option<&str>, code: bool) -> View {
    View {
        repository,
        revision: revision.filter(|r| !r.is_empty()).map(str::to_owned),
        code,
    }
}

/// Where `url` points inside a repository on a code-hosting site, or `None` when it is not on
/// one (module header: the layouts read).
#[must_use]
pub fn view(url: &str) -> Option<View> {
    let p = parts(url)?;
    let host = p.host.as_str();
    let seg = |k: usize| p.path.get(k).copied();
    match host {
        // <owner>/<repository>/<revision>/<path>
        "raw.githubusercontent.com" => {
            let repo = identity("github.com", p.path.get(..2).unwrap_or(&p.path));
            return Some(make_view(repo, seg(2), true));
        }
        // <user>/<id>/raw/<revision>/<file>
        "gist.githubusercontent.com" => {
            let repo = identity("gist.github.com", p.path.get(1..2).unwrap_or_default());
            let revision = if seg(2) == Some("raw") { seg(3) } else { None };
            return Some(make_view(repo, revision, true));
        }
        // <user>/<id>/<revision>, or <id>
        "gist.github.com" => {
            let (id, revision) = if p.path.len() >= 2 {
                (seg(1), seg(2))
            } else {
                (seg(0), None)
            };
            let repo = identity("gist.github.com", id.as_slice());
            return Some(make_view(repo, revision, true));
        }
        _ => {}
    }
    // gitweb: ?p=<repository>;a=<action>;f=<path>;h=<object>;hb=<revision>. A `p` alone is
    // gitweb's only when it names a repository (`….git`) or comes with an action.
    if let Some(project) = p.query("p").filter(|project| {
        std::path::Path::new(project)
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("git"))
            || p.query("a").is_some()
    }) {
        let action = p.query("a").unwrap_or_default();
        let revision = p.query("hb").or_else(|| {
            matches!(action, "commit" | "commitdiff")
                .then(|| p.query("h"))
                .flatten()
        });
        let code = GITWEB_CODE_ACTIONS.contains(&action) || p.query("f").is_some();
        let repo = format!("{}?p={project}", identity(host, &p.path));
        return Some(make_view(repo, revision, code));
    }
    // Gitiles: <repository>/+<command>/<revision>/<path>
    if let Some(k) = p.path.iter().position(|s| s.starts_with('+')) {
        let command = seg(k).unwrap_or_default();
        let repo = identity(host, p.path.get(..k).unwrap_or_default());
        let code = GITILES_CODE_COMMANDS.contains(&command);
        return Some(make_view(
            repo,
            seg(k.saturating_add(1)).filter(|_| code),
            code,
        ));
    }
    // GitLab, on any host: <group>/…/<repository>/-/<view>/<revision>/<path>
    if let Some(k) = p
        .path
        .iter()
        .position(|s| *s == "-")
        .filter(|&k| k > 0 && seg(k.saturating_add(1)).is_some_and(|v| VIEWS.contains(&v)))
    {
        let repo = identity(host, p.path.get(..k).unwrap_or_default());
        return Some(make_view(repo, seg(k.saturating_add(2)), true));
    }
    let cgit = (host.starts_with("cgit.") || (host.starts_with("git.") && host != "git.sr.ht"))
        || p.path.iter().any(|s| matches!(*s, "cgit" | "cgit.cgi"));
    if !is_forge_host(host) && !cgit {
        return None;
    }
    // GitHub's layout, and cgit's: the first view segment ends the repository's path.
    let first = if cgit { 1 } else { 2 };
    let at = (first..p.path.len()).find(|&k| seg(k).is_some_and(|v| VIEWS.contains(&v)));
    let Some(k) = at else {
        // A page about a repository (its home, issues or releases), not code.
        let repo = identity(host, p.path.get(..2).unwrap_or(&p.path));
        return Some(make_view(repo, None, false));
    };
    let repo = identity(host, p.path.get(..k).unwrap_or_default());
    // cgit names the revision in its query; a self-hosted forge on a `git.` host that has none
    // follows GitHub's layout.
    let slot = if seg(k) == Some("src")
        && seg(k.saturating_add(1)).is_some_and(|s| matches!(s, "commit" | "branch" | "tag"))
    {
        seg(k.saturating_add(2))
    } else {
        seg(k.saturating_add(1))
    };
    let revision = cgit.then(|| p.query("id")).flatten().or(slot);
    Some(make_view(repo, revision, true))
}

/// Whether `name` (a file name, or the last segment of a path) ends in a source file's extension,
/// directly or before a viewer's `.html`.
fn has_code_extension(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.strip_suffix(".html").unwrap_or(&name);
    name.rsplit_once('.')
        .is_some_and(|(stem, ext)| !stem.is_empty() && CODE_EXTENSIONS.contains(&ext))
}

/// Whether a web address points at source code.
#[must_use]
pub fn is_code(url: &str) -> bool {
    if view(url).is_some_and(|v| v.code) {
        return true;
    }
    let Some(p) = parts(url) else {
        return false;
    };
    p.path.last().is_some_and(|last| has_code_extension(last))
        || p.query
            .iter()
            .any(|(_, v)| has_code_extension(v.rsplit('/').next().unwrap_or_default()))
}

/// Whether a web address is on a code-hosting site, where a link can and must be pinned.
#[must_use]
pub fn on_forge(url: &str) -> bool {
    view(url).is_some()
}

/// Whether `text` is a full Git object name: 40 or 64 lower-case hexadecimal digits.
#[must_use]
pub fn is_full_commit(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f'))
}

/// Whether the revision slot of a web address on a code-hosting site names a full commit.
#[must_use]
pub fn is_pinned(url: &str) -> bool {
    view(url)
        .and_then(|v| v.revision)
        .is_some_and(|r| is_full_commit(&r))
}

/// Which repository or file a web address belongs to: the repository of its [`view`] on a
/// code-hosting site, otherwise the address without its scheme, query or fragment.
#[must_use]
pub fn repository(url: &str) -> String {
    if let Some(v) = view(url) {
        return v.repository;
    }
    match parts(url) {
        Some(p) => identity(&p.host, &p.path),
        None => url.to_owned(),
    }
}

/// Which source a link names, for the ledger: the link without its fragment, so one file at one
/// revision is one source whatever line range a header shows.
#[must_use]
pub fn source_key(link: &str) -> String {
    link.split('#').next().unwrap_or(link).to_owned()
}

#[cfg(test)]
#[allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::disallowed_methods,
    reason = "tests may use native arithmetic, casts and std reductions"
)]
mod tests {
    use super::*;

    const SHA: &str = "40d72165ceeeb0a61977d6fdf74d929bfdd77b7b";

    #[test]
    fn finds_every_form_once() {
        let text = "Shewchuk (<https://doi.org/10.1007/PL00009321>; code \
                    <https://www.cs.cmu.edu/x/predicates.c>), [EMBER](https://doi.org/10.1145/3528223.3530181), \
                    doi:10.1016/j.cad.2020.102856, see https://example.org/a. Chapter 28 \
                    (`projects/engines/geometry/architecture/28-testing.md`) and \
                    `projects/engines/geometry/architecture/`, not `projects/x.rs` nor `src/lib.md` \
                    nor `projects/spare-x/README.md`; evidence \
                    `projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md`; \
                    again <https://doi.org/10.1007/PL00009321>.";
        let found: Vec<(String, LinkKind)> =
            find(text).into_iter().map(|l| (l.text, l.kind)).collect();
        assert_eq!(
            found,
            [
                ("https://doi.org/10.1007/PL00009321".into(), LinkKind::Web),
                (
                    "https://www.cs.cmu.edu/x/predicates.c".into(),
                    LinkKind::Web
                ),
                (
                    "https://doi.org/10.1145/3528223.3530181".into(),
                    LinkKind::Web
                ),
                ("doi:10.1016/j.cad.2020.102856".into(), LinkKind::Doi),
                ("https://example.org/a".into(), LinkKind::Web),
                (
                    "projects/engines/geometry/architecture/28-testing.md".into(),
                    LinkKind::Repo
                ),
                (
                    "projects/engines/geometry/architecture/".into(),
                    LinkKind::Repo
                ),
                (
                    "projects/kit/notes/G01-evidence/05-exact-arithmetic-determinism.md".into(),
                    LinkKind::Repo
                ),
            ]
        );
        assert_eq!(find("no links, http:// alone is not one: xhttps://a.b"), []);
    }

    #[test]
    fn a_doi_has_a_prefix_a_slash_and_a_suffix() {
        for yes in ["10.1000/182", "10.1007/PL00009321", "10.1000.10/abc"] {
            assert!(is_doi(yes), "{yes}");
        }
        for no in [
            "10.",
            "10.1000",
            "10.1000/",
            "10./x",
            "10.a/x",
            "11.1000/x",
            "10.1..2/x",
        ] {
            assert!(!is_doi(no), "{no}");
        }
        assert_eq!(find("doi:10. and doi:10.1000. and doi:10.1000/"), []);
        assert_eq!(find("https://doi.org/10.1000 and https://doi.org/"), []);
        assert_eq!(find("doi:10.1000/182.")[0].text, "doi:10.1000/182");
    }

    #[test]
    fn code_pinned_and_repository() {
        let geogram = format!(
            "https://github.com/BrunoLevy/geogram/blob/{SHA}/src/lib/geogram/numerics/interval_nt.h#L455-L462"
        );
        assert!(is_code(&geogram));
        assert!(is_pinned(&geogram));
        assert!(on_forge(&geogram));
        assert_eq!(repository(&geogram), "github.com/BrunoLevy/geogram");
        assert_eq!(
            source_key(&geogram),
            format!(
                "https://github.com/BrunoLevy/geogram/blob/{SHA}/src/lib/geogram/numerics/interval_nt.h"
            )
        );
        let master = "https://github.com/rust-lang/compiler-builtins/tree/master/libm/src/math";
        assert!(is_code(master));
        assert!(!is_pinned(master));
        assert_eq!(repository(master), "github.com/rust-lang/compiler-builtins");
        let predicates = "https://www.cs.cmu.edu/afs/cs/project/quake/public/code/predicates.c";
        assert!(is_code(predicates));
        assert!(!on_forge(predicates));
        assert_eq!(
            repository("https://prng.di.unimi.it/splitmix.c#x"),
            "prng.di.unimi.it/splitmix.c"
        );
        assert_eq!(
            repository("https://gitlab.inria.fr/gappa/gappa/-/blob/17e2e9a/README.md"),
            "gitlab.inria.fr/gappa/gappa"
        );
        for not_code in [
            "https://github.com/proptest-rs/proptest",
            "https://doi.org/10.1007/PL00009321",
            "https://www.rfc-editor.org/rfc/rfc8259",
            "https://doc.rust-lang.org/reference/comments.html",
            "https://docs.rs/libm/0.2.16",
        ] {
            assert!(!is_code(not_code), "{not_code}");
        }
        assert!(on_forge("https://github.com/proptest-rs/proptest"));
        assert!(!is_pinned("https://github.com/a/b/blob/7642943/x.rs"));
        assert!(!is_pinned(&format!(
            "https://github.com/a/b/blob/{}/x.rs",
            SHA.to_uppercase()
        )));
        // A full commit name in a path segment that is not the revision slot pins nothing.
        assert!(!is_pinned(&format!(
            "https://github.com/a/b/blob/main/data/{SHA}"
        )));
    }

    #[test]
    fn every_code_host_layout_reads_its_revision() {
        let pinned = [
            format!("https://gitee.com/o/r/blob/{SHA}/src/a.c"),
            format!("https://codeberg.org/o/r/src/commit/{SHA}/a.rs"),
            format!("https://bitbucket.org/o/r/src/{SHA}/a.c"),
            format!("https://git.sr.ht/~u/r/tree/{SHA}/item/a.c"),
            format!("https://salsa.debian.org/g/sub/r/-/blob/{SHA}/a.c"),
            format!("https://chromium.googlesource.com/external/r/+/{SHA}/a.cc"),
            format!("https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c?id={SHA}"),
            format!("https://git.kernel.org/pub/scm/a/b.git/tree/Makefile?h=master&id={SHA}"),
            format!("https://sourceware.org/git/?p=glibc.git;a=blob;f=s_sin.c;hb={SHA}"),
            format!("https://raw.githubusercontent.com/o/r/{SHA}/a.c"),
            format!("https://gist.github.com/u/0123456789abcdef0123/{SHA}"),
            format!("https://sourceforge.net/p/proj/code/ci/{SHA}/tree/a.c"),
        ];
        for url in &pinned {
            assert!(on_forge(url), "{url}");
            assert!(is_code(url), "{url}");
            assert!(is_pinned(url), "{url}");
        }
        let unpinned = [
            "https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c",
            "https://gitee.com/mirrors/example/blob/master/src/a.c",
            "https://salsa.debian.org/science-team/example/-/blob/master/src/a.c",
            "https://invent.kde.org/graphics/example/-/blob/master/src/a.cpp",
            "https://chromium.googlesource.com/external/example/+/refs/heads/main/src/a.cc",
            "https://sourceware.org/git/?p=glibc.git;a=blob;f=sysdeps/ieee754/dbl-64/s_sin.c;hb=HEAD",
            "https://gist.github.com/someone/0123456789abcdef0123",
        ];
        for url in unpinned {
            assert!(on_forge(url), "{url}");
            assert!(is_code(url), "{url}");
            assert!(!is_pinned(url), "{url}");
        }
        assert_eq!(
            repository("https://cgit.freebsd.org/src/tree/lib/msun/src/s_sinpi.c"),
            "cgit.freebsd.org/src"
        );
        assert_eq!(
            repository("https://chromium.googlesource.com/external/r/+/refs/heads/main/a.cc"),
            "chromium.googlesource.com/external/r"
        );
        assert_eq!(
            repository("https://sourceware.org/git/?p=glibc.git;a=blob;f=s_sin.c;hb=HEAD"),
            "sourceware.org/git?p=glibc.git"
        );
        assert!(is_code(
            "https://docs.rs/libm/0.2.16/src/libm/math/sin.rs.html"
        ));
        assert!(!on_forge(
            "https://docs.rs/libm/0.2.16/src/libm/math/sin.rs.html"
        ));
        assert!(is_code("https://example.org/view?file=src/a.cpp"));
    }
}
