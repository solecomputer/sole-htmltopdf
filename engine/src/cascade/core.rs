use super::{
    ComputedStyle, Diagnostics, Dom, Error, FontSize, INITIAL_FONT_SIZE, INITIAL_MARGIN,
    SpacingEdges, StyledNode, StyledTree, Walk, Warning, css,
};

pub(crate) fn cascade(
    dom: Dom,
    linked_texts: Vec<Option<String>>,
    colours: crate::contracts::Colours,

    medium: crate::contracts::Evaluated,
    link_policy: crate::contracts::LinkPolicy,

    seed: Option<crate::contracts::RootSeed>,
) -> Result<(StyledTree, Vec<Warning>, crate::contracts::LinkRefusals), Error> {
    let linked_texts = linked_texts
        .into_iter()
        .map(|source| {
            source.map(|text| crate::inputs::AuthorSheet {
                origin: "<linked stylesheet>".to_owned(),
                text,
                base: None,
            })
        })
        .collect();
    cascade_with_author_sheets(
        dom,
        linked_texts,
        colours,
        medium,
        link_policy,
        seed,
        Vec::new(),
    )
}

pub(crate) fn cascade_with_author_sheets(
    dom: Dom,
    linked_texts: Vec<Option<crate::inputs::AuthorSheet>>,
    colours: crate::contracts::Colours,
    medium: crate::contracts::Evaluated,
    link_policy: crate::contracts::LinkPolicy,
    seed: Option<crate::contracts::RootSeed>,
    author_sheets: Vec<crate::inputs::AuthorSheet>,
) -> Result<(StyledTree, Vec<Warning>, crate::contracts::LinkRefusals), Error> {
    let style = initial_style(seed.as_ref())?;

    let Dom {
        root, sheets, base, ..
    } = dom;

    let mut diagnostics = Diagnostics::default();
    let mut link_refusals = crate::contracts::LinkRefusals::default();

    let mut rules: Vec<css::Rule> = Vec::new();
    let mut furniture = crate::contracts::Furniture::default();
    let mut author_page = crate::contracts::AuthorPage::default();
    let mut font_faces = Vec::new();
    let ua = css::parse_sheet(css::USER_AGENT_STYLESHEET, css::Origin::UserAgent, medium);

    debug_assert!(
        ua.refused_selectors.is_empty() && ua.malformed.is_empty(),
        "the user-agent stylesheet must parse cleanly"
    );
    rules.extend(ua.rules);

    let mut linked_texts = linked_texts.into_iter();
    for sheet in sheets {
        let (source, media) = match sheet {
            crate::contracts::SheetRef::Inline { text, media } => (
                Some(crate::inputs::AuthorSheet {
                    origin: "<style>".to_owned(),
                    text,
                    base: match &base {
                        crate::contracts::DocumentBase::Directory(path) => Some(path.clone()),
                        crate::contracts::DocumentBase::None => None,
                    },
                }),
                media,
            ),
            crate::contracts::SheetRef::Linked { media, .. } => {
                (linked_texts.next().flatten(), media)
            }
        };
        if let Some(list) = media.as_deref()
            && crate::css::media_admits(
                list,
                medium,
                &mut diagnostics.media_unsupported,
                &mut diagnostics.media_malformed,
            ) != crate::css::MediaAdmits::Yes
        {
            continue;
        }
        let Some(source) = source else {
            continue;
        };
        let sheet = css::parse_sheet(&source.text, css::Origin::Author, medium);
        append_font_faces(&mut font_faces, &sheet, &source);
        merge_diagnostics(&mut diagnostics, &sheet);

        if sheet.page.seen {
            crate::css::merge_furniture(&mut furniture, sheet.page.furniture);
            author_page.merge(sheet.page.author);
        }
        diagnostics.refused.merge(&sheet.page.refused);
        diagnostics.translated.merge(&sheet.page.translated);
        rules.extend(sheet.rules);
    }
    let supplied_warnings = append_author_sheets(
        author_sheets,
        medium,
        &mut rules,
        &mut furniture,
        &mut author_page,
        &mut font_faces,
    );

    let styled = Walk::new(
        &rules,
        &mut diagnostics,
        colours,
        link_policy,
        &mut link_refusals,
    )
    .style_node(&root, &style, None)
    .unwrap_or(StyledNode {
        tag: root.tag().map(str::to_owned),
        text: None,
        style,
        children: Vec::new(),
        image: None,
        link: None,
    });
    let mut warnings = diagnostics.into_warnings();
    warnings.extend(supplied_warnings);
    Ok((
        StyledTree {
            author_page,
            font_faces,
            furniture,
            root: styled,
        },
        warnings,
        link_refusals,
    ))
}

