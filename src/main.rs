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
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::env;
use libc::ENOENT;
use fuse::{
    Filesystem,
    ReplyDirectory,
    ReplyAttr,
    Request,
    FileType,
    FileAttr,
};
use inotify::{
    Inotify,
    WatchMask,
};

/// Returns all active LUKS volume names from dmsetup.
fn get_luks_vols() -> Vec<String> {
    let output = match Command::new("/usr/sbin/dmsetup").arg("ls").output() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("ERROR: Failed to run dmsetup: {}", e);
            return Vec::new();
        }
    };

    let text = match from_utf8(&output.stdout) {
        Ok(s) => s.to_string(),
        Err(e) => {
            eprintln!("ERROR: Failed to parse dmsetup output: {}", e);
            return Vec::new();
        }
    };

    let vols: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, '\t');
            let name = parts.next()?;
            if name.starts_with("luks-") {
                Some(name.to_string())
            } else {
                None
            }
        })
        .collect();

    if vols.is_empty() {
        eprintln!("WARNING: No LUKS volumes found");
    } else {
        eprintln!("Found {} LUKS volume(s): {:?}", vols.len(), vols);
    }

    vols
}

/// Locks all LUKS volumes and forces an immediate system reboot.
/// In test mode, only logs what would happen without performing any actions.
fn do_the_lock(armed: bool, luks_vols: &[String]) {
    if !armed {
        eprintln!("TEST MODE: Would have locked {} volume(s): {:?}", luks_vols.len(), luks_vols);
        return;
    }

    eprintln!("LOCKDOWN TRIGGERED: locking {} volume(s)", luks_vols.len());

    // Enable sysrq
    match File::create(Path::new("/proc/sys/kernel/sysrq")) {
        Ok(mut f) => { let _ = f.write_all(b"1"); }
        Err(e) => eprintln!("WARNING: Could not enable sysrq: {}", e),
    }

    // Lock every LUKS volume
    for vol in luks_vols {
        eprintln!("Locking LUKS volume: {}", vol);
        match Command::new("/usr/sbin/cryptsetup").args(&["luksClose", vol]).output() {
            Ok(_) => {}
            Err(e) => eprintln!("WARNING: Failed to close {}: {}", vol, e),
        }
    }

    // Trigger immediate reboot via sysrq "b" (reboot)
    match File::create(Path::new("/proc/sysrq-trigger")) {
        Ok(mut f) => { let _ = f.write_all(b"b"); }
        Err(e) => eprintln!("WARNING: Could not write sysrq-trigger: {}", e),
    }

    // Fallback: hard reboot via systemd
    match Command::new("/usr/sbin/reboot").args(&["--force", "--force"]).spawn() {
        Ok(_) => {}
        Err(e) => eprintln!("ERROR: Failed to invoke reboot: {}", e),
    }
}

/// Loads a list of process names that are allowed to access the canary path
/// without triggering lockdown. Each non-empty, non-comment line is one name.
fn load_allowlist(path: &str) -> Vec<String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    BufReader::new(file)
        .lines()
        .filter_map(|l| l.ok())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect()
}

/// Returns true if the process with the given PID has a name in the allowlist.
fn is_process_allowed(pid: i32, allowlist: &[String]) -> bool {
    if allowlist.is_empty() {
        return false;
    }
    match stat(pid) {
        Ok(info) => {
            let allowed = allowlist.iter().any(|a| *a == info.command);
            if allowed {
                eprintln!("Allowing access by whitelisted process '{}' (pid {})", info.command, pid);
            }
            allowed
        }
        Err(e) => {
            eprintln!("WARNING: Could not read procinfo for pid {}: {}", pid, e);
            false
        }
    }
}

fn notify(armed: bool, path: String, luks_vols: Vec<String>) {
    let mut inotify = Inotify::init()
        .expect("Error while initializing inotify instance");

    inotify
        .add_watch(&path, WatchMask::ALL_EVENTS)
        .expect("Failed to add file watch");

    eprintln!("Watching path: {}", path);

    let mut buffer = [0u8; 1024];
    let events = inotify.read_events_blocking(&mut buffer)
        .expect("Error while reading inotify events");

    for _event in events {
        do_the_lock(armed, &luks_vols);
    }
}

struct LockFS {
    armed: bool,
    luks_vols: Vec<String>,
    allowlist: Vec<String>,
}

const CREATE_TIME: Timespec = Timespec { sec: 1381237736, nsec: 0 };

impl Filesystem for LockFS {
    fn getattr(&mut self, _req: &Request, ino: u64, reply: ReplyAttr) {
        if ino == 1 {
            reply.attr(
                &Timespec { sec: 1, nsec: 0 },
                &FileAttr {
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
                },
            );
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

        let pid = _req.pid() as i32;
        match stat(pid) {
            Ok(ref info) => eprintln!("Canary accessed by process '{}' (pid {})", info.command, pid),
            Err(_) => eprintln!("Canary accessed by pid {}", pid),
        }

        if is_process_allowed(pid, &self.allowlist) {
            return;
        }

        eprintln!("Triggering lockdown for LUKS volumes: {:?}", self.luks_vols);
        do_the_lock(self.armed, &self.luks_vols);
    }
}

fn fuse_watch(armed: bool, path: String, luks_vols: Vec<String>, allowlist: Vec<String>) {
    let options = ["-o", "allow_other"]
        .iter()
        .map(|o| o.as_ref())
        .collect::<Vec<&OsStr>>();
    fuse::mount(
        LockFS { luks_vols, armed, allowlist },
        &path,
        &options,
    ).expect("Failed to mount FUSE filesystem");
}

fn usage() -> ! {
    eprintln!("Usage: canarywatcher [test|arm] [notify|fuse] <path> [allowlist_file]");
    eprintln!();
    eprintln!("  test|arm        - test mode logs actions; arm mode executes lockdown");
    eprintln!("  notify|fuse     - watch via inotify or a FUSE mount");
    eprintln!("  path            - file or directory to watch");
    eprintln!("  allowlist_file  - optional file with process names that may access the path");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 || args.len() > 5 {
        usage();
    }

    let arm_arg = &args[1];
    let option  = &args[2];
    let path    = args[3].clone();

    let armed = match arm_arg.as_str() {
        "test" => false,
        "arm"  => true,
        _      => usage(),
    };

    let allowlist_path = args.get(4).map(|s| s.as_str()).unwrap_or("");
    let allowlist = if !allowlist_path.is_empty() {
        let list = load_allowlist(allowlist_path);
        eprintln!("Loaded {} allowlisted process(es) from '{}'", list.len(), allowlist_path);
        list
    } else {
        Vec::new()
    };

    let luks_vols = get_luks_vols();
    if luks_vols.is_empty() {
        eprintln!("WARNING: No LUKS volumes detected; lockdown will have nothing to close");
    }

    if armed {
        eprintln!("ARMED: lockdown is active");
    } else {
        eprintln!("TEST MODE: actions will be logged but not executed");
    }

    match option.as_str() {
        "notify" => notify(armed, path, luks_vols),
        "fuse"   => fuse_watch(armed, path, luks_vols, allowlist),
        _        => usage(),
    }
}
