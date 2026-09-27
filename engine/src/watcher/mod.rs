use crate::discovery;
use anyhow::Result;
use colored::Colorize;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Changes closer together than this are handled by one rebuild.
const DEBOUNCE: Duration = Duration::from_millis(500);

pub fn watch<F>(dir: &str, mut on_change: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    let (tx, rx) = mpsc::channel();
    let mut watcher = notify::recommended_watcher(tx)?;
    watcher.watch(Path::new(dir), RecursiveMode::Recursive)?;

    println!(
        "{} {} {} {}",
        "👀".bold(),
        "flint_build".cyan().bold(),
        "is watching in".white(),
        dir.underline()
    );

    let is_relevant = |result: notify::Result<Event>| match result {
        Ok(event) => {
            log::debug!("Watcher event: {:?}", event);
            triggers_rebuild(&event)
        }
        Err(e) => {
            eprintln!("  {} {}", "❌".red().bold(), e.to_string().red().bold());
            false
        }
    };

    while let Ok(result) = rx.recv() {
        if !is_relevant(result) {
            continue;
        }
        // Wait until relevant events stop arriving, so one save (or a burst of them) is one rebuild.
        // Irrelevant events (reads, .g.dart writes) must not push the deadline back.
        let mut deadline = Instant::now() + DEBOUNCE;
        loop {
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(result) => {
                    if is_relevant(result) {
                        deadline = Instant::now() + DEBOUNCE;
                    }
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return Ok(()),
            }
        }

        println!(
            "\n{} {}",
            "🔄".yellow().bold(),
            "Change detected! Rebuilding...".bold()
        );
        if let Err(e) = on_change() {
            eprintln!("  {} {}", "❌".red(), e.to_string().red());
        }
    }

    Ok(())
}

/// Only real changes to non-generated files trigger a build. Flint's own reads of sources (access events)
/// and its writes to `.g.dart` files must not, or watch mode rebuilds forever (spec 0001, R5).
fn triggers_rebuild(event: &Event) -> bool {
    !matches!(event.kind, EventKind::Access(_))
        && event
            .paths
            .iter()
            .any(|path| !discovery::is_generated_file(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, AccessMode, DataChange, ModifyKind};

    fn event(kind: EventKind, paths: &[&str]) -> Event {
        paths
            .iter()
            .fold(Event::new(kind), |event, path| event.add_path(path.into()))
    }

    #[test]
    fn test_triggers_rebuild() {
        let modify = EventKind::Modify(ModifyKind::Data(DataChange::Any));
        let open = EventKind::Access(AccessKind::Open(AccessMode::Any));

        assert!(triggers_rebuild(&event(modify, &["lib/user.dart"])));
        assert!(!triggers_rebuild(&event(open, &["lib/user.dart"])));
        assert!(!triggers_rebuild(&event(modify, &["lib/user.g.dart"])));
        assert!(triggers_rebuild(&event(
            modify,
            &["lib/user.g.dart", "lib/user.dart"]
        )));
    }
}
