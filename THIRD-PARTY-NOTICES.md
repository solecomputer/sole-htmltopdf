# Third-Party Notices

This distribution uses Rust dependencies under their respective licences as recorded by the package metadata.

The `cssparser` and `cssparser-macros` crates are licensed under MPL-2.0; their source is available from <https://github.com/servo/rust-cssparser>. The `selectors` crate is licensed under MPL-2.0; its source is available from <https://github.com/servo/stylo>. The `dtoa-short` crate is licensed under MPL-2.0; its source is available from <https://github.com/upsuper/dtoa-short>.

## Adobe Font Metrics

`engine/src/metrics.rs` in the workspace (`src/metrics.rs` in the engine package) contains character widths extracted from the Adobe Font Metrics files for the PDF core-14 fonts and re-indexed from glyph names to WinAnsi byte positions. Adobe's terms are retained verbatim:

> This file and the 14 PostScript(R) AFM files it accompanies may be used, copied, and distributed for any purpose and without charge, with or without modification, provided that all copyright notices are retained; that the AFM files are not distributed without this file; that all modifications to this file or any of the AFM files are prominently noted in the modified file(s); and that this paragraph is not modified. Adobe Systems has no responsibility or obligation to support the use of the AFM files.

Modification notice: that Rust file is not an AFM file. It contains character widths extracted from the AFM files and re-expressed as Rust source, re-indexed from glyph names to WinAnsi byte positions, together with each face's Ascender, Descender, and FontBBox. No other AFM data is reproduced.

The source AFM copyright notices are retained beside that modification notice in
`engine/src/metrics.rs` in the workspace and `src/metrics.rs` in the engine package.
