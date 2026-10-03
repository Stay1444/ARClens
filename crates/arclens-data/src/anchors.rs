//! Placing markers on the in-game map: labels read off the screen are
//! matched to the source's named markers, and the matches fix the map's
//! pan and zoom.

use arclens_core::{Marker, PointPair, Transform};

/// Minimum similarity (normalised Levenshtein) for a label match: OCR
/// slips a letter or two ("Rubv Residence"), names are long and distinct.
const MIN_SIMILARITY: f64 = 0.8;
/// How far (fraction of the screen) a match may sit from the fitted
/// transform and still count as agreeing.
const TOLERANCE: f32 = 0.012;

/// A label read from the screen: its text and centre, normalised to the
/// screen (`0..=1`).
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenLabel {
    pub text: String,
    pub center: (f32, f32),
}

/// The map → normalised-screen transform for this view, and how many labels
/// agreed with it. `None` when fewer than two labels match consistently.
pub fn locate_view(labels: &[ScreenLabel], markers: &[Marker]) -> Option<(Transform, usize)> {
    let pairs = match_labels(labels, markers);
    Transform::fit_uniform_robust(&pairs, TOLERANCE, 2)
}

/// `(marker position, screen centre)` for each label that names a marker.
/// A name shared by several markers is ambiguous and skipped.
pub fn match_labels(labels: &[ScreenLabel], markers: &[Marker]) -> Vec<PointPair> {
    let named: Vec<(String, &Marker)> = markers
        .iter()
        .filter_map(|m| m.label.as_deref().map(|l| (key(l), m)))
        .filter(|(k, _)| !k.is_empty())
        .collect();
    labels
        .iter()
        .filter_map(|label| {
            let wanted = key(&label.text);
            if wanted.len() < 3 {
                return None; // timers, stray letters
            }
            let mut best: Option<(f64, &Marker)> = None;
            let mut ambiguous = false;
            for (name, marker) in &named {
                let score = strsim::normalized_levenshtein(&wanted, name);
                if score < MIN_SIMILARITY {
                    continue;
                }
                match best {
                    Some((b, m)) if (score - b).abs() < 1e-9 && m.position != marker.position => {
                        ambiguous = true;
                    }
                    Some((b, _)) if score <= b => {}
                    _ => {
                        best = Some((score, marker));
                        ambiguous = false;
                    }
                }
            }
            let (_, marker) = best.filter(|_| !ambiguous)?;
            Some(((marker.position.x, marker.position.y), label.center))
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
    use arclens_core::{MapId, MapPoint};

    fn place(name: &str, x: f32, y: f32) -> Marker {
        Marker {
            id: name.into(),
            map: MapId::new("dam"),
            category: "labels".into(),
            subcategory: None,
            position: MapPoint::new(x, y),
            label: Some(name.into()),
            locked: false,
        }
    }

    fn places() -> Vec<Marker> {
        vec![
            place("Pattern House", 1000.0, -3000.0),
            place("Generator Hall", 1100.0, -2800.0),
            place("Pipeline Tower", 1300.0, -2400.0),
            place("The Breach", 900.0, -2200.0),
            place("Ruby Residence", 400.0, -2500.0),
        ]
    }

    #[test]
    fn locates_the_view_from_read_labels() {
        let truth = Transform::scale_translate(0.0004, 0.0004, -0.1, 1.4);
        let at = |x: f32, y: f32| truth.apply((x, y));
        let labels = vec![
            ScreenLabel {
                text: "Pattern House ".into(),
                center: at(1000.0, -3000.0),
            },
            ScreenLabel {
                text: "Generator Hal1".into(),
                center: at(1100.0, -2800.0),
            },
            ScreenLabel {
                text: "Pipeline Tower".into(),
                center: at(1300.0, -2400.0),
            },
            // Noise: a timer and a cut-off label.
            ScreenLabel {
                text: "13:57".into(),
                center: (0.5, 0.5),
            },
            ScreenLabel {
                text: "ITower".into(),
                center: (0.3, 0.9),
            },
        ];
        let (t, agree) = locate_view(&labels, &places()).unwrap();
        assert_eq!(agree, 3);
        let (x, y) = t.apply((900.0, -2200.0));
        let (ex, ey) = at(900.0, -2200.0);
        assert!((x - ex).abs() < 1e-3 && (y - ey).abs() < 1e-3);
    }

    #[test]
    fn needs_two_consistent_matches() {
        let labels = vec![ScreenLabel {
            text: "The Breach".into(),
            center: (0.4, 0.4),
        }];
        assert!(locate_view(&labels, &places()).is_none());
    }

    #[test]
    fn skips_names_that_appear_twice() {
        let mut markers = places();
        markers.push(place("The Breach", 50.0, -50.0));
        let labels = vec![ScreenLabel {
            text: "The Breach".into(),
            center: (0.4, 0.4),
        }];
        assert!(match_labels(&labels, &markers).is_empty());
    }
}
