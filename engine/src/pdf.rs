use std::collections::{BTreeMap, BTreeSet};

use pdf_writer::types::{
    ActionType, AnnotationType, CidFontType, FontFlags, Predictor, SystemInfo, UnicodeCmap,
};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

use crate::contracts::{DisplayItem, DisplayList, PdfBytes, Warning};
use crate::encoding;
use crate::metrics::Face;
use read_fonts::TableProvider;

pub(crate) struct Written {
    pub(crate) pdf: PdfBytes,
    pub(crate) warnings: Vec<Warning>,
}

const MAX_LINK_ANNOTATIONS: usize = 10_000;

const MAX_SUBSET_BYTES_PER_FACE: usize = 8 * 1024 * 1024;

const MAX_SUBSET_BYTES_PER_DOCUMENT: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CidKey {
    glyph: u16,
    extraction: String,
}

struct EmbeddedData {
    face: crate::font::FontId,
    resource: Vec<u8>,
    base_name: Vec<u8>,
    bytes: Vec<u8>,
    cid_to_gid: Vec<u8>,
    widths: Vec<f32>,
    cmap: Vec<u8>,
    cids: BTreeMap<CidKey, u16>,
    bbox: [f32; 4],
    ascent: f32,
    descent: f32,
}

struct EmbeddedFont {
    data: EmbeddedData,
    type0: Ref,
    cid_font: Ref,
    descriptor: Ref,
    font_file: Ref,
    gid_map: Ref,
    cmap: Ref,
}

fn emitted_gid(is_subset: bool, remapper: &subsetter::GlyphRemapper, original: u16) -> Option<u16> {
    if is_subset {
        remapper.get(original)
    } else {
        Some(original)
    }
}

struct FontObjects {
    base: Vec<(Face, Ref)>,
    embedded: Vec<EmbeddedFont>,
    unembedded: usize,
}

fn allocate_fonts(
    list: &DisplayList,
    fonts: &crate::font::FontTable,
    alloc: &mut Ref,
) -> FontObjects {
    let base = faces_used(list)
        .into_iter()
        .map(|face| (face, alloc.bump()))
        .collect();
    let (embedded_data, unembedded) = prepare_embedded_fonts(list, fonts);
    let embedded = embedded_data
        .into_iter()
        .map(|data| EmbeddedFont {
            data,
            type0: alloc.bump(),
            cid_font: alloc.bump(),
            descriptor: alloc.bump(),
            font_file: alloc.bump(),
            gid_map: alloc.bump(),
            cmap: alloc.bump(),
        })
        .collect();
    FontObjects {
        base,
        embedded,
        unembedded,
    }
}

fn run_keys(run: &crate::font::PositionedRun) -> Vec<CidKey> {
    let mut seen = BTreeSet::new();
    run.glyphs
        .iter()
        .map(|glyph| {
            let extraction = if seen.insert(glyph.cluster) {
                let start = usize::try_from(glyph.cluster).unwrap_or(run.source.len());

                let end = run
                    .glyphs
                    .iter()
                    .filter_map(|next| usize::try_from(next.cluster).ok())
                    .filter(|next| *next > start)
                    .min()
                    .unwrap_or(run.source.len());
                run.source.get(start..end).unwrap_or_default().to_owned()
            } else {
                String::new()
            };
            CidKey {
                glyph: glyph.id,
                extraction,
            }
        })
        .collect()
}

