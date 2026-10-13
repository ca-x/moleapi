# Supervised runner service installation

## Objective

Install the native `runner agent` as a supervised per-user service, using a private workspace-scoped credential file. The installer itself never contacts the hosted server, never embeds bearer contents in a service definition and never starts a service during application build or package installation.

## Linux systemd user service

- `moleapi-cli --server URL --token-file FILE runner service print --id UUID` renders an exact user-unit preview. `install --id UUID [--variables-file FILE] [--replace]` writes `moleapi-runner-UUID.service` to the current user's systemd unit directory, reloads the user manager and enables/starts it. `status --id UUID` queries the user manager. `uninstall --id UUID` disables/stops only an identified MoleAPI unit, removes it and reloads the manager.
- Runner IDs must be canonical UUIDs. The credential file and optional variables file must be existing regular, non-symlink files readable only by the current user (mode with no group/other bits); their canonical absolute paths appear as arguments, not contents. `--token-env` is rejected for installation. The current executable path is embedded as an absolute argument. The unit quotes and escapes arguments for systemd, restarts on failure after 5 seconds, uses `UMask=0077` and runs under the current user without opening an inbound port.
- Installation refuses an existing unit unless `--replace` is used and the file has the exact managed marker and runner ID. Uninstall refuses foreign or modified files. Unit writes are atomic and mode 0600. Manager failures return an actionable error without deleting the generated unit silently.
## macOS launchd user agent

The same CLI commands install `com.moleapi.runner.UUID.plist` under the current user's `~/Library/LaunchAgents`. The plist uses an argument array, XML escaping, a managed marker and checksum, private atomic writes, RunAtLoad, KeepAlive and a five-second restart throttle. Install bootstraps the current GUI user domain; replacement first boots out a loaded agent; status reports installed/loaded, and uninstall boots out before removing only a verified owned plist. Credential and variables files use the same private regular-file requirement. The CLI never places their contents in the plist.

## Windows per-user scheduled task

The same commands use Task Scheduler with an interactive logon trigger, current-user SID, a single running instance, no execution time limit, and up to 999 restart attempts at one-minute intervals. `install` registers and starts a task under `\\MoleAPI\\Runner-UUID`; `status` reports installed/registered, and `uninstall` ends and removes only a verified managed task. The XML manifest stays under `%LOCALAPPDATA%\\MoleAPI\\runner-services`, with a managed marker and checksum. The token and optional variables files must be regular non-symlink files inside the current user's `%LOCALAPPDATA%` tree, inheriting its private ACL; bearer contents never enter the XML or command line. This task runs while the user is logged in, not as a machine-wide Windows Service.

Linux/macOS cross-compilation and Windows XML generation do not establish native installation gates; each manager requires actual platform verification.

## Acceptance

Write generator/escaping and CLI process fixtures with a temporary user config directory and fake `systemctl`, checking private files, replace/uninstall ownership, exact commands and no token in stdout/unit. Test execution is deferred until the user-requested final verification phase.
