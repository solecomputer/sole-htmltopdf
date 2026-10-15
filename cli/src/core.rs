use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sole_htmltopdf::{InputRef, PageSize, RenderOptions};

use crate::args;
use crate::contracts::{Destination, Error, Invocation, Notice, Orientation, Source};
use crate::flags;
use crate::host::{LocalFiles, NativeHost};

const VERSION: &str = concat!(
    "sole-htmltopdf ",
    env!("CARGO_PKG_VERSION"),
    " (wkhtmltopdf 0.12.6 command-line compatible)"
);

const HELP: &str = "\
usage: sole-htmltopdf [OPTION]... <input> <output>

  <input>   file path, or - for stdin
  <output>  file path, or - for stdout

This is a drop-in replacement for wkhtmltopdf 0.12.6's command line: every
option it documents is accepted. Options this version does not implement yet
are reported by name on stderr and the document is rendered without them.

Implemented: --page-size, --page-width, --page-height, --orientation,
  --margin-top/-bottom/-left/-right (-T/-B/-L/-R), --encoding, --title,
  --zoom, --quiet/-q, --enable-local-file-access, --disable-local-file-access.

--zoom takes a factor between 0.1 and 10 (default 1). It RE-LAYS OUT the
document rather than magnifying the finished page, so text rewraps and the
page count can change. The page furniture does not move with it.

Ours, not wkhtmltopdf's:
  --strict   treat any unimplemented option as an error (exit 2). For CI.
  --stylesheet <path>   add author CSS; repeatable, in occurrence order.
  --asset-root <dir>    confine document resources; base for stdin resources.

Lengths take a unit: 10mm, 1cm, 1in, 72pt, 96px. A bare number means
millimetres, as wkhtmltopdf does. A px is 1/96in, so 96px is 72pt.

Local file access is DENIED by default: a document cannot read from disk
unless --enable-local-file-access is given.
";

fn extended_help() -> String {
    let mut out = String::from(HELP);
    out.push_str(
        "\nEVERY RECOGNISED OPTION\n\
         =======================\n\
         Each option below is accepted by the parser. The right-hand column\n\
         is what this version actually does with it.\n\n",
    );
    for flag in flags::FLAGS {
        let short = flag
            .short
            .map_or_else(|| "    ".to_owned(), |letter| format!("-{letter}, "));
        let values = match flag.value {
            flags::Value::None => String::new(),
            flags::Value::One(name) => format!(" <{name}>"),
            flags::Value::Repeatable(name) => format!(" <{name}> (repeatable)"),
            flags::Value::Two(first, second) => format!(" <{first}> <{second}>"),
        };
        let spelling = format!("{short}{}{values}", flag.long);
        let disposition = match flag.disposition {
            flags::Disposition::Implemented => "implemented".to_owned(),
            flags::Disposition::Translated(difference) => format!("accepted, but {difference}"),
            flags::Disposition::Unsupported => {
                "recognised, NOT implemented in this version".to_owned()
            }
            flags::Disposition::Refused(reason) => format!("refused: {reason}"),

            flags::Disposition::ModeDependent { modern, emulation } => {
                format!("without --quirks: {modern}; with --quirks: {emulation}")
            }
        };
        let _ = writeln!(out, "  {spelling:<46}{disposition}");
    }
    out
}

pub(crate) fn run<I: IntoIterator<Item = std::ffi::OsString>>(arguments: I) -> u8 {
    match execute(arguments) {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "sole-htmltopdf: {error}");
            error.code()
        }
    }
}

