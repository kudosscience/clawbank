//! FAL pause-rule gate (ADR-0004 enforcement, Safety 01).
//!
//! At FAL-2 no FAL-3-gated capability may exist anywhere in the tree. This
//! test fails the build — locally and in CI (`rust-test` runs
//! `scripts/ci/rust-check.sh test`, i.e. `cargo test --all`, so the gate
//! cannot be skipped on any PR) — the moment one appears while
//! [`clawbank_safety::FAL_LEVEL`] is still below 3.
//!
//! ## Scope: the whole tree, fail-closed
//!
//! The walk starts at the workspace root and reads every file at any depth,
//! so a smuggled capability trips the gate wherever it lands — including a
//! `Cargo.toml` manifest (an `escrow-sdk` dependency trips it too) and a
//! hidden file such as `.escrow_stub.rs`. Matching is case-,
//! separator-, and camelCase-insensitive (`ComputeMarketplace`,
//! `compute_marketplace`, and `computemarketplace` all match), so
//! identifiers cannot dodge the gate by picking separators. Only
//! non-product paths are skipped: `.git/` and `target/` build output,
//! hidden dot-directories (local tooling clutter such as `.agent/`),
//! symlinked directories (never traversed: no cycles, no escape from the
//! tree), and the generated `Cargo.lock` (which names the curve25519
//! `fiat-crypto` primitive — not a bridge). Prose that must discuss gated capabilities is
//! allowlisted: `docs/adr/` decision records and `docs/safety/` safety docs,
//! plus the two gate-owned files that define the level and this term list. A
//! proposal doc never satisfies the pause rule; the evidence procedure in
//! `docs/safety/fal-level.md` still applies before the level may rise.
//!
//! ## Allowlist
//!
//! Only the paths in `ALLOWLIST_PREFIXES` / `ALLOWLIST_FILES` may name gated
//! terms. Anything else matching is a smuggled capability until the
//! pause-gate evidence (safeguards + evaluations + external reviewer) lands
//! and the level rises — including a *new* file inside
//! `crates/clawbank-safety/` itself, which is deliberately not blanket
//! allowlisted.

use std::path::{Path, PathBuf};

/// FAL-3-gated capability markers, in canonical form: lowercase words joined
/// by single spaces (matched case-insensitively).
///
/// Covers the ADR-0004 trigger — escrow, lending/borrowing, margin/yield,
/// bridges, compute marketplace, policy-based autonomous spending — plus the
/// concrete FAL-3 mechanisms it names (HTLC, multi-sig, fiat).
/// Deliberately absent: bare `borrow` (every `borrow_mut` would trip it) and
/// bare `lend` (trips on `blend`); the longer borrow/lend-family terms below
/// carry that signal without the std collisions.
const GATED_TERMS: &[&str] = &[
    "escrow",
    "lending",
    "lender",
    "lenders",
    "lends",
    "borrowing",
    "borrower",
    "borrowed",
    "borrows",
    "loan",
    "margin",
    "yield",
    "bridge",
    "htlc",
    "fiat",
    "multisig",
    "multi sig",
    "marketplace",
    "compute market",
    "autonomous spend",
    "spending polic",
    "auto spend",
    "autospend",
    "policy based spend",
];

/// Prose subtrees allowed to *discuss* gated capabilities (trailing slash).
const ALLOWLIST_PREFIXES: &[&str] = &["docs/adr/", "docs/safety/"];

/// Gate-owned files allowed to *name* gated terms (exact repo-root paths).
const ALLOWLIST_FILES: &[&str] = &[
    "crates/clawbank-safety/src/lib.rs",
    "crates/clawbank-safety/tests/fal_gate.rs",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn is_allowlisted(rel_unix: &str, prefixes: &[&str], files: &[&str]) -> bool {
    files.contains(&rel_unix) || prefixes.iter().any(|p| rel_unix.starts_with(p))
}

/// Canonicalize text for capability matching: lowercase, split camelCase
/// boundaries, and fold every other non-alphanumeric run to a single space.
/// `ComputeMarketplace`, `compute_marketplace`, and `compute-marketplace`
/// all become `compute marketplace`, so identifiers cannot dodge the gate
/// by picking separators.
fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_alnum = false;
    let mut prev_lower = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            if prev_alnum && prev_lower && ch.is_ascii_uppercase() {
                out.push(' ');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = ch.is_ascii_lowercase();
            prev_alnum = true;
        } else if prev_alnum {
            out.push(' ');
            prev_alnum = false;
            prev_lower = false;
        }
    }
    out
}

