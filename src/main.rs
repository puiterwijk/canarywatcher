extern crate inotify;

use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
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

        println!("Enabling sysrq");
        match File::create(Path::new("/proc/sys/kernel/sysrq")) {
            Err(e) => {
                println!("Unable to enable sysrq");
                Some(false)
            },
            Ok(mut file) => {
                file.write_all("1".as_bytes());
                Some(true)
            }
        };

        println!("Locking file system");
        Command::new("/usr/sbin/cryptsetup")
                .arg("luksClose")
                .arg("luks-f13f0bb2-d2b6-42d3-9b5b-d9f02b8dcb37")
                .spawn()
                .expect("Failed to lock");

        println!("Raising Elephants Is So Utterly Boring");
        match File::create(Path::new("/proc/sysrq-trigger")) {
            Err(e) => {
                println!("Unable to open sysrq-trigger");
                Some(false)
            },
            Ok(mut file) => {
                file.write_all("b".as_bytes()).expect("Failed");
                Some(true)
            }
        };

        println!("And hard shutdown");
        Command::new("/usr/sbin/reboot")
                .arg("--force")
                .arg("--force")
                .spawn()
                .expect("Failed to reboot");

        println!("And we are gone");
        return;
    }
}
