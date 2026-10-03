//! Is ARC Raiders running? Answered from the process list (names, paths
//! and command lines, what `ps` or Task Manager shows), on every platform
//! through `sysinfo`. This lists processes only: it never opens, reads or
//! signals the game (see AGENTS.md rule 1).
//!
//! Inside a Flatpak sandbox other processes are invisible, so the game is
//! never found there; capture then needs the "always" mode.

use futures::SinkExt as _;
use futures::channel::mpsc;
use iced::Subscription;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// How often to look.
const POLL: Duration = Duration::from_secs(5);

/// Whether a process is the game, from its name, executable path and
/// command line joined by spaces: its Unreal executable
/// (`PioneerGame*.exe`, natively on Windows or under Proton) or anything
/// started from its Steam install directory.
pub fn is_game_cmdline(cmdline: &str) -> bool {
    let lower = cmdline.to_lowercase().replace('\\', "/");
    lower.contains("pioneergame") && lower.contains(".exe")
        || lower.contains("steamapps/common/arc raiders/")
}

/// Looks through the process list once.
pub fn game_running(system: &mut System) -> bool {
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet),
    );
    system.processes().values().any(|process| {
        let mut line = process.name().to_string_lossy().into_owned();
        if let Some(exe) = process.exe() {
            line.push(' ');
            line.push_str(&exe.to_string_lossy());
        }
        for arg in process.cmd() {
            line.push(' ');
            line.push_str(&arg.to_string_lossy());
        }
        is_game_cmdline(&line)
    })
}

/// Emits whether the game runs: once at start, then on every change.
pub fn subscription() -> Subscription<bool> {
    Subscription::run(|| {
        iced::stream::channel(4, async |mut output: mpsc::Sender<bool>| {
            let mut last = None;
            let mut system = Some(System::new());
            loop {
                // Kept between polls: only new processes are read in full.
                let (running, kept) = tokio::task::spawn_blocking(move || {
                    let mut system = system.unwrap_or_default();
                    (game_running(&mut system), system)
                })
                .await
                .map_or((false, None), |(running, kept)| (running, Some(kept)));
                system = kept;
                if last != Some(running) {
                    tracing::info!(running, "ARC Raiders process");
                    last = Some(running);
                    if output.send(running).await.is_err() {
                        return;
                    }
                }
                tokio::time::sleep(POLL).await;
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_the_game_under_proton() {
        assert!(is_game_cmdline(
            r"Z:\home\me\.local\share\Steam\steamapps\common\ARC Raiders\PioneerGame\Binaries\Win64\PioneerGame-Win64-Shipping.exe -dx12"
        ));
        assert!(is_game_cmdline(
            "/home/me/.local/share/Steam/steamapps/common/ARC Raiders/PioneerGame.exe"
        ));
    }

    #[test]
    fn recognises_the_game_on_windows() {
        assert!(is_game_cmdline(
            r"PioneerGame-Win64-Shipping.exe C:\Program Files (x86)\Steam\steamapps\common\ARC Raiders\PioneerGame\Binaries\Win64\PioneerGame-Win64-Shipping.exe"
        ));
    }

    #[test]
    fn lists_processes() {
        let mut system = System::new();
        // Whether the game is found depends on the machine; the scan must
        // at least see this test.
        let _ = game_running(&mut system);
        assert!(!system.processes().is_empty());
    }

    #[test]
    fn ignores_other_processes() {
        assert!(!is_game_cmdline("./target/release/arclens"));
        assert!(!is_game_cmdline(
            "/usr/bin/kate /home/me/notes/pioneergame-log.txt"
        ));
        assert!(!is_game_cmdline("steam -silent"));
    }
}