fn execute<I: IntoIterator<Item = std::ffi::OsString>>(arguments: I) -> Result<(), Error> {
    let (invocation, notices) = match args::read(arguments)? {
        args::Request::Help => {
            println!("{HELP}");
            return Ok(());
        }
        args::Request::ExtendedHelp => {
            println!("{}", extended_help());
            return Ok(());
        }
        args::Request::Version => {
            println!("{VERSION}");
            return Ok(());
        }
        args::Request::Render(invocation, notices) => (*invocation, notices),
    };

    let strict_notice_details = notices
        .iter()
        .filter(|notice| {
            matches!(
                notice,
                Notice::Unsupported { .. }
                    | Notice::UnsupportedUserStylesheet
                    | Notice::TocUnsupported
                    | Notice::CoverSkipped { .. }
                    | Notice::BundleMemberMissing { .. }
                    | Notice::InputsIgnored { .. }
            )
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let strict_needs_document_diagnostics = invocation.strict
        && !strict_notice_details.is_empty()
        && notices
            .iter()
            .filter(|notice| {
                matches!(
                    notice,
                    Notice::Unsupported { .. }
                        | Notice::UnsupportedUserStylesheet
                        | Notice::TocUnsupported
                        | Notice::CoverSkipped { .. }
                        | Notice::BundleMemberMissing { .. }
                        | Notice::InputsIgnored { .. }
                )
            })
            .all(|notice| matches!(notice, Notice::BundleMemberMissing { .. }));
    if invocation.strict && !strict_needs_document_diagnostics {
        refuse_if_any(strict_notice_details.clone())?;
    }

    for notice in &notices {
        if strict_needs_document_diagnostics {
            continue;
        }

        if invocation.quiet
            && matches!(
                notice,
                Notice::Translated { .. } | Notice::CoverRenderedAsPage { .. }
            )
        {
            continue;
        }
        let _ = writeln!(std::io::stderr(), "sole-htmltopdf: {notice}");
    }

    let inputs = gather_inputs(&invocation)?;

    let asset_root = canonical_asset_root(&invocation)?;
    let host = NativeHost::new(
        source_directory(&invocation),
        asset_root.clone(),
        if invocation.allow_local_files {
            LocalFiles::Allowed
        } else {
            LocalFiles::Denied
        },
    );
    let outcome = sole_htmltopdf::render(&inputs, options_for(&invocation, asset_root)?, &host)?;

    for warning in &outcome.warnings {
        if let sole_htmltopdf::Warning::FurnitureTemplate { band, notice } = warning {
            let _ = writeln!(
                std::io::stderr(),
                "sole-htmltopdf: {} band: {}",
                band,
                crate::furniture::say(notice)
            );
            continue;
        }
        let _ = writeln!(std::io::stderr(), "sole-htmltopdf: warning: {warning}");
    }

    if invocation.strict {
        refuse_if_any(
            strict_notice_details.into_iter().chain(
                outcome
                    .warnings
                    .iter()
                    .map(|warning| format!("the document: {warning}")),
            ),
        )?;
    }

    write_output(&invocation, outcome.pdf.as_slice())
}

fn refuse_if_any<I: IntoIterator<Item = String>>(diagnostics: I) -> Result<(), Error> {
    let details: Vec<String> = diagnostics.into_iter().collect();
    if details.is_empty() {
        return Ok(());
    }
    Err(Error::StrictRefusal {
        count: details.len(),
        details: details.join("\n  "),
    })
}

fn options_for(
    invocation: &Invocation,
    asset_root: Option<PathBuf>,
) -> Result<RenderOptions, Error> {
    let explicit_page =
        match (invocation.page_width, invocation.page_height) {
            (Some(width), Some(height)) => Some(PageSize::try_new(width, height).map_err(
                |_| Error::InvalidValue {
                    name: "--page-width",
                    value: format!("{width} x {height}"),
                    expected: "finite, positive page dimensions",
                },
            )?),
            (Some(_), None) | (None, Some(_)) => {
                return Err(Error::InvalidValue {
                    name: "--page-width/--page-height",
                    value: "only one of the two".to_owned(),
                    expected: "both dimensions, or neither",
                });
            }
            (None, None) => match &invocation.page_size {
                Some(name) => Some(PageSize::by_name(name).map_err(|_| Error::InvalidValue {
                    name: "--page-size",
                    value: name.clone(),
                    expected: "a known page size such as A4 or Letter",
                })?),
                None => None,
            },
        };
    let mut options = RenderOptions::document_defaults();
    if let Some(page) = explicit_page {
        options.set_page_size(page);
    }
    if let Some(orientation) = invocation.orientation {
        options.set_page_orientation(match orientation {
            Orientation::Portrait => sole_htmltopdf::PageOrientation::Portrait,
            Orientation::Landscape => sole_htmltopdf::PageOrientation::Landscape,
        });
    }
    options.mode = invocation.mode.clone();
    apply_margins(invocation, &mut options)?;
    options.title.clone_from(&invocation.title);
    options.colours = invocation.colours;

    if let Some(zoom) = invocation.zoom {
        options.zoom = zoom;
    }
    options.medium = invocation.medium;
    options.images = invocation.images;
    options.author_stylesheets = invocation
        .stylesheets
        .iter()
        .cloned()
        .map(sole_htmltopdf::StylesheetRef::Path)
        .collect();
    options.asset_root = asset_root;

    let source_name = match &invocation.source {
        Source::File(path) => Some(path.display().to_string()),
        Source::Stdin => None,
    };
    let desugared = crate::furniture::desugar(
        &invocation.furniture,
        source_name.as_deref(),
        crate::furniture::now(),
    );

    for notice in &desugared.notices {
        let _ = writeln!(std::io::stderr(), "sole-htmltopdf: {notice}");
    }
    options.furniture = desugared.furniture;

    options
        .furniture_bindings
        .clone_from(&invocation.furniture.replacements);

    options.header_html = invocation
        .furniture
        .header_html
        .clone()
        .map(sole_htmltopdf::BandSource::Path);
    options.footer_html = invocation
        .furniture
        .footer_html
        .clone()
        .map(sole_htmltopdf::BandSource::Path);
    options.furniture_facts = desugared.facts;

    options.encoding =
        match &invocation.encoding {
            Some(name) => Some(sole_htmltopdf::Encoding::from_name(name).ok_or_else(|| {
                Error::InvalidValue {
                    name: "--encoding",
                    value: name.clone(),
                    expected: "utf-8, iso-8859-1 or windows-1252",
                }
            })?),
            None => None,
        };
    Ok(options)
}

fn apply_margins(invocation: &Invocation, options: &mut RenderOptions) -> Result<(), Error> {
    let side = |value: f32| {
        sole_htmltopdf::Length::try_new(value, "applying a page margin").map_err(Error::Render)
    };
    if let Some(value) = invocation.margins.top {
        options.set_page_margin_top(side(value)?);
    }
    if let Some(value) = invocation.margins.right {
        options.set_page_margin_right(side(value)?);
    }
    if let Some(value) = invocation.margins.bottom {
        options.set_page_margin_bottom(side(value)?);
    }
    if let Some(value) = invocation.margins.left {
        options.set_page_margin_left(side(value)?);
    }
    Ok(())
}

fn source_directory(invocation: &Invocation) -> Option<PathBuf> {
    let Source::File(path) = &invocation.source else {
        return None;
    };
    path.parent().map(|parent| {
        if parent.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            parent.to_path_buf()
        }
    })
}

fn canonical_asset_root(invocation: &Invocation) -> Result<Option<PathBuf>, Error> {
    let Some(root) = &invocation.asset_root else {
        return Ok(None);
    };
    let shown = root.display().to_string();
    let canonical = root
        .canonicalize()
        .map_err(|cause| Error::AssetRootUnreadable {
            path: shown.clone(),
            cause: cause.to_string(),
        })?;
    if !canonical.is_dir() {
        return Err(Error::AssetRootUnreadable {
            path: shown,
            cause: "not a directory".to_owned(),
        });
    }
    if let Some(source) = source_directory(invocation) {
        let real_source =
            source
                .canonicalize()
                .map_err(|cause| Error::InputDirectoryUnreadable {
                    path: source.display().to_string(),
                    cause: cause.to_string(),
                })?;
        if !real_source.starts_with(&canonical) {
            return Err(Error::AssetRootMismatch {
                root: canonical.display().to_string(),
                input_directory: real_source.display().to_string(),
            });
        }
    }
    Ok(Some(canonical))
}

fn gather_inputs(invocation: &Invocation) -> Result<Vec<InputRef>, Error> {
    match &invocation.source {
        Source::Stdin => {
            let limit = RenderOptions::new(PageSize::a4()).max_input_bytes;
            let mut bytes = Vec::new();

            let read_ceiling = u64::try_from(limit.saturating_add(1)).unwrap_or(u64::MAX);
            std::io::stdin()
                .take(read_ceiling)
                .read_to_end(&mut bytes)
                .map_err(|cause| Error::InputUnreadable {
                    path: "<stdin>".to_owned(),
                    cause: cause.to_string(),
                })?;
            if bytes.len() > limit {
                return Err(Error::InputUnreadable {
                    path: "<stdin>".to_owned(),
                    cause: format!("input exceeds the {limit}-byte limit"),
                });
            }
            Ok(vec![InputRef::Inline {
                bytes,
                origin: "<stdin>".to_owned(),
            }])
        }
        Source::File(path) => Ok(vec![InputRef::Path(path.clone())]),
    }
}

fn write_output(invocation: &Invocation, bytes: &[u8]) -> Result<(), Error> {
    match &invocation.destination {
        Destination::Stdout => {
            let mut stdout = std::io::stdout();
            stdout
                .write_all(bytes)
                .and_then(|()| stdout.flush())
                .map_err(|cause| Error::OutputUnwritable {
                    path: "<stdout>".to_owned(),
                    cause: cause.to_string(),
                })
        }
        Destination::File(path) => write_atomically(path, bytes),
    }
}

fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let unwritable = |cause: &dyn std::fmt::Display| Error::OutputUnwritable {
        path: display_path(path),
        cause: cause.to_string(),
    };

    let temporary = temporary_beside(path);

    let write_then_persist = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    };

    write_then_persist().map_err(|cause| {
        let _ = std::fs::remove_file(&temporary);
        unwritable(&cause)
    })
}

fn temporary_beside(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}
