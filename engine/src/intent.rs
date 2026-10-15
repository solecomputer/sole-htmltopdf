use crate::contracts::{Edges, Error, PageSize, RenderOptions, RenderingIntent, Scale};

#[must_use = "the resolved intent is the geometry the pipeline executes"]
pub(crate) fn resolve(
    options: &RenderOptions,
    physical_page: PageSize,
    margin: Edges,
) -> Result<RenderingIntent, Error> {
    let physical_usable_height = crate::contracts::UsableHeight::new(physical_page, margin)?;
    let physical_usable_width = crate::contracts::UsableWidth::new(physical_page, margin)?;

    let contributed = crate::emulation::shrink_factor(options)?;
    let factor = match contributed {
        Some(shrink) => options.zoom.get() * shrink.get(),
        None => options.zoom.get(),
    };

    #[allow(clippy::float_cmp)]
    let is_identity = factor == 1.0;
    if is_identity {
        return Ok(RenderingIntent {
            layout_page: physical_page,
            layout_margin: margin,
            physical_page,
            output: Scale::ONE,
            usable_height: physical_usable_height,
            usable_width: physical_usable_width,
        });
    }

    enlarge(physical_page, margin, Scale::try_new(factor)?)
}

fn enlarge(
    physical_page: PageSize,
    margin: Edges,
    output: Scale,
) -> Result<RenderingIntent, Error> {
    let scale = output.get();

    let horizontal = margin.left.get() + margin.right.get();
    let vertical = margin.top.get() + margin.bottom.get();

    let layout_width = horizontal + (physical_page.width().get() - horizontal) / scale;
    let layout_height = vertical + (physical_page.height().get() - vertical) / scale;

    let layout_page = PageSize::try_new(layout_width, layout_height)?;

    Ok(RenderingIntent {
        layout_page,
        layout_margin: margin,
        physical_page,
        output,

        usable_height: crate::contracts::UsableHeight::new(layout_page, margin)?,

        usable_width: crate::contracts::UsableWidth::new(layout_page, margin)?,
    })
}
