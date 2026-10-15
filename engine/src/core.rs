use crate::contracts::{
    Error, HostError, InputRef, LinkPolicy, LinkRefusals, RenderHost, RenderOptions, RenderOutcome,
    Requester, Warning,
};
use crate::{boxes, cascade, display, intent, layout, paginate, parse, pdf};

pub fn render(
    inputs: &[InputRef],

    options: RenderOptions,
    host: &impl RenderHost,
) -> Result<RenderOutcome, Error> {
    if inputs.is_empty() {
        return Err(Error::NoInput);
    }

    let mut warnings: Vec<Warning> = Vec::new();
    let (bytes, origin) = assemble(inputs, host, options.max_input_bytes, &mut warnings)?;

    let prepared = crate::inputs::prepare(inputs, &options, host, bytes.len())?;
    let base = prepared.base;

    let image_base = image_base(&base);

    let (dom, parse_warnings) = parse::parse(
        bytes,
        &origin,
        base,
        options.max_nodes,
        options.max_depth,
        options.encoding,
    )?;
    warnings.extend(parse_warnings);

    let medium = evaluated_medium(options.medium, &options.mode);
    let link_policy = resolve_link_policy(&options.mode);
    let (styled, mut link_refusals) = cascade_body(
        dom,
        host,
        &options,
        medium,
        link_policy,
        prepared.author_sheets,
        &mut warnings,
    )?;

    let font_faces = styled.font_faces.clone();
    let mut assets = acquire_fonts(font_faces, &options.mode, host, &mut warnings);
    let (css_furniture, author_page, mut box_tree) =
        build_boxes(styled, &assets.fonts, &mut warnings)?;

    let intent = resolve_page_intent(&options, author_page, &mut warnings)?;

    let world = crate::acquire::World {
        host,
        base: image_base.as_deref(),
        images: options.images,

        container: intent.usable_width.extent(),
        usable: intent.usable_height.get(),
    };
    acquire_images(&mut box_tree, &world, &mut assets, &mut warnings);

    if assets.profiles_ignored() > 0 {
        warnings.push(crate::contracts::Warning::ImageColourProfileIgnored {
            count: assets.profiles_ignored(),
        });
    }

    let fragments = layout::flow(
        box_tree,
        intent.usable_width,
        crate::contracts::UsableWidth::span(
            intent.layout_margin.left.get() * (1.0 - 1.0 / intent.output.get()),
            intent.physical_page.width().get() / intent.output.get(),
        )?,
        intent.usable_height,
    )?;
    warn_about_the_inline_axis(&fragments, &intent, &mut warnings);

    let mut furniture = css_furniture;
    crate::css::merge_furniture(&mut furniture, options.furniture.clone());

    refuse_network_bands(&options)?;
    let subdocuments = read_band_sources(
        &options,
        host,
        &mut furniture,
        &assets.fonts,
        prepared.caller_bytes,
        &mut warnings,
        &mut link_refusals,
    );

    let document_facts = document_facts(&options);

    let mut pages = paginate::paginate(
        fragments,
        intent.layout_page,
        intent.layout_margin,
        intent.usable_height,
        &mut warnings,
    )?;

    stamp_furniture(
        &mut pages,
        &furniture,
        &subdocuments,
        intent.usable_height,
        &intent,
        &crate::compose::Templating {
            document: &document_facts,
            bindings: &options.furniture_bindings,
        },
        &mut warnings,
    )?;

    let title = options.title;
    finish_pages(
        pages,
        &intent,
        title.as_deref(),
        &assets,
        warnings,
        link_refusals,
    )
}

fn finish_pages(
    pages: crate::contracts::Pages,
    intent: &crate::contracts::RenderingIntent,
    title: Option<&str>,
    assets: &crate::acquire::Assets,
    warnings: Vec<Warning>,
    link_refusals: LinkRefusals,
) -> Result<RenderOutcome, Error> {
    let display_list = display::build(
        pages,
        intent.physical_page,
        intent.layout_margin,
        intent.output,
    )?;
    Ok(finish(display_list, title, assets, warnings, link_refusals))
}

