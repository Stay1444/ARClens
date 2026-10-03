//! Debug helper: OCR every footer cell of every tooltip.
use arclens_vision::{NameReader, PanelParams, find_panels, footer, footer_cells};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let model = PathBuf::from(std::env::var("ARCLENS_OCR_MODEL")?);
    let reader = NameReader::from_model_file(&model)?;
    for path in std::env::args().skip(1) {
        let frame = image::open(&path)?.into_rgb8();
        for panel in find_panels(&frame, &PanelParams::default()) {
            let Some(foot) = footer(&frame, panel) else {
                continue;
            };
            let cells: Vec<_> = footer_cells(&frame, foot)
                .into_iter()
                .map(|c| (c.height, reader.read_text(&frame, c).ok().flatten()))
                .collect();
            println!("{path}: {cells:?}");
        }
    }
    Ok(())
}
