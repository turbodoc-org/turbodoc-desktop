//! A fixed pool of worker threads for network and disk work, so a burst of image fetches never
//! spawns a thread each, and the UI thread never blocks. Jobs are plain closures.
use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};

type Job = Box<dyn FnOnce() + Send + 'static>;

struct Pool {
    urgent: Mutex<Sender<Job>>,
    bulk: Mutex<Sender<Job>>,
}

fn pool() -> &'static Pool {
    static POOL: OnceLock<Pool> = OnceLock::new();
    POOL.get_or_init(|| Pool { urgent: Mutex::new(lane("td-net", 4)), bulk: Mutex::new(lane("td-media", 4)) })
}

fn lane(name: &str, threads: usize) -> Sender<Job> {
    let (tx, rx) = channel::<Job>();
    let rx = std::sync::Arc::new(Mutex::new(rx));
    for i in 0..threads {
        let rx = rx.clone();
        let _ = std::thread::Builder::new().name(format!("{name}-{i}")).spawn(move || {
            loop {
                let job = match rx.lock() {
                    Ok(guard) => guard.recv(),
                    Err(_) => return,
                };
                match job {
                    Ok(job) => job(),
                    Err(_) => return,
                }
            }
        });
    }
    tx
}

/// Runs `job` on the request lane (API calls, auth): the user is waiting on these.
pub fn spawn(job: impl FnOnce() + Send + 'static) {
    if let Ok(tx) = pool().urgent.lock() {
        let _ = tx.send(Box::new(job));
    }
}

/// Runs `job` on the media lane (preview images, favicons), so a page of thumbnails never
/// delays a save.
pub fn spawn_bulk(job: impl FnOnce() + Send + 'static) {
    if let Ok(tx) = pool().bulk.lock() {
        let _ = tx.send(Box::new(job));
    }
}
