use anyhow::Result;
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

    println!("🚀 flint is watching for changes in {}...", dir);

    // 4. The Event Loop
    for res in rx {
        match res {
            Ok(_) => {
                println!("🔄 Change detected! Rebuilding...");
                if let Err(e) = on_change() {
                    eprintln!("❌ Build failed: {}", e);
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}
