//! IPC between the companion app (`arclens`) and the overlay
//! (`arclens-overlay`).
//!
//! Transport: a Unix domain socket on Linux, a named pipe on Windows
//! ([`Endpoint`]), owned by the companion app. Framing: one JSON object per
//! line (newline-delimited JSON), so messages are easy to inspect with
//! `socat`.
//!
//! Versioning: every connection starts with a [`Hello`] from each side. Bump
//! [`PROTOCOL_VERSION`] on any breaking change to [`ToOverlay`] /
//! [`ToApp`].

use arclens_core::{Advice, Item, ItemId, MapId, Marker, Rarity, Transform};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

mod transport;
pub use transport::{Endpoint, Listener};

pub const PROTOCOL_VERSION: u32 = 14;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
}

/// Messages from the companion app to the overlay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToOverlay {
    Hello(Hello),
    /// Show or hide the overlay surface entirely. Hidden means *unmapped*,
    /// so the compositor can go back to direct scanout.
    SetVisible {
        visible: bool,
    },
    /// Switch between click-through (`false`) and interactive (`true`).
    SetInteractive {
        interactive: bool,
    },
    /// Show an item card (quick lookup or detected hovered item).
    ShowItem {
        item: Box<Item>,
        advice: Advice,
        /// Local path of the item's icon, if cached. Optional, so older
        /// peers keep working without a protocol bump.
        #[serde(default)]
        icon: Option<PathBuf>,
        /// Display names aligned with `item.recycles_into` (the overlay has
        /// no catalog to resolve ids).
        #[serde(default)]
        recycle_names: Vec<String>,
    },
    /// The item whose in-game tooltip is under the cursor (detected from the
    /// screen). Drawn next to `anchor` regardless of the manual visibility
    /// toggle, until [`ToOverlay::ClearHover`].
    ShowHover {
        item: Box<Item>,
        advice: Advice,
        #[serde(default)]
        icon: Option<PathBuf>,
        #[serde(default)]
        recycle_names: Vec<String>,
        /// The game's tooltip, normalised to the screen (`0..=1`).
        anchor: NormRect,
        /// Which side of the tooltip the hovered item is on: the card goes
        /// on the other side so it doesn't cover it.
        #[serde(default)]
        item_side: ItemSide,
        /// Other game panels the card shouldn't cover (normalised), e.g.
        /// the trader's purchase panel.
        #[serde(default)]
        avoid: Vec<NormRect>,
    },
    /// The in-game tooltip is gone.
    ClearHover,
    /// Draw these markers using `transform`: map space → screen normalised
    /// to `0..=1` (the overlay scales it to its own size).
    ShowMarkers {
        map: MapId,
        markers: Vec<Marker>,
        transform: Transform,
        /// Dense groups drawn as one shaded area each (their members are
        /// not in `markers`).
        #[serde(default)]
        areas: Vec<arclens_core::MarkerArea>,
        /// Draw only inside this part of the screen (the map's viewport).
        #[serde(default)]
        clip: Option<NormRect>,
    },
    /// The map panned or zoomed: same markers, new transform (sent at
    /// capture rate while the map moves).
    MoveMarkers {
        transform: Transform,
        #[serde(default)]
        clip: Option<NormRect>,
    },
    ClearMarkers,
    /// Where the pointer is over the map (normalised to the screen;
    /// `None`: unknown), for marker tooltips. The overlay can't see the
    /// pointer itself: it is click-through there.
    Pointer {
        at: Option<(f32, f32)>,
    },
    /// The in-game map is open: show the marker filter panel (clickable;
    /// the rest of the overlay stays click-through).
    ShowMapPanel {
        panel: MapPanel,
    },
    /// The in-game map closed.
    HideMapPanel,
    /// The game's main menu is on screen: show the general card there.
    ShowMenuCard {
        card: MenuCard,
    },
    HideMenuCard,
    /// The user's overlay settings; sent on connect and on every change.
    Configure {
        settings: OverlaySettings,
    },
    /// Items matching the overlay's quick search for `query`, best first.
    /// The overlay drops results for a query it no longer shows.
    SearchResults {
        query: String,
        hits: Vec<SearchHit>,
    },
}