fn authored_uses(list: &DisplayList) -> Vec<(crate::font::FontId, Vec<CidKey>)> {
    let mut uses: Vec<(crate::font::FontId, Vec<CidKey>)> = Vec::new();
    for page in &list.pages {
        for item in &page.items {
            let DisplayItem::Text {
                positioned: Some(run),
                ..
            } = item
            else {
                continue;
            };
            let position = uses
                .iter()
                .position(|(face, _)| *face == run.face)
                .unwrap_or_else(|| {
                    uses.push((run.face, Vec::new()));
                    uses.len().saturating_sub(1)
                });
            if let Some((_, keys)) = uses.get_mut(position) {
                for key in run_keys(run) {
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
        }
    }
    uses
}

fn prepare_embedded_fonts(
    list: &DisplayList,
    fonts: &crate::font::FontTable,
) -> (Vec<EmbeddedData>, usize) {
    let mut prepared = Vec::new();
    let mut refused = 0_usize;
    let mut total_bytes = 0_usize;
    for (position, (face, keys)) in authored_uses(list).into_iter().enumerate() {
        let Some(entry) = fonts.entry(face) else {
            refused = refused.saturating_add(1);
            continue;
        };
        let Ok(font) = harfrust::FontRef::new(&entry.bytes) else {
            refused = refused.saturating_add(1);
            continue;
        };
        let Ok(head) = font.head() else {
            refused = refused.saturating_add(1);
            continue;
        };
        let Ok(hmtx) = font.hmtx() else {
            refused = refused.saturating_add(1);
            continue;
        };
        if keys.len() >= usize::from(u16::MAX) {
            refused = refused.saturating_add(1);
            continue;
        }
        let mut remapper = subsetter::GlyphRemapper::new();
        for key in &keys {
            remapper.remap(key.glyph);
        }
        let subset = subsetter::subset(&entry.bytes, 0, &remapper)
            .ok()
            .filter(|subset| subset.len() < entry.bytes.len());
        let is_subset = subset.is_some();
        let bytes = subset.unwrap_or_else(|| entry.bytes.to_vec());
        if bytes.len() > MAX_SUBSET_BYTES_PER_FACE
            || total_bytes.saturating_add(bytes.len()) > MAX_SUBSET_BYTES_PER_DOCUMENT
        {
            refused = refused.saturating_add(1);
            continue;
        }
        total_bytes = total_bytes.saturating_add(bytes.len());

        let mut cids = BTreeMap::new();
        let mut cid_to_gid = vec![0_u8, 0_u8];
        let mut widths = vec![0.0_f32];
        let upem = f32::from(entry.units_per_em);
        let info = SystemInfo {
            registry: Str(b"Adobe"),
            ordering: Str(b"Identity"),
            supplement: 0,
        };
        let cmap_name = format!("E27Unicode{position}").into_bytes();
        let mut cmap = UnicodeCmap::new(Name(&cmap_name), info);
        for (index, key) in keys.into_iter().enumerate() {
            let Ok(cid) = u16::try_from(index.saturating_add(1)) else {
                continue;
            };
            let Some(emitted_gid) = emitted_gid(is_subset, &remapper, key.glyph) else {
                continue;
            };
            cid_to_gid.extend_from_slice(&emitted_gid.to_be_bytes());
            let advance = hmtx
                .advance(read_fonts::types::GlyphId::new(u32::from(key.glyph)))
                .map_or(0.0, |width| f32::from(width) * 1_000.0 / upem);
            widths.push(advance);
            cmap.pair_with_multiple(cid, key.extraction.chars());
            cids.insert(key, cid);
        }
        let scale = 1_000.0 / upem;
        prepared.push(EmbeddedData {
            face,
            resource: format!("EF{position}").into_bytes(),
            base_name: format!("E27Subset{position}").into_bytes(),
            bytes,
            cid_to_gid,
            widths,
            cmap: cmap.finish().to_vec(),
            cids,
            bbox: [
                f32::from(head.x_min()) * scale,
                f32::from(head.y_min()) * scale,
                f32::from(head.x_max()) * scale,
                f32::from(head.y_max()) * scale,
            ],
            ascent: f32::from(entry.ascent) * scale,
            descent: f32::from(entry.descent) * scale,
        });
    }
    (prepared, refused)
}

fn show_text(
    content: &mut Content,
    bytes: &[u8],
    x: crate::contracts::Length,
    y: crate::contracts::Length,
    font_size: crate::contracts::FontSize,
    face: crate::metrics::Face,
    colour: crate::contracts::Rgb,
) {
    let (red, green, blue) = colour.channels();
    content.save_state();
    content.set_fill_rgb(red, green, blue);
    content.begin_text();
    content.set_font(Name(face.resource_name()), font_size.get());
    content.next_line(x.get(), y.get());

    content.show(Str(bytes));
    content.end_text();
    content.restore_state();
}

fn fill_rect(
    content: &mut Content,
    x: crate::contracts::Length,
    y: crate::contracts::Length,
    width: crate::contracts::Length,
    height: crate::contracts::Length,
    colour: crate::contracts::Rgb,
) {
    let (red, green, blue) = colour.channels();
    content.save_state();
    content.set_fill_rgb(red, green, blue);
    content.rect(x.get(), y.get(), width.get(), height.get());
    content.fill_nonzero();
    content.restore_state();
}

fn draw_image(
    content: &mut Content,
    name: Name<'_>,
    x: crate::contracts::Length,
    y: crate::contracts::Length,
    width: crate::contracts::Length,
    height: crate::contracts::Length,
) {
    content.save_state();
    content.transform([width.get(), 0.0, 0.0, height.get(), x.get(), y.get()]);
    content.x_object(name);
    content.restore_state();
}

fn write_image_xobject(pdf: &mut Pdf, id: Ref, image: &crate::image::Embeddable) {
    let mut xobject = pdf.image_xobject(id, image.bytes());
    xobject
        .width(i32::from(image.width()))
        .height(i32::from(image.height()))
        .bits_per_component(8);
    match image.colour() {
        crate::image::ColourSpace::Gray => xobject.color_space().device_gray(),
        crate::image::ColourSpace::Rgb => xobject.color_space().device_rgb(),
    }
    match image.payload() {
        crate::image::Payload::JpegFile => {
            xobject.filter(Filter::DctDecode);
        }
        crate::image::Payload::PngIdat => {
            xobject.filter(Filter::FlateDecode);
            xobject
                .decode_parms()
                .predictor(Predictor::PngOptimum)
                .colors(i32::from(image.colour().components()))
                .bits_per_component(8)
                .columns(i32::from(image.width()));
        }
    }
    xobject.finish();
}

fn images_used(list: &DisplayList) -> Vec<usize> {
    let mut used: Vec<usize> = Vec::new();
    for page in &list.pages {
        for item in &page.items {
            if let DisplayItem::Image { asset, .. } = item
                && !used.contains(asset)
            {
                used.push(*asset);
            }
        }
    }
    used
}

fn faces_used(list: &DisplayList) -> Vec<Face> {
    let mut faces: Vec<Face> = Vec::new();
    for page in &list.pages {
        for item in &page.items {
            if let DisplayItem::Text { face, .. } = item
                && !faces.contains(face)
            {
                faces.push(*face);
            }
        }
    }
    faces
}

fn write_fonts(pdf: &mut Pdf, font_ids: &[(Face, Ref)]) {
    for (face, font_id) in font_ids {
        pdf.type1_font(*font_id)
            .base_font(Name(face.base_font().as_bytes()))
            .encoding_predefined(Name(b"WinAnsiEncoding"))
            .finish();
    }
}

fn write_embedded_fonts(pdf: &mut Pdf, fonts: &[EmbeddedFont]) {
    let info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    for font in fonts {
        let base_name = Name(&font.data.base_name);
        pdf.type0_font(font.type0)
            .base_font(base_name)
            .encoding_predefined(Name(b"Identity-H"))
            .descendant_font(font.cid_font)
            .to_unicode(font.cmap);
        let mut cid_font = pdf.cid_font(font.cid_font);
        cid_font
            .subtype(CidFontType::Type2)
            .base_font(base_name)
            .system_info(info)
            .font_descriptor(font.descriptor)
            .default_width(0.0)
            .cid_to_gid_map_stream(font.gid_map);
        cid_font
            .widths()
            .consecutive(0, font.data.widths.iter().copied());
        cid_font.finish();

        pdf.font_descriptor(font.descriptor)
            .name(base_name)
            .flags(FontFlags::NON_SYMBOLIC)
            .bbox(Rect::new(
                font.data.bbox[0],
                font.data.bbox[1],
                font.data.bbox[2],
                font.data.bbox[3],
            ))
            .italic_angle(0.0)
            .ascent(font.data.ascent)
            .descent(font.data.descent)
            .cap_height(font.data.ascent)
            .stem_v(80.0)
            .font_file2(font.font_file);
        let length = i32::try_from(font.data.bytes.len()).unwrap_or(i32::MAX);
        pdf.stream(font.font_file, &font.data.bytes)
            .pair(Name(b"Length1"), length);
        pdf.stream(font.gid_map, &font.data.cid_to_gid);
        pdf.cmap(font.cmap, &font.data.cmap)
            .name(Name(&font.data.resource))
            .system_info(info);
    }
}

fn show_authored_text(
    content: &mut Content,
    run: &crate::font::PositionedRun,
    font: &EmbeddedFont,
    x: crate::contracts::Length,
    y: crate::contracts::Length,
    font_size: crate::contracts::FontSize,
    colour: crate::contracts::Rgb,
) {
    let (red, green, blue) = colour.channels();
    content.save_state();
    content.set_fill_rgb(red, green, blue);
    content.begin_text();
    content.set_font(Name(&font.data.resource), font_size.get());
    let keys = run_keys(run);
    let mut advance = 0.0_f32;
    for (glyph, key) in run.glyphs.iter().zip(keys) {
        let Some(cid) = font.data.cids.get(&key).copied() else {
            continue;
        };
        content.set_text_matrix([
            1.0,
            0.0,
            0.0,
            1.0,
            x.get() + advance + glyph.x_offset,
            y.get() + glyph.y_offset,
        ]);
        let shown = cid.to_be_bytes();
        let mut positioned = content.show_positioned();
        let mut items = positioned.items();
        items.show(Str(&shown));

        items.adjust(-glyph.x_advance * 1_000.0 / font_size.get());
        items.finish();
        positioned.finish();
        advance += glyph.x_advance;
    }
    content.end_text();
    content.restore_state();
}

struct PageObject {
    page_id: Ref,
    content_id: Ref,
    tree_id: Ref,
    size: (f32, f32),
    annotations: Vec<Ref>,
}

fn write_page_object(
    pdf: &mut Pdf,
    ids: PageObject,
    font_ids: &[(Face, Ref)],
    embedded: &[EmbeddedFont],
    on_this_page: &[&(usize, Ref, Vec<u8>)],
) {
    let (width, height) = ids.size;
    let mut page_object = pdf.page(ids.page_id);
    page_object
        .parent(ids.tree_id)
        .media_box(Rect::new(0.0, 0.0, width, height))
        .contents(ids.content_id);
    if !ids.annotations.is_empty() {
        page_object.annotations(ids.annotations);
    }
    let mut resources = page_object.resources();
    let mut fonts = resources.fonts();
    for (face, font_id) in font_ids {
        fonts.pair(Name(face.resource_name()), *font_id);
    }
    for font in embedded {
        fonts.pair(Name(&font.data.resource), font.type0);
    }
    fonts.finish();

    if !on_this_page.is_empty() {
        let mut xobjects = resources.x_objects();
        for (_, id, name) in on_this_page {
            xobjects.pair(Name(name), *id);
        }
        xobjects.finish();
    }
    resources.finish();
    page_object.finish();
}

fn write_link_annotation(pdf: &mut Pdf, id: Ref, link: &crate::contracts::PdfLink) {
    let mut annotation = pdf.annotation(id);
    annotation.subtype(AnnotationType::Link).rect(Rect::new(
        link.x.get(),
        link.y.get(),
        link.x.get() + link.width.get(),
        link.y.get() + link.height.get(),
    ));
    annotation.border(0.0, 0.0, 0.0, None);
    annotation
        .action()
        .action_type(ActionType::Uri)
        .uri(Str(link.target.as_str().as_bytes()));
    annotation.finish();
}

fn clamp_links(list: &mut DisplayList) -> usize {
    let mut remaining = MAX_LINK_ANNOTATIONS;
    let mut omitted = 0_usize;
    for page in &mut list.pages {
        let retained = page.links.len().min(remaining);
        omitted = omitted.saturating_add(page.links.len().saturating_sub(retained));
        page.links.truncate(retained);
        remaining = remaining.saturating_sub(retained);
    }
    omitted
}

fn finish_warnings(
    omitted_links: usize,
    undrawable: usize,
    unembedded_fonts: usize,
    unrepresentable: usize,
    first_unrepresentable: Option<char>,
) -> Vec<Warning> {
    let mut warnings = Vec::new();
    if omitted_links > 0 {
        warnings.push(Warning::LinkAnnotationsOmitted {
            count: omitted_links,
            limit: MAX_LINK_ANNOTATIONS,
        });
    }

    if undrawable > 0 {
        warnings.push(Warning::ImageSkipped {
            count: undrawable,
            cause: "its asset was not in the document's table when the page was written",
        });
    }
    if unembedded_fonts > 0 {
        warnings.push(Warning::AuthoredFontsNotEmbedded {
            count: unembedded_fonts,
        });
    }
    if let Some(first) = first_unrepresentable {
        warnings.push(Warning::UnrepresentableCharacters {
            count: unrepresentable,
            first,
        });
    }
    warnings
}

struct Ink {
    content: Content,

    unrepresentable: usize,
    first_unrepresentable: Option<char>,

    undrawable: usize,
}

fn page_content(
    items: Vec<DisplayItem>,
    on_this_page: &[&(usize, Ref, Vec<u8>)],
    embedded: &[EmbeddedFont],
    sheet: (f32, f32),
) -> Ink {
    let mut unrepresentable = 0_usize;
    let mut first_unrepresentable: Option<char> = None;
    let mut undrawable = 0_usize;
    let mut content = Content::new();

    content.save_state();
    content.rect(0.0, 0.0, sheet.0, sheet.1);
    content.clip_nonzero();
    content.end_path();

    for item in items {
        match item {
            DisplayItem::Text {
                text,
                x,
                y,
                font_size,
                face,
                colour,
                clip,
                continued,
                positioned,
            } => {
                if let Some(band) = clip {
                    content.save_state();

                    content.rect(0.0, band.y.get(), sheet.0, band.height.get());
                    content.clip_nonzero();
                    content.end_path();
                }
                if continued {
                    content
                        .begin_marked_content_with_properties(Name(b"Span"))
                        .properties()
                        .actual_text(TextStr(""));
                }
                if let Some(run) = positioned.as_ref()
                    && let Some(font) = embedded.iter().find(|font| font.data.face == run.face)
                {
                    show_authored_text(&mut content, run, font, x, y, font_size, colour);
                } else {
                    let encoded = encoding::to_winansi(&text);

                    if !continued {
                        unrepresentable = unrepresentable.saturating_add(encoded.unrepresentable);
                        if first_unrepresentable.is_none() {
                            first_unrepresentable = encoded.first_unrepresentable;
                        }
                    }
                    show_text(&mut content, &encoded.bytes, x, y, font_size, face, colour);
                }
                if continued {
                    content.end_marked_content();
                }
                if clip.is_some() {
                    content.restore_state();
                }
            }
            DisplayItem::Rect {
                x,
                y,
                width,
                height,
                colour,
            } => fill_rect(&mut content, x, y, width, height, colour),
            DisplayItem::Image {
                asset,
                x,
                y,
                width,
                height,
            } => match on_this_page.iter().find(|(id, ..)| *id == asset) {
                Some((_, _, name)) => {
                    draw_image(&mut content, Name(name), x, y, width, height);
                }
                None => undrawable = undrawable.saturating_add(1),
            },
        }
    }

    content.restore_state();

    Ink {
        content,
        unrepresentable,
        first_unrepresentable,
        undrawable,
    }
}

pub(crate) fn write(
    mut list: DisplayList,
    title: Option<&str>,
    assets: &crate::acquire::Assets,
) -> Written {
    let omitted_links = clamp_links(&mut list);
    let mut alloc = Ref::new(1);
    let catalog_id = alloc.bump();
    let tree_id = alloc.bump();

    let fonts = allocate_fonts(&list, &assets.fonts, &mut alloc);

    let drawn = images_used(&list);
    let image_ids: Vec<(usize, Ref, Vec<u8>)> = drawn
        .iter()
        .enumerate()
        .map(|(position, asset)| (*asset, alloc.bump(), format!("Im{position}").into_bytes()))
        .collect();
    let info_id = alloc.bump();

    let page_ids: Vec<(Ref, Ref, Vec<Ref>)> = list
        .pages
        .iter()
        .map(|page| {
            let page_id = alloc.bump();
            let content_id = alloc.bump();
            let annotations = page.links.iter().map(|_| alloc.bump()).collect();
            (page_id, content_id, annotations)
        })
        .collect();

    let mut pdf = Pdf::new();
    pdf.catalog(catalog_id).pages(tree_id);

    let mut tree = pdf.pages(tree_id);
    tree.kids(page_ids.iter().map(|(page_id, _, _)| *page_id));
    tree.count(page_count(page_ids.len()));
    tree.finish();

    write_fonts(&mut pdf, &fonts.base);
    write_embedded_fonts(&mut pdf, &fonts.embedded);
    for (asset, id, _) in &image_ids {
        if let Some(image) = assets.get(crate::acquire::AssetId::from_index(*asset)) {
            write_image_xobject(&mut pdf, *id, image);
        }
    }
    let mut undrawable = 0_usize;

    let width = list.page_size.width().get();
    let height = list.page_size.height().get();

    let mut unrepresentable = 0_usize;
    let mut first_unrepresentable: Option<char> = None;

    for (page, (page_id, content_id, annotation_ids)) in list.pages.into_iter().zip(page_ids) {
        let on_this_page: Vec<&(usize, Ref, Vec<u8>)> = image_ids
            .iter()
            .filter(|(asset, ..)| {
                page.items.iter().any(|item| {
                    matches!(item, DisplayItem::Image { asset: drawn, .. } if drawn == asset)
                })
            })
            .filter(|(asset, ..)| {
                assets
                    .get(crate::acquire::AssetId::from_index(*asset))
                    .is_some()
            })
            .collect();

        write_page_object(
            &mut pdf,
            PageObject {
                page_id,
                content_id,
                tree_id,
                size: (width, height),
                annotations: annotation_ids.clone(),
            },
            &fonts.base,
            &fonts.embedded,
            &on_this_page,
        );

        for (link, annotation_id) in page.links.iter().zip(annotation_ids) {
            write_link_annotation(&mut pdf, annotation_id, link);
        }

        let ink = page_content(page.items, &on_this_page, &fonts.embedded, (width, height));
        unrepresentable = unrepresentable.saturating_add(ink.unrepresentable);
        if first_unrepresentable.is_none() {
            first_unrepresentable = ink.first_unrepresentable;
        }
        undrawable = undrawable.saturating_add(ink.undrawable);
        let content = ink.content;
        pdf.stream(content_id, &content.finish());
    }

    let mut info = pdf.document_info(info_id);
    info.producer(TextStr("sole-htmltopdf"));

    if let Some(title) = title {
        info.title(TextStr(title));
    }
    info.finish();

    let warnings = finish_warnings(
        omitted_links,
        undrawable,
        fonts.unembedded,
        unrepresentable,
        first_unrepresentable,
    );

    Written {
        pdf: PdfBytes::new(pdf.finish()),
        warnings,
    }
}

fn page_count(count: usize) -> i32 {
    i32::try_from(count).unwrap_or(i32::MAX)
}