/// Canonical gated terms matched against `normalized`: each term matches
/// when it appears word-separated. Multi-word terms additionally match fully
/// concatenated (`computemarketplace`, `spendingpolicy`). Single-word terms
/// match separated-only: the squashed form glues adjacent tokens together,
/// so e.g. `Borrow<str>` would otherwise phantom-match `borrows`.
fn match_terms(normalized: &str) -> Vec<String> {
    let squashed: String = normalized.chars().filter(|c| *c != ' ').collect();
    GATED_TERMS
        .iter()
        .filter(|term| {
            normalized.contains(**term)
                || (term.contains(' ') && squashed.contains(&term.replace(' ', "")))
        })
        .map(|term| term.to_string())
        .collect()
}

/// Walk `root`, returning one entry per offending file: its path plus every
/// gated term found. Hidden dot-directories, `target/` build output, and
/// symlinked directories are never traversed (a symlink to an ancestor
/// must not hang the gate, nor may the scan escape the tree); hidden
/// *files* are scanned like any other file. Also skips generated
/// `Cargo.lock` files, allowlisted prose/owner paths, and non-UTF-8 files
/// (binaries).
fn scan_tree(root: &Path, prefixes: &[&str], files: &[&str]) -> Vec<(PathBuf, Vec<String>)> {
    let mut hits: Vec<(PathBuf, Vec<String>)> = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let ftype = match entry.file_type() {
                Ok(ftype) => ftype,
                Err(_) => continue,
            };
            let path = entry.path();
            if ftype.is_symlink() {
                // Never traverse symlinked directories; symlinked files
                // are still content-scanned below (a dangling link simply
                // fails to read and is skipped).
                if path.is_dir() {
                    continue;
                }
            } else if ftype.is_dir() {
                // Tooling and build output are not product capabilities.
                if name.starts_with('.') || name == "target" {
                    continue;
                }
                stack.push(path);
                continue;
            } else if !ftype.is_file() {
                continue;
            }
            if name == "Cargo.lock" {
                continue;
            }
            let rel_unix = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if is_allowlisted(&rel_unix, prefixes, files) {
                continue;
            }
            let bytes = match std::fs::read(&path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let text = match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => continue,
            };
            let terms = match_terms(&normalize(&text));
            if !terms.is_empty() {
                hits.push((path, terms));
            }
        }
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0));
    hits
}

#[test]
fn fal_level_pins_mvp_at_two() {
    assert_eq!(
        clawbank_safety::FAL_LEVEL,
        2,
        "FAL_LEVEL must stay 2 until the pause-gate evidence lands; \
         see docs/safety/fal-level.md, not just the constant"
    );
}