/// How the overlay looks, as the user set it in the app.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OverlaySettings {
    /// Size of everything the overlay draws (1.0: as designed).
    #[serde(default = "OverlaySettings::default_scale")]
    pub scale: f32,
    /// Where the pinned item card sits.
    #[serde(default)]
    pub corner: Corner,
    /// How opaque card and panel backgrounds are (0.5–1).
    #[serde(default = "OverlaySettings::default_opacity")]
    pub opacity: f32,
}

impl OverlaySettings {
    /// Scales the app offers.
    pub const SCALES: [f32; 5] = [0.8, 0.9, 1.0, 1.25, 1.5];

    /// Opacities the app offers.
    pub const OPACITIES: [f32; 4] = [0.6, 0.75, 0.9, 1.0];

    const fn default_scale() -> f32 {
        1.0
    }

    const fn default_opacity() -> f32 {
        1.0
    }

    /// `scale` limited to what the overlay supports.
    pub fn clamped_scale(&self) -> f32 {
        if self.scale.is_finite() {
            self.scale.clamp(0.5, 2.0)
        } else {
            1.0
        }
    }
}

impl Default for OverlaySettings {
    fn default() -> Self {
        Self {
            scale: Self::default_scale(),
            corner: Corner::default(),
            opacity: Self::default_opacity(),
        }
    }
}

/// A screen corner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomLeft,
        Self::BottomRight,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::TopLeft => "Top left",
            Self::TopRight => "Top right",
            Self::BottomLeft => "Bottom left",
            Self::BottomRight => "Bottom right",
        }
    }

    pub const fn is_left(self) -> bool {
        matches!(self, Self::TopLeft | Self::BottomLeft)
    }

    pub const fn is_top(self) -> bool {
        matches!(self, Self::TopLeft | Self::TopRight)
    }
}

/// One quick-search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: ItemId,
    pub name: String,
    #[serde(default)]
    pub rarity: Option<Rarity>,
    /// Local path of the item's icon, if cached.
    #[serde(default)]
    pub icon: Option<PathBuf>,
}

/// What the overlay's map panel shows: the map's marker kinds. The app
/// owns the filter; the overlay sends toggles back as [`ToApp`] messages
/// and gets a new panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPanel {
    pub map_name: String,
    /// Marker kinds of this map, with counts and whether they are shown.
    #[serde(default)]
    pub categories: Vec<PanelCategory>,
    /// The map's condition as read on screen ("Hurricane"), if known.
    /// Markers of other conditions are left out of `categories`.
    #[serde(default)]
    pub condition: Option<String>,
    /// Presets suited to this map and condition, best first.
    #[serde(default)]
    pub presets: Vec<PanelPreset>,
    /// The preset applied last, and whether toggles changed it since.
    #[serde(default)]
    pub active_preset: Option<String>,
    #[serde(default)]
    pub edited: bool,
}

/// General information for the main menu.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct MenuCard {
    /// Map conditions running now, ending soonest first.
    #[serde(default)]
    pub active: Vec<CardEvent>,
    /// Starting next, soonest first.
    #[serde(default)]
    pub upcoming: Vec<CardEvent>,
    /// Workshop levels built and in total, when the player set them.
    #[serde(default)]
    pub workshop: Option<(u32, u32)>,
    /// The player's progress, one line per part of the game.
    #[serde(default)]
    pub progress: Vec<ProgressLine>,
}

