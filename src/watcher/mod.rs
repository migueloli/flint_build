use anyhow::Result;
use colored::Colorize;
use notify::RecursiveMode;
use notify_debouncer_mini::new_debouncer;
use std::path::Path;
use std::time::Duration;

pub fn watch<F>(dir: &str, mut on_change: F) -> Result<()>
where
    F: FnMut() -> Result<()>,
{
    // 1. Create a channel to receive events
    let (tx, rx) = std::sync::mpsc::channel();

    // 2. Setup the debouncer (waits 200ms after a change so we don't rebuild 100 times while typing)
    let mut debouncer = new_debouncer(Duration::from_millis(500), tx)?;

    // 3. Start watching the directory
    debouncer
        .watcher()
        .watch(Path::new(dir), RecursiveMode::Recursive)?;

    println!(
        "{} {} {} {}",
        "👀".bold(),
        "flint".cyan().bold(),
        "is watching in".white(),
        dir.underline()
    );

    // 4. The Event Loop
    for res in rx {
        match res {
            Ok(events) => {
                log::debug!("Watcher events: {:?}", events);
                println!(
                    "\n{} {}",
                    "🔄".yellow().bold(),
                    "Change detected! Rebuilding...".bold()
                );
                if let Err(e) = on_change() {
                    eprintln!("  {} {}", "❌".red(), e.to_string().red());
                }
            }
            Err(e) => println!("  {} {:?}", "❌".red().bold(), e.to_string().red().bold()),
        }
    }

    Ok(())
}
