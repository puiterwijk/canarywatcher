/** 
 * Copyright (c) 2018, Patrick Uiterwijk <patrick@puiterwijk.org>
 * All rights reserved.
 *
 * This file is part of CanaryWatcher.
 *
 * CanaryWatcher is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License.
 *
 * CanaryWatcher is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with CanaryWatcher.  If not, see <http://www.gnu.org/licenses/>.
 */

extern crate inotify;

use std::str::from_utf8;
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
    println!("Arming...");

    let mut luksvol = None;

    let dmset = Command::new("/usr/sbin/dmsetup")
                       .arg("ls")
                       .output()
                       .expect("Failed to get dmset")
                       .stdout;
    let dmset = &dmset;
    let dmset = from_utf8(dmset).expect("Unable to parse string");
    for val in dmset.split("\n") {
        let split = val.split("\t").collect::<Vec<&str>>();
        if split.len() == 2 {
            if split[0].contains("luks-") {
                luksvol = Some(split[0].to_string());
            }
        }
    }
    let luksvol = luksvol.expect("No luks volume found");

    println!("Using luks volume {:?}", luksvol);

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
    println!("Armed!");

    let events = inotify.read_events_blocking(&mut buffer)
        .expect("Error while reading events");

    for event in events {
        println!("Triggered!");

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
                .arg(luksvol)
                .output();

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
