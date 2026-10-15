use crate::contracts::{Error, RenderMode, RenderOptions, Scale, SmartShrinking};

const SMART_SHRINK_SCALE: f32 = 0.800_223;

#[must_use = "the contributed factor is the whole answer; dropping it silently renders unscaled"]
pub(crate) fn shrink_factor(options: &RenderOptions) -> Result<Option<Scale>, Error> {
    let shrink = match &options.mode {
        RenderMode::Modern => SmartShrinking::Off,
        RenderMode::Wkhtmltopdf0126(bundle) => bundle.smart_shrinking(),
    };

    match shrink {
        SmartShrinking::Off => Ok(None),
        SmartShrinking::On => Ok(Some(Scale::try_new(SMART_SHRINK_SCALE)?)),
    }
}