fn acquire_fonts(
    font_faces: Vec<crate::font::FaceRequest>,
    mode: &crate::contracts::RenderMode,
    host: &impl RenderHost,
    warnings: &mut Vec<Warning>,
) -> crate::acquire::Assets {
    let mut assets = crate::acquire::Assets::default();
    if matches!(mode, crate::contracts::RenderMode::Modern) {
        assets.set_fonts(crate::font::FontTable::acquire(font_faces, host, warnings));
    } else if !font_faces.is_empty() {
        warnings.push(Warning::AuthoredFontsIgnoredInLegacy {
            count: font_faces.len(),
        });
    }
    assets
}

fn resolve_page_intent(
    options: &RenderOptions,
    author_page: crate::contracts::AuthorPage,
    warnings: &mut Vec<Warning>,
) -> Result<crate::contracts::RenderingIntent, Error> {
    let baseline = options.baseline_page_geometry();
    let author_requested = author_page.has_refusable_geometry();
    let (selected, author_contributed) = options.authored_page_geometry(author_page);
    let selected = if matches!(options.mode, crate::contracts::RenderMode::Modern) {
        selected
    } else {
        if author_requested {
            warnings.push(Warning::AuthorPageIgnored {
                reason: "the --quirks profile keeps caller page geometry",
            });
        }
        baseline
    };
    match intent::resolve(options, selected.size, selected.margin) {
        Ok(intent) => Ok(intent),
        Err(Error::PageMarginsExceedPage { .. }) if author_contributed => {
            warnings.push(Warning::AuthorPageIgnored {
                reason: "the authored size and margins leave no usable content area; caller/default geometry was used",
            });
            intent::resolve(options, baseline.size, baseline.margin)
        }
        Err(error) => Err(error),
    }
}

fn finish(
    display_list: crate::contracts::DisplayList,

    title: Option<&str>,
    assets: &crate::acquire::Assets,
    mut warnings: Vec<Warning>,
    link_refusals: LinkRefusals,
) -> RenderOutcome {
    let written = pdf::write(display_list, title, assets);
    warnings.extend(written.warnings);
    warnings.extend(assets.fonts.shaping_warnings());
    if let Some(warning) = link_refusals.into_warning() {
        warnings.push(warning);
    }
    RenderOutcome {
        pdf: written.pdf,
        warnings,
    }
}

fn image_base(base: &crate::contracts::DocumentBase) -> Option<std::path::PathBuf> {
    match base {
        crate::contracts::DocumentBase::Directory(directory) => Some(directory.clone()),
        crate::contracts::DocumentBase::None => None,
    }
}

fn resolve_link_policy(mode: &crate::contracts::RenderMode) -> LinkPolicy {
    match mode {
        crate::contracts::RenderMode::Modern => LinkPolicy::External,
        crate::contracts::RenderMode::Wkhtmltopdf0126(_) => LinkPolicy::ProfileUnavailable,
    }
}

fn cascade_body(
    dom: crate::contracts::Dom,
    host: &impl RenderHost,
    options: &RenderOptions,
    medium: crate::contracts::Evaluated,
    link_policy: LinkPolicy,
    author_sheets: Vec<crate::inputs::AuthorSheet>,
    warnings: &mut Vec<Warning>,
) -> Result<(crate::contracts::StyledTree, LinkRefusals), Error> {
    let linked_texts = resolve_linked_texts(&dom, host, medium, warnings);
    let (styled, css_warnings, link_refusals) = cascade::cascade_with_author_sheets(
        dom,
        linked_texts,
        options.colours,
        medium,
        link_policy,
        None,
        author_sheets,
    )?;
    warnings.extend(css_warnings);
    Ok((styled, link_refusals))
}

fn build_boxes(
    styled: crate::contracts::StyledTree,
    fonts: &crate::font::FontTable,
    warnings: &mut Vec<Warning>,
) -> Result<
    (
        crate::contracts::Furniture,
        crate::contracts::AuthorPage,
        crate::contracts::BoxTree,
    ),
    Error,
> {
    let furniture = styled.furniture.clone();
    let author_page = styled.author_page;
    Ok((
        furniture,
        author_page,
        boxes::build_with_fonts(styled, warnings, Some(fonts))?,
    ))
}

