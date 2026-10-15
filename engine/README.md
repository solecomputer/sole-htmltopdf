# sole-htmltopdf-engine

**A dual-mode HTML-to-PDF engine in Rust, with a modern renderer and a
wkhtmltopdf 0.12.6 compatibility profile.** Built by
[Sole Computer](https://sole.computer).

Embed the same renderer that powers `sole-htmltopdf` while keeping control of
input, resources, and output in your own host. Modern rendering is the default;
select `RenderMode::Wkhtmltopdf0126(Bundle::wkhtmltopdf_0_12_6())` for the
compatibility profile.

```toml
[dependencies]
sole-htmltopdf-engine = "0.0.1"
```

```rust
use sole_htmltopdf_engine::{render, InputRef, RenderHost, RenderOptions};
```

Supply an `InputRef`, a `RenderHost`, and `RenderOptions`; `render` returns PDF
bytes together with structured warnings. Applications that want the ready-made
command line and the shorter `sole_htmltopdf` import can depend on
`sole-htmltopdf` instead.

The engine is designed for bounded, local, static documents. The engine owns no
filesystem, network, clock, or environment access. Filesystem resources are
resolved through the supplied RenderHost; inline inputs, inline styles and data:
resources are supplied directly. It does not execute JavaScript.

Licensed under either MIT or Apache-2.0, at your option. See `LICENSE-MIT`,
`LICENSE-APACHE`, and `THIRD-PARTY-NOTICES.md`.
