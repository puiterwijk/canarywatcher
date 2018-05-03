# CanaryWatcher

This is a small tool that can watch for any actions on a specific file or directory,
and when anything touches it, it will lock your luks volume and forcefully, immediately
turn off your computer.

This means that if someone is lurking around your files, they will get locked out.

## DO NOTE: Disable gnome tracker and equivalent tools, or they will trigger it!

Also, you want to run this with setuid or root, or it won't be able to perform its tasks.