fn assemble(
    inputs: &[InputRef],
    host: &impl RenderHost,
    max_input_bytes: usize,
    warnings: &mut Vec<Warning>,
) -> Result<(Vec<u8>, String), Error> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut origins: Vec<String> = Vec::new();
    let mut failures: Vec<HostError> = Vec::new();

    for input in inputs {
        match input {
            InputRef::Inline {
                bytes: inline,
                origin,
            } => {
                admit(&mut bytes, inline, max_input_bytes)?;
                origins.push(origin.clone());
            }
            InputRef::Path(path) => {
                match host.read_file(path, remaining(&bytes, max_input_bytes), Requester::Caller) {
                    Ok(contents) => {
                        admit(&mut bytes, &contents, max_input_bytes)?;
                        origins.push(path.display().to_string());
                    }
                    Err(cause) => {
                        warnings.push(Warning::ResourceSkipped {
                            path: path.display().to_string(),
                            cause: cause.to_string(),
                        });

                        failures.push(cause);
                    }
                }
            }
        }
    }

    if origins.is_empty() {
        return Err(match <[HostError; 1]>::try_from(failures) {
            Ok([only]) => Error::Host(only),
            Err(many) if many.is_empty() => Error::NoInput,
            Err(many) => Error::AllInputsFailed {
                causes: many.iter().map(ToString::to_string).collect(),
            },
        });
    }

    Ok((bytes, origins.join(", ")))
}

const MAX_LINKED_STYLESHEETS: usize = 20;

const MAX_STYLESHEET_BYTES: usize = 4 * 1024 * 1024;

fn evaluated_medium(
    medium: crate::contracts::Medium,
    mode: &crate::contracts::RenderMode,
) -> crate::contracts::Evaluated {
    use crate::contracts::{Evaluated, Medium, RenderMode};
    match medium {
        Medium::Print => Evaluated::Print,
        Medium::Screen => Evaluated::Screen,
        Medium::Default => match mode {
            RenderMode::Modern => Evaluated::Print,
            RenderMode::Wkhtmltopdf0126(_) => Evaluated::Screen,
        },
    }
}

