use notify::{recommended_watcher, RecursiveMode, Watcher as _};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "/Users/adm/Documents/Repos/flux/examples/counter".into());
    let (tx, rx) = channel();
    let mut w = recommended_watcher(tx).expect("watcher");
    w.watch(std::path::Path::new(&dir), RecursiveMode::Recursive).expect("watch");
    println!("watching {dir} (backend=recommended) for 30s; edit the file now");

    let start = Instant::now();
    let mut got = 0;
    while start.elapsed() < Duration::from_secs(30) {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(e)) => { got += 1; println!("EVENT#{} {:?} {:?}", got, e.kind, e.paths); }
            Ok(Err(err)) => println!("ERR {:?}", err),
            Err(_) => {}
        }
    }
    println!("done, got {got} events");
}