/// "Quests: 12 of 100", for the menu card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressLine {
    pub label: String,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardEvent {
    pub name: String,
    pub map: String,
    /// Unix ms: when it ends (running) or starts (upcoming). The overlay
    /// counts down on its own clock.
    pub at_ms: i64,
    /// Local path of the condition's icon, if cached.
    #[serde(default)]
    pub icon: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelPreset {
    /// Sent back in [`ToApp::ApplyPreset`].
    pub id: String,
    pub name: String,
    /// Made for this condition (rather than general).
    #[serde(default)]
    pub for_condition: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelCategory {
    /// Source id, sent back in toggles.
    pub id: String,
    pub label: String,
    pub count: usize,
    pub shown: bool,
    #[serde(default)]
    pub subcategories: Vec<PanelCategory>,
}

/// Left or right of the game's tooltip.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemSide {
    #[default]
    Left,
    Right,
}

/// A rectangle normalised to the screen: `0..=1` on both axes, origin
/// top-left. Resolution- and scale-factor-independent.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A monitor in the compositor's logical coordinate space, as reported by
/// the screen-cast portal. Passed to the overlay as `--monitor x,y,w,h` so it
/// opens on the monitor that is being captured (where the game is).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl MonitorRect {
    /// Command-line flag carrying a [`MonitorRect`].
    pub const FLAG: &'static str = "--monitor";

    pub fn to_arg(self) -> String {
        format!("{},{},{},{}", self.x, self.y, self.width, self.height)
    }

    pub fn from_arg(arg: &str) -> Option<Self> {
        let mut parts = arg.split(',').map(|p| p.trim().parse::<i32>());
        let rect = Self {
            x: parts.next()?.ok()?,
            y: parts.next()?.ok()?,
            width: parts.next()?.ok()?,
            height: parts.next()?.ok()?,
        };
        parts.next().is_none().then_some(rect)
    }
}

/// Messages from the overlay to the companion app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToApp {
    Hello(Hello),
    /// The user typed into the overlay's quick-search box; the app answers
    /// with [`ToOverlay::SearchResults`].
    Search {
        query: String,
    },
    /// The user picked a quick-search result: show its card.
    PickItem {
        id: ItemId,
    },
    /// Map panel: show or hide a marker category…
    ToggleMarkerCategory {
        category: String,
    },
    /// …or one subcategory of it.
    ToggleMarkerSubcategory {
        category: String,
        subcategory: String,
    },
    ShowAllMarkers,
    HideAllMarkers,
    /// The player picked a map preset in the panel.
    ApplyPreset {
        id: String,
    },
    /// The overlay is about to exit.
    Bye,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed message: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("peer speaks protocol {peer}, we speak {PROTOCOL_VERSION}")]
    VersionMismatch { peer: u32 },
}

type ReadHalf = Box<dyn AsyncRead + Send + Unpin>;
type WriteHalf = Box<dyn AsyncWrite + Send + Unpin>;

/// Reading half of a newline-delimited JSON connection.
pub struct Receiver {
    lines: tokio::io::Lines<BufReader<ReadHalf>>,
}

impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

impl Receiver {
    /// Next message, or `Ok(None)` when the peer closed the connection.
    pub async fn recv<T: for<'de> Deserialize<'de>>(&mut self) -> Result<Option<T>, Error> {
        match self.lines.next_line().await? {
            Some(line) => Ok(Some(serde_json::from_str(&line)?)),
            None => Ok(None),
        }
    }
}

/// Writing half of a newline-delimited JSON connection.
pub struct Sender {
    write: WriteHalf,
}

