//! Placing markers on the in-game map: labels read off the screen are
//! matched to the source's named markers, and the matches fix the map's
//! pan and zoom.

use crate::labels::MapLabel;
use arclens_core::{PointPair, Transform};

/// Minimum similarity (normalised Levenshtein) for a label match: OCR
/// slips a letter or two ("Rubv Residence"), names are long and distinct.
const MIN_SIMILARITY: f64 = 0.8;
/// How far (fraction of the frame width) a match may sit from the fitted
/// transform and still count as agreeing. The source's label positions are
/// hand-placed: on Dam they sit 2–50 px (at 2560 px wide) from where the
/// game draws the text, typically ~20 px.
const TOLERANCE: f32 = 0.016;

/// A label read from the screen: its text and box in frame pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLabel {
    pub text: String,
    /// `(x, y, width, height)` of the text.
    pub rect: (f32, f32, f32, f32),
}

impl ScreenLabel {
    /// The point of the text that a source label's position marks.
    fn anchor(&self, centered: bool) -> (f32, f32) {
        let (x, y, w, h) = self.rect;
        if centered {
            (x + w / 2.0, y + h / 2.0)
        } else {
            (x, y)
        }
    }
}

/// The map → normalised-screen (`0..=1`) transform for this view, and how
/// many labels agreed with it. `frame` is the frame size in pixels: the fit
/// runs in pixels, where the view's scale is the same on both axes.
/// `None` when fewer than two labels match consistently.
pub fn locate_view(
    labels: &[ScreenLabel],
    frame: (f32, f32),
    known: &[MapLabel],
) -> Option<(Transform, usize)> {
    let pairs = match_labels(labels, known);
    let tolerance = TOLERANCE * frame.0;
    let (mut fit, mut agree) = Transform::fit_uniform_robust(&pairs, tolerance, 2)?;
    // Refine: refit on the agreeing pairs until the set settles.
    for _ in 0..3 {
        let inliers: Vec<PointPair> = pairs
            .iter()
            .copied()
            .filter(|(from, to)| {
                let (x, y) = fit.apply(*from);
                (x - to.0).hypot(y - to.1) <= tolerance
            })
            .collect();
        let Some(refit) = Transform::fit_uniform(&inliers) else {
            break;
        };
        let settled = inliers.len() == agree;
        (fit, agree) = (refit, inliers.len());
        if settled {
            break;
        }
    }
    (agree >= 2).then(|| {
        (
            Transform::scale_translate(
                fit.a / frame.0,
                fit.d / frame.1,
                fit.tx / frame.0,
                fit.ty / frame.1,
            ),
            agree,
        )
    })
}

/// `(source position, screen point)` for each label read that names a
/// known label. A name known twice is ambiguous and skipped.
pub fn match_labels(labels: &[ScreenLabel], known: &[MapLabel]) -> Vec<PointPair> {
    let named: Vec<(String, &MapLabel)> = known
        .iter()
        .map(|l| (key(&l.text), l))
        .filter(|(k, _)| !k.is_empty())
        .collect();
    labels
        .iter()
        .filter_map(|label| {
            let wanted = key(&label.text);
            if wanted.len() < 3 {
                return None; // timers, stray letters
            }
            let mut best: Option<(f64, &MapLabel)> = None;
            let mut ambiguous = false;
            for (name, candidate) in &named {
                let score = strsim::normalized_levenshtein(&wanted, name);
                if score < MIN_SIMILARITY {
                    continue;
                }
                match best {
                    Some((b, l))
                        if (score - b).abs() < 1e-9 && l.position != candidate.position =>
                    {
                        ambiguous = true;
                    }
                    Some((b, _)) if score <= b => {}
                    _ => {
                        best = Some((score, candidate));
                        ambiguous = false;
                    }
                }
            }
            let (_, known) = best.filter(|_| !ambiguous)?;
            Some((
                (known.position.x, known.position.y),
                label.anchor(known.centered),
            ))
        })
        .collect()
}

/// Case- and punctuation-insensitive form of a label.
fn key(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arclens_core::MapPoint;

    fn place(name: &str, x: f32, y: f32) -> MapLabel {
        MapLabel {
            text: name.into(),
            position: MapPoint::new(x, y),
            centered: false,
        }
    }

    fn places() -> Vec<MapLabel> {
        vec![
            place("Pattern House", 1000.0, 3000.0),
            place("Generator Hall", 1100.0, 2800.0),
            place("Pipeline Tower", 1300.0, 2400.0),
            place("The Breach", 900.0, 2200.0),
            place("Ruby Residence", 400.0, 2500.0),
        ]
    }

    /// A text box whose top-left corner is at `corner`.
    fn label(text: &str, corner: (f32, f32)) -> ScreenLabel {
        ScreenLabel {
            text: text.into(),
            rect: (corner.0, corner.1, 150.0, 20.0),
        }
    }

    const FRAME: (f32, f32) = (2560.0, 1440.0);

    #[test]
    fn locates_the_view_from_read_labels() {
        // Map units -> frame pixels: one scale on both axes.
        let truth = Transform::scale_translate(0.6, 0.6, -100.0, -1000.0);
        let at = |x: f32, y: f32| truth.apply((x, y));
        let labels = vec![
            label("Pattern House ", at(1000.0, 3000.0)),
            label("Generator Hal1", at(1100.0, 2800.0)),
            label("Pipeline Tower", at(1300.0, 2400.0)),
            // Noise: a timer and a cut-off label.
            label("13:57", (1280.0, 700.0)),
            label("ITower", (700.0, 1300.0)),
        ];
        let (t, agree) = locate_view(&labels, FRAME, &places()).unwrap();
        assert_eq!(agree, 3);
        // The result maps to the screen normalised per axis.
        let (x, y) = t.apply((900.0, 2200.0));
        let (ex, ey) = at(900.0, 2200.0);
        assert!((x - ex / FRAME.0).abs() < 1e-4, "{x} vs {}", ex / FRAME.0);
        assert!((y - ey / FRAME.1).abs() < 1e-4, "{y} vs {}", ey / FRAME.1);
    }

    #[test]
    fn centred_labels_use_the_box_centre() {
        let known = [MapLabel {
            centered: true,
            ..place("The Breach", 900.0, 2200.0)
        }];
        let pairs = match_labels(&[label("The Breach", (100.0, 200.0))], &known);
        assert_eq!(pairs, vec![((900.0, 2200.0), (175.0, 210.0))]);
    }

    #[test]
    fn needs_two_consistent_matches() {
        let labels = vec![label("The Breach", (400.0, 400.0))];
        assert!(locate_view(&labels, FRAME, &places()).is_none());
    }

    #[test]
    fn skips_names_that_appear_twice() {
        let mut known = places();
        known.push(place("The Breach", 50.0, 50.0));
        let labels = vec![label("The Breach", (400.0, 400.0))];
        assert!(match_labels(&labels, &known).is_empty());
    }
}
