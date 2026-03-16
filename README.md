# Smart Launcher Scripts

This repository contains Windows batch-based smart launchers that control application-dependent services only when needed. The goal is to reduce unnecessary background service activity while preserving a predictable startup workflow for target engineering applications.

## Repository Structure

- `Ansys Smart Launcher/`
  - `Ansys Electronics.bat`
  - `README.md`
- `Multisim Smart Launcher/`
  - `Multisim.bat`
  - `README.md`

## What These Scripts Do

Each launcher follows a controlled runtime sequence:

1. Starts required Windows services in a defined order.
2. Waits for a short initialization interval.
3. Starts the target application executable.
4. Monitors the process state in a loop.
5. Stops non-required related services after the app exits.
6. Closes itself after a short delay.

This pattern keeps service state aligned with real application usage instead of keeping all related services running continuously.

## How to Use

1. Open the launcher folder for your target application.
2. Right-click the `.bat` file and create a desktop shortcut.
3. Rename the shortcut and customize its icon if needed:
   - Right-click shortcut -> `Properties` -> `Change Icon...`
4. Enable one-click elevated execution:
   - Right-click shortcut -> `Properties` -> `Advanced...` -> enable `Run as administrator`
5. Launch the application from this shortcut for service-aware startup/shutdown behavior.

## Operational Requirements

- Run on Windows with administrative rights for service control commands.
- Verify service display names and executable paths match your local installation.
- Keep the launcher console open while the target application is running.

## Legal Disclaimer

This repository is provided for legitimate system administration and automation use only. It must not be used to violate software license terms, organizational policies, or applicable law.

All scripts are provided **"as is"**, without warranties of any kind. The author accepts no responsibility or liability for any usage-related outcomes, including but not limited to misconfiguration, service interruption, data loss, system instability, direct damages, or indirect damages. The user is solely responsible for lawful, licensed, and policy-compliant use.
