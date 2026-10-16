# Changelog

## 1.0.0

Lockstep replaces the Ansys and Multisim batch scripts with one app for any program.

- Choose a program's folder and the program to open; its services are found in the folder and
  from the same publisher.
- Lockstep's shortcut starts the services, waits until they run, then opens the program.
- After the program closes and a wait (30 seconds by default, set per app), the programs left
  running from its folders are asked to close and ended if they do not, and its services stop.
  Opening the program again during the wait keeps everything running.
- One administrator prompt when an app is saved lets your account start and stop its services;
  services that start with Windows can be set to start by hand. Removing the app undoes both.
- Never closed: Windows' own programs, other people's, a service's process whose service the app
  does not stop, and what another open app uses.
- Optionally also watches apps opened another way, from the tray.
- A history of every session; English and Turkish, light and dark theme.
