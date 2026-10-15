use std::ffi::OsString;
use std::path::PathBuf;

use crate::contracts::{Destination, Error, Invocation, Margins, Notice, Orientation, Source};
use crate::flags::{self, Disposition, Value};

const STREAM: &str = "-";

pub(crate) enum Request {
    Help,

    ExtendedHelp,
    Version,
    Render(Box<Invocation>, Vec<Notice>),
}

pub(crate) fn read<I: IntoIterator<Item = OsString>>(arguments: I) -> Result<Request, Error> {
    let argv: Vec<OsString> = arguments.into_iter().collect();
    let mut settings = Settings::default();
    let mut notices: Vec<Notice> = Vec::new();

    let mut deferred: Vec<(&'static str, &'static str, &'static str)> = Vec::new();
    let mut positional: Vec<OsString> = Vec::new();
    let mut index = 0;
    let mut options_ended = false;

    while let Some(argument) = argv.get(index) {
        index += 1;
        let Some(text) = option_text(argument, options_ended, &mut positional)? else {
            continue;
        };

        if options_ended || !text.starts_with('-') || text == STREAM {
            positional.push(argument.clone());
            continue;
        }

        if text == "--" {
            options_ended = true;
            continue;
        }

        let matched: Vec<(&flags::Flag, Vec<OsString>)> = if text.starts_with("--") {
            let (name, inline) = match text.split_once('=') {
                Some((name, value)) => (name.to_owned(), Some(OsString::from(value))),
                None => (text.clone(), None),
            };
            let Some(flag) = flags::by_long(&name) else {
                return Err(Error::UnknownOption { name });
            };
            let values = take_values(flag.value, inline, &argv, &mut index, &name)?;
            vec![(flag, values)]
        } else {
            let mut collected = Vec::new();
            for letter in text.chars().skip(1) {
                let Some(flag) = flags::by_short(letter) else {
                    return Err(Error::UnknownOption {
                        name: format!("-{letter}"),
                    });
                };
                let values = take_values(flag.value, None, &argv, &mut index, flag.long)?;
                collected.push((flag, values));
            }
            collected
        };

        for (flag, values) in matched {
            match flag.long {
                "--help" => return Ok(Request::Help),

                "--extended-help" => return Ok(Request::ExtendedHelp),
                "--version" => return Ok(Request::Version),
                _ => {}
            }
            match flag.disposition {
                Disposition::Implemented => apply(flag.long, &values, &mut settings)?,
                Disposition::Translated(difference) => {
                    apply(flag.long, &values, &mut settings)?;
                    notices.push(Notice::Translated {
                        flag: flag.long,
                        difference,
                    });
                }

                Disposition::Unsupported if flag.long == "--user-style-sheet" => {
                    notices.push(Notice::UnsupportedUserStylesheet);
                }
                Disposition::Unsupported => notices.push(Notice::Unsupported { flag: flag.long }),

                Disposition::ModeDependent { modern, emulation } => {
                    apply(flag.long, &values, &mut settings)?;
                    deferred.push((flag.long, modern, emulation));
                }
                Disposition::Refused(reason) => {
                    return Err(Error::RefusedOption {
                        name: flag.long,
                        reason,
                    });
                }
            }
        }
    }

    finish(settings, deferred, positional, notices)
}

fn finish(
    settings: Settings,
    deferred: Vec<(&'static str, &'static str, &'static str)>,
    positional: Vec<OsString>,
    mut notices: Vec<Notice>,
) -> Result<Request, Error> {
    let (source, destination, positional_notices) = ends(positional)?;
    notices.extend(positional_notices);

    let mode = resolve_mode(&settings, deferred, &mut notices);
    if settings.asset_root.is_some() && settings.allow_local_files {
        notices.push(Notice::Translated {
            flag: "--enable-local-file-access",
            difference: "--asset-root is the sole document confinement grant, so broad local access adds nothing",
        });
    }
    Ok(Request::Render(
        Box::new(Invocation {
            source,
            destination,
            allow_local_files: settings.allow_local_files,
            stylesheets: settings.stylesheets,
            asset_root: settings.asset_root,
            page_size: settings.page_size,
            orientation: settings.orientation,
            page_width: settings.page_width,
            page_height: settings.page_height,
            margins: settings.margins,
            encoding: settings.encoding,
            title: settings.title,
            quiet: settings.quiet,
            strict: settings.strict,
            colours: settings.colours,
            zoom: settings.zoom,
            medium: settings.medium.unwrap_or_default(),
            images: settings.images,
            furniture: settings.furniture,
            mode,
        }),
        notices,
    ))
}

fn option_text(
    argument: &OsString,
    options_ended: bool,
    positional: &mut Vec<OsString>,
) -> Result<Option<String>, Error> {
    if let Some(text) = argument.to_str() {
        return Ok(Some(text.to_owned()));
    }
    if !options_ended && argument.as_encoded_bytes().starts_with(b"-") {
        return Err(Error::InvalidOptionEncoding {
            value: argument.to_string_lossy().into_owned(),
        });
    }
    positional.push(argument.clone());
    Ok(None)
}

fn take_values(
    value: Value,
    inline: Option<OsString>,
    argv: &[OsString],
    index: &mut usize,
    name: &str,
) -> Result<Vec<OsString>, Error> {
    let arity = value.arity();
    let mut values = Vec::with_capacity(arity);
    if let Some(inline) = inline {
        if arity == 0 {
            return Err(Error::UnexpectedValue {
                name: name.to_owned(),
            });
        }
        values.push(inline);
    }
    while values.len() < arity {
        let Some(next) = argv.get(*index) else {
            return Err(Error::MissingValue {
                name: name.to_owned(),
                arity,
            });
        };
        *index += 1;
        values.push(next.clone());
    }
    Ok(values)
}

#[allow(
    clippy::struct_excessive_bools,
    reason = "one field per command-line switch; grouping would invent categories the grammar does not have"
)]
#[derive(Debug, Default)]
struct Settings {
    allow_local_files: bool,
    stylesheets: Vec<PathBuf>,
    asset_root: Option<PathBuf>,

