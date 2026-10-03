//! Picking the Wayland output (monitor) the overlay should live on.
//!
//! The companion app passes the captured monitor's logical rectangle
//! (`--monitor x,y,w,h`, from the screen-cast portal). Layer surfaces are
//! bound to outputs by *name*, so we list the outputs once and match by
//! logical position.

use arclens_ipc::MonitorRect;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::{delegate_output, delegate_registry, registry_handlers};
use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_output;
use wayland_client::{Connection, QueueHandle};

/// A connected output's name and logical rectangle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub name: String,
    pub rect: MonitorRect,
}

/// Name of the output matching `monitor`, if any.
pub fn output_for(monitor: MonitorRect) -> Option<String> {
    match list_outputs() {
        Ok(outputs) => {
            tracing::debug!(?outputs, "outputs");
            best_match(&outputs, monitor).map(|o| o.name.clone())
        }
        Err(error) => {
            tracing::warn!(%error, "could not list outputs");
            None
        }
    }
}

/// Exact logical-position match first; otherwise the output containing the
/// monitor's centre (portals may report slightly different geometry).
pub fn best_match(outputs: &[Output], monitor: MonitorRect) -> Option<&Output> {
    outputs
        .iter()
        .find(|o| o.rect.x == monitor.x && o.rect.y == monitor.y)
        .or_else(|| {
            let (cx, cy) = (
                monitor.x + monitor.width / 2,
                monitor.y + monitor.height / 2,
            );
            outputs.iter().find(|o| {
                (o.rect.x..o.rect.x + o.rect.width).contains(&cx)
                    && (o.rect.y..o.rect.y + o.rect.height).contains(&cy)
            })
        })
}

fn list_outputs() -> anyhow::Result<Vec<Output>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init(&conn)?;
    let qh = queue.handle();
    let mut state = Lister {
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
    };
    // Two round-trips: bind outputs, then receive their (xdg-)output info.
    queue.roundtrip(&mut state)?;
    queue.roundtrip(&mut state)?;
    Ok(state
        .outputs
        .outputs()
        .filter_map(|o| state.outputs.info(&o))
        .filter_map(|info| {
            let (x, y) = info.logical_position.unwrap_or(info.location);
            let (width, height) = info.logical_size?;
            Some(Output {
                name: info.name?,
                rect: MonitorRect {
                    x,
                    y,
                    width,
                    height,
                },
            })
        })
        .collect())
}

struct Lister {
    registry: RegistryState,
    outputs: OutputState,
}

impl OutputHandler for Lister {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

delegate_output!(Lister);
delegate_registry!(Lister);

impl ProvidesRegistryState for Lister {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers! { OutputState }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(name: &str, x: i32, y: i32, width: i32, height: i32) -> Output {
        Output {
            name: name.into(),
            rect: MonitorRect {
                x,
                y,
                width,
                height,
            },
        }
    }

    #[test]
    fn matches_by_position_then_by_centre() {
        let outputs = [
            output("DP-1", 0, 0, 2560, 1440),
            output("HDMI-A-1", 2560, 0, 1920, 1080),
        ];
        let exact = MonitorRect {
            x: 2560,
            y: 0,
            width: 1920,
            height: 1080,
        };
        assert_eq!(
            best_match(&outputs, exact).map(|o| o.name.as_str()),
            Some("HDMI-A-1")
        );
        // Slightly off geometry still lands on the monitor containing its centre.
        let fuzzy = MonitorRect {
            x: 4,
            y: 2,
            width: 2560,
            height: 1440,
        };
        assert_eq!(
            best_match(&outputs, fuzzy).map(|o| o.name.as_str()),
            Some("DP-1")
        );
        let nowhere = MonitorRect {
            x: -5000,
            y: 0,
            width: 100,
            height: 100,
        };
        assert_eq!(best_match(&outputs, nowhere), None);
    }
}
