//! Debug helper: draw detected panels (red) and name lines (green).
//!
//! `cargo run -p arclens-vision --example annotate -- in.jpg out.png`

use arclens_vision::{PanelParams, Rect, find_panels, name_line};
use image::{Rgb, RgbImage};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: annotate <in> <out>");
        std::process::exit(2);
    };
    let Ok(img) = image::open(&input) else {
        eprintln!("cannot read {input}");
        std::process::exit(1);
    };
    let mut frame = img.into_rgb8();
    let started = std::time::Instant::now();
    let panels = find_panels(&frame, &PanelParams::default());
    let names: Vec<_> = panels.iter().map(|p| name_line(&frame, *p)).collect();
    eprintln!("detect: {:.2?}", started.elapsed());
    drop(names);
    for panel in &panels {
        let name = name_line(&frame, *panel);
        println!("panel {panel:?} name {name:?}");
        outline(&mut frame, *panel, Rgb([255, 0, 0]));
        if let Some(name) = name {
            outline(&mut frame, name, Rgb([0, 200, 0]));
        }
    }
    if let Err(error) = frame.save(&output) {
        eprintln!("cannot write {output}: {error}");
        std::process::exit(1);
    }
}

fn outline(frame: &mut RgbImage, r: Rect, color: Rgb<u8>) {
    let (w, h) = frame.dimensions();
    for t in 0..3 {
        for x in r.x..r.right().min(w) {
            for y in [r.y + t, r.bottom().saturating_sub(1 + t)] {
                if y < h {
                    frame.put_pixel(x, y, color);
                }
            }
        }
        for y in r.y..r.bottom().min(h) {
            for x in [r.x + t, r.right().saturating_sub(1 + t)] {
                if x < w {
                    frame.put_pixel(x, y, color);
                }
            }
        }
    }
}