    quirks: bool,

    smart_shrinking: Option<sole_htmltopdf::SmartShrinking>,
    page_size: Option<String>,
    orientation: Option<Orientation>,
    page_width: Option<f32>,
    page_height: Option<f32>,
    margins: Margins,
    encoding: Option<String>,
    title: Option<String>,
    quiet: bool,
    strict: bool,

    colours: sole_htmltopdf::Colours,

    zoom: Option<sole_htmltopdf::Zoom>,

    medium: Option<sole_htmltopdf::Medium>,
    images: sole_htmltopdf::Images,

    furniture: crate::furniture::Raw,
}

fn resolve_mode(
    settings: &Settings,
    deferred: Vec<(&'static str, &'static str, &'static str)>,
    notices: &mut Vec<Notice>,
) -> sole_htmltopdf::RenderMode {
    let mode = if settings.quirks {
        let mut bundle = sole_htmltopdf::Bundle::wkhtmltopdf_0_12_6();
        if let Some(choice) = settings.smart_shrinking {
            bundle = bundle.with_smart_shrinking(choice);
        }

        for member in sole_htmltopdf::Bundle::unimplemented_members() {
            notices.push(Notice::BundleMemberMissing { member });
        }
        sole_htmltopdf::RenderMode::Wkhtmltopdf0126(bundle)
    } else {
        sole_htmltopdf::RenderMode::Modern
    };
    for (flag, modern, emulation) in deferred {
        notices.push(Notice::Translated {
            flag,
            difference: if settings.quirks { emulation } else { modern },
        });
    }
    mode
}

fn apply(long: &'static str, values: &[OsString], settings: &mut Settings) -> Result<(), Error> {
    if let Some(result) = apply_path_input(long, values, settings) {
        return result;
    }
    let first = || {
        values
            .first()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    match long {
        "--quirks" => settings.quirks = true,

        "--enable-smart-shrinking" => {
            settings.smart_shrinking = Some(sole_htmltopdf::SmartShrinking::On);
        }
        "--disable-smart-shrinking" => {
            settings.smart_shrinking = Some(sole_htmltopdf::SmartShrinking::Off);
        }
        "--page-size" => settings.page_size = Some(first()),
        "--page-width" => settings.page_width = Some(unit(&first(), long)?),
        "--page-height" => settings.page_height = Some(unit(&first(), long)?),
        "--orientation" => {
            let value = first();

            if value.eq_ignore_ascii_case("Landscape") {
                settings.orientation = Some(Orientation::Landscape);
            } else if value.eq_ignore_ascii_case("Portrait") {
                settings.orientation = Some(Orientation::Portrait);
            } else {
                return Err(Error::InvalidValue {
                    name: long,
                    value,
                    expected: "Portrait or Landscape",
                });
            }
        }
        "--margin-top" => settings.margins.top = Some(unit(&first(), long)?),
        "--margin-bottom" => settings.margins.bottom = Some(unit(&first(), long)?),
        "--margin-left" => settings.margins.left = Some(unit(&first(), long)?),
        "--margin-right" => settings.margins.right = Some(unit(&first(), long)?),
        "--encoding" => settings.encoding = Some(first()),
        "--title" => settings.title = Some(first()),
        "--quiet" => settings.quiet = true,
        "--strict" => settings.strict = true,
        "--enable-local-file-access" => settings.allow_local_files = true,
        "--disable-local-file-access" => settings.allow_local_files = false,

        "--print-media-type" => settings.medium = Some(sole_htmltopdf::Medium::Print),
        "--no-print-media-type" => settings.medium = Some(sole_htmltopdf::Medium::Screen),

        "--zoom" => {
            let text = first();
            let not_a_number = || Error::InvalidValue {
                name: long,
                value: text.clone(),
                expected: "a number, such as 1.5",
            };
            let out_of_bound = || Error::InvalidValue {
                name: long,
                value: text.clone(),
                expected: "a factor between 0.1 and 10",
            };
            let factor: f32 = text.parse().map_err(|_| not_a_number())?;
            settings.zoom =
                Some(sole_htmltopdf::Zoom::try_new(factor).map_err(|_| out_of_bound())?);
        }
        "--background" => settings.colours = sole_htmltopdf::Colours::On,
        "--no-background" => settings.colours = sole_htmltopdf::Colours::Off,

        "--header-left" => settings.furniture.header[0] = Some(first()),
        "--header-center" => settings.furniture.header[1] = Some(first()),
        "--header-right" => settings.furniture.header[2] = Some(first()),
        "--footer-left" => settings.furniture.footer[0] = Some(first()),
        "--footer-center" => settings.furniture.footer[1] = Some(first()),
        "--footer-right" => settings.furniture.footer[2] = Some(first()),
        "--header-spacing" => settings.furniture.header_spacing = Some(unit(&first(), long)?),
        "--footer-spacing" => settings.furniture.footer_spacing = Some(unit(&first(), long)?),
        "--header-line" => settings.furniture.header_line = Some(true),
        "--no-header-line" => settings.furniture.header_line = Some(false),
        "--footer-line" => settings.furniture.footer_line = Some(true),
        "--no-footer-line" => settings.furniture.footer_line = Some(false),
        "--header-font-name" => settings.furniture.header_font_name = Some(first()),
        "--footer-font-name" => settings.furniture.footer_font_name = Some(first()),

        "--header-font-size" => {
            settings.furniture.header_font_size = Some(points(&first(), long)?);
        }
        "--footer-font-size" => {
            settings.furniture.footer_font_size = Some(points(&first(), long)?);
        }
        "--replace" => {
            let name = first();
            let value = values
                .get(1)
                .map(|value| value.to_string_lossy().into_owned())
                .unwrap_or_default();
            settings.furniture.replacements.push((name, value));
        }
        "--images" => settings.images = sole_htmltopdf::Images::On,
        "--no-images" => settings.images = sole_htmltopdf::Images::Off,
        "--help" | "--extended-help" | "--version" => {}
        _ => {
            return Err(Error::UnhandledImplementedFlag { name: long });
        }
    }
    Ok(())
}

fn apply_path_input(
    long: &'static str,
    values: &[OsString],
    settings: &mut Settings,
) -> Option<Result<(), Error>> {
    let path = || PathBuf::from(values.first().cloned().unwrap_or_default());
    match long {
        "--stylesheet" => {
            let path = path();
            if path.as_os_str() == STREAM {
                return Some(Err(Error::InvalidValue {
                    name: long,
                    value: STREAM.to_owned(),
                    expected: "a stylesheet path (stdin is only an HTML input)",
                }));
            }
            settings.stylesheets.push(path);
            Some(Ok(()))
        }
        "--asset-root" => {
            let path = path();
            if path.as_os_str() == STREAM {
                return Some(Err(Error::InvalidValue {
                    name: long,
                    value: STREAM.to_owned(),
                    expected: "a directory path",
                }));
            }
            if settings.asset_root.is_some() {
                return Some(Err(Error::InvalidValue {
                    name: long,
                    value: path.display().to_string(),
                    expected: "at most one asset root",
                }));
            }
            settings.asset_root = Some(path);
            Some(Ok(()))
        }
        "--header-html" => {
            settings.furniture.header_html = Some(path());
            Some(Ok(()))
        }
        "--footer-html" => {
            settings.furniture.footer_html = Some(path());
            Some(Ok(()))
        }
        _ => None,
    }
}

fn points(text: &str, name: &'static str) -> Result<f32, Error> {
    let trimmed = text.trim();
    let number = trimmed.strip_suffix("pt").unwrap_or(trimmed).trim();
    let invalid = || Error::InvalidValue {
        name,
        value: text.to_owned(),
        expected: "a font size in points, such as 12 or 12pt",
    };
    let value: f32 = number.parse().map_err(|_| invalid())?;
    if !value.is_finite() || value <= 0.0 {
        return Err(invalid());
    }
    Ok(value)
}

fn unit(text: &str, name: &'static str) -> Result<f32, Error> {
    let trimmed = text.trim();
    let split = trimmed
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(trimmed.len());
    let (number, suffix) = trimmed.split_at(split);
    let invalid = || Error::InvalidValue {
        name,
        value: text.to_owned(),
        expected: "a length such as 10mm, 1cm, 1in, 72pt or 96px",
    };
    let value: f32 = number.parse().map_err(|_| invalid())?;

    if !value.is_finite() || value < 0.0 {
        return Err(invalid());
    }
    let points = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "mm" | "millimeter" => value * 72.0 / 25.4,
        "cm" | "centimeter" => value * 720.0 / 25.4,
        "m" | "meter" => value * 72_000.0 / 25.4,
        "in" | "inch" => value * 72.0,
        "pt" | "point" => value,
        "pc" | "pica" => value * 12.0,
        "didot" => value * 1.07,
        "cicero" => value * 12.84,

        "px" | "pixel" => value * 72.0 / 96.0,
        _ => return Err(invalid()),
    };
    Ok(points)
}

enum Object {
    Page { path: OsString, named: bool },
    Cover { path: OsString },
    Toc,
}

fn objects(positional: Vec<OsString>) -> Vec<Object> {
    let mut out: Vec<Object> = Vec::new();
    let mut iterator = positional.into_iter();
    while let Some(token) = iterator.next() {
        match token.to_string_lossy().as_ref() {
            "page" => {
                if let Some(path) = iterator.next() {
                    out.push(Object::Page { path, named: true });
                }
            }
            "cover" => {
                if let Some(path) = iterator.next() {
                    out.push(Object::Cover { path });
                }
            }
            "toc" => out.push(Object::Toc),
            _ => out.push(Object::Page {
                path: token,
                named: false,
            }),
        }
    }
    out
}

fn ends(positional: Vec<OsString>) -> Result<(Source, Destination, Vec<Notice>), Error> {
    let mut objects = objects(positional);

    let Some(Object::Page {
        path: output,
        named: false,
    }) = objects.pop()
    else {
        return Err(Error::Usage);
    };

    let mut notices: Vec<Notice> = Vec::new();
    let mut pages: Vec<OsString> = Vec::new();
    let mut covers: Vec<OsString> = Vec::new();
    let mut toc_requested = false;
    for object in objects {
        match object {
            Object::Toc => toc_requested = true,
            Object::Cover { path } => covers.push(path),
            Object::Page { path, .. } => pages.push(path),
        }
    }

    let mut remaining = pages.into_iter();
    let input = if let Some(page) = remaining.next() {
        page
    } else {
        let cover = covers.first().cloned().ok_or(Error::Usage)?;
        notices.push(Notice::CoverRenderedAsPage {
            path: cover.to_string_lossy().into_owned(),
        });
        covers.remove(0);
        cover
    };

    if toc_requested {
        notices.push(Notice::TocUnsupported);
    }
    for cover in &covers {
        notices.push(Notice::CoverSkipped {
            path: cover.to_string_lossy().into_owned(),
        });
    }
    let ignored: Vec<String> = remaining
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    if !ignored.is_empty() {
        notices.push(Notice::InputsIgnored { paths: ignored });
    }

    let input_text = input.to_string_lossy().into_owned();

    let lowered = input_text.to_ascii_lowercase();
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return Err(Error::NetworkInput { url: input_text });
    }

    let source = if input_text == STREAM {
        Source::Stdin
    } else {
        Source::File(PathBuf::from(strip_file_scheme(&input)))
    };
    let destination = if output.to_string_lossy() == STREAM {
        Destination::Stdout
    } else {
        Destination::File(PathBuf::from(output))
    };
    Ok((source, destination, notices))
}

fn strip_file_scheme(input: &OsString) -> OsString {
    let text = input.to_string_lossy();
    text.strip_prefix("file://")
        .map_or_else(|| input.clone(), |rest| OsString::from(rest.to_owned()))
}