impl std::fmt::Debug for Sender {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

impl Sender {
    pub async fn send<T: Serialize>(&mut self, msg: &T) -> Result<(), Error> {
        let mut buf = serde_json::to_vec(msg)?;
        buf.push(b'\n');
        self.write.write_all(&buf).await?;
        Ok(())
    }
}

/// Wraps a connection's byte halves into typed halves.
fn split(
    read: impl AsyncRead + Send + Unpin + 'static,
    write: impl AsyncWrite + Send + Unpin + 'static,
) -> (Receiver, Sender) {
    (
        Receiver {
            lines: BufReader::new(Box::new(read) as ReadHalf).lines(),
        },
        Sender {
            write: Box::new(write),
        },
    )
}

/// Checks a peer's [`Hello`].
pub fn check_version(hello: &Hello) -> Result<(), Error> {
    if hello.protocol == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(Error::VersionMismatch {
            peer: hello.protocol,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_settings_default_missing_fields_and_clamp_scale() {
        let settings: OverlaySettings = serde_json::from_str("{}").unwrap();
        assert_eq!(settings, OverlaySettings::default());
        let settings: OverlaySettings =
            serde_json::from_str(r#"{"scale":9.0,"corner":"bottom_left"}"#).unwrap();
        assert!((settings.clamped_scale() - 2.0).abs() < f32::EPSILON);
        assert_eq!(settings.corner, Corner::BottomLeft);
    }

    /// `scripts/overlay-demo.jsonl` must keep matching the protocol.
    #[test]
    fn demo_script_parses() {
        let script = include_str!("../../../scripts/overlay-demo.jsonl");
        for line in script.lines().filter(|l| !l.trim().is_empty()) {
            let msg: ToOverlay = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{e}: {}", &line[..line.len().min(80)]));
            if let ToOverlay::Hello(hello) = msg {
                assert_eq!(hello.protocol, PROTOCOL_VERSION);
            }
        }
    }

    /// A private endpoint for one test.
    fn test_endpoint(dir: &tempfile::TempDir) -> Endpoint {
        if cfg!(windows) {
            let unique = dir.path().file_name().unwrap().to_string_lossy();
            Endpoint::at(format!(r"\\.\pipe\arclens-test-{unique}"))
        } else {
            Endpoint::at(dir.path().join("test.sock"))
        }
    }

    #[tokio::test]
    async fn round_trips_messages_over_the_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let endpoint = test_endpoint(&dir);
        let mut listener = endpoint.bind().await.unwrap();

        let client = tokio::spawn({
            let endpoint = endpoint.clone();
            async move {
                let (mut rx, mut tx) = endpoint.connect().await.unwrap();
                tx.send(&ToApp::Search {
                    query: "gear".into(),
                })
                .await
                .unwrap();
                rx.recv::<ToOverlay>().await.unwrap()
            }
        });

        let (mut rx, mut tx) = listener.accept().await.unwrap();
        let msg: ToApp = rx.recv().await.unwrap().unwrap();
        assert_eq!(
            msg,
            ToApp::Search {
                query: "gear".into()
            }
        );
        tx.send(&ToOverlay::SetVisible { visible: true })
            .await
            .unwrap();

        assert_eq!(
            client.await.unwrap(),
            Some(ToOverlay::SetVisible { visible: true })
        );
        // Only one instance listens at a time.
        assert_eq!(
            endpoint.bind().await.unwrap_err().kind(),
            std::io::ErrorKind::AddrInUse
        );
    }

    #[test]
    fn wire_format_is_tagged_json() {
        let json = serde_json::to_string(&ToOverlay::SetInteractive { interactive: true }).unwrap();
        assert_eq!(json, r#"{"type":"set_interactive","interactive":true}"#);
    }

    #[test]
    fn monitor_rect_round_trips_through_its_flag() {
        let rect = MonitorRect {
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(MonitorRect::from_arg(&rect.to_arg()), Some(rect));
        assert_eq!(MonitorRect::from_arg("1,2,3"), None);
        assert_eq!(MonitorRect::from_arg("1,2,3,4,5"), None);
        assert_eq!(MonitorRect::from_arg("a,2,3,4"), None);
    }

    #[test]
    fn rejects_other_protocol_versions() {
        assert!(
            check_version(&Hello {
                protocol: PROTOCOL_VERSION
            })
            .is_ok()
        );
        assert!(matches!(
            check_version(&Hello { protocol: 999 }),
            Err(Error::VersionMismatch { peer: 999 })
        ));
    }
}
