# Shared engines

Heavy engines shared across programs: renderer, 2D drawing, text and PDF (Spare Arch and Spare Studio), and video and audio (Spare Studio and Spare Social). Each is a Rust crate in `crates/`. They depend only on the shared kit, never on a program.