#[test]
fn no_fal3_capabilities_below_fal3() {
    if clawbank_safety::FAL_LEVEL >= 3 {
        return;
    }
    let root = workspace_root();
    let hits = scan_tree(&root, ALLOWLIST_PREFIXES, ALLOWLIST_FILES);
    assert!(
        hits.is_empty(),
        "FAL-3-gated capability found at FAL-{}; raising the level requires \
         the pause-gate evidence (safeguards + evaluations + external reviewer, \
         see docs/safety/fal-level.md), not just new code:\n{}",
        clawbank_safety::FAL_LEVEL,
        hits.iter()
            .map(|(path, terms)| format!(
                "  {}: {}",
                path.strip_prefix(&root).unwrap_or(path).display(),
                terms.join(", ")
            ))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

#[cfg(test)]
mod scanner_tests {
    use super::*;

    fn fixture_tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, contents) in files {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }
        dir
    }

    fn scan(dir: &tempfile::TempDir) -> Vec<(PathBuf, Vec<String>)> {
        scan_tree(dir.path(), ALLOWLIST_PREFIXES, ALLOWLIST_FILES)
    }

    #[test]
    fn flags_an_escrow_stub_outside_the_allowlist() {
        let dir = fixture_tree(&[("crates/clawbank/src/escrow_stub.rs", "pub struct Escrow;\n")]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1.contains(&"escrow".to_string()));
    }

    #[test]
    fn flags_a_stub_anywhere_in_the_tree() {
        let dir = fixture_tree(&[("notes/idea.txt", "add lending vault next quarter\n")]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1.contains(&"lending".to_string()));
    }

    #[test]
    fn flags_borrow_family_terms() {
        let dir = fixture_tree(&[("crates/app/src/pool.rs", "// borrowers post collateral\n")]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1.contains(&"borrower".to_string()));
    }

    #[test]
    fn flags_camel_underscore_and_hyphen_variants() {
        let dir = fixture_tree(&[
            ("crates/app/src/a.rs", "pub struct ComputeMarketplace;\n"),
            ("crates/app/src/b.rs", "pub struct SpendingPolicy;\n"),
            (
                "crates/app/src/c.rs",
                "fn autonomous_spending_policy() {}\n",
            ),
            ("crates/app/src/d.rs", "// auto-spend cap\n"),
        ]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 4);
    }

    #[test]
    fn flags_hidden_files() {
        let dir = fixture_tree(&[("crates/app/src/.escrow_stub.rs", "pub struct Escrow;\n")]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1.contains(&"escrow".to_string()));
    }

    #[test]
    fn clean_transfer_code_passes() {
        let dir = fixture_tree(&[(
            "crates/clawbank/src/transfer.rs",
            "pub struct Transfer { pub amount: u64, pub nonce: u64 }\n",
        )]);
        assert!(scan(&dir).is_empty());
    }

    #[test]
    fn std_borrow_idioms_do_not_trip_the_gate() {
        let dir = fixture_tree(&[(
            "crates/app/src/cache.rs",
            "let v = cell.borrow_mut();\nlet w: &dyn Borrow<str> = &s;\n",
        )]);
        assert!(scan(&dir).is_empty());
    }

    #[test]
    fn owner_files_may_name_gated_terms_but_new_sibling_files_may_not() {
        let dir = fixture_tree(&[
            (
                "crates/clawbank-safety/src/lib.rs",
                "pub const FAL_LEVEL: u8 = 2; // escrow, lending, bridge\n",
            ),
            (
                "crates/clawbank-safety/tests/fal_gate.rs",
                "// gate terms: escrow lending bridge\n",
            ),
            (
                "crates/clawbank-safety/src/lending.rs",
                "pub struct LendingPool;\n",
            ),
        ]);
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].0.ends_with("crates/clawbank-safety/src/lending.rs"));
    }

    #[test]
    fn safety_prose_may_discuss_gated_terms() {
        let dir = fixture_tree(&[
            (
                "docs/adr/0004-safety-risk-levels.md",
                "escrow, lending, bridge\n",
            ),
            ("docs/safety/fal-level.md", "fiat bridge, marketplace\n"),
        ]);
        assert!(scan(&dir).is_empty());
    }

    #[test]
    fn skips_tooling_generated_and_binary_files() {
        let dir = fixture_tree(&[
            (".git/objects/escrow", "escrow lending bridge"),
            (".agent/notes.md", "escrow lending bridge"),
            ("target/debug/escrow_blob", "escrow lending bridge"),
            ("Cargo.lock", "name = \"fiat-crypto\"\n"),
            ("crates/app/Cargo.toml", "[package]\nname = \"app\"\n"),
        ]);
        std::fs::write(dir.path().join("crates/app/blob.bin"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(scan(&dir).is_empty());
    }

    #[test]
    #[cfg(unix)]
    fn does_not_follow_symlinked_directories() {
        let dir = fixture_tree(&[("crates/app/src/real.rs", "pub struct Escrow;\n")]);
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
        // Must terminate (no cycle through `loop/`) and report real.rs once.
        let hits = scan(&dir);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].0.ends_with("crates/app/src/real.rs"));
    }
}
