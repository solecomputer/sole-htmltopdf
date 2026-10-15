use crate::contracts::{Length, Scale};

pub(crate) fn to_physical(anchor: Length, layout: Length, output: Scale) -> f32 {
    if output == Scale::ONE {
        return layout.get();
    }
    anchor.get() + output.get() * (layout.get() - anchor.get())
}

pub(crate) fn scale_extent(extent: Length, output: Scale) -> f32 {
    output.get() * extent.get()
}
