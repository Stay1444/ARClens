//! Is ARC Raiders running? Answered from process command lines in `/proc`,
//! the same information `ps` shows. This lists processes only: it never
//! opens, reads or signals the game (see AGENTS.md rule 1).
//!
//! Inside a Flatpak sandbox other processes are invisible, so the game is
//! never found there; capture then needs the "always" mode.

use futures::SinkExt as _;
use futures::channel::mpsc;
use iced::Subscription;
use std::time::Duration;

/// How often to look.
const POLL: Duration = Duration::from_secs(5);

/// Whether a process command line (arguments joined by spaces) is the game:
/// its Unreal executable (`PioneerGame*.exe`, run by Proton) or anything
/// started from its Steam install directory.
pub fn is_game_cmdline(cmdline: &str) -> bool {
    let lower = cmdline.to_lowercase().replace('\\', "/");
    lower.contains("pioneergame") && lower.contains(".exe")
        || lower.contains("steamapps/common/arc raiders/")
}

/// Scans `/proc` once.
pub fn game_running() -> bool {
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return false;
    };
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .bytes()
                .all(|b| b.is_ascii_digit())
        })
        .filter_map(|e| std::fs::read(e.path().join("cmdline")).ok())
        .any(|raw| {
            let cmdline: String = String::from_utf8_lossy(&raw).replace('\0', " ");
            is_game_cmdline(&cmdline)
        })
}

/// Emits whether the game runs: once at start, then on every change.
pub fn subscription() -> Subscription<bool> {
    Subscription::run(|| {
        iced::stream::channel(4, async |mut output: mpsc::Sender<bool>| {
            let mut last = None;
            loop {
                let running = tokio::task::spawn_blocking(game_running)
                    .await
                    .unwrap_or(false);
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
    fn ignores_other_processes() {
        assert!(!is_game_cmdline("./target/release/arclens"));
        assert!(!is_game_cmdline(
            "/usr/bin/kate /home/me/notes/pioneergame-log.txt"
        ));
        assert!(!is_game_cmdline("steam -silent"));
    }
}
