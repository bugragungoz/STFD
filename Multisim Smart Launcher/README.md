# Multisim Smart Launcher

## Purpose
Automates NI service orchestration for Multisim runtime on Windows. The launcher enables required NI background services before startup, launches `multisim.exe`, monitors process lifetime, and disables the services when execution ends.

## Operational Notes
- Run with administrative privileges to permit service state transitions.
- Validate executable path and service display names against your installed NI stack.
- Keep the launcher console open during runtime; it is responsible for process monitoring.

## Legal Disclaimer
This script is intended as a service-management wrapper for system optimization. When the target application is not running, it keeps unnecessary related services stopped; before launch, it starts required services in sequence and then opens the application; it monitors the process and, after the application exits, stops unnecessary services and terminates itself after a short delay.

This project is provided "as is". The author accepts no responsibility or liability for any usage-related issues, including misconfiguration, service interruption, data loss, system instability, or any direct/indirect damages. The user is solely responsible for lawful, licensed, and policy-compliant use.