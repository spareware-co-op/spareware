# Shared engines

Heavy engines shared by the design tools (Spare Arch and Spare Studio): renderer, 2D drawing, text, PDF, and video and audio. Each is a Rust crate in `crates/`. They depend only on the shared kit, never on a program.