fn report_skipped_images(
    skipped: &[crate::acquire::Skipped],
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    use crate::acquire::Skipped;
    let count = |wanted: Skipped| skipped.iter().filter(|cause| **cause == wanted).count();

    let denied = count(Skipped::Denied);
    if denied > 0 {
        warnings.push(crate::contracts::Warning::ImageSkipped {
            count: denied,
            cause: "local file access is off; pass --enable-local-file-access to allow it",
        });
    }
    for (cause, text) in [
        (
            Skipped::NoBase,
            "the document has no directory to resolve a relative source \
             against; it was read from standard input, so use a file input \
             or an inline data: source",
        ),
        (
            Skipped::Disabled,
            "images are off; the alt text was drawn instead",
        ),
        (
            Skipped::EscapesDirectory,
            "it refers outside the document's directory",
        ),
        (
            Skipped::OutsideRoot,
            "its resolved path is outside the permitted asset root",
        ),
        (Skipped::NotFound, "the file was not found"),
        (
            Skipped::TooLarge,
            "the file is larger than the image ceiling",
        ),
        (Skipped::Unreadable, "the file could not be read"),
        (
            Skipped::OverBudget,
            "the document's image budget is already spent",
        ),
    ] {
        let seen = count(cause);
        if seen > 0 {
            warnings.push(crate::contracts::Warning::ImageSkipped {
                count: seen,
                cause: text,
            });
        }
    }

    let mut named: Vec<(&'static str, usize)> = Vec::new();
    for cause in skipped {
        if let Skipped::Refused(what) = cause {
            match named.iter_mut().find(|(seen, _)| *seen == *what) {
                Some((_, count)) => *count = count.saturating_add(1),
                None => named.push((what, 1)),
            }
        }
    }
    for (what, count) in named {
        warnings.push(crate::contracts::Warning::ImageSkipped { count, cause: what });
    }
}

pub(crate) fn confine(base: &std::path::Path, href: &str) -> Option<std::path::PathBuf> {
    use std::path::Component;

    let candidate = std::path::Path::new(href);
    if candidate.is_absolute() {
        return None;
    }

    if href.contains("://") || href.starts_with("file:") || href.starts_with("data:") {
        return None;
    }

    let mut depth = 0_i32;
    for component in candidate.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                depth -= 1;

                if depth < 0 {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(base.join(candidate))
}

fn resolve_linked_texts(
    dom: &crate::contracts::Dom,
    host: &impl RenderHost,
    medium: crate::contracts::Evaluated,
    warnings: &mut Vec<Warning>,
) -> Vec<Option<crate::inputs::AuthorSheet>> {
    use crate::contracts::SheetRef;

    let linked: Vec<(&String, bool)> = dom
        .sheets
        .iter()
        .filter_map(|sheet| match sheet {
            SheetRef::Linked { href, media } => Some((
                href,
                media.as_deref().is_none_or(|list| {
                    crate::css::media_admits_quietly(list, medium) == crate::css::MediaAdmits::Yes
                }),
            )),
            SheetRef::Inline { .. } => None,
        })
        .collect();

    let applicable: Vec<&String> = linked
        .iter()
        .filter_map(|(href, applies)| applies.then_some(*href))
        .collect();

    let crate::contracts::DocumentBase::Directory(base) = &dom.base else {
        if let Some(first) = applicable.first() {
            warnings.push(Warning::StylesheetUnresolvable {
                href: if applicable.len() == 1 {
                    (*first).clone()
                } else {
                    format!("{first} and {} more", applicable.len() - 1)
                },
            });
        }

        return vec![None; linked.len()];
    };

    let mut sheets = Vec::new();
    let mut spent = 0_usize;
    let mut fetched = 0_usize;

    for (href, applies) in &linked {
        if !*applies {
            sheets.push(None);
            continue;
        }
        if fetched >= MAX_LINKED_STYLESHEETS {
            sheets.push(None);
            continue;
        }
        fetched += 1;
        let Some(path) = confine(base, href) else {
            warnings.push(Warning::StylesheetSkipped {
                path: (*href).clone(),
                cause: "refers outside the document's directory".to_owned(),
            });
            sheets.push(None);
            continue;
        };
        let remaining = MAX_STYLESHEET_BYTES.saturating_sub(spent);
        match host.read_file(&path, remaining, Requester::Document) {
            Ok(bytes) => {
                spent = spent.saturating_add(bytes.len());
                if let Ok(text) = String::from_utf8(bytes) {
                    sheets.push(Some(crate::inputs::AuthorSheet {
                        origin: (*href).clone(),
                        text,
                        base: path.parent().map(std::path::Path::to_path_buf),
                    }));
                } else {
                    warnings.push(Warning::StylesheetSkipped {
                        path: (*href).clone(),
                        cause: "not valid UTF-8".to_owned(),
                    });
                    sheets.push(None);
                }
            }
            Err(cause) => {
                warnings.push(Warning::StylesheetSkipped {
                    path: (*href).clone(),
                    cause: cause.to_string(),
                });
                sheets.push(None);
            }
        }
    }

    if applicable.len() > MAX_LINKED_STYLESHEETS {
        warnings.push(Warning::StylesheetSkipped {
            path: format!("{} more", applicable.len() - MAX_LINKED_STYLESHEETS),
            cause: format!("the {MAX_LINKED_STYLESHEETS}-stylesheet limit was reached"),
        });
    }
    sheets
}

fn remaining(buffer: &[u8], limit: usize) -> usize {
    limit.saturating_sub(buffer.len())
}

fn admit(buffer: &mut Vec<u8>, addition: &[u8], limit: usize) -> Result<(), Error> {
    let total = buffer.len().saturating_add(addition.len());
    if total > limit {
        return Err(Error::InputTooLarge { limit });
    }
    buffer.extend_from_slice(addition);
    Ok(())
}

fn document_facts(options: &RenderOptions) -> crate::contracts::DocumentFacts {
    crate::contracts::DocumentFacts {
        title: options
            .furniture_facts
            .title
            .clone()
            .or_else(|| options.title.clone()),
        ..options.furniture_facts.clone()
    }
}

fn stamp_furniture(
    pages: &mut crate::contracts::Pages,
    furniture: &crate::contracts::Furniture,
    subdocuments: &crate::contracts::Subdocuments,
    usable: crate::contracts::UsableHeight,
    intent: &crate::contracts::RenderingIntent,
    templating: &crate::compose::Templating<'_>,
    warnings: &mut Vec<crate::contracts::Warning>,
) -> Result<(), Error> {
    if furniture.is_empty() {
        return Ok(());
    }

    let mut furniture = furniture.clone();
    let refuse = |band: &mut Option<crate::contracts::MarginBand>,
                  available: crate::contracts::Length,
                  edge: &'static str,
                  warnings: &mut Vec<crate::contracts::Warning>| {
        let Some(present) = band.as_ref() else {
            return;
        };
        if available.get() > 0.0 && present.spacing.get() >= available.get() {
            warnings.push(crate::contracts::Warning::FurnitureRefused {
                edge,
                needed: present.spacing.get(),
                available: available.get(),
            });
            *band = None;
        }
    };
    refuse(
        &mut furniture.top,
        intent.layout_margin.top,
        "--header-spacing",
        warnings,
    );
    refuse(
        &mut furniture.bottom,
        intent.layout_margin.bottom,
        "--footer-spacing",
        warnings,
    );
    if furniture.is_empty() {
        return Ok(());
    }

    let spacing = |band: Option<&crate::contracts::MarginBand>| band.map(|band| band.spacing);
    let frame = crate::contracts::PageFrame::derive(
        intent.physical_page,
        intent.layout_margin,
        spacing(furniture.top.as_ref()),
        spacing(furniture.bottom.as_ref()),
    )?;
    crate::compose::stamp(
        pages,
        &furniture,
        subdocuments,
        usable,
        &frame,
        templating,
        warnings,
    )?;

    Ok(())
}

fn name_undrawable_ink(band: &'static str, ink: &BandInk, warnings: &mut Vec<Warning>) {
    for (kind, count) in [
        ("a background", ink.backgrounds),
        ("borders", ink.borders),
        ("table geometry", ink.tables),
    ] {
        if count > 0 {
            warnings.push(crate::contracts::Warning::FurnitureInkNotDrawn { band, kind, count });
        }
    }
}

fn refuse_network_bands(options: &RenderOptions) -> Result<(), Error> {
    for source in [options.header_html.as_ref(), options.footer_html.as_ref()]
        .into_iter()
        .flatten()
    {
        if let crate::contracts::BandSource::Path(path) = source {
            let shown = path.display().to_string();
            if ["http://", "https://", "ftp://", "//"]
                .iter()
                .any(|scheme| shown.starts_with(scheme))
            {
                return Err(Error::NetworkInput { origin: shown });
            }
        }
    }
    Ok(())
}

fn read_band_sources(
    options: &RenderOptions,
    host: &impl RenderHost,
    furniture: &mut crate::contracts::Furniture,
    fonts: &crate::font::FontTable,
    spent: usize,
    warnings: &mut Vec<Warning>,
    link_refusals: &mut LinkRefusals,
) -> crate::contracts::Subdocuments {
    let mut subdocuments = crate::contracts::Subdocuments::default();

    let mut spent = spent;
    for (source, band, named) in [
        (options.header_html.as_ref(), &mut furniture.top, "top"),
        (
            options.footer_html.as_ref(),
            &mut furniture.bottom,
            "bottom",
        ),
    ] {
        let Some(source) = source else { continue };

        let read = match source {
            crate::contracts::BandSource::Markup(text) => {
                if text.len() > options.max_input_bytes.saturating_sub(spent) {
                    Err(Error::InputTooLarge {
                        limit: options.max_input_bytes,
                    })
                } else {
                    Ok(text.clone())
                }
            }
            crate::contracts::BandSource::Path(path) => host
                .read_file(
                    path,
                    options.max_input_bytes.saturating_sub(spent),
                    Requester::Caller,
                )
                .map_err(Error::Host)
                .and_then(|bytes| {
                    String::from_utf8(bytes).map_err(|error| Error::NotUtf8 {
                        origin: path.display().to_string(),
                        position: error.utf8_error().valid_up_to(),
                    })
                }),
        };
        let markup = match read {
            Ok(markup) => {
                spent = spent.saturating_add(markup.len());
                markup
            }
            Err(error) => {
                warnings.push(crate::contracts::Warning::FurnitureDocumentRefused {
                    band: named,
                    reason: error.to_string(),
                });
                continue;
            }
        };

        let mut band_warnings = Vec::new();

        let compiled = match compile_band(
            &markup,
            seed_for(band.as_ref()),
            options,
            host,
            fonts,
            &mut band_warnings,
            link_refusals,
        ) {
            Ok(compiled) => compiled,
            Err(error) => {
                warnings.extend(band_warnings.into_iter().map(|inner| {
                    crate::contracts::Warning::FurnitureDocument {
                        band: named,
                        inner: Box::new(inner),
                    }
                }));
                warnings.push(crate::contracts::Warning::FurnitureDocumentRefused {
                    band: named,
                    reason: error.to_string(),
                });
                continue;
            }
        };
        warnings.extend(band_warnings.into_iter().map(|inner| {
            crate::contracts::Warning::FurnitureDocument {
                band: named,
                inner: Box::new(inner),
            }
        }));
        let (compiled, ink) = compiled;
        name_undrawable_ink(named, &ink, warnings);
        let id = subdocuments.add(compiled);
        match band {
            Some(existing) => {
                let Some(centre) = existing.content.get_mut(1) else {
                    continue;
                };
                if !centre.is_empty() {
                    warnings.push(crate::contracts::Warning::FurnitureBoxDisplaced {
                        band: named,
                        box_name: "centre",
                    });
                }
                *centre = crate::contracts::BoxContent::Document(id);
            }
            None => *band = Some(crate::contracts::MarginBand::for_subdocument(id)),
        }
    }
    subdocuments
}

fn compile_band(
    markup: &str,
    seed: Option<crate::contracts::RootSeed>,
    options: &RenderOptions,
    host: &impl RenderHost,
    fonts: &crate::font::FontTable,
    warnings: &mut Vec<Warning>,
    link_refusals: &mut LinkRefusals,
) -> Result<(crate::contracts::BoxTree, BandInk), Error> {
    let (dom, parse_warnings) = parse::parse(
        markup.as_bytes().to_vec(),
        "furniture",
        crate::contracts::DocumentBase::None,
        options.max_nodes,
        options.max_depth,
        options.encoding,
    )?;
    warnings.extend(parse_warnings);
    let medium = evaluated_medium(options.medium, &options.mode);
    let linked = resolve_linked_texts(&dom, host, medium, warnings);
    let linked = linked
        .into_iter()
        .map(|source| source.map(|source| source.text))
        .collect();
    let (styled, css_warnings, band_link_refusals) = cascade::cascade(
        dom,
        linked,
        options.colours,
        medium,
        LinkPolicy::FurnitureUnavailable,
        seed,
    )?;
    link_refusals.merge(&band_link_refusals);
    warnings.extend(css_warnings);
    if !styled.font_faces.is_empty() {
        warnings.push(Warning::FurnitureFontFacesIgnored {
            count: styled.font_faces.len(),
        });
    }
    if !styled.author_page.is_empty() {
        warnings.push(Warning::AuthorPageIgnored {
            reason: "page geometry in a header/footer subdocument cannot establish the body page",
        });
    }

    let mut tree = boxes::build_with_fonts(styled, warnings, Some(fonts))?;

    let mut images = 0_usize;
    strip_band_images(&mut tree.root, &mut images);
    if images > 0 {
        warnings.push(Warning::FurnitureImageNotDrawn { count: images });
    }

    let mut ink = BandInk::default();
    survey_band_ink(&tree.root, &mut ink);
    if tree.canvas.is_some() {
        ink.backgrounds = ink.backgrounds.saturating_add(1);
    }
    Ok((tree, ink))
}

fn seed_for(band: Option<&crate::contracts::MarginBand>) -> Option<crate::contracts::RootSeed> {
    let band = band?;
    Some(crate::contracts::RootSeed {
        font_size: Some(band.font_size),

        font_family: Some(match band.face {
            crate::metrics::Face::Helvetica
            | crate::metrics::Face::HelveticaBold
            | crate::metrics::Face::HelveticaOblique
            | crate::metrics::Face::HelveticaBoldOblique => crate::contracts::FontFamily::Helvetica,
            crate::metrics::Face::Courier
            | crate::metrics::Face::CourierBold
            | crate::metrics::Face::CourierOblique
            | crate::metrics::Face::CourierBoldOblique => crate::contracts::FontFamily::Courier,
            crate::metrics::Face::TimesRoman
            | crate::metrics::Face::TimesBold
            | crate::metrics::Face::TimesItalic
            | crate::metrics::Face::TimesBoldItalic => crate::contracts::FontFamily::Times,
        }),
        colour: Some(band.colour),
    })
}

#[derive(Debug, Default)]
struct BandInk {
    backgrounds: usize,
    borders: usize,
    tables: usize,
}

fn survey_band_ink(node: &crate::contracts::BoxNode, seen: &mut BandInk) {
    if node.surface != crate::contracts::Surface::Canvas {
        seen.backgrounds = seen.backgrounds.saturating_add(1);
    }
    if node.paints {
        seen.borders = seen.borders.saturating_add(1);
    }
    if matches!(
        node.display,
        crate::contracts::Display::Table
            | crate::contracts::Display::Row
            | crate::contracts::Display::RowGroup
            | crate::contracts::Display::Cell
    ) {
        seen.tables = seen.tables.saturating_add(1);
    }
    for child in &node.children {
        survey_band_ink(child, seen);
    }
}

fn strip_band_images(node: &mut crate::contracts::BoxNode, count: &mut usize) {
    if let crate::contracts::InlineContent::Replaced(request) = &node.content {
        *count = count.saturating_add(1);
        node.content = match &request.alt {
            Some(alt) if !alt.is_empty() => crate::contracts::InlineContent::Text(alt.clone()),
            _ => crate::contracts::InlineContent::Empty,
        };
    }
    for child in &mut node.children {
        strip_band_images(child, count);
    }
}

fn warn_if_boxes_left_the_sheet(
    fragments: &crate::contracts::FragmentTree,
    intent: &crate::contracts::RenderingIntent,
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    if fragments.overrun.0 > 0 {
        warnings.push(crate::contracts::Warning::TextReachesPastTheSheet {
            count: fragments.overrun.0,

            worst: fragments.overrun.1 * intent.output.get(),
        });
    }
    let Some(worst) = fragments.escaped.worst() else {
        return;
    };
    warnings.push(crate::contracts::Warning::BoxOutsideTheSheet {
        count: fragments.escaped.count(),

        worst: worst.distance * intent.output.get(),
        edge: worst.edge,

        remedy: match worst.cause {
            crate::layout::EscapeCause::DeclaredWidth => {
                "a declared width exceeds the page; reduce it or widen the page"
            }
            crate::layout::EscapeCause::Displacement => {
                "the box is pushed off the paper rather than sized off it; \
                 check for a negative margin or offset"
            }
            crate::layout::EscapeCause::Solved => {
                "the content cannot be narrowed by a page flag; change the document"
            }
        },
    });
}

fn warn_if_images_were_fitted(
    fragments: &crate::contracts::FragmentTree,
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    let count = fragments.refitted.0;
    if count == 0 {
        return;
    }
    let smallest = fragments.refitted.1.clamp(0.0, 1.0);
    warnings.push(crate::contracts::Warning::ImageScaledToFit { count, smallest });
}

fn warn_about_the_inline_axis(
    fragments: &crate::contracts::FragmentTree,
    intent: &crate::contracts::RenderingIntent,
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    warn_if_boxes_left_the_sheet(fragments, intent, warnings);
    warn_if_images_were_fitted(fragments, warnings);
}

fn acquire_images(
    box_tree: &mut crate::contracts::BoxTree,
    world: &crate::acquire::World<'_>,
    assets: &mut crate::acquire::Assets,
    warnings: &mut Vec<crate::contracts::Warning>,
) {
    let mut skipped = Vec::new();
    crate::acquire::resolve_tree(&mut box_tree.root, world, assets, &mut skipped);
    report_skipped_images(&skipped, warnings);
}
