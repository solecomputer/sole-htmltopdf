# sole-htmltopdf

`sole-htmltopdf` is a serious Rust successor to wkhtmltopdf: a real attempt to
replace a long-used project abandoned upstream, not merely wrap it or preserve
it in amber. It already turns bounded, local, static HTML into PDF, speaks the
wkhtmltopdf 0.12.6 command line, and pairs that compatibility path with a modern
rendering mode built to keep growing toward today's document needs.

**Dual-mode HTML-to-PDF in Rust: a modern renderer that also speaks the
wkhtmltopdf 0.12.6 command line.** Built by [Sole Computer](https://sole.computer).

Move a wkhtmltopdf workflow to a maintained Rust tool without changing the
command line around it, or start fresh with a modern rendering mode. The
`sole-htmltopdf` package gives you the executable and the `sole_htmltopdf` Rust
API under one public name.

## Get started

```sh
cargo install sole-htmltopdf
sole-htmltopdf input.html output.pdf
cat input.html | sole-htmltopdf - - > output.pdf
sole-htmltopdf --strict --stylesheet print.css input.html output.pdf
```

The modern mode is the default. Add `--quirks` when you want the wkhtmltopdf
0.12.6 rendering profile:

```sh
sole-htmltopdf --quirks invoice.html invoice.pdf
```

Input and output can each be a path or `-` for a pipeline. Use `--stylesheet`
to add author CSS, `--asset-root` to grant a document access to one local
resource tree, and `--strict` to make any named loss fail the job. The command
line accepts the wkhtmltopdf 0.12.6 option set.

## Use it from Rust

```toml
[dependencies]
sole-htmltopdf = "0.0.1"
```

```rust
use sole_htmltopdf::{render, InputRef, RenderHost, RenderOptions};
```

The library re-exports the separately packaged `sole-htmltopdf-engine`, so the
CLI and embedded use share the same renderer and types.

`sole-htmltopdf` is designed for bounded, local, static documents. Rendering is
offline: it fetches no network resources and runs no JavaScript.
Document-discovered local resources are denied by default. --asset-root grants
one confined tree; --enable-local-file-access grants the input file's directory
when no asset root is set.

Licensed under either MIT or Apache-2.0, at your option. See `LICENSE-MIT`,
`LICENSE-APACHE`, and `THIRD-PARTY-NOTICES.md`.
