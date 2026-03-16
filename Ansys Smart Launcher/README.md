# Ansys Smart Launcher

## Purpose
Automates the Windows service lifecycle required by ANSYS Electronics Desktop sessions. The launcher starts required license-related services, launches `ansysedt.exe`, monitors the process state, and shuts services down after the application exits.

## Operational Notes
- Run with administrative privileges to allow service control operations (`net start` / `net stop`).
- Ensure executable path and Windows service display names match your local installation.
- Keep the console window open while ANSYS is active; the monitor loop runs in that session.

## Legal Disclaimer
This script is intended as a service-management wrapper for system optimization. When the target application is not running, it keeps unnecessary related services stopped; before launch, it starts required services in sequence and then opens the application; it monitors the process and, after the application exits, stops unnecessary services and terminates itself after a short delay.

This project is provided "as is". The author accepts no responsibility or liability for any usage-related issues, including misconfiguration, service interruption, data loss, system instability, or any direct/indirect damages. The user is solely responsible for lawful, licensed, and policy-compliant use.