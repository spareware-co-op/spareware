# Spare Studio status

**Stage: Mapping.** Working name. Map: [ARCHITECTURE.md](ARCHITECTURE.md). No code yet. An owner is inviting ArtCraft's author to come under the Spareware umbrella; the answer shapes the Design stage.

## Work board

Sizes: Small fits a little spare, Medium a fair amount, Large a lot. Who has a piece and its state live on its issue; check pieces out with `python tools/piece.py checkout <ID>` (see AGENTS.md).

| ID | Piece | Kind | Size | Done when | Issue |
| --- | --- | --- | --- | --- | --- |
| S01 | ArtCraft's architecture compared with ours | Research note | Medium | How their apps are built (crates, rendering, document model, licences), what could be shared, from reading their source | [#30](https://github.com/spareware-co-op/spareware/issues/30) |
| S02 | Studio document model: layers and non-destructive edits as operations | Design note | Large | One model that photo, vector, layout and video can all use, with how undo, history and sync work | [#31](https://github.com/spareware-co-op/spareware/issues/31) |
| S03 | Colour management: ICC and OpenColorIO | Research note | Medium | How open tools do it, licences, and a recommended approach for print, photo and video | [#32](https://github.com/spareware-co-op/spareware/issues/32) |
| S04 | Video and audio: decoding, encoding and codec licensing | Research note | Medium | Options compared (FFmpeg linked separately, platform codecs, pure-Rust decoders) with licence and patent risks | [#33](https://github.com/spareware-co-op/spareware/issues/33) |
| S05 | Text engine: fonts, shaping and page layout | Research note | Medium | Recommended open libraries with licences, and what Spare Layout needs on top | [#34](https://github.com/spareware-co-op/spareware/issues/34) |
| S06 | File formats: what we can legally read and write | Research note | Small | Each Adobe format checked against a published specification, with links | [#35](https://github.com/spareware-co-op/spareware/issues/35) |
| S07 | AI tools policy | Decision note | Small | Options for models, cost and consent; an owner decides | [#36](https://github.com/spareware-co-op/spareware/issues/36) |
| S08 | First app to build | Decision note | Small | Spare Photo, PDF and Draw ranked on demand, effort and what ArtCraft already covers | [#37](https://github.com/spareware-co-op/spareware/issues/37) |