# Spare OS: architecture map

*Stage: Mapping. Every choice below is open until an owner accepts a decision note.*

A free operating system for desktops and laptops, so nobody has to pay for Windows, with every Spare app built in and a one-step move from Windows. We don't write a kernel from scratch: Linux has run most of the world's servers and phones for decades. Spare OS is a distribution: a proven Linux base, a desktop, the Spare apps, safe updates, and tools for moving across and managing many machines.

## Goals

- Free on any ordinary PC or laptop, including old ones Windows no longer supports.
- Feels familiar to Windows users from the first minute.
- Updates that can't leave a machine broken.
- Offices, schools and co-ops can manage many machines without paid licences.

## Its parts

### Base and desktop

**High level:** a mature Linux distribution as the base, and a desktop that feels familiar to Windows users.

**Low level:** base and desktop are chosen on stability, hardware support, update model and how long each release is supported (pieces OS01 and OS02).

### Spare apps built in

**High level:** Spare Office, Spare Studio, Spare Arch and the rest come preinstalled, signed in with one Spareware login, and update through the same channel as the system.

### Running Windows programs

**High level:** many Windows programs run through compatibility layers (Wine, and Proton for games). Some won't, for example programs that need Windows-only drivers or anti-cheat. The plan is to replace those with Spare apps over time, starting with Spare Arch for Revit.

**Low level:** a published, tested list of what runs and what doesn't (piece OS03).

### Moving from Windows

**High level:** a guided move that brings across files, browser bookmarks, email accounts and Wi-Fi settings, and can keep Windows alongside until the person is ready.

### Updates that can't break

**High level:** the system updates in one piece and can roll back to the last working version at start-up.

**Low level:** image-based ("atomic") updates are the proven method for this (piece OS05).

### Managing many machines

**High level:** offices and schools set up, update and secure many machines from one place, and sign in with central accounts, without paying for Windows Server or device-management licences.

**Low level:** open directory and sign-in services and open device management (piece OS06).

### Security

**High level:** full-disk encryption, secure boot support, automatic security updates and a firewall on by default.

## Licences

Linux and most of a distribution's parts are GPL or similar. That's fine: an operating system is a collection of separate programs, each under its own licence, and Spareware's rule of no GPL code applies to our own libraries, not to the separate programs a distribution ships. Each shipped part keeps its licence and credit.

## The hard parts

- Windows-only programs people depend on at work (Revit, some Adobe apps, some accounting software).
- Hardware: printers, scanners, Wi-Fi and fingerprint readers that only ship Windows drivers.
- Support: people need help switching, so the co-op needs a member support network.

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Base | Debian; Fedora; Ubuntu; NixOS; another | Research first (piece OS01) |
| Desktop | KDE Plasma; GNOME; another | KDE Plasma for a Windows-like feel, to confirm (piece OS02) |
| Updates | Image-based (atomic); traditional packages | Image-based |
| Pre-installed PCs | Partner with PC makers; download only | Download first, partners later |

The work board is [STATUS.md](STATUS.md).
