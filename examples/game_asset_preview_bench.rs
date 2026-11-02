//! Measure CPU preview preparation separately from loading and GPU rendering.
use anyhow::{Context, Result};
use std::{path::Path, time::Instant};

fn main() -> Result<()> {
    let path = std::env::args().nth(1).context("expected asset path")?;
    let asset = gcad::asset::author::load(Path::new(&path), &[])?;
    let clip = (!asset.scene.clips.is_empty()).then_some(0);
    let playback = gcad::view::asset::Playback::new(&asset)?;
    for i in 0..3 {
        let start = Instant::now();
        let frame = playback.frame(clip, i as f64 * 0.1)?;
        println!("frame {i}: {:.2} ms, {} surfaces", start.elapsed().as_secs_f64() * 1000., frame.scene.surfaces.len());
        std::hint::black_box(frame);
    }
    Ok(())
}
