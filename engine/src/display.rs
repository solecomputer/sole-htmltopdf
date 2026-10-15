use crate::contracts::{
    DisplayItem, DisplayList, DisplayPage, Edges, Error, FontSize, Length, PageSize, Pages, Scale,
};
use crate::space::{scale_extent, to_physical};

pub(crate) fn build(
    pages: Pages,
    physical_page: PageSize,
    margin: Edges,
    output: Scale,
) -> Result<DisplayList, Error> {
    let page_size = physical_page;
    let page_height = page_size.height().get();
    let page_width = page_size.width();
    let canvas = pages.canvas;
    let (anchor_x, anchor_y) = (margin.left, margin.top);

    let mut display_pages = Vec::with_capacity(pages.pages.len());
    for page in pages.pages {
        let links = map_links(page.links, page_size, anchor_x, anchor_y, output)?;
        let mut items = Vec::with_capacity(page.fragments.len());
        for fragment in page.fragments {
            let baseline = page_height - to_physical(anchor_y, fragment.y, output);
            let x = to_physical(anchor_x, fragment.x, output);

            let font_size = output.get() * fragment.font_size.get();
            items.push(DisplayItem::Text {
                text: fragment.text,
                x: Length::try_new(x, "mapping x to physical space")?,
                y: Length::try_new(baseline, "flipping to PDF coordinates")?,
                font_size: FontSize::try_new(font_size)?,
                face: fragment.face,
                positioned: fragment.positioned.map(|run| run.scaled(output.get())),

                colour: fragment.colour,

                clip: match fragment.clip {
                    None => None,
                    Some(band) => {
                        let top = to_physical(anchor_y, band.top, output);
                        let height = scale_extent(band.height, output);
                        Some(crate::contracts::PdfBand {
                            y: Length::try_new(
                                page_height - top - height,
                                "flipping a text clip to PDF coordinates",
                            )?,
                            height: Length::try_new(height, "scaling a text clip")?,
                        })
                    }
                },
                continued: fragment.continued,
            });
        }

        let mut ordered = Vec::with_capacity(
            page.bands.len() + page.decorations.len() + page.images.len() + items.len(),
        );

        if let Some(colour) = canvas {
            ordered.push(DisplayItem::Rect {
                x: Length::try_new(0.0, "the canvas's x")?,
                y: Length::try_new(0.0, "the canvas's y")?,
                width: page_width,
                height: Length::try_new(page_height, "the canvas's height")?,
                colour,
            });
        }
        for rect in page.bands.into_iter().chain(page.decorations) {
            let top = to_physical(anchor_y, rect.y, output);
            let height = scale_extent(rect.height, output);
            let bottom = page_height - (top + height);
            ordered.push(DisplayItem::Rect {
                x: Length::try_new(
                    to_physical(anchor_x, rect.x, output),
                    "mapping a rect's x to physical space",
                )?,
                y: Length::try_new(bottom, "flipping a rect to PDF coordinates")?,
                width: Length::try_new(scale_extent(rect.width, output), "scaling a rect's width")?,
                height: Length::try_new(height, "scaling a rect's height")?,
                colour: rect.colour,
            });
        }

        for picture in page.images {
            let top = to_physical(anchor_y, picture.y, output);
            let height = scale_extent(picture.height, output);
            let bottom = page_height - (top + height);
            ordered.push(DisplayItem::Image {
                asset: picture.asset,
                x: Length::try_new(
                    to_physical(anchor_x, picture.x, output),
                    "mapping an image's x to physical space",
                )?,
                y: Length::try_new(bottom, "flipping an image to PDF coordinates")?,
                width: Length::try_new(
                    scale_extent(picture.width, output),
                    "scaling an image's width",
                )?,
                height: Length::try_new(height, "scaling an image's height")?,
            });
        }

        ordered.extend(items);
        flip_furniture(page.furniture, page_height, &mut ordered)?;
        display_pages.push(DisplayPage {
            items: ordered,
            links,
        });
    }

    Ok(DisplayList {
        pages: display_pages,
        page_size,
    })
}

fn map_links(
    links: Vec<crate::contracts::PlacedLink>,
    page: PageSize,
    anchor_x: Length,
    anchor_y: Length,
    output: Scale,
) -> Result<Vec<crate::contracts::PdfLink>, Error> {
    let mut mapped = Vec::new();
    for link in links {
        let left = to_physical(anchor_x, link.x, output);
        let top = to_physical(anchor_y, link.y, output);
        let width = scale_extent(link.width, output);
        let height = scale_extent(link.height, output);
        let right = (left + width).min(page.width().get());
        let left = left.max(0.0);
        let physical_top = page.height().get() - top;
        let upper = physical_top.min(page.height().get());
        let lower = (physical_top - height).max(0.0);
        if right > left && upper > lower {
            mapped.push(crate::contracts::PdfLink {
                target: link.target,
                x: Length::try_new(left, "clipping a link's physical left edge")?,
                y: Length::try_new(lower, "clipping a link's physical bottom edge")?,
                width: Length::try_new(right - left, "clipping a link's physical width")?,
                height: Length::try_new(upper - lower, "clipping a link's physical height")?,
            });
        }
    }
    Ok(mapped)
}

fn flip_furniture(
    furniture: Vec<crate::contracts::PlacedFurniture>,
    page_height: f32,
    ordered: &mut Vec<DisplayItem>,
) -> Result<(), Error> {
    for piece in furniture {
        match piece {
            crate::contracts::PlacedFurniture::Text {
                text,
                x,
                baseline,
                font_size,
                face,
                positioned,
                colour,
            } => ordered.push(DisplayItem::Text {
                text,
                x,

                y: Length::try_new(
                    page_height - baseline.get(),
                    "flipping furniture text to PDF coordinates",
                )?,
                font_size,
                face,
                positioned,
                colour,

                clip: None,
                continued: false,
            }),
            crate::contracts::PlacedFurniture::Rule {
                x,
                y,
                width,
                height,
                colour,
            } => ordered.push(DisplayItem::Rect {
                x,

                y: Length::try_new(
                    page_height - (y.get() + height.get()),
                    "flipping a furniture rule to PDF coordinates",
                )?,
                width,
                height,
                colour,
            }),
        }
    }

    Ok(())
}
