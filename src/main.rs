/** 
 * Copyright (c) 2018, Patrick Uiterwijk <patrick@puiterwijk.org>
 * All rights reserved.
 *
 * This file is part of CanaryWatcher.
 *
 * CanaryWatcher is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 3.
 *
 * CanaryWatcher is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with CanaryWatcher.  If not, see <http://www.gnu.org/licenses/>.
 */

extern crate fuse;
extern crate libc;
extern crate inotify;
extern crate time;
extern crate procinfo;

use procinfo::pid::stat;
use time::Timespec;
use std::ffi::OsStr;
use std::string::String;
use std::str::from_utf8;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::env;
use libc::ENOENT;
use fuse::{
    Filesystem,
    ReplyDirectory,
    ReplyEntry,
    ReplyAttr,
    ReplyData,
    Request,
    FileType,
    FileAttr,
};
use inotify::{
    Inotify,
    WatchMask,
};

fn getLuksVol() -> String {
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

    luksvol
}

fn doTheLock(armed: bool, luksvol: String) {
    println!("Locking");

    if !armed {
        println!("Would've locked");
        return;
    }

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
}

fn notify(armed: bool, path: String, luksvol: String) {
    let mut inotify = Inotify::init()
        .expect("Error while initializing inotify instance");

    inotify
        .add_watch(
            path,
            WatchMask::ALL_EVENTS
        )
        .expect("Failed to add file watch");

    let mut buffer = [0; 1024];
    let events = inotify.read_events_blocking(&mut buffer)
        .expect("Error while reading events");

    for event in events {
        doTheLock(armed, luksvol.to_string());
    }
}

struct LockFS {
    armed: bool,
    luksvol: String,
}

const CREATE_TIME: Timespec = Timespec { sec: 1381237736, nsec: 0};

impl Filesystem for LockFS {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            reply.attr(&Timespec{sec: 1, nsec: 0},
                       &FileAttr{
                           ino: 1,
                           size: 0,
                           blocks: 0,
                           kind: FileType::Directory,
                           perm: 0o777,
                           nlink: 2,
                           uid: 0,
                           gid: 0,
                           rdev: 0,
                           flags: 0,
                           atime: CREATE_TIME,
                           mtime: CREATE_TIME,
                           crtime: CREATE_TIME,
                           ctime: CREATE_TIME,
                       });
            // Reply
        } else {
            reply.error(ENOENT);
        }
    }

    fn readdir(&mut self, _req: &Request, ino: u64, _fh: u64, offset: i64, mut reply: ReplyDirectory) {
        if ino == 1 {
            if offset == 0 {
                reply.add(1, 0, FileType::Directory, ".");
                reply.add(1, 1, FileType::Directory, "..");
            }
            reply.ok();
        } else {
            reply.error(ENOENT);
        }

        let procinfo = stat(_req.pid() as i32);
        println!("Causing proc: {:?}", procinfo);

        println!("Locking up {:?}", self.luksvol);

        doTheLock(self.armed, self.luksvol.to_string());
    }
}

fn fuse(armed: bool, path: String, luksvol: String) {
    let options = ["-o", "allow_other"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(LockFS{luksvol:luksvol, armed: armed}, &path, &options).unwrap();
}

fn main() {
    let luksvol = getLuksVol();

    if env::args_os().len() != 4 {
        panic!("Usage: <program> [test|arm] [notify|fuse] <path>");
        return;
    }

    let args: Vec<String> = env::args().collect();
    let armarg = & args[1];
    let option = &args[2];
    let path = (&args[3]).to_string();
    let armed = armarg != "test";

    if armarg != "test" && armarg != "arm" {
        panic!("Usage: <program> [test|arm] [notify|fuse] <path>");
        return;
    }
    if armed {
        println!("Arming!");
    }

    if option == "notify" {
        notify(armed, path, luksvol);
    } else if option == "fuse" {
        fuse(armed, path, luksvol);
    } else {
        panic!("Usage: <program> [notify|fuse] <path>");
    }
}
