# Corpora

The corpus register (chapter 28.12 of the kernel architecture). `manifest.json` lists every outside collection of models the tests run over, with its source, version, SHA-256, licence, and whether we may store it. A stored corpus lives in a folder named by its id, in full and unmodified; a file nothing lists fails CI.

`geom_test::corpus` holds the rules, and `corpus` (`cargo run -p geom-test --bin corpus -- --help`) lists, checks and fetches. Fetched corpora land in a cache outside the repository, each checked against its digest before anything reads it. A folder's digest is SHA-256 over one line per file, `<sha256> <size> <path>\n`, in byte order of path.
