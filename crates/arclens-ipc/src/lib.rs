//! IPC between the companion app (`arclens`) and the overlay
//! (`arclens-overlay`).
//!
//! Transport: a Unix domain socket at `$XDG_RUNTIME_DIR/arclens.sock`,
//! owned by the companion app. Framing: one JSON object per line
//! (newline-delimited JSON), so messages are easy to inspect with `socat`.
//!
//! Versioning: every connection starts with a [`Hello`] from each side. Bump
//! [`PROTOCOL_VERSION`] on any breaking change to [`ToOverlay`] /
//! [`ToApp`].

use arclens_core::{Advice, Item, MapId, Marker, Transform};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

pub const PROTOCOL_VERSION: u32 = 4;

/// Default socket path: `$XDG_RUNTIME_DIR/arclens.sock`, falling back to the
/// temp dir when the variable is unset (non-systemd systems).
pub fn socket_path() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map_or_else(std::env::temp_dir, PathBuf::from)
        .join("arclens.sock")
}

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
    },
    /// The in-game tooltip is gone.
    ClearHover,
    /// Draw these markers using `transform` (map space → screen pixels).
    ShowMarkers {
        map: MapId,
        markers: Vec<Marker>,
        transform: Transform,
    },
    ClearMarkers,
    /// The in-game map is open: show the panel with its conditions and the
    /// marker filter (clickable; the rest of the overlay stays
    /// click-through).
    ShowMapPanel {
        panel: MapPanel,
    },
    /// The in-game map closed.
    HideMapPanel,
}

/// What the overlay's map panel shows. The app owns the filter; the
/// overlay sends toggles back as [`ToApp`] messages and gets a new panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapPanel {
    pub map_name: String,
    /// Conditions running now; `at_ms` is when each ends.
    #[serde(default)]
    pub active: Vec<PanelEvent>,
    /// Next conditions; `at_ms` is when each starts.
    #[serde(default)]
    pub upcoming: Vec<PanelEvent>,
    /// Marker kinds of this map, with counts and whether they are shown.
    #[serde(default)]
    pub categories: Vec<PanelCategory>,
    /// Credit for the data shown.
    #[serde(default)]
    pub attribution: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelEvent {
    pub name: String,
    /// Unix milliseconds.
    pub at_ms: i64,
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
    /// The user typed into the overlay's quick-search box.
    Search {
        query: String,
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

/// Reading half of a newline-delimited JSON connection.
#[derive(Debug)]
pub struct Receiver {
    lines: tokio::io::Lines<BufReader<OwnedReadHalf>>,
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
#[derive(Debug)]
pub struct Sender {
    write: OwnedWriteHalf,
}

impl Sender {
    pub async fn send<T: Serialize>(&mut self, msg: &T) -> Result<(), Error> {
        let mut buf = serde_json::to_vec(msg)?;
        buf.push(b'\n');
        self.write.write_all(&buf).await?;
        Ok(())
    }
}

/// Splits a connected stream into typed halves.
pub fn split(stream: UnixStream) -> (Receiver, Sender) {
    let (read, write) = stream.into_split();
    (
        Receiver {
            lines: BufReader::new(read).lines(),
        },
        Sender { write },
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
    use tokio::net::UnixListener;

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

    #[tokio::test]
    async fn round_trips_messages_over_a_socket() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();

        let client = tokio::spawn({
            let path = path.clone();
            async move {
                let (mut rx, mut tx) = split(UnixStream::connect(path).await.unwrap());
                tx.send(&ToApp::Search {
                    query: "gear".into(),
                })
                .await
                .unwrap();
                rx.recv::<ToOverlay>().await.unwrap()
            }
        });

        let (stream, _) = listener.accept().await.unwrap();
        let (mut rx, mut tx) = split(stream);
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
