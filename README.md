# CanaryWatcher

This is a small tool that can watch for any actions on a specific file or directory,
and when anything touches it, it will lock your LUKS volume(s) and forcefully, immediately
turn off your computer.

This means that if someone is lurking around your files, they will get locked out.

Also, you want to run this with setuid or root, or it won't be able to perform its tasks.

## Usage

```
canarywatcher [test|arm] [notify|fuse] <path> [allowlist_file]
```

- `test|arm` — `test` logs what would happen without executing; `arm` performs the real lockdown
- `notify|fuse` — `notify` watches via inotify; `fuse` mounts a virtual filesystem over the path
- `path` — file or directory to watch
- `allowlist_file` — *(optional)* path to a file listing process names that may access the canary without triggering lockdown

## Process allowlist

Instead of disabling system tools like gnome-tracker, you can list trusted process names in an
allowlist file — one process name per line. Lines starting with `#` are treated as comments.

Example `/etc/canarywatcher/allowlist`:
```
# Allow backup daemon to scan without triggering lockdown
baloo_file
tracker-miner-f
```

When using `fuse` mode, any process whose name matches an entry in this file will be permitted
to access the canary directory without triggering a lockdown. Note: `notify` mode cannot filter
by process (inotify does not expose the accessing PID), so the allowlist only applies to `fuse` mode.

## Multiple LUKS volumes

CanaryWatcher automatically discovers **all** active LUKS volumes (any `dmsetup` entry prefixed
with `luks-`) and locks every one of them during lockdown. No configuration is required.

## Systemd service

See `canary.service` for an example service unit. Adjust the path and optionally add the
allowlist file as the fifth argument.