fn append_font_faces(
    faces: &mut Vec<crate::font::FaceRequest>,
    sheet: &css::Sheet,
    source: &crate::inputs::AuthorSheet,
) {
    faces.extend(
        sheet
            .font_faces
            .iter()
            .cloned()
            .map(|rule| crate::font::FaceRequest {
                rule,
                base: source.base.clone(),
                origin: source.origin.clone(),
            }),
    );
}

fn merge_diagnostics(diagnostics: &mut Diagnostics, sheet: &css::Sheet) {
    diagnostics.unsupported.merge(&sheet.unsupported);
    diagnostics.malformed.merge(&sheet.malformed);
    diagnostics.unterminated |= sheet.unterminated;
    diagnostics.refused.merge(&sheet.refused_selectors);
    diagnostics.page_selectors.merge(&sheet.page_selectors);
    diagnostics
        .media_unsupported
        .merge(&sheet.media_unsupported);
    diagnostics.media_malformed.merge(&sheet.media_malformed);
    diagnostics.media_too_deep = diagnostics
        .media_too_deep
        .saturating_add(sheet.media_too_deep);
}

fn append_author_sheets(
    author_sheets: Vec<crate::inputs::AuthorSheet>,
    medium: crate::contracts::Evaluated,
    rules: &mut Vec<css::Rule>,
    furniture: &mut crate::contracts::Furniture,
    author_page: &mut crate::contracts::AuthorPage,
    font_faces: &mut Vec<crate::font::FaceRequest>,
) -> Vec<Warning> {
    let mut warnings = Vec::new();
    for supplied in author_sheets {
        let sheet = css::parse_sheet(&supplied.text, css::Origin::Author, medium);
        append_font_faces(font_faces, &sheet, &supplied);
        let mut supplied_diagnostics = Diagnostics::default();
        supplied_diagnostics.unsupported.merge(&sheet.unsupported);
        supplied_diagnostics.malformed.merge(&sheet.malformed);
        supplied_diagnostics.unterminated |= sheet.unterminated;
        supplied_diagnostics.refused.merge(&sheet.refused_selectors);
        supplied_diagnostics
            .page_selectors
            .merge(&sheet.page_selectors);
        supplied_diagnostics
            .media_unsupported
            .merge(&sheet.media_unsupported);
        supplied_diagnostics
            .media_malformed
            .merge(&sheet.media_malformed);
        supplied_diagnostics.media_too_deep = sheet.media_too_deep;
        if sheet.page.seen {
            crate::css::merge_furniture(furniture, sheet.page.furniture);
            author_page.merge(sheet.page.author);
        }
        supplied_diagnostics.refused.merge(&sheet.page.refused);
        supplied_diagnostics
            .translated
            .merge(&sheet.page.translated);
        rules.extend(sheet.rules);
        let issues = supplied_diagnostics
            .into_warnings()
            .into_iter()
            .map(|warning| warning.to_string())
            .collect::<Vec<_>>();
        if !issues.is_empty() {
            warnings.push(Warning::StylesheetDiagnostics {
                path: supplied.origin,
                issues,
            });
        }
    }
    warnings
}

fn initial_style(seed: Option<&crate::contracts::RootSeed>) -> Result<ComputedStyle, Error> {
    Ok(ComputedStyle {
        font_size: match seed.and_then(|seed| seed.font_size) {
            Some(size) => size,
            None => FontSize::try_new(INITIAL_FONT_SIZE)?,
        },
        margin: SpacingEdges::uniform(crate::contracts::Spacing::points(INITIAL_MARGIN)),
        padding: SpacingEdges::default(),
        line_height: crate::contracts::Spacing::em(css::NORMAL_LINE_HEIGHT),
        border: crate::contracts::BorderEdges::default(),
        width: crate::contracts::Width::Auto,
        box_sizing: crate::contracts::BoxSizing::ContentBox,
        display: crate::contracts::Display::Block,
        flex_container: crate::contracts::FlexContainerStyle::default(),
        flex_item: crate::contracts::FlexItemStyle::default(),
        text_align: crate::contracts::TextAlign::Left,
        font_family: seed
            .and_then(|seed| seed.font_family)
            .unwrap_or(crate::contracts::FontFamily::Times),
        font_families: std::sync::Arc::from([]),
        font_weight: crate::contracts::FontWeight::Normal,
        font_style: crate::contracts::FontStyle::Normal,
        colour: seed
            .and_then(|seed| seed.colour)
            .unwrap_or(crate::contracts::Rgb::BLACK),
        surface: crate::contracts::Surface::Canvas,
        paints: false,
    })
}
