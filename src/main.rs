extern crate inotify;

use std::process::Command;
use std::env;
use inotify::{
    Inotify,
    watch_mask,
};

fn main() {
    let mut inotify = Inotify::init()
        .expect("Error while initializing inotify instance");

    for arg in env::args().skip(1) {
        inotify
            .add_watch(
                arg,
                watch_mask::ALL_EVENTS
            )
            .expect("Failed to add file watch");
    }

    let mut buffer = [0; 1024];
    let events = inotify.read_events_blocking(&mut buffer)
        .expect("Error while reading events");

    for event in events {
        println!("Got event: {:?}", event.name);
        Command::new("/usr/sbin/cryptsetup")
                .arg("luksClose")
                .arg("/dev/sda3")
                .spawn()
                .expect("Failed to lock");
    }
}
