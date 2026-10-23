# Spare OS: everything to build

*Stage: Mapping. Written from [features.json](features.json), which is the source; the architecture is in [ARCHITECTURE.md](ARCHITECTURE.md) and the work board in [STATUS.md](STATUS.md).*

A free Linux-based operating system for ordinary PCs and laptops, with every Spare app built in, a one-step move from Windows and updates that can't leave a machine broken. Offices, schools and co-ops can run and manage many machines without paid licences. This map lists everything a full replacement for paying for Windows needs, top down: 21 areas, 193 things a person can do, and 450 buildable parts. It was drawn up by walking every journey: getting and installing it, the first start, moving from Windows, daily use, edge cases, families, offices and schools, the app store and the people who publish in it, payments, refunds and tips, safety and safe remote help, accessibility from the disk unlock screen on, offline use, many languages and countries, taking data away or going back to Windows, moderation of the store and forum, how the running costs are paid, and the law. Spare OS is a distribution: proven open programs do most of the work, each keeping its own licence, software that is not open source is shipped or offered only under a written policy, and the co-op builds the parts that make them one simple, safe system.

**Priority:** core means needed for a first usable release (117 features); next means soon after (57); later means after that (19). **Depends on** names kit libraries (kit-...), kit services (svc-...), shared engines (engine-...) or other parts by ID, including kit parts (KIT-...) mapped in the [kit map](../kit/FEATURES.md). **Piece** links a part to an open piece on a work board: Spare OS pieces (OS..) on [STATUS.md](STATUS.md), Spare Kit pieces (K..) on the [kit work board](../kit/STATUS.md).

## Areas

| ID | Area | Features | Parts |
| --- | --- | --- | --- |
| OS-A1 | [Base system and hardware](#os-a1-base-system-and-hardware) | 6 | 26 |
| OS-A2 | [Get, try and install](#os-a2-get-try-and-install) | 8 | 30 |
| OS-A3 | [First start](#os-a3-first-start) | 7 | 13 |
| OS-A4 | [Moving from Windows](#os-a4-moving-from-windows) | 9 | 21 |
| OS-A5 | [Desktop and windows](#os-a5-desktop-and-windows) | 13 | 30 |
| OS-A6 | [Settings and system tools](#os-a6-settings-and-system-tools) | 10 | 19 |
| OS-A7 | [Files and storage](#os-a7-files-and-storage) | 8 | 15 |
| OS-A8 | [Built-in apps](#os-a8-built-in-apps) | 9 | 18 |
| OS-A9 | [App store](#os-a9-app-store) | 10 | 27 |
| OS-A10 | [Running Windows and phone programs](#os-a10-running-windows-and-phone-programs) | 6 | 16 |
| OS-A11 | [Updates, backup and recovery](#os-a11-updates-backup-and-recovery) | 8 | 26 |
| OS-A12 | [Security and privacy](#os-a12-security-and-privacy) | 11 | 25 |
| OS-A13 | [Users, sign-in and family](#os-a13-users-sign-in-and-family) | 6 | 11 |
| OS-A14 | [Devices and connections](#os-a14-devices-and-connections) | 13 | 29 |
| OS-A15 | [Accessibility](#os-a15-accessibility) | 11 | 20 |
| OS-A16 | [Languages and regions](#os-a16-languages-and-regions) | 8 | 16 |
| OS-A17 | [Games and media](#os-a17-games-and-media) | 7 | 10 |
| OS-A18 | [AI, commands and automation](#os-a18-ai-commands-and-automation) | 6 | 19 |
| OS-A19 | [Managing many machines](#os-a19-managing-many-machines) | 14 | 30 |
| OS-A20 | [Help, support and community](#os-a20-help-support-and-community) | 12 | 26 |
| OS-A21 | [Legal duties and licences](#os-a21-legal-duties-and-licences) | 11 | 23 |

## OS-A1 Base system and hardware

The proven Linux base, the build line that turns it into signed Spare OS images, and the work that makes it run well on ordinary and old PCs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A1-F1 Run on a proven Linux base | Use a stable, well-supported Linux system underneath, without the co-op writing or maintaining a kernel. | core | OS-A1-F1-P1 to P6 |
| OS-A1-F2 Work on ordinary and old PCs | Install on most PCs and laptops from the last 15 years, including ones Windows 11 refuses to run on. | core | OS-A1-F2-P1 to P7 |
| OS-A1-F3 Get drivers without hunting | Have devices work straight away, and install any extra driver a device needs with one click. | core | OS-A1-F3-P1 to P5 |
| OS-A1-F4 Update device firmware | Update the PC's start-up firmware and device firmware from inside Spare OS, including Secure Boot certificate and revocation updates. | core | OS-A1-F4-P1 to P2 |
| OS-A1-F5 Get good battery life and sleep | Close the lid and open it again with everything working, and get battery life close to what Windows gives. | core | OS-A1-F5-P1 to P4 |
| OS-A1-F6 Run on ARM laptops and small boards | Use Spare OS on ARM laptops and Raspberry Pi-class computers as well as ordinary PCs. | later | OS-A1-F6-P1 to P2 |

### OS-A1 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A1-F1-P1 | **Base distribution decision**: Compare Debian, Fedora, Ubuntu, NixOS and others on stability, hardware support, update model and support length, and pick one. | research | medium |  | [OS01](STATUS.md) |
| OS-A1-F1-P2 | **System image build line**: Builds every Spare OS image from a declared recipe on the kit's release pipeline, so each release is the same on every machine. | service | large | OS-A1-F1-P1, KIT-A22-F2-P1 |  |
| OS-A1-F1-P3 | **Image recipe and package list**: The public, versioned list of every package, setting and default that goes into an image. | data | medium | OS-A1-F1-P1 |  |
| OS-A1-F1-P4 | **Reproducible build checks**: Rebuilds each release independently and compares the result, following the kit's reproducible build rules, so anyone can check an image matches its source. | service | medium | OS-A1-F1-P2, KIT-A22-F2-P3 |  |
| OS-A1-F1-P5 | **Signing keys and key ceremony**: Keeps the keys that sign images, packages and boot files offline under the kit's two-person key control, with a written, witnessed process for using and replacing them. | service | medium | OS-A1-F1-P2, KIT-A22-F2-P3 |  |
| OS-A1-F1-P6 | **Upstream first**: Sends our fixes, drivers and translations, including te reo Māori, to the open projects Spare OS ships, and keeps a short public list of the patches we still carry. | service | medium | OS-A1-F1-P3 |  |
| OS-A1-F2-P1 | **Hardware support targets**: Sets the minimum processor, memory and disk, with no TPM or new-processor requirement, and checks it against common old PCs. | research | medium | OS-A1-F1-P1 |  |
| OS-A1-F2-P2 | **Kernel track**: Chooses which Linux kernel series each release follows, balancing new hardware support against stability. | research | small | OS-A1-F1-P1 |  |
| OS-A1-F2-P3 | **Light mode for old machines**: A lighter desktop profile with fewer effects and background services for PCs with 4 GB of memory or less. | interface | medium | OS-A5-F1-P2 |  |
| OS-A1-F2-P4 | **Hardware test lab**: A pool of real PCs, laptops, printers and docks, many donated by members, that every release is tested on. | service | large | OS-A1-F1-P2, OS-A1-F2-P7 |  |
| OS-A1-F2-P5 | **Hardware compatibility list**: A public, searchable list of PC models and devices with how well each works, built from the lab and members' reports. | data | medium | OS-A1-F2-P4 |  |
| OS-A1-F2-P6 | **Old start-up modes decision**: Decides whether to support PCs that start in legacy BIOS mode or have Windows on an MBR disk, as many 2011 to 2013 PCs do and dual boot then needs, and how that fits signed start-up and image updates. | research | medium | OS-A1-F2-P1, OS-A11-F1-P1, OS-A12-F2-P1 |  |
| OS-A1-F2-P7 | **Wipe standard for donated PCs**: A written standard for wiping every donated PC before it enters the test lab or the refurbishment programme, with a record of each wipe, because donors' personal data on those disks stays the co-op's responsibility under privacy law (example: New Zealand's Privacy Act). | legal | small | OS-A11-F6-P4, KIT-A18-F14-P1 |  |
| OS-A1-F3-P1 | **Driver detection and offers**: Spots devices that need an extra driver or firmware file and offers the right one from the store. | library | medium | OS-A1-F1-P2 |  |
| OS-A1-F3-P2 | **Graphics drivers**: Ships or offers graphics drivers, including NVIDIA's, signed so they load with Secure Boot on, following the policy for software that is not open source. | integration | large | OS-A12-F2-P2, OS-A21-F1-P4 |  |
| OS-A1-F3-P3 | **Firmware files package**: Packages the firmware files Wi-Fi cards, Bluetooth and graphics chips need, keeping each maker's redistribution terms and following the policy for software that is not open source. | data | small | OS-A1-F1-P3, OS-A21-F1-P4 |  |
| OS-A1-F3-P4 | **Missing-driver report**: Tells the person which device has no driver, what that means, and with their consent reports it so the co-op can chase it. | interface | small | OS-A20-F4-P1 |  |
| OS-A1-F3-P5 | **Laptops with two graphics chips**: Picks the graphics chip for each app, uses the power-saving chip by default, and is tested on the lab's work and gaming laptops. | library | medium | OS-A1-F3-P2, OS-A1-F2-P4 |  |
| OS-A1-F4-P1 | **Firmware update service link**: Uses the open firmware update service that PC makers publish to, checking each update's signature, and is how a PC with Windows removed gets new Secure Boot certificates and revocation lists. | integration | medium | OS-A11-F1-P2 |  |
| OS-A1-F4-P2 | **Firmware updates screen**: Shows available firmware updates in plain words, with a warning to keep the laptop plugged in. | interface | small | OS-A6-F1-P1 |  |
| OS-A1-F5-P1 | **Power profiles**: Balanced, quiet and performance modes that adjust the processor, screen and background work. | library | medium | OS-A6-F1-P1 |  |
| OS-A1-F5-P2 | **Sleep and wake testing**: Tests sleep, hibernate and wake on the lab's laptops each release and records which models have problems. | research | medium | OS-A1-F2-P4 |  |
| OS-A1-F5-P3 | **Battery health and charge limit**: Shows battery wear and lets people cap charging at 80% on laptops that support it. | interface | small | OS-A6-F1-P1 |  |
| OS-A1-F5-P4 | **Hibernation with signed start-up and encryption**: Decides how hibernation works with Secure Boot and full-disk encryption, since the kernel lockdown that signed start-up turns on blocks hibernation unless the saved memory image is protected, and offers hibernate only where it works. | research | medium | OS-A12-F1-P1, OS-A12-F2-P1, OS-A1-F5-P2 |  |
| OS-A1-F6-P1 | **ARM support research**: Checks which ARM laptops and boards have enough open driver support to be worth a release. | research | medium | OS-A1-F1-P1 |  |
| OS-A1-F6-P2 | **ARM image builds**: Adds ARM images to the build line once a target is chosen. | service | medium | OS-A1-F1-P2, OS-A1-F6-P1 |  |

## OS-A2 Get, try and install

Every way a person gets Spare OS onto a PC: downloading it, making a USB stick or getting one ready-made, trying it without changes, installing beside or instead of Windows, installing many PCs at once, buying a PC with it already on, and later old Macs and Chromebooks, all usable with a screen reader from the first screen.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A2-F1 Download Spare OS | Download Spare OS free from the website, with a simple check that the file has not been tampered with. | core | OS-A2-F1-P1 to P3 |
| OS-A2-F2 Make a USB stick from Windows | Turn a USB stick into a Spare OS installer from a Windows PC with one small program. | core | OS-A2-F2-P1 to P5 |
| OS-A2-F3 Try it without installing | Start Spare OS from the USB stick and check Wi-Fi, sound, screen and printer before changing anything on the PC. | core | OS-A2-F3-P1 to P3 |
| OS-A2-F4 Install beside Windows or instead of it | Install Spare OS next to Windows, choosing at start-up, or replace Windows completely, without losing files. | core | OS-A2-F4-P1 to P8 |
| OS-A2-F5 Install many PCs at once | Install Spare OS on a room full of PCs over the network or from an answer file, without clicking through each one. | next | OS-A2-F5-P1 to P3 |
| OS-A2-F6 Buy a PC with Spare OS on it | Buy a new or refurbished PC with Spare OS already installed and supported. | later | OS-A2-F6-P1 to P4 |
| OS-A2-F7 Get Spare OS without making a USB stick | Get a ready-made Spare OS stick at cost, borrow one from a library or helper, or start the installer from Windows. | next | OS-A2-F7-P1 to P2 |
| OS-A2-F8 Put Spare OS on an old Mac or Chromebook | Install Spare OS on an Intel Mac that no longer gets macOS updates, or a school Chromebook past its update end date. | later | OS-A2-F8-P1 to P2 |

### OS-A2 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A2-F1-P1 | **Download website**: A plain page that picks the right image for the person's PC and explains each step in short sentences. | interface | medium | OS-A1-F1-P2 |  |
| OS-A2-F1-P2 | **Mirrors and peer-to-peer downloads**: A network of download mirrors, including member-run ones, plus torrent downloads, all checked against signed update metadata, with costs shown on the ledger. | service | medium | OS-A1-F1-P2, svc-ledger, OS-A11-F1-P7 |  |
| OS-A2-F1-P3 | **Signature check**: Checks the downloaded image's signature automatically in the USB maker, and explains how to check it by hand. | library | small | OS-A1-F1-P5 |  |
| OS-A2-F2-P1 | **USB maker for Windows**: Our own small, open program that downloads the image, checks it and writes it to a USB stick. | interface | medium | OS-A2-F1-P3, OS-A2-F2-P4 |  |
| OS-A2-F2-P2 | **USB maker for Mac and Linux**: The same USB maker built for macOS and Linux. | interface | small | OS-A2-F2-P1, OS-A2-F2-P4 |  |
| OS-A2-F2-P3 | **Boot menu help**: A list by PC maker of the key that opens the boot menu, plus how to turn off Windows fast start-up first. | data | small |  |  |
| OS-A2-F2-P4 | **Signed Windows and Mac builds**: Signs the USB maker and Windows-side helper with a co-op Authenticode certificate (Microsoft's Artifact Signing issues them to organisations in the countries it lists, such as New Zealand) and Mac builds with Apple Developer ID notarisation, through the kit's signed releases and held by the legal entity with costs on the ledger, so SmartScreen and Smart App Control let them run; the certificates belong in the kit for every Spare desktop app. | integration | medium | OS-A1-F1-P5, svc-ledger, KIT-A22-F2-P3, KIT-A14-F1-P1 |  |
| OS-A2-F2-P5 | **Accessible, translated Windows tools**: Makes the USB maker and the Windows-side helper work fully with Windows screen readers, the keyboard alone and high contrast, and translates them, since they are the first Spare programs a blind person runs. | interface | small | OS-A2-F2-P1, OS-A4-F4-P2, KIT-A19-F1-P1, KIT-A19-F2-P1 |  |
| OS-A2-F3-P1 | **Live session**: A full Spare OS session that runs from the USB stick and leaves the PC's disk untouched. | library | medium | OS-A1-F1-P2 |  |
| OS-A2-F3-P2 | **Hardware check report**: Shows which devices work and which need attention, and with consent adds the result to the compatibility list. | interface | medium | OS-A1-F2-P5 |  |
| OS-A2-F3-P3 | **Accessibility and language before install**: Screen reader, zoom, high contrast and language choice can be turned on from the first screen of the USB session and stay on through the installer. | interface | medium | OS-A2-F3-P1, OS-A15-F1-P1, OS-A15-F2-P1, OS-A15-F3-P1, OS-A16-F1-P1 |  |
| OS-A2-F4-P1 | **Installer framework decision**: Compares existing open installers on safety, disk handling and how easily they can be made simple. | research | small | OS-A1-F1-P1 |  |
| OS-A2-F4-P2 | **Installer app**: Guides the person through install in a few screens, with safe defaults and a clear summary before anything changes. | interface | large | OS-A2-F4-P1, OS-A18-F1-P1, OS-A2-F3-P3, OS-A21-F10-P1 |  |
| OS-A2-F4-P3 | **Safe Windows partition shrinking**: Shrinks the Windows partition from inside Windows through the Windows-side helper, because a BitLocker volume cannot be resized from Linux, or from the installer only when BitLocker is off, after making sure the person has the recovery key; Windows 11 turns device encryption on by default on many PCs. | library | medium | OS-A2-F4-P2, OS-A4-F4-P2 |  |
| OS-A2-F4-P4 | **Start-up chooser**: Adds Spare OS and Windows to a start-up menu that picks the last used system by default. | library | medium | OS-A2-F4-P2 |  |
| OS-A2-F4-P5 | **Encryption at install**: Turns on full-disk encryption during install by default, with the recovery key shown and saved. | library | medium | OS-A12-F1-P1 |  |
| OS-A2-F4-P6 | **Install failure recovery**: If install stops part way, leaves Windows untouched and saves a log the person can send for help. | interface | small | OS-A2-F4-P2, OS-A20-F4-P1 |  |
| OS-A2-F4-P7 | **Accessible start-up menu**: The start-up menu plays a sound when it appears, can speak or sound out each choice, uses large clear text, and offers a choice that starts with the screen reader on. | library | medium | OS-A2-F4-P4, OS-A15-F7-P1 |  |
| OS-A2-F4-P8 | **Hidden disk check**: Spots Intel RST or VMD storage modes that hide the disk from the installer, and explains the fix in plain steps that keep Windows starting. | library | small | OS-A2-F4-P2 |  |
| OS-A2-F5-P1 | **Answer file format**: A documented file that answers every installer question in advance. | data | medium | OS-A2-F4-P2 |  |
| OS-A2-F5-P2 | **Network install**: Starts install over the office or school network, pulling the image from a local server. | service | medium | OS-A19-F2-P1 |  |
| OS-A2-F5-P3 | **Factory install mode**: Installs now and leaves language, account and Wi-Fi for the new owner's first start, for PC sellers and refurbishers. | interface | medium | OS-A2-F4-P2 |  |
| OS-A2-F6-P1 | **PC maker partnership terms**: The agreement a PC maker or refurbisher signs to sell PCs with Spare OS, covering support and naming. | legal | medium | OS-A21-F7-P2, OS-A21-F9-P1, KIT-A14-F1-P1 |  |
| OS-A2-F6-P2 | **Refurbished PC programme**: How local repairers take old Windows 10 PCs, wipe them to the written standard, refurbish them with Spare OS and sell them on at a fair price. | research | medium | OS-A2-F5-P3, OS-A11-F6-P4, OS-A1-F2-P7 |  |
| OS-A2-F6-P3 | **Works with Spare OS badge**: A badge for hardware that passes the lab's tests, listed on the compatibility list. | data | small | OS-A1-F2-P5 |  |
| OS-A2-F6-P4 | **Pre-installed image and recovery**: The factory image partners install, with a recovery option that returns the PC to as-new using the same reset as the system's own repair tools. | service | medium | OS-A2-F5-P3, OS-A11-F6-P1, OS-A11-F6-P2 |  |
| OS-A2-F7-P1 | **Ready-made sticks and loans**: Ready-made Spare OS sticks sold at cost, and loan sticks held by libraries and member helpers, with costs on the ledger. | service | small | svc-payments, svc-ledger, OS-A20-F2-P1 |  |
| OS-A2-F7-P2 | **Start the installer from Windows**: Researches starting the Spare OS installer straight from Windows without a USB stick, and how that stays safe with BitLocker and Secure Boot. | research | medium | OS-A4-F4-P2, OS-A2-F4-P1, OS-A2-F4-P3 |  |
| OS-A2-F8-P1 | **Old Intel Macs**: Checks which Intel Macs that no longer get macOS updates run Spare OS well, including start-up, Wi-Fi, keyboards and the security chip on later models. | research | medium | OS-A1-F2-P1, OS-A1-F2-P4 |  |
| OS-A2-F8-P2 | **Chromebooks past their update date**: Checks how school Chromebooks past their update end date can start Spare OS, what firmware change each needs and the risk of each step, before offering it. | research | medium | OS-A1-F2-P1, OS-A1-F2-P4 |  |

## OS-A3 First start

The first few minutes after install or on a new PC: language, keyboard, Wi-Fi, an account, privacy choices and a short welcome, all usable by everyone from the first screen.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A3-F1 Set up in a few minutes | Choose language, keyboard, time zone, Wi-Fi and a name in a few plain steps. | core | OS-A3-F1-P1 to P3 |
| OS-A3-F2 Make an account without going online | Create a local account on the PC with no internet and no online login required. | core | OS-A3-F2-P1 to P2 |
| OS-A3-F3 Connect a Spareware login | Optionally connect a Spareware login so Spare apps, files and chosen settings follow the person to any PC. | core | OS-A3-F3-P1 to P2 |
| OS-A3-F4 Make clear privacy choices | See every data-sharing option in plain words and choose, with all sharing off by default. | core | OS-A3-F4-P1 to P2 |
| OS-A3-F5 Take a short welcome tour | Learn where things are, especially for someone coming from Windows. | next | OS-A3-F5-P1 to P2 |
| OS-A3-F6 Pick a layout | Choose a desktop layout that suits them: a familiar taskbar layout, a dock layout, or a simple large-button layout. | next | OS-A3-F6-P1 |
| OS-A3-F7 Restore from another PC or backup | Start a new PC by restoring files, apps and settings from a backup or another Spare OS PC. | next | OS-A3-F7-P1 |

### OS-A3 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A3-F1-P1 | **First-start setup app**: The step-by-step screens that run on first start, each with a clear back button and skip where it is safe. | interface | large | OS-A16-F1-P1, OS-A6-F1-P1 |  |
| OS-A3-F1-P2 | **Time zone and region guess**: Suggests time zone and regional formats from the chosen language and keyboard, with an optional online check through the private location lookup. | library | small | OS-A16-F4-P1, OS-A14-F11-P1 |  |
| OS-A3-F1-P3 | **Accessibility from the first screen**: Screen reader, magnifier and high contrast can be turned on from the very first screen with a keyboard shortcut or by voice, carrying on any choices made in the USB session. | interface | medium | OS-A15-F1-P1, OS-A15-F2-P1, OS-A2-F3-P3 |  |
| OS-A3-F2-P1 | **Local account creation**: Makes the first user an administrator with a password or PIN, entirely on the device. | library | small | OS-A13-F1-P1 |  |
| OS-A3-F2-P2 | **Age range at each account's set-up**: Asks the age range at the set-up of every account on the PC where the law requires it, not only the first user's, stores only the range and keeps it on the device. | library | small | OS-A21-F4-P1, OS-A13-F1-P1 |  |
| OS-A3-F3-P1 | **Spareware login at setup**: Signs in with a passkey or password through the accounts service, and links it to the local account. | integration | medium | svc-accounts |  |
| OS-A3-F3-P2 | **Settings sync choice**: Lets the person choose which settings sync between their PCs, all off until they turn them on, with Wi-Fi passwords and other secrets left out until end-to-end encrypted sync (OS-A12-F11) is ready. | library | medium | kit-sync, OS-A6-F1-P1 |  |
| OS-A3-F4-P1 | **Privacy choices screen**: One screen listing each choice, what is sent, to whom and why, with nothing pre-ticked. | interface | small | OS-A12-F5-P1 |  |
| OS-A3-F4-P2 | **Spare OS privacy notice**: Adds what Spare OS itself collects, in plain words, to the kit's collection notices, checked against privacy law in each country, starting with New Zealand. | legal | small | KIT-A18-F5-P1, OS-A21-F6-P1 |  |
| OS-A3-F5-P1 | **Welcome tour**: A short, skippable tour of the start menu, files, settings, store and help. | interface | medium | OS-A5-F1-P2 |  |
| OS-A3-F5-P2 | **Where did it go guide**: A searchable list that maps common Windows tasks and places to where they are in Spare OS. | data | medium | OS-A20-F1-P1 |  |
| OS-A3-F6-P1 | **Layout presets**: Our own designed layouts built on the desktop's settings, with no copied visuals from other systems. | data | medium | OS-A5-F1-P2 |  |
| OS-A3-F7-P1 | **Restore during setup**: Offers restore from a backup drive, Spareware storage or a nearby PC as one of the first-start steps. | interface | medium | OS-A11-F5-P1 |  |

## OS-A4 Moving from Windows

The guided move: files, browser data, email, Wi-Fi and personal settings brought across, a report of which programs have replacements, and Windows kept alongside until the person is ready.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A4-F1 Bring my files across | Copy Documents, Pictures, Desktop, Music and Videos from Windows on the same PC, another PC or a backup drive. | core | OS-A4-F1-P1 to P5 |
| OS-A4-F2 Bring my browser across | Bring bookmarks, history and saved passwords from Edge, Chrome or Firefox into the Spare OS browser. | core | OS-A4-F2-P1 |
| OS-A4-F3 Bring my email, contacts and calendars | Keep using the same email accounts, contacts and calendars without setting them up again. | core | OS-A4-F3-P1 to P2 |
| OS-A4-F4 Bring my Wi-Fi and network settings | Connect to known Wi-Fi networks, printers and network drives straight away after the move. | core | OS-A4-F4-P1 to P2 |
| OS-A4-F5 Find replacements for my programs | See each Windows program they had, and whether it runs on Spare OS, has a Spare app replacement, or has an open alternative. | core | OS-A4-F5-P1 to P4 |
| OS-A4-F6 Bring my look and preferences | Keep wallpaper, mouse and keyboard settings, region formats and default printer as they were. | next | OS-A4-F6-P1 |
| OS-A4-F7 Keep Windows alongside, then remove it | Keep Windows to start into while getting used to Spare OS, then safely reclaim its space later. | core | OS-A4-F7-P1 to P3 |
| OS-A4-F8 Open Windows drives and files | Open NTFS and exFAT drives, BitLocker-locked drives with the key, and Windows shortcuts and file types. | core | OS-A4-F8-P1 to P2 |
| OS-A4-F9 Move with help from a person | Book a trained member helper to do the move with them, in person or remotely. | next | OS-A4-F9-P1 |

### OS-A4 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A4-F1-P1 | **Move-across design**: Decides what is brought across and how, and how Windows is kept alongside. | research | medium |  | [OS04](STATUS.md) |
| OS-A4-F1-P2 | **Windows folder reader**: Finds each Windows user's folders on an NTFS partition or drive and reads them safely without changing them. | library | medium | OS-A4-F1-P1, OS-A4-F8-P1 |  |
| OS-A4-F1-P3 | **Move-across app**: One guided app that shows what was found, lets the person tick what to bring, copies it and reports what moved. | interface | large | OS-A4-F1-P1, OS-A18-F1-P1 |  |
| OS-A4-F1-P4 | **PC-to-PC transfer**: Moves files and settings from an old PC to a new one over the local network or a cable. | library | medium | OS-A4-F1-P3 |  |
| OS-A4-F1-P5 | **Cloud-only files check**: Spots files that only exist as cloud placeholders on Windows and helps the person download them before the move. | integration | medium | OS-A4-F1-P2, OS-A4-F4-P2 |  |
| OS-A4-F2-P1 | **Browser profile importer**: Imports bookmarks and history from browser profile files, and passwords only through the browser's own export, never by breaking its encryption. | integration | medium | OS-A4-F1-P2, OS-A8-F2-P1 |  |
| OS-A4-F3-P1 | **Account finder**: Lists the email, calendar and contact accounts the person used on Windows so they can sign in to them again in one place. | integration | medium | OS-A4-F4-P2, OS-A8-F6-P2 |  |
| OS-A4-F3-P2 | **Outlook data file import**: Reads Outlook data files from Microsoft's published file format specification and imports mail, contacts and calendars; the reader belongs in the kit or an engine so Spare Office's importer uses the same one. | integration | medium | OS-A4-F3-P1, OS-A8-F6-P1 |  |
| OS-A4-F4-P1 | **Wi-Fi and printer import**: Imports Wi-Fi networks, printers and mapped network drives from the Windows-side helper's bundle. | integration | small | OS-A4-F4-P2, OS-A14-F1-P1 |  |
| OS-A4-F4-P2 | **Windows-side helper**: A small open program the person runs on Windows before moving, which packs up settings, Wi-Fi networks, printers and the program list using Windows' own export tools. | interface | medium | OS-A4-F1-P1, OS-A2-F2-P4 |  |
| OS-A4-F5-P1 | **Installed program scan**: Reads the list of installed programs from the helper's bundle or the Windows partition. | library | medium | OS-A4-F4-P2 |  |
| OS-A4-F5-P2 | **Program equivalents list**: Open data mapping common Windows programs to Spare apps, open alternatives, or how well they run in the compatibility layer. | data | large | OS-A10-F1-P2 |  |
| OS-A4-F5-P3 | **Replacement report**: A plain report: ready to use, will run, needs a replacement, or blocks switching for now, with links to install each. | interface | medium | OS-A4-F5-P2, OS-A9-F1-P1 |  |
| OS-A4-F5-P4 | **Paid licence warning**: Warns the person to deactivate paid programs such as design, accounting and office software, and to save their product keys, before Windows is removed, since many licences limit activations. | data | small | OS-A4-F5-P1 |  |
| OS-A4-F6-P1 | **Settings translator**: Turns Windows settings from the helper's bundle into the matching Spare OS settings, and lists any it cannot match. | library | medium | OS-A4-F4-P2, OS-A6-F1-P1 |  |
| OS-A4-F7-P1 | **Dual start manager**: The settings page for the start-up chooser, where the person picks the default system and how long the menu waits. | interface | small | OS-A2-F4-P4 |  |
| OS-A4-F7-P2 | **Reclaim Windows space**: Checks nothing unmoved remains and paid licences are released, offers a backup, then removes Windows and grows Spare OS into the space. | library | medium | OS-A2-F4-P3, OS-A11-F4-P1, OS-A4-F5-P4 |  |
| OS-A4-F7-P3 | **Watch for Windows boot blocks**: Tests each Windows update that changes Secure Boot rules on the lab's dual-boot PCs, as Microsoft's August 2024 SBAT update did when it stopped many Linux systems starting, and ships fixes before members are hit. | service | small | OS-A12-F2-P1, OS-A1-F2-P4, OS-A2-F4-P4 |  |
| OS-A4-F8-P1 | **NTFS and exFAT read and write**: Uses the kernel's open NTFS and exFAT support so Windows drives open like any other. | integration | small | OS-A1-F1-P1 |  |
| OS-A4-F8-P2 | **BitLocker drive unlock**: Unlocks BitLocker drives with the person's password or recovery key through an existing open tool shipped as a separate program, once an owner has cleared it. | integration | medium | OS-A4-F8-P1 |  |
| OS-A4-F9-P1 | **Helper booking**: Links the move-across app to the member support network, so the helper sees only what the person shares. | integration | small | OS-A20-F2-P1 |  |

## OS-A5 Desktop and windows

The desktop a person sees every day: taskbar, start menu, search, window arranging, notifications, quick settings, screenshots, themes, sounds, touch screens, the touchpad and the lock screen. It should feel familiar to Windows users while being our own design.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A5-F1 Use a familiar desktop | Find a taskbar, start menu, tray and clock where a Windows user expects them, from the first minute. | core | OS-A5-F1-P1 to P5 |
| OS-A5-F2 Find anything from the start menu | Type in one box to find apps, files, settings, quick sums and answers. | core | OS-A5-F2-P1 to P2 |
| OS-A5-F3 Arrange windows | Snap windows to halves, quarters and grids, maximise, minimise and switch between them by keyboard or mouse. | core | OS-A5-F3-P1 to P3 |
| OS-A5-F4 Use several desktops and screens | Work across virtual desktops and several monitors, each with its own scaling. | core | OS-A5-F4-P1 to P2 |
| OS-A5-F5 See and act on notifications | See notifications in one place, reply or act from them, and silence them while working. | core | OS-A5-F5-P1 to P3 |
| OS-A5-F6 Change common settings quickly | Turn Wi-Fi, Bluetooth, volume, brightness and power mode up or down from one small panel. | core | OS-A5-F6-P1 |
| OS-A5-F7 Copy and paste with history | Paste from recent copies, and insert emoji and special characters. | core | OS-A5-F7-P1 to P2 |
| OS-A5-F8 Take screenshots and record the screen | Capture the whole screen, a window or an area, mark it up, and record the screen as video. | core | OS-A5-F8-P1 to P3 |
| OS-A5-F9 Make it look my way | Choose dark or light mode, accent colour, wallpaper, text size and night light. | core | OS-A5-F9-P1 to P3 |
| OS-A5-F10 Use touch screens and the on-screen keyboard | Use a touch laptop or tablet with gestures and an on-screen keyboard. | core | OS-A5-F10-P1 to P2 |
| OS-A5-F11 Lock the screen and switch users | Lock the PC, sign back in, and let another person sign in without closing the first person's work. | core | OS-A5-F11-P1 |
| OS-A5-F12 Glance at widgets | Glance at weather, calendar and chosen news, with no ads and only from sources the person picks. | later | OS-A5-F12-P1 to P2 |
| OS-A5-F13 Use the touchpad | Scroll, swipe between desktops and apps, and change touchpad settings such as tap to click and speed. | core | OS-A5-F13-P1 |

### OS-A5 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A5-F1-P1 | **Desktop decision**: Compare KDE Plasma, GNOME and others on how familiar they can be made, stability and accessibility, and pick one. | research | medium | OS-A1-F1-P1 | [OS02](STATUS.md) |
| OS-A5-F1-P2 | **Spare OS layout and theme**: Our own default layout, colours and visual style following the Spareware design guide, set up on the chosen desktop. | interface | large | OS-A5-F1-P1 |  |
| OS-A5-F1-P3 | **Spare OS icon set**: Our own icons for apps, files and the system, drawn from scratch with no copied artwork. | interface | large | OS-A5-F1-P2 |  |
| OS-A5-F1-P4 | **Default fonts**: Sets open interface fonts, and installs engine-text's shared metric-matched fonts system-wide so documents keep their layout. | data | small | engine-text, OS-A16-F3-P1 |  |
| OS-A5-F1-P5 | **Spare OS sounds and pointers**: Our own system sounds and pointer set, made from scratch with no copied sounds or artwork. | data | medium | OS-A5-F1-P2 |  |
| OS-A5-F2-P1 | **Desktop search service**: Searches apps, settings, files and commands as the person types, from a local index. | library | large | OS-A7-F3-P1, OS-A18-F1-P2 |  |
| OS-A5-F2-P2 | **Search results panel**: Shows grouped results in the start menu with keyboard control and clear actions. | interface | medium | OS-A5-F2-P1 |  |
| OS-A5-F3-P1 | **Window snapping layouts**: Drag-to-edge and keyboard snapping into simple layouts, remembered per screen. | interface | medium | OS-A5-F1-P1 |  |
| OS-A5-F3-P2 | **Window switcher**: Switch between open windows with a keyboard shortcut and a clear preview. | interface | small | OS-A5-F1-P1 |  |
| OS-A5-F3-P3 | **Familiar keyboard shortcuts**: The shortcuts Windows users already know for common actions, set as defaults and listed in help. | data | small | OS-A5-F1-P1 |  |
| OS-A5-F4-P1 | **Virtual desktops**: Create, switch and move windows between desktops. | interface | small | OS-A5-F1-P1 |  |
| OS-A5-F4-P2 | **Several monitors and mixed scaling**: Remembers each monitor's layout and sharpness setting, and moves windows back when a screen is unplugged. | library | medium | OS-A5-F1-P1 |  |
| OS-A5-F5-P1 | **Notification centre**: Lists notifications by app with actions, and keeps a short history. | interface | medium | OS-A5-F1-P1 |  |
| OS-A5-F5-P2 | **Spare app notifications link**: Delivers notifications from Spare apps and the messaging service to the desktop, respecting each person's choices. | integration | small | svc-messaging, OS-A5-F5-P1 |  |
| OS-A5-F5-P3 | **Focus and do not disturb**: Quiet hours and focus sessions that hold back notifications except chosen ones. | interface | small | OS-A5-F5-P1 |  |
| OS-A5-F6-P1 | **Quick settings panel**: A panel from the tray with the most used switches and sliders. | interface | medium | OS-A6-F1-P1 |  |
| OS-A5-F7-P1 | **Clipboard history**: Keeps recent copies on the device only, skips password fields, and can be cleared or turned off. | library | small | OS-A5-F1-P1 |  |
| OS-A5-F7-P2 | **Emoji and symbol picker**: A keyboard shortcut that opens emoji, symbols and macron letters to insert anywhere. | interface | small | OS-A16-F3-P1 |  |
| OS-A5-F8-P1 | **Screenshot tool**: Captures, marks up with arrows, text and blur, and copies or saves. | interface | medium | engine-draw2d |  |
| OS-A5-F8-P2 | **Screen recorder**: Records the screen or a window with sound to a common video format. | interface | medium | engine-media, OS-A14-F8-P1 |  |
| OS-A5-F8-P3 | **Copy text from the screen**: Picks text out of a screenshot or image so it can be copied. | library | medium | OS-A18-F4-P1 |  |
| OS-A5-F9-P1 | **Themes and dark mode**: Light, dark and high-contrast themes that every built-in app follows. | interface | medium | OS-A5-F1-P2 |  |
| OS-A5-F9-P2 | **Wallpaper collection**: Wallpapers made and given by members and photographers, each with its licence and credit. | data | small |  |  |
| OS-A5-F9-P3 | **Night light**: Warms the screen colour in the evening on a schedule. | library | small |  |  |
| OS-A5-F10-P1 | **Touch gestures and tablet layout**: Larger targets and swipe gestures when a keyboard is folded away or detached. | interface | medium | OS-A5-F1-P1 |  |
| OS-A5-F10-P2 | **On-screen keyboard**: A full on-screen keyboard that appears in text boxes on touch screens, with every installed layout. | interface | medium | OS-A16-F2-P1 |  |
| OS-A5-F11-P1 | **Lock and sign-in screen**: The sign-in and lock screens, with user switching and accessibility options visible. | interface | medium | OS-A13-F1-P1 |  |
| OS-A5-F12-P1 | **Widgets panel**: A panel of small cards the person chooses, drawn from local apps and open data. | interface | medium | OS-A5-F1-P2 |  |
| OS-A5-F12-P2 | **Open weather and news sources**: Finds weather and news sources whose terms allow use, starting with New Zealand ones. | research | small |  |  |
| OS-A5-F13-P1 | **Touchpad gestures and settings**: Two- and three-finger gestures for scrolling and switching desktops and apps, plus settings for tapping, speed, palm rejection and turning the touchpad off while typing. | interface | medium | OS-A5-F1-P1, OS-A6-F1-P1 |  |

## OS-A6 Settings and system tools

One place to change every setting, plus the everyday system tools: seeing what's running, freeing space, default apps, system information, devices and logs, disks, the command line, and tools for developers.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A6-F1 Change any setting in one place | Find and change every setting in one settings app, searching in plain words. | core | OS-A6-F1-P1 to P2 |
| OS-A6-F2 See what's running and stop it | See which apps use the processor, memory, disk and network, end a stuck app, and choose what starts with the PC. | core | OS-A6-F2-P1 to P2 |
| OS-A6-F3 Free up disk space | See what fills the disk and clear caches, old system versions and leftovers safely. | core | OS-A6-F3-P1 |
| OS-A6-F4 Choose default apps | Pick which app opens each kind of file and link. | core | OS-A6-F4-P1 |
| OS-A6-F5 See system information | See the PC model, parts, Spare OS version and support end date, and copy them for a help request. | core | OS-A6-F5-P1 to P2 |
| OS-A6-F6 Set date, time and power buttons | Keep the clock right, and choose what the power button and lid do. | core | OS-A6-F6-P1 to P2 |
| OS-A6-F7 Manage disks | Format, partition, check the health of and mount drives. | next | OS-A6-F7-P1 to P2 |
| OS-A6-F8 Use a command line | Open a terminal and run any system action as a typed command. | next | OS-A6-F8-P1 to P2 |
| OS-A6-F9 Run virtual machines and developer tools | Run virtual machines and containers, and set up programming tools in one click. | next | OS-A6-F9-P1 to P3 |
| OS-A6-F10 See devices and system logs | See every device with its driver and whether it works, turn a device off, and read the system log when helping someone. | core | OS-A6-F10-P1 to P2 |

### OS-A6 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A6-F1-P1 | **Settings app**: One app with every setting grouped by plain-word topic, where each change is a system command that can be undone. | interface | large | OS-A18-F1-P1, OS-A5-F1-P2 |  |
| OS-A6-F1-P2 | **Settings search**: Finds settings from everyday words, including the names Windows uses for them. | library | medium | OS-A6-F1-P1, OS-A18-F1-P2 |  |
| OS-A6-F2-P1 | **Task manager**: Lists apps and background services with their use of the processor, memory, disk, network and battery, and ends them safely. | interface | medium | OS-A18-F1-P1 |  |
| OS-A6-F2-P2 | **Start-up apps control**: Lists everything that starts automatically and lets the person turn each off. | library | small | OS-A6-F2-P1 |  |
| OS-A6-F3-P1 | **Storage overview and clean-up**: Shows space by type and offers safe clean-ups, never deleting the person's own files without asking. | interface | medium | OS-A11-F1-P3 |  |
| OS-A6-F4-P1 | **Default apps and file links**: Stores defaults per file type and link type, and asks once when a new app could take over. | library | small | OS-A6-F1-P1 |  |
| OS-A6-F5-P1 | **About this PC**: One page with model, processor, memory, graphics, storage, version and support dates. | interface | small | OS-A6-F1-P1, OS-A11-F7-P1 |  |
| OS-A6-F5-P2 | **Copyable system report**: A text report of the PC's set-up that the person can read before sharing for help. | library | small | OS-A20-F4-P1 |  |
| OS-A6-F6-P1 | **Date, time and network time**: Sets the clock automatically from network time servers, with time zone changes for travel. | library | small |  |  |
| OS-A6-F6-P2 | **Power button and lid settings**: Chooses sleep, hibernate, shut down or nothing for the power button and lid, offering hibernate only where it works with encryption and signed start-up. | interface | small | OS-A1-F5-P1, OS-A1-F5-P4 |  |
| OS-A6-F7-P1 | **Disk utility**: Formats, partitions, encrypts and checks drives, with plain warnings before anything is erased. | interface | medium | OS-A18-F1-P1 |  |
| OS-A6-F7-P2 | **Drive health warnings**: Reads drive health data and warns early when a drive looks likely to fail. | library | small | OS-A5-F5-P1 |  |
| OS-A6-F8-P1 | **Terminal app**: A tabbed terminal for power users and for following help guides. | interface | small |  |  |
| OS-A6-F8-P2 | **Spare command-line tool**: One command-line tool that runs every system command, with help text for each. | library | medium | OS-A18-F1-P1, OS-A18-F1-P2 |  |
| OS-A6-F9-P1 | **Virtual machine manager**: Creates and runs virtual machines from an open virtualisation stack with a simple interface. | integration | medium | OS-A18-F1-P1 |  |
| OS-A6-F9-P2 | **Container tools**: Open container tools set up so developers can run containers without extra steps. | integration | medium |  |  |
| OS-A6-F9-P3 | **Developer set-up packs**: One-click packs that install common programming languages and tools from the store. | data | small | OS-A9-F1-P3 |  |
| OS-A6-F10-P1 | **Devices page**: Lists each device, its driver and whether it works, links to the missing-driver report, and can turn a device off. | interface | medium | OS-A6-F1-P1, OS-A1-F3-P1, OS-A18-F1-P1 |  |
| OS-A6-F10-P2 | **System log viewer**: Shows system logs in plain words with filters by time, app and severity, for helpers and IT staff. | interface | small | OS-A18-F1-P1 |  |

## OS-A7 Files and storage

Finding, opening and managing files: the file manager, archives, fast search, network folders, Spareware storage and other clouds, USB drives and phones, earlier versions and tags.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A7-F1 Browse and manage files | Browse, copy, move, rename and delete files in a file manager with tabs, previews and a recycle bin. | core | OS-A7-F1-P1 to P3 |
| OS-A7-F2 Open zip and other archives | Open, make and extract zip, 7z, tar and other archive files from the file manager. | core | OS-A7-F2-P1 |
| OS-A7-F3 Find files fast | Find any file by name or by words inside it within a second. | core | OS-A7-F3-P1 to P2 |
| OS-A7-F4 Use shared and network folders | Open office and home network folders and drives, and share a folder with others on the network. | core | OS-A7-F4-P1 to P2 |
| OS-A7-F5 Keep files in Spareware storage | Keep chosen folders in the person's Spareware storage, synced across PCs, with files downloaded only when opened. | next | OS-A7-F5-P1 to P3 |
| OS-A7-F6 Use USB drives, phones and cameras | Plug in a USB drive, phone or camera and see its files, and remove it safely. | core | OS-A7-F6-P1 to P2 |
| OS-A7-F7 Get back an earlier version of a file | Restore an earlier version of a file or a deleted folder from automatic snapshots. | core | OS-A7-F7-P1 |
| OS-A7-F8 Tag and organise files | Add tags and favourites, and see recently used files in every app's open dialog. | later | OS-A7-F8-P1 |

### OS-A7 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A7-F1-P1 | **File manager**: A file manager with tabs, a side bar of places, list and grid views, and copy progress that can be paused. | interface | large | OS-A5-F1-P2, OS-A18-F1-P1 |  |
| OS-A7-F1-P2 | **Thumbnails and previews**: Shows thumbnails and quick previews of pictures, video, PDFs and office files. | library | medium | engine-pdf, engine-media |  |
| OS-A7-F1-P3 | **Recycle bin with restore**: Deleted files go to a bin first and can be put back where they came from. | library | small |  |  |
| OS-A7-F2-P1 | **Archive support**: Open archive tools wired into the file manager's right-click menu. | integration | small | OS-A7-F1-P1 |  |
| OS-A7-F3-P1 | **File index**: A local index of file names and contents that runs when the PC is idle, skips private folders the person picks, and never leaves the device. | library | large | kit-store |  |
| OS-A7-F3-P2 | **Content readers**: Pull text out of documents, PDFs and other common formats for the index. | library | medium | OS-A7-F3-P1, engine-pdf |  |
| OS-A7-F4-P1 | **Network folders**: Opens Windows-style network shares and home network drives, and remembers them as mapped places. | integration | medium | OS-A7-F1-P1 |  |
| OS-A7-F4-P2 | **Share a folder**: Shares a folder on the local network from its right-click menu with simple permissions. | interface | small | OS-A7-F4-P1 |  |
| OS-A7-F5-P1 | **Desktop sync client**: Syncs chosen folders with the person's Spareware storage on the kit, handling conflicts and offline changes; one client for every Spare program, so it belongs in the kit and Spare OS never depends on Spare Office's drive. | integration | large | kit-sync, kit-store, svc-accounts, KIT-A15-F7-P2 |  |
| OS-A7-F5-P2 | **Files on demand**: Shows every cloud file but downloads it only when opened, and frees space for files not used lately, as part of the same shared sync client. | library | large | OS-A7-F5-P1 |  |
| OS-A7-F5-P3 | **Other cloud accounts**: Connects other storage over open protocols, and other providers through their published interfaces where terms allow. | integration | medium | OS-A8-F6-P2 |  |
| OS-A7-F6-P1 | **Removable drives**: Opens new drives, warns before unplugging during a copy, and handles common drive formats. | library | small | OS-A4-F8-P1 |  |
| OS-A7-F6-P2 | **Photo import from phones and cameras**: Imports photos and videos from phones and cameras plugged in by USB into dated folders, skipping ones already imported, with no phone link needed. | integration | medium | OS-A7-F6-P1 |  |
| OS-A7-F7-P1 | **File version history**: Takes automatic local snapshots of the person's folders and shows earlier versions from the right-click menu, so files come back after a mistake or ransomware. | library | medium | OS-A11-F4-P1 |  |
| OS-A7-F8-P1 | **Tags and favourites**: Stores tags with the files and shows them in the file manager and search. | library | small | OS-A7-F1-P1, OS-A7-F3-P1 |  |

## OS-A8 Built-in apps

What is on the PC from the first start: every Spare app with one login, a private web browser that warns about scam sites, the small everyday apps, media, mail and contacts shared by all apps, maps, meetings, fonts and disc burning.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A8-F1 Use every Spare app straight away | Open Spare Office, Spare Studio, Spare Construct, Spare Social and the marketplace apps from the start menu, already signed in. | core | OS-A8-F1-P1 to P4 |
| OS-A8-F2 Browse the web | Use a fast, private web browser that keeps up with security fixes. | core | OS-A8-F2-P1 to P3 |
| OS-A8-F3 Use the small everyday apps | Use a text editor, calculator, clocks and alarms, image viewer, PDF viewer, camera and sound recorder. | core | OS-A8-F3-P1 to P3 |
| OS-A8-F4 Play music and video | Play common music and video files, and discs where the law allows it. | core | OS-A8-F4-P1 |
| OS-A8-F5 Make quick photo edits | Crop, rotate, straighten and adjust photos, and draw simple pictures, without opening a big app. | next | OS-A8-F5-P1 |
| OS-A8-F6 Share contacts, calendars and accounts across apps | Add mail, calendar and contact accounts once and have every app use them. | core | OS-A8-F6-P1 to P2 |
| OS-A8-F7 See maps and directions | Search places and get directions on open map data. | later | OS-A8-F7-P1 |
| OS-A8-F8 Join video calls and share the screen | Join calls in Spare apps or any other service's web or desktop app, with camera, microphone and screen sharing working. | core | OS-A8-F8-P1 |
| OS-A8-F9 Install fonts and burn discs | Install a font by double-clicking it, and burn files, music or photos to CDs and DVDs on PCs that have a drive. | next | OS-A8-F9-P1 to P2 |

### OS-A8 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A8-F1-P1 | **Spare apps bundle**: Decides which Spare apps ship in the image and which are one click away in the store, keeping the download size sensible. | data | medium | OS-A9-F1-P2 |  |
| OS-A8-F1-P2 | **One login for all Spare apps**: Shares the Spareware login from the system with every Spare app, so the person signs in once. | integration | medium | svc-accounts, OS-A3-F3-P1 |  |
| OS-A8-F1-P3 | **Spare apps on the system update channel**: Updates Spare apps through the same channel and schedule as the system. | integration | medium | OS-A11-F1-P2 |  |
| OS-A8-F1-P4 | **Desktop entries for web-based Spare apps**: Gives the marketplace apps that run in a browser their own start menu entries, windows and notifications. | interface | medium | OS-A9-F6-P1 |  |
| OS-A8-F2-P1 | **Browser choice**: Compares the main open browsers on security update speed, privacy, licence and website compatibility, and picks a default. | research | medium | OS-A1-F1-P1 |  |
| OS-A8-F2-P2 | **Browser defaults**: Sets private defaults with no paid search placement, and documents every default and why. | data | small | OS-A8-F2-P1 |  |
| OS-A8-F2-P3 | **Phishing and scam site warnings**: Warns before phishing and scam websites using a block list checked privately on the device, and spots fake 'your PC is infected' pages, so people new to Spare OS are protected as they were on Windows. | integration | small | OS-A8-F2-P1 |  |
| OS-A8-F3-P1 | **Small apps set**: Chooses mature open apps for each everyday job and fits them to the Spare OS look. | research | medium | OS-A5-F1-P1 |  |
| OS-A8-F3-P2 | **PDF viewer and form filler**: Opens, searches, fills and signs PDF forms. | integration | medium | engine-pdf |  |
| OS-A8-F3-P3 | **Camera app**: Takes photos and video from the built-in or USB camera. | integration | small | OS-A14-F8-P1 |  |
| OS-A8-F4-P1 | **Media player**: A media player for local files and playlists that handles every codec Spare OS can legally ship. | integration | medium | engine-media, OS-A17-F3-P1 |  |
| OS-A8-F5-P1 | **Quick photo editor**: Simple edits in the image viewer, handing off to Spare Studio for anything bigger. | interface | medium | engine-draw2d |  |
| OS-A8-F6-P1 | **System contacts and calendar store**: One local store for contacts and calendars that syncs over open protocols and any app can read with permission. | library | medium | kit-store, OS-A12-F4-P1 |  |
| OS-A8-F6-P2 | **Online accounts panel**: Adds email, calendar, contact and storage accounts once in settings for every app to use. | interface | medium | OS-A6-F1-P1 |  |
| OS-A8-F7-P1 | **Maps app**: A maps app on the shared maps service, with offline maps for chosen regions. | integration | medium | svc-maps |  |
| OS-A8-F8-P1 | **Call readiness**: Makes camera, microphone, echo control and screen sharing work in browsers and call apps through the system's sharing permissions. | integration | small | OS-A14-F8-P1, OS-A14-F8-P2 |  |
| OS-A8-F9-P1 | **Font installer**: Previews a font file and installs it for one user or the whole PC by double-click, and removes it again from settings. | interface | small | OS-A16-F3-P1 |  |
| OS-A8-F9-P2 | **Disc burning**: Burns data, music and photo discs from the file manager with an open burning tool, for older PCs that still have a drive. | integration | small | OS-A7-F1-P1 |  |

## OS-A9 App store

Installing, updating and removing software from one trusted store or other sources the person adds: permissions shown up front, verified makers, ratings, reports, age ratings, payments, refunds and tips at cost, the maker's side of publishing, plugins, web apps, parental approval and offline installs.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A9-F1 Install apps from the store or elsewhere | Browse, search, install, update and remove apps from one store, and still add other open app sources or install a maker's own app file. | core | OS-A9-F1-P1 to P4 |
| OS-A9-F2 Know what an app can reach | See what an app can reach (files, camera, microphone, location, network) before installing, and change it later. | core | OS-A9-F2-P1 |
| OS-A9-F3 Trust who made an app | See whether an app comes from its real maker, read ratings and reviews, and report a harmful app. | core | OS-A9-F3-P1 to P4 |
| OS-A9-F4 Pay for or tip app makers | Buy paid apps or tip open-source makers, with the at-cost fee shown and the rest going to the maker. | next | OS-A9-F4-P1 to P5 |
| OS-A9-F5 Add plugins and AI tools | Find and install plugins for Spare apps and AI tools that connect through MCP, with the same permissions view. | next | OS-A9-F5-P1 |
| OS-A9-F6 Install a website as an app | Install a website as an app with its own window, icon and notifications. | core | OS-A9-F6-P1 |
| OS-A9-F7 Ask a parent before installing or buying | Let a child ask to install or buy an app, and a parent approve or decline from their own account, with spending limits and age ratings to guide them. | next | OS-A9-F7-P1 to P4 |
| OS-A9-F8 Install apps without good internet | Install and update apps from a local cache or a USB drive, for schools and places with poor internet. | later | OS-A9-F8-P1 |
| OS-A9-F9 Publish an app | As an app maker, sign up, agree the maker terms, upload and sign versions, follow review, appeal a rejection, see sales and get paid. | next | OS-A9-F9-P1 to P5 |
| OS-A9-F10 Pay makers in other countries safely | Know payouts to app makers abroad never reach sanctioned people. | later | OS-A9-F10-P1 |

### OS-A9 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A9-F1-P1 | **Store app**: Browse by category, search, see screenshots and details, install, update and remove. | interface | large | OS-A9-F1-P3 |  |
| OS-A9-F1-P2 | **App packaging format decision**: Compares sandboxed app formats on security, size, update speed and how many apps already use them, and picks one. | research | medium | OS-A1-F1-P1 |  |
| OS-A9-F1-P3 | **App catalogue service**: Holds the list of apps, their versions and checks, drawing on existing open app collections plus Spare apps. | service | large | kit-store, svc-accounts, OS-A9-F1-P2 |  |
| OS-A9-F1-P4 | **Other app sources and app files**: Lets the person add other open app sources and install app files from a maker's website, with a clear warning about who checked them and the same sandbox as store apps. | library | medium | OS-A9-F1-P2, OS-A12-F4-P1 |  |
| OS-A9-F2-P1 | **Permissions view**: Shows an app's permissions in plain words before install and in settings afterwards, with switches to change them. | interface | medium | OS-A12-F4-P1 |  |
| OS-A9-F3-P1 | **Maker verification**: Checks that an app's publisher really controls the project, and shows a mark when they do. | service | medium | svc-accounts, svc-trust |  |
| OS-A9-F3-P2 | **Ratings and reviews**: Ratings and reviews from people who installed the app, never from agents, with the trust service's tools against fakes. | service | medium | svc-trust, KIT-A17-F9-P2 |  |
| OS-A9-F3-P3 | **Report and takedown**: Lets anyone report a harmful app; decisions and removals go in the public log with the rule behind them. | service | medium | svc-trust, OS-A21-F5-P1 |  |
| OS-A9-F3-P4 | **Submission checks**: Automated malware and policy checks on every new app version before it is listed, sharing its scanners with the kit's plugin review once that is ready. | service | medium | OS-A9-F1-P3 |  |
| OS-A9-F4-P1 | **App payments**: Takes payments and tips through the regulated payment provider, with the at-cost fee on the ledger. | integration | medium | svc-payments, svc-ledger, OS-A9-F9-P1, OS-A9-F9-P3 | [K02](../kit/STATUS.md) |
| OS-A9-F4-P2 | **Tax on digital sales**: Works out and collects sales tax, VAT and GST on app sales where the store must, on the kit's sales tax engine, starting with New Zealand. | legal | medium | svc-payments, KIT-A4-F1-P2 |  |
| OS-A9-F4-P3 | **Refunds and consumer guarantees for apps**: Refunds for app purchases on the kit's refunds service, following each country's consumer law for digital content, such as New Zealand's consumer guarantees and the EU's digital content rules. | legal | medium | KIT-A3-F5-P1, KIT-A18-F8-P1, OS-A21-F10-P2 |  |
| OS-A9-F4-P4 | **Receipts and tax invoices for apps**: Gives every app purchase a receipt, and a tax invoice where the law needs one, through the kit. | integration | small | KIT-A3-F7-P1, KIT-A4-F2-P1, OS-A9-F4-P1 |  |
| OS-A9-F4-P5 | **App subscriptions**: Lets makers sell subscriptions that the person can see and cancel in one place, on the kit's regular payments. | integration | small | KIT-A3-F12-P1, OS-A9-F4-P1 |  |
| OS-A9-F5-P1 | **Plugin and MCP tool listings**: Lists sandboxed plugins and MCP tools in the store with their declared permissions, after the kit's plugin review and signing. | integration | medium | kit-plugin, kit-mcp, OS-A9-F1-P3, KIT-A16-F6-P2 |  |
| OS-A9-F6-P1 | **Web app install**: Turns a site into an app from the browser or the store, in its own window. | integration | small | OS-A8-F2-P1 |  |
| OS-A9-F7-P1 | **Install approvals**: Holds a child's install request until a parent approves, using the age range and family settings. | library | medium | OS-A13-F4-P1, OS-A21-F4-P2 |  |
| OS-A9-F7-P2 | **Purchase approvals and spending limits**: Holds a child's purchase until a parent approves and sets spending limits per child, meeting app store laws that need parent consent for purchases as well as downloads, such as Texas's, which courts let take effect in 2026. | library | medium | OS-A9-F7-P1, OS-A9-F4-P1, OS-A21-F4-P1 |  |
| OS-A9-F7-P3 | **Age ratings for apps and games**: Gives every app and game an age rating from an open, questionnaire-based rating scheme, shown in the store and used by family settings. | data | medium | OS-A9-F1-P3 |  |
| OS-A9-F7-P4 | **Game classification law**: Checks each country's classification law for games sold through the store, starting with New Zealand's, and how ratings and labels must be shown. | legal | small |  |  |
| OS-A9-F8-P1 | **Offline app bundles and local cache**: Copies apps and updates once to a local server or drive and installs from there. | service | medium | OS-A19-F4-P1 |  |
| OS-A9-F9-P1 | **Maker account and agreement**: A maker account and a plain-words maker agreement covering listing rules, fees at cost, payouts and takedowns. | legal | small | svc-accounts, KIT-A18-F4-P1, OS-A9-F3-P1, OS-A21-F10-P1 |  |
| OS-A9-F9-P2 | **Maker portal**: Where makers upload and sign versions, follow review status, see installs and sales figures, and answer reviews. | interface | medium | KIT-A16-F7-P1, OS-A9-F1-P3, OS-A9-F3-P4 |  |
| OS-A9-F9-P3 | **Maker payouts and tax details**: Collects makers' payout and tax details through the kit and pays out what their apps earn less the at-cost fee. | integration | medium | KIT-A3-F3-P1, KIT-A4-F3-P2, OS-A9-F9-P1 |  |
| OS-A9-F9-P4 | **Rejections and appeals**: Tells a maker why a version or an app was rejected or removed, with the rule behind it, and how to appeal. | service | small | KIT-A10-F8-P1, KIT-A11-F5-P1, OS-A21-F5-P1 |  |
| OS-A9-F9-P5 | **Signing key moves**: Lets a maker rotate an app's signing key or hand the app to a new maker without breaking updates for the people who have it. | library | small | OS-A9-F3-P1, OS-A9-F9-P2 |  |
| OS-A9-F10-P1 | **Sanctions screening for maker payouts**: Screens payouts to makers abroad against sanctions lists through the kit before any money is sent. | integration | small | KIT-A4-F8-P1, OS-A9-F9-P3 |  |

## OS-A10 Running Windows and phone programs

Keeping people's existing programs working: a tested list of what runs, Windows programs that start with a double-click through a compatibility layer, a Windows virtual machine for the stubborn few, Android apps, and a plan to replace Windows-only work programs over time.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A10-F1 Know what will run before switching | Search a list of Windows programs and see how well each runs on Spare OS before deciding to switch. | core | OS-A10-F1-P1 to P3 |
| OS-A10-F2 Double-click a Windows program to run it | Run a Windows installer or program by double-clicking it, and find it in the start menu afterwards. | core | OS-A10-F2-P1 to P4 |
| OS-A10-F3 Run the stubborn few in Windows | Run programs that no compatibility layer can handle inside a Windows virtual machine, using a Windows licence the person already owns. | next | OS-A10-F3-P1 to P3 |
| OS-A10-F4 Report what works | Report in one click how well a Windows program ran, which feeds the public database. | next | OS-A10-F4-P1 |
| OS-A10-F5 Run Android apps | Run Android apps in a contained space on the PC, installed from open app stores. | later | OS-A10-F5-P1 to P2 |
| OS-A10-F6 See Windows-only work programs being replaced | See which Windows-only programs stop people switching and what the co-op is doing about each one. | next | OS-A10-F6-P1 to P3 |

### OS-A10 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A10-F1-P1 | **Compatibility research**: Which common work programs run through compatibility layers, which don't, and why. | research | medium |  | [OS03](STATUS.md) |
| OS-A10-F1-P2 | **Tested compatibility database**: Open data of programs, versions and how well they run, including whether each works with a screen reader, from our own tests and members' reports. | data | large | OS-A10-F1-P1 |  |
| OS-A10-F1-P3 | **Automated compatibility runs**: Installs and starts the most common programs on each new release and records whether they still work. | service | large | OS-A10-F2-P1, OS-A1-F2-P4 |  |
| OS-A10-F2-P1 | **Compatibility layer packaging**: Ships the open Windows compatibility layer as a separate program, kept current and tested. | integration | large | OS-A10-F1-P1 |  |
| OS-A10-F2-P2 | **Per-program fix recipes**: Known settings and extra parts each tested program needs, applied automatically when it installs. | data | medium | OS-A10-F1-P2 |  |
| OS-A10-F2-P3 | **Sandboxed Windows program launcher**: Runs each Windows program in its own sandbox by default, with no compatibility-layer drive that maps the whole file system, only the folders it needs and a warning before running one downloaded from the web, and adds start menu entries and a clean uninstall. | interface | medium | OS-A10-F2-P1, OS-A12-F4-P1 |  |
| OS-A10-F2-P4 | **Windows fonts guidance**: Never ships Microsoft fonts; uses size-matched open fonts and explains how a person can add fonts they already own. | legal | small | OS-A5-F1-P4 |  |
| OS-A10-F3-P1 | **Windows virtual machine helper**: Sets up a Windows virtual machine from the person's own Windows installer and licence, with shared folders. | integration | medium | OS-A6-F9-P1 |  |
| OS-A10-F3-P2 | **Windows apps in their own windows**: Shows programs from the virtual machine as ordinary windows on the Spare OS desktop. | integration | large | OS-A10-F3-P1 |  |
| OS-A10-F3-P3 | **Licence guidance**: Explains plainly that Spare OS never supplies Windows and the person needs their own valid licence. | legal | small |  |  |
| OS-A10-F4-P1 | **Compatibility report form**: A short form with the program, version and result, sent with consent and checked before it updates the database. | interface | small | OS-A10-F1-P2 |  |
| OS-A10-F5-P1 | **Android container research**: Compares open ways to run Android apps on Linux, their hardware needs and the app store terms that apply. | research | medium | OS-A1-F1-P1 |  |
| OS-A10-F5-P2 | **Android app support**: Ships the chosen Android container as an optional, sandboxed add-on, with any Android app store offered under the policy for software that is not open source. | integration | large | OS-A10-F5-P1, OS-A12-F4-P1, OS-A21-F1-P4 |  |
| OS-A10-F6-P1 | **Switching blockers list**: Anonymous, opt-in counts of programs and services that blocked people's moves, such as exam device checks and device-compliance rules, from move-across scans and reports. | data | medium | OS-A4-F5-P1 |  |
| OS-A10-F6-P2 | **Replacement roadmap**: Public list linking each top blocker to a Spare program or open project that replaces it, starting with Spare Construct for Revit. | data | small | OS-A10-F6-P1 |  |
| OS-A10-F6-P3 | **NCEA digital exam approval**: Works with each country's exam bodies so Spare OS PCs pass their device checks, starting with NZQA for New Zealand's NCEA digital exams, since its published device guidance names Chrome OS, Windows and Mac and does not mention Linux (current table unverified). | research | medium | OS-A8-F2-P1 |  |

## OS-A11 Updates, backup and recovery

Updates that install as one piece and roll back if a start-up fails, update metadata that mirrors cannot replay, fast security fixes, whole-PC backups, restore onto a new PC, repair or erase a PC, published support periods, and taking data away in open formats or going back to Windows.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A11-F1 Get updates that can't break the PC | Get updates in the background that apply as one piece on the next restart, never half-installed. | core | OS-A11-F1-P1 to P7 |
| OS-A11-F2 Roll back to the last working version | Choose the previous version at start-up, and have it chosen automatically if a new version fails to start. | core | OS-A11-F2-P1 to P2 |
| OS-A11-F3 Get security fixes fast | Get security fixes within days, and urgent ones without waiting for a scheduled update. | core | OS-A11-F3-P1 to P2 |
| OS-A11-F4 Back up the whole PC | Back up files, apps and settings automatically to a drive, a network folder or Spareware storage, encrypted. | core | OS-A11-F4-P1 to P3 |
| OS-A11-F5 Restore onto a new PC | Restore files, apps and settings from a backup onto a new or repaired PC. | core | OS-A11-F5-P1 |
| OS-A11-F6 Fix a PC that won't start | Start a repair mode from the start-up menu or a USB stick, repair the system or reset it while keeping files, or erase it before passing it on. | core | OS-A11-F6-P1 to P4 |
| OS-A11-F7 Know how long my version is supported | See how long each version gets fixes, and get warned well before it ends with a one-click upgrade. | core | OS-A11-F7-P1 to P2 |
| OS-A11-F8 Take my data with me | Export files, settings and the app list in open, documented formats, delete what the co-op holds, and leave Spare OS cleanly if they choose. | core | OS-A11-F8-P1 to P5 |

### OS-A11 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A11-F1-P1 | **Image-based update research**: Compares image-based update systems and how each rolls back. | research | medium | OS-A1-F1-P1 | [OS05](STATUS.md) |
| OS-A11-F1-P2 | **Update server and channels**: Publishes signed system images and protected update metadata to stable, test and long-support channels, with running costs on the ledger. | service | large | OS-A1-F1-P2, OS-A1-F1-P5, svc-ledger |  |
| OS-A11-F1-P3 | **Background update agent**: Downloads and stages the next system version while the person works, then switches on restart. | library | large | OS-A11-F1-P1 |  |
| OS-A11-F1-P4 | **Small delta downloads**: Downloads only what changed between versions, to save data on slow or capped connections. | library | medium | OS-A11-F1-P2 |  |
| OS-A11-F1-P5 | **Local network sharing of updates**: Lets PCs on the same network share updates instead of each downloading them. | library | medium | OS-A11-F1-P3, OS-A11-F1-P6 |  |
| OS-A11-F1-P6 | **Update metadata protection**: Signs update metadata that expires, never lets version numbers go backwards and signs one snapshot of each whole release, as The Update Framework does, so no mirror or peer can freeze updates, serve an old flawed image or mix versions. | library | medium | OS-A11-F1-P2, OS-A1-F1-P5 |  |
| OS-A11-F1-P7 | **Mirror and peer rules**: Terms and a privacy notice for member-run mirrors, saying what they may log and for how long, and a plain warning that peer-to-peer downloads show the person's IP address to other peers. | legal | small | OS-A11-F1-P6, KIT-A18-F5-P1 |  |
| OS-A11-F2-P1 | **Start counting and automatic rollback**: Marks a new version good only after a successful start and falls back if it fails twice. | library | medium | OS-A11-F1-P3 |  |
| OS-A11-F2-P2 | **Version choice at start-up**: Lists the last few versions in the start-up menu with their dates. | interface | small | OS-A11-F2-P1 |  |
| OS-A11-F3-P1 | **Security fix line**: Builds, tests and ships security fixes on a faster track than feature updates. | service | medium | OS-A11-F1-P2, OS-A12-F8-P1 |  |
| OS-A11-F3-P2 | **Update timing rules**: Never restarts mid-work, respects metered and mobile data, and lets people pick restart times. | library | small | OS-A11-F1-P3 |  |
| OS-A11-F4-P1 | **Backup engine**: An open, encrypted, de-duplicating backup tool chosen on licence and proven reliability. | library | large | kit-store |  |
| OS-A11-F4-P2 | **Backup app**: Sets a backup target and schedule in a few clicks and shows when the last good backup ran. | interface | medium | OS-A11-F4-P1 |  |
| OS-A11-F4-P3 | **Spareware storage backup target**: Sends encrypted backups to Spareware storage at cost, with only the person holding the key, and any police or court request for them handled through the kit's published process. | integration | medium | OS-A11-F4-P1, svc-accounts, svc-ledger, KIT-A10-F9-P3 |  |
| OS-A11-F5-P1 | **Full restore flow**: Rebuilds a PC from a backup, reinstalling apps from the store rather than copying them. | interface | medium | OS-A11-F4-P1, OS-A3-F1-P1 |  |
| OS-A11-F6-P1 | **Recovery mode**: A small repair system that checks the disk, rolls back versions, and copies files off. | library | medium | OS-A11-F2-P1 |  |
| OS-A11-F6-P2 | **Reset keeping files**: Puts the system back to as-new while keeping the person's files and a list of apps to reinstall. | library | medium | OS-A11-F1-P3 |  |
| OS-A11-F6-P3 | **Recovery USB maker**: Makes a recovery USB stick from settings. | interface | small | OS-A2-F2-P1 |  |
| OS-A11-F6-P4 | **Erase this PC**: Erases the PC before it is sold, given away or refurbished, by destroying the disk's encryption key or by a checked wipe, signs it out of the Spareware login and device finding, and reinstalls a fresh system that opens at first-start set-up. | library | medium | OS-A12-F1-P1, OS-A11-F6-P1, svc-accounts |  |
| OS-A11-F7-P1 | **Support period policy**: A public policy for how long each release gets security fixes, as product security law requires. | data | small | OS-A21-F3-P1 |  |
| OS-A11-F7-P2 | **Version upgrade assistant**: Checks apps and hardware before a major upgrade and moves the PC across in one step. | interface | medium | OS-A11-F1-P3 |  |
| OS-A11-F8-P1 | **Open settings export**: Exports every setting as a documented, readable file that can be read without Spare OS. | data | small | OS-A18-F1-P2 |  |
| OS-A11-F8-P2 | **Delete my data from co-op services**: Adds everything Spare OS's services hold, such as synced settings and backups, to the kit's data export and account deletion, and confirms when done. | integration | small | svc-accounts, OS-A11-F4-P3, KIT-A18-F1-P1, KIT-A18-F2-P1 |  |
| OS-A11-F8-P3 | **Leaving guide**: A plain guide to moving files and settings to another system, including back to Windows. | data | small | OS-A20-F1-P1 |  |
| OS-A11-F8-P4 | **Give the PC back to Windows**: Removes Spare OS from a PC that also has Windows, returns the disk space and the start-up menu to Windows, and checks Windows starts before finishing. | library | medium | OS-A2-F4-P4, OS-A4-F4-P2 |  |
| OS-A11-F8-P5 | **Export passwords and contacts**: Exports the password vault, contacts and calendars in open formats other systems can import. | library | small | OS-A12-F7-P2, OS-A8-F6-P1 |  |

## OS-A12 Security and privacy

Safe by default: full-disk encryption, a signed start-up chain with Secure Boot, a firewall, sandboxed apps, a privacy dashboard with nothing shared by default, passwords and passkeys, a security team that publishes fixes, and help with lost laptops.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A12-F1 Encrypt the whole disk | Have the disk encrypted by default, unlocked with a password or the PC's security chip, with a recovery key kept safe. | core | OS-A12-F1-P1 to P4 |
| OS-A12-F2 Start up securely | Keep Secure Boot on, so only signed start-up files and drivers load. | core | OS-A12-F2-P1 to P4 |
| OS-A12-F3 Block attacks from the network | Have a firewall on by default, with simple choices for home, work and public networks. | core | OS-A12-F3-P1 |
| OS-A12-F4 Keep each app in its own box | Know that apps can't read other apps' data or reach the camera, microphone, files or location without asking. | core | OS-A12-F4-P1 to P2 |
| OS-A12-F5 See and control my data | See what Spare OS and each app send, with usage statistics off unless the person turns them on. | core | OS-A12-F5-P1 to P2 |
| OS-A12-F6 Scan files from other systems | Scan downloads and USB drives for malware, mainly to avoid passing Windows malware on to others. | next | OS-A12-F6-P1 |
| OS-A12-F7 Keep passwords and passkeys safe | Store passwords and passkeys in a built-in vault and sign in to sites with passkeys. | core | OS-A12-F7-P1 to P2 |
| OS-A12-F8 Hear about security problems and report them | Report a security flaw safely, and read plain advisories on what was fixed and what to do. | core | OS-A12-F8-P1 to P2 |
| OS-A12-F9 Find, lock or wipe a lost laptop | Find a lost laptop's last known place, lock it or wipe it from another device. | later | OS-A12-F9-P1 to P4 |
| OS-A12-F10 Sign in with fingerprint or face | Sign in and approve admin prompts with a fingerprint or a face camera. | next | OS-A12-F10-P1 to P2 |
| OS-A12-F11 Sync passwords between my PCs | Keep passwords, passkeys and Wi-Fi passwords the same on all their PCs, encrypted so only they can read them. | next | OS-A12-F11-P1 |

### OS-A12 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A12-F1-P1 | **Full-disk encryption**: Encrypts the system and home folders with proven open disk encryption. | library | medium | OS-A1-F1-P1 |  |
| OS-A12-F1-P2 | **Security chip unlock**: Unlocks the disk automatically on the same unchanged PC using its TPM, with a PIN option. | library | medium | OS-A12-F2-P1 |  |
| OS-A12-F1-P3 | **Recovery key safe-keeping**: Lets the person print the recovery key or save it to their Spareware account; organisations keep keys in their directory through OS-A19-F1-P4. | integration | small | svc-accounts |  |
| OS-A12-F1-P4 | **Accessible start-up unlock screen**: The disk unlock screen speaks or beeps, works with a braille display where possible, uses the keyboard layout the passphrase was set with, shows clearly that it is waiting and uses large text, since everyone meets it before the screen reader loads. | interface | medium | OS-A12-F1-P1, OS-A15-F7-P1, OS-A15-F1-P2, OS-A16-F2-P1 |  |
| OS-A12-F2-P1 | **Signed start-up chain**: Gets the first-stage boot loader signed by the certificate authority PC firmware trusts, and keeps up with certificate changes such as the 2026 expiry of the 2011 certificates. | integration | large | OS-A1-F1-P5 |  |
| OS-A12-F2-P2 | **Signed drivers and modules**: Signs kernel modules and extra drivers with Spare OS keys, and enrols the key for drivers built on the PC. | library | medium | OS-A12-F2-P1 |  |
| OS-A12-F2-P3 | **Revocation list updates**: Applies firmware revocation list updates so known-bad boot loaders are blocked. | integration | small | OS-A1-F4-P1 |  |
| OS-A12-F2-P4 | **Signing accounts and agreements**: The accounts and agreements the co-op needs to have boot files signed, held by the legal entity. | legal | small | KIT-A14-F1-P1 |  |
| OS-A12-F3-P1 | **Firewall and network profiles**: Blocks incoming connections by default and asks in plain words when an app wants to accept them. | library | small | OS-A6-F1-P1 |  |
| OS-A12-F4-P1 | **App sandbox and permission prompts**: Runs store apps in sandboxes and asks the person when an app needs something outside its box. | library | large | OS-A9-F1-P2 |  |
| OS-A12-F4-P2 | **In-use indicators**: Shows a clear sign whenever the camera, microphone, screen sharing or location is in use, and by which app. | interface | small | OS-A12-F4-P1 |  |
| OS-A12-F5-P1 | **Privacy dashboard**: One page showing each kind of data, who can reach it and what is sent, with switches. | interface | medium | OS-A6-F1-P1 |  |
| OS-A12-F5-P2 | **Opt-in usage statistics**: Collects only counts people agree to, aggregates them so no one is identifiable, and publishes the results, moving onto the kit's private usage counts (KIT-A22-F8-P1) once the kit brings them forward. | service | medium |  |  |
| OS-A12-F6-P1 | **On-demand malware scanner**: An open malware scanner run on request or on new drives, with signature updates. | integration | small |  |  |
| OS-A12-F7-P1 | **System keyring**: Stores app passwords and keys encrypted and unlocked with the person's sign-in. | library | small |  |  |
| OS-A12-F7-P2 | **Password manager and passkeys**: A password and passkey manager that works in the browser and apps and can import from others, with syncing between PCs added by OS-A12-F11. | integration | medium | svc-accounts, OS-A12-F7-P1 |  |
| OS-A12-F8-P1 | **Security response team and disclosure policy**: A named team and set response times for Spare OS, taking reports through the kit's page for reporting weaknesses. | service | medium | OS-A21-F3-P1, KIT-A22-F5-P3 |  |
| OS-A12-F8-P2 | **Security advisories**: Public advisories for each fix, in plain words and in a machine-readable form. | data | small | OS-A12-F8-P1 |  |
| OS-A12-F9-P1 | **Find my device**: Reports location only when the owner turns it on and asks, through the accounts and maps services. | service | medium | svc-accounts, svc-maps, OS-A14-F11-P1 |  |
| OS-A12-F9-P2 | **Remote lock and wipe**: Locks or erases a lost PC from the owner's account or the organisation's console. | library | medium | OS-A19-F3-P1 |  |
| OS-A12-F9-P3 | **Requests for device locations and backups**: Sends police and court requests for device locations and backups held by co-op services through the kit's published process, counts them in the transparency report, and keeps the data where the data-location rules say. | legal | small | KIT-A10-F9-P3, KIT-A10-F15-P1, KIT-A18-F9-P1, OS-A12-F9-P1 |  |
| OS-A12-F9-P4 | **Stalking safeguards**: Tells every user of a shared or family PC when device finding is on and when it was last used, never lets another account turn it on silently, and shows the person at the PC a clear notice they can act on. | library | small | OS-A12-F9-P1, OS-A13-F1-P1 |  |
| OS-A12-F10-P1 | **Fingerprint sign-in**: Uses the fingerprint readers supported in OS-A14-F9-P1 for sign-in and admin prompts, with the print stored only on the device. | integration | medium | OS-A14-F9-P1 |  |
| OS-A12-F10-P2 | **Face sign-in research**: Checks open face sign-in for infrared cameras against photo and spoofing attacks before offering it. | research | medium |  |  |
| OS-A12-F11-P1 | **End-to-end encrypted secrets sync**: Syncs the password vault, passkeys and Wi-Fi passwords between the person's PCs end-to-end encrypted, on the kit's chosen encryption. | library | medium | OS-A12-F7-P2, OS-A3-F3-P2, kit-sync, KIT-A15-F12-P1 |  |

## OS-A13 Users, sign-in and family

Who uses the PC and how they get in: separate users, guests, Spareware login with offline sign-in, child accounts and parental controls, password recovery and safe admin prompts.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A13-F1 Have separate users on one PC | Give each person their own account, files and settings on a shared PC. | core | OS-A13-F1-P1 to P2 |
| OS-A13-F2 Let a guest use the PC | Let a visitor use the PC in a guest session that is wiped when they sign out. | next | OS-A13-F2-P1 |
| OS-A13-F3 Sign in with a Spareware login | Sign in to the PC with a Spareware login and keep signing in when offline. | next | OS-A13-F3-P1 to P2 |
| OS-A13-F4 Keep children safe | Set up child accounts with screen time, age-suitable apps, spending limits and web filtering, managed from a parent's account. | next | OS-A13-F4-P1 to P4 |
| OS-A13-F5 Get back in after forgetting a password | Reset a forgotten password with a recovery key, another admin, or the Spareware login. | core | OS-A13-F5-P1 |
| OS-A13-F6 Make admin changes safely | Approve system changes with an admin password when needed, and use the PC as a standard user day to day. | core | OS-A13-F6-P1 |

### OS-A13 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A13-F1-P1 | **User accounts service**: Creates, changes and removes users through system commands, standard users by default. | library | medium | OS-A18-F1-P1 |  |
| OS-A13-F1-P2 | **Private home folders**: Keeps each user's files encrypted and readable only by them. | library | medium | OS-A12-F1-P1 |  |
| OS-A13-F2-P1 | **Guest session**: A temporary account with no access to other users' files, erased at sign-out. | library | small | OS-A13-F1-P1 |  |
| OS-A13-F3-P1 | **Spareware login on the sign-in screen**: Signs in through the accounts service using standard open sign-in protocols, with passkeys first. | integration | large | svc-accounts, OS-A13-F1-P1 |  |
| OS-A13-F3-P2 | **Offline sign-in**: Keeps a secure local copy of the sign-in so the person can get in without internet. | library | medium | OS-A13-F3-P1 |  |
| OS-A13-F4-P1 | **Child accounts and family settings**: Links child accounts to parents and applies age-based defaults from the age range and app age ratings, following the kit's rules for parents and carers. | library | large | OS-A13-F1-P1, OS-A21-F4-P2, KIT-A1-F8-P2, OS-A9-F7-P3 |  |
| OS-A13-F4-P2 | **Screen time limits**: Daily limits and bedtimes per child, with a request for more time. | library | medium | OS-A13-F4-P1 |  |
| OS-A13-F4-P3 | **Web filter for children**: Optional family-safe filtering of websites at the network level, with the filter's rules visible to parents. | integration | medium | OS-A13-F4-P1 |  |
| OS-A13-F4-P4 | **Parent's view**: Lets parents see and approve requests from their own phone or PC. | interface | medium | svc-accounts, svc-messaging, OS-A13-F4-P1 |  |
| OS-A13-F5-P1 | **Password recovery flow**: Offers each recovery route the person set up, without ever weakening disk encryption. | interface | medium | OS-A12-F1-P3 |  |
| OS-A13-F6-P1 | **Admin prompts**: Clear prompts saying exactly what will change, logged as system commands. | library | small | OS-A18-F1-P1 |  |

## OS-A14 Devices and connections

Getting online and using hardware: Wi-Fi and mobile data, work and school networks, VPNs, printers and scanners, Bluetooth, the person's phone, pens, cameras and sound, fingerprint readers and smart cards, screens, projectors and docks, private location, and true colours on screen and in print.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A14-F1 Get online | Connect to Wi-Fi, wired networks, phone hotspots and mobile data, including networks with a sign-in page. | core | OS-A14-F1-P1 to P5 |
| OS-A14-F2 Use a VPN | Connect to a work or personal VPN, including the common types offices use. | core | OS-A14-F2-P1 |
| OS-A14-F3 Print | Print to home and office printers, most with no driver at all, and share a printer. | core | OS-A14-F3-P1 to P3 |
| OS-A14-F4 Scan documents | Scan from scanners and all-in-one printers, and turn scans into searchable PDFs. | core | OS-A14-F4-P1 to P2 |
| OS-A14-F5 Pair Bluetooth devices | Pair headphones, speakers, mice, keyboards and controllers, and switch between them easily. | core | OS-A14-F5-P1 |
| OS-A14-F6 Link my phone | Send files and links between phone and PC, see phone notifications on the PC, and use the phone as a remote. | next | OS-A14-F6-P1 to P3 |
| OS-A14-F7 Use pens and drawing tablets | Use a pen on a touch screen or a drawing tablet with pressure, tilt and buttons set per app. | next | OS-A14-F7-P1 |
| OS-A14-F8 Use cameras, microphones and speakers | Use built-in and USB cameras, microphones and speakers, and switch between them from the tray. | core | OS-A14-F8-P1 to P3 |
| OS-A14-F9 Use fingerprint readers, smart cards and security keys | Use a fingerprint reader, a work smart card or a hardware security key. | next | OS-A14-F9-P1 to P2 |
| OS-A14-F10 Use screens, projectors and docks | Plug in monitors, projectors and USB-C or Thunderbolt docks, and show the screen on a TV. | core | OS-A14-F10-P1 to P3 |
| OS-A14-F11 Share my location only when I choose | Let the PC and chosen apps know roughly where it is for time zones, maps, weather and finding a lost laptop, only with consent. | core | OS-A14-F11-P1 to P2 |
| OS-A14-F12 See true colours on screen and in print | Have colours match across screens and printers, and calibrate screens, including HDR ones. | next | OS-A14-F12-P1 to P2 |
| OS-A14-F13 Look and sound better on calls | Remove background noise from the microphone, and blur or frame the camera background, in any call app. | next | OS-A14-F13-P1 |

### OS-A14 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A14-F1-P1 | **Network connections**: Manages Wi-Fi, wired, hotspot and mobile data connections with an open network manager. | integration | medium | OS-A6-F1-P1 |  |
| OS-A14-F1-P2 | **Sign-in page detection**: Spots cafe and hotel networks that need a sign-in page and opens it safely. | interface | small | OS-A14-F1-P1 |  |
| OS-A14-F1-P3 | **Mobile data and phone hotspots**: Supports built-in mobile data modems and hotspots, counting data on capped connections. | integration | small | OS-A14-F1-P1 |  |
| OS-A14-F1-P4 | **Work and school Wi-Fi and proxies**: Connects to enterprise Wi-Fi that uses certificates, including eduroam, and follows system proxy and automatic proxy settings. | integration | small | OS-A14-F1-P1 |  |
| OS-A14-F1-P5 | **One trusted certificate store**: One place to install a trusted root certificate that the browser, sandboxed apps and the compatibility layer all honour, as school networks that inspect encrypted traffic, such as N4L's, need. | library | medium | OS-A14-F1-P1, OS-A12-F4-P1, OS-A8-F2-P1, OS-A10-F2-P1 |  |
| OS-A14-F2-P1 | **VPN connections**: Supports the common open VPN types, plus the open clients that connect to popular office VPN products. | integration | medium | OS-A14-F1-P1 |  |
| OS-A14-F3-P1 | **Printing system**: An open printing system with driverless printing for modern printers and automatic discovery on the network. | integration | medium |  |  |
| OS-A14-F3-P2 | **Driver packs for older printers**: Open drivers for older printers, matched by model, with the compatibility list showing what works. | data | medium | OS-A14-F3-P1, OS-A1-F2-P5 |  |
| OS-A14-F3-P3 | **Print dialog**: One print dialog for every app, with preview, page choices and paper sizes. | interface | small | OS-A14-F3-P1 |  |
| OS-A14-F4-P1 | **Scanning system**: Open scanning support with driverless scanning for modern devices. | integration | medium |  |  |
| OS-A14-F4-P2 | **Scan app**: Scans single pages or many into a PDF, with text recognition so the PDF can be searched. | interface | medium | OS-A14-F4-P1, OS-A18-F4-P1, engine-pdf |  |
| OS-A14-F5-P1 | **Bluetooth support**: Open Bluetooth support with good audio quality codecs and simple pairing. | integration | medium | OS-A14-F8-P2 |  |
| OS-A14-F6-P1 | **Phone link**: Pairs Android and iPhone over the local network using an open phone-link protocol, with the phone side built per the phone apps decision. | integration | large | svc-messaging | [K04](../kit/STATUS.md) |
| OS-A14-F6-P2 | **Nearby sharing between PCs**: Sends files to nearby Spare OS PCs on the same network with the receiver's consent. | library | medium | OS-A14-F6-P1 |  |
| OS-A14-F6-P3 | **Phone as a webcam**: Uses the person's phone as a webcam for calls over the phone link. | integration | small | OS-A14-F6-P1, OS-A14-F8-P1 |  |
| OS-A14-F7-P1 | **Pen and tablet support**: Open drivers and a settings page for pens and drawing tablets, including button mapping. | integration | medium | OS-A6-F1-P1 |  |
| OS-A14-F8-P1 | **Cameras and screen sharing**: Makes cameras and screen sharing work through the modern media system with per-app permission. | integration | small | OS-A12-F4-P1 |  |
| OS-A14-F8-P2 | **Sound devices and mixer**: Handles speakers, headsets and microphones, per-app volume and automatic switching when a headset is plugged in. | integration | medium |  |  |
| OS-A14-F8-P3 | **Built-in laptop cameras**: Checks the MIPI cameras common on Intel laptops from 2022 on, which have weak Linux support, and gets the ones that can work going so video calls don't break. | research | medium | OS-A14-F8-P1, OS-A1-F2-P4 |  |
| OS-A14-F9-P1 | **Fingerprint readers**: Supports the fingerprint readers that have open drivers, listed on the compatibility list. | integration | medium | OS-A1-F2-P5 |  |
| OS-A14-F9-P2 | **Smart cards and security keys**: Supports smart cards for government and office sign-in and hardware security keys for websites. | integration | small |  |  |
| OS-A14-F10-P1 | **Screen hotplug and projector mode**: Detects new screens and offers duplicate, extend or second-screen-only in one step. | library | medium | OS-A5-F4-P2 |  |
| OS-A14-F10-P2 | **Cast to a TV**: Shows the screen on smart TVs and streaming boxes through open casting protocols where they exist. | integration | medium | OS-A14-F10-P1 |  |
| OS-A14-F10-P3 | **Docks and Thunderbolt approval**: Supports docks and asks before trusting a new Thunderbolt device. | integration | small |  |  |
| OS-A14-F11-P1 | **Private location lookup**: Works out location from nearby Wi-Fi through an open provider that speaks the retired Mozilla service's protocol, such as beaconDB, or one the co-op runs at cost on the maps service, since Linux lost its usual source when Mozilla's closed in 2024; patent claims like those that led Mozilla to settle are checked before the co-op runs its own. | integration | medium | svc-maps, OS-A12-F4-P1 |  |
| OS-A14-F11-P2 | **Location data rules**: Says what is sent to look up a location, what is kept and for how long, and asks consent before the first lookup. | legal | small | KIT-A18-F5-P1, KIT-A18-F6-P1, OS-A14-F11-P1 |  |
| OS-A14-F12-P1 | **Colour profiles**: Applies a colour profile to each monitor and printer so every app shows and prints the same colours. | library | medium | OS-A5-F4-P2, OS-A14-F3-P1 |  |
| OS-A14-F12-P2 | **Screen calibration**: Calibrates screens with a measuring device and sets up high dynamic range screens, for people who design for print. | interface | medium | OS-A14-F12-P1, OS-A17-F5-P2 |  |
| OS-A14-F13-P1 | **Voice and camera effects**: Noise suppression for microphones, and background blur and framing for cameras, run on the device and offered to every app as virtual devices. | library | medium | OS-A14-F8-P1, OS-A14-F8-P2, OS-A18-F4-P2 |  |

## OS-A15 Accessibility

Usable by everyone from the first screen, including the disk unlock screen: a screen reader with braille and familiar commands, magnifier, high contrast and colour filters, other ways to type and click, voice control and dictation, live captions, hearing support, natural voices including te reo Māori, and the accessibility law Spare OS must meet.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A15-F1 Use a screen reader | Use the whole system and every Spare app with a screen reader, from the first start, including on a braille display. | core | OS-A15-F1-P1 to P5 |
| OS-A15-F2 Zoom in and make things bigger | Zoom the screen, make text and the pointer larger, and keep it that way everywhere. | core | OS-A15-F2-P1 to P2 |
| OS-A15-F3 See the screen clearly | Use high contrast, colour filters for colour blindness, and less motion and transparency. | core | OS-A15-F3-P1 |
| OS-A15-F4 Type and click another way | Use sticky keys, slow keys, mouse keys, an on-screen keyboard, switch access or dwell clicking. | core | OS-A15-F4-P1 to P2 |
| OS-A15-F5 Control the PC by voice | Open apps, click, scroll and type by voice, offline. | next | OS-A15-F5-P1 to P2 |
| OS-A15-F6 Read captions for any sound | See live captions for any sound playing on the PC, made on the device. | next | OS-A15-F6-P1 |
| OS-A15-F7 Hear text read aloud | Have any text read aloud in a natural voice, including the example country's (for New Zealand: New Zealand English and te reo Māori). | core | OS-A15-F7-P1 to P2 |
| OS-A15-F8 Rely on Spare OS meeting accessibility law | Rely on Spare OS meeting accessibility law and standards wherever it is offered, such as the European Accessibility Act, which covers consumer operating systems. | core | OS-A15-F8-P1 to P2 |
| OS-A15-F9 Use eye or head tracking | Move the pointer and click with eye or head tracking hardware. | later | OS-A15-F9-P1 |
| OS-A15-F10 Use the PC with hearing loss | Hear sound in one ear or with uneven hearing, and see a flash when the PC plays an alert sound. | core | OS-A15-F10-P1 |
| OS-A15-F11 Pair my hearing aids | Pair Bluetooth LE Audio hearing aids and hear the PC through them. | next | OS-A15-F11-P1 |

### OS-A15 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A15-F1-P1 | **Screen reader**: Ships an open screen reader set up to start by shortcut, with every built-in app labelled for it. | integration | large | OS-A5-F1-P1 |  |
| OS-A15-F1-P2 | **Braille displays**: Supports common refreshable braille displays through the screen reader. | integration | medium | OS-A15-F1-P1 |  |
| OS-A15-F1-P3 | **Accessibility checks for every screen**: Automated and human checks that every Spare OS screen, including the start-up menu and the disk unlock screen, works with the screen reader and keyboard alone. | service | medium | OS-A15-F1-P1 |  |
| OS-A15-F1-P4 | **Unlock without seeing the screen**: A tested way into an encrypted PC for people who cannot use the unlock screen, such as unlocking with the security chip on an unchanged PC or with a trusted helper, checked with blind testers each release. | interface | small | OS-A12-F1-P2, OS-A12-F1-P4 |  |
| OS-A15-F1-P5 | **Familiar screen reader commands**: Our own screen reader key map that people coming from NVDA or JAWS will find familiar, plus guides for blind people switching from Windows. | data | small | OS-A15-F1-P1 |  |
| OS-A15-F2-P1 | **Magnifier**: Full-screen and lens zoom that follows the pointer, keyboard focus and typing. | interface | medium | OS-A5-F1-P2 |  |
| OS-A15-F2-P2 | **Text and pointer size**: System-wide text scaling and pointer size that all built-in apps honour. | library | small | OS-A6-F1-P1 |  |
| OS-A15-F3-P1 | **High contrast and colour filters**: High-contrast themes, colour-blind filters and reduce-motion settings applied across the desktop and apps. | interface | medium | OS-A5-F9-P1 |  |
| OS-A15-F4-P1 | **Keyboard access options**: Sticky, slow and bounce keys and mouse keys, each turned on from settings or a shortcut. | library | small | OS-A6-F1-P1 |  |
| OS-A15-F4-P2 | **Switch access and dwell click**: Scanning switch access and dwell clicking for people who use one or two switches or head pointers. | interface | medium | OS-A5-F10-P2 |  |
| OS-A15-F5-P1 | **Voice control**: Spoken commands mapped to system commands, with numbered labels on screen to click by voice, on the shared speech engine, handing commands for Spare apps to the kit's voice control (KIT-A17-F8-P1) once the kit builds it, so there is one voice path, not two. | interface | large | OS-A18-F4-P1, OS-A18-F1-P1 |  |
| OS-A15-F5-P2 | **Dictation into any text box**: Types what the person says into any app, with spoken punctuation and corrections. | interface | medium | OS-A18-F4-P1, OS-A16-F2-P1 |  |
| OS-A15-F6-P1 | **Live captions**: Captions for any audio, including calls and videos, in a movable window, made on the device. | interface | medium | OS-A18-F4-P1 |  |
| OS-A15-F7-P1 | **Text-to-speech voices**: Open, natural voices for the main languages, used by the screen reader and read-aloud. | library | medium |  |  |
| OS-A15-F7-P2 | **Indigenous language voices**: Works with language and data experts on voices for indigenous languages, starting with a te reo Māori voice made with Māori experts on terms that respect Māori data sovereignty. | research | medium | OS-A15-F7-P1, OS-A16-F1-P2 |  |
| OS-A15-F8-P1 | **Accessibility conformance report**: A published report against the European standard for accessible technology and WCAG, updated each release. | legal | medium | OS-A15-F1-P3, OS-A15-F8-P2 |  |
| OS-A15-F8-P2 | **Accessibility law research**: Which accessibility laws apply to a free operating system in each country, such as New Zealand and the EU, and what each requires; the research belongs in the kit for every Spare app. | legal | medium |  |  |
| OS-A15-F9-P1 | **Eye and head tracking research**: Finds which trackers have open drivers and how they fit with dwell clicking. | research | small | OS-A15-F4-P2 |  |
| OS-A15-F10-P1 | **Mono audio, balance and visual alerts**: Mono audio, left and right balance, and a screen flash for alert sounds, each set from accessibility settings. | library | small | OS-A14-F8-P2, OS-A6-F1-P1 |  |
| OS-A15-F11-P1 | **Hearing aid pairing**: Pairs Bluetooth LE Audio hearing aids and streams sound to them, tested on the lab's hearing aids. | integration | medium | OS-A14-F5-P1, OS-A1-F2-P4 |  |

## OS-A16 Languages and regions

International by design, with New Zealand as the first example country: every screen translatable, the example country's languages first-class (for New Zealand: te reo Māori and Pacific languages, with their own keyboards, dictionaries and te reo speech), every keyboard and script, local formats, spelling, country packs and on-device translation.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A16-F1 Use Spare OS in my language | Use every screen of Spare OS in their own language, with the example country's languages ready from the start (for New Zealand: New Zealand English and te reo Māori). | core | OS-A16-F1-P1 to P4 |
| OS-A16-F2 Type in any language | Type in any language with the right keyboard layout or input method, including macrons for te reo Māori. | core | OS-A16-F2-P1 to P3 |
| OS-A16-F3 See every script and emoji | Read every writing system and emoji correctly, including right-to-left languages. | core | OS-A16-F3-P1 to P2 |
| OS-A16-F4 Use local formats | See dates, times, numbers, money, paper size and units the local way. | core | OS-A16-F4-P1 |
| OS-A16-F5 Check spelling in my language | Get spell checking in their language in every app, including te reo Māori and New Zealand English. | core | OS-A16-F5-P1 to P2 |
| OS-A16-F6 Get a set-up that fits my country | Get the right public holidays, emergency information, legal notices and download mirrors for their country. | next | OS-A16-F6-P1 to P2 |
| OS-A16-F7 Translate text on the device | Translate text and web pages without sending them anywhere. | later | OS-A16-F7-P1 |
| OS-A16-F8 Speak to the PC in my language | Dictate, see captions and control the PC by voice in smaller and indigenous languages, such as te reo Māori, not only in English. | next | OS-A16-F8-P1 |

### OS-A16 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A16-F1-P1 | **Translation workflow**: Spare OS's texts on the kit's member translation workflow, with review before release, linked to the translation platforms of the open projects Spare OS ships. | service | medium | KIT-A19-F1-P3 |  |
| OS-A16-F1-P2 | **Fluent speaker translation**: A full translation into the example country's languages, starting with te reo Māori, done with Māori language experts, with agreed terms for new technical words. | data | large | OS-A16-F1-P1 |  |
| OS-A16-F1-P3 | **Pacific languages**: Translations for Samoan, Tongan, Cook Islands Māori and other Pacific languages, led by native speakers. | data | medium | OS-A16-F1-P1 |  |
| OS-A16-F1-P4 | **Text built for translation**: Every interface text kept in translatable form through the kit's translation framework, with room for longer words and plural rules. | library | small | OS-A16-F1-P1, KIT-A19-F1-P1 |  |
| OS-A16-F2-P1 | **Keyboard layouts and input methods**: Ships open input methods for Chinese, Japanese, Korean, Indic and other scripts, switched from the tray. | integration | medium |  |  |
| OS-A16-F2-P2 | **Māori macrons by default**: Sets keyboards for languages that use macrons, such as te reo Māori, to type macron vowels with a simple key combination out of the box. | data | small | OS-A16-F2-P1 |  |
| OS-A16-F2-P3 | **Pacific keyboards**: Ready-to-use Samoan, Tongan and Cook Islands Māori keyboard layouts that type the ʻokina or fakauʻa and macrons. | data | small | OS-A16-F2-P1 |  |
| OS-A16-F3-P1 | **Fonts for all scripts and emoji**: Open font families that cover every major writing system and current emoji. | data | medium |  |  |
| OS-A16-F3-P2 | **Right-to-left checks**: Checks that every Spare OS screen lays out correctly in right-to-left languages. | service | small | OS-A16-F1-P1 |  |
| OS-A16-F4-P1 | **Regional formats**: Regional settings for dates, numbers, currency, A4 or Letter paper and metric or imperial units. | library | small |  |  |
| OS-A16-F5-P1 | **Spelling dictionaries**: Open spelling dictionaries for each supported language, shared by all apps. | data | small |  |  |
| OS-A16-F5-P2 | **Pacific spelling dictionaries**: Spelling dictionaries for Samoan, Tongan, Cook Islands Māori and other Pacific languages, built with native speakers on terms they agree. | data | medium | OS-A16-F5-P1, OS-A16-F1-P3 |  |
| OS-A16-F6-P1 | **Country packs**: Spare OS's own settings, such as default mirrors and local help contacts, added to the kit's country packs, starting with New Zealand. | data | medium | OS-A16-F4-P1, KIT-A19-F6-P1, KIT-A19-F9-P1 |  |
| OS-A16-F6-P2 | **Local mirrors per country**: Download and update mirrors in each country where members run them. | service | small | OS-A2-F1-P2 |  |
| OS-A16-F7-P1 | **Offline translation**: Runs open translation models on the device for chosen language pairs. | library | medium | OS-A18-F4-P2 |  |
| OS-A16-F8-P1 | **Indigenous language speech recognition**: Builds speech recognition for indigenous languages for dictation, live captions and voice control, starting with te reo Māori, made with Māori language and data experts, respecting the community licence terms that existing te reo speech data comes with. | research | large | KIT-A18-F16-P1, OS-A18-F4-P1, OS-A15-F7-P2 |  |

## OS-A17 Games and media

Play and watch: Windows games through game store apps and a compatibility layer, controllers, every common media format shipped legally, paid streaming services, game performance, recording, and low-delay audio for musicians.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A17-F1 Play Windows games | Install game store apps and play most Windows games, and see which games will not run and why. | core | OS-A17-F1-P1 to P2 |
| OS-A17-F2 Use game controllers | Plug in or pair common game controllers and have them work in games straight away. | core | OS-A17-F2-P1 |
| OS-A17-F3 Play every common media format | Play common audio and video formats, with patent licences handled properly. | core | OS-A17-F3-P1 to P2 |
| OS-A17-F4 Watch paid streaming services | Watch subscription video services in the browser at their full supported quality. | core | OS-A17-F4-P1 |
| OS-A17-F5 Get the best game performance | Turn on a game mode, see frame rates, and use high dynamic range and variable refresh screens. | next | OS-A17-F5-P1 to P2 |
| OS-A17-F6 Record and stream gameplay | Record game clips and stream to any service the person chooses. | later | OS-A17-F6-P1 |
| OS-A17-F7 Make music with low delay | Use audio interfaces and music software with low delay for recording and live playing. | later | OS-A17-F7-P1 |

### OS-A17 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A17-F1-P1 | **Game store apps**: Packages the main game store apps that support Linux, with their compatibility tools set up, offered under the policy for software that is not open source. | integration | medium | OS-A10-F2-P1, OS-A21-F1-P4 |  |
| OS-A17-F1-P2 | **Anti-cheat compatibility list**: Lists popular games whose anti-cheat blocks them on Linux, so people know before switching. | data | small | OS-A10-F1-P2 |  |
| OS-A17-F2-P1 | **Controller support**: Open drivers and settings for common controllers, including wireless ones. | integration | small | OS-A14-F5-P1 |  |
| OS-A17-F3-P1 | **Codec licensing research**: Which common video and audio formats carry patent licences, and how other distributions ship them legally. | legal | medium |  |  |
| OS-A17-F3-P2 | **Hardware video decoding**: Uses the graphics chip to play video smoothly and save battery. | integration | medium | OS-A1-F3-P2, OS-A17-F3-P1 |  |
| OS-A17-F4-P1 | **Browser content protection**: Offers the content protection module streaming sites need, on terms its owner allows and under the policy for software that is not open source. | legal | small | OS-A8-F2-P1, OS-A21-F1-P4 |  |
| OS-A17-F5-P1 | **Game mode and performance overlay**: Raises a game's priority, pauses background work and shows frame rate when asked. | integration | small | OS-A6-F2-P1 |  |
| OS-A17-F5-P2 | **High dynamic range and variable refresh**: Supports high dynamic range and variable refresh screens in games and video. | integration | medium | OS-A5-F4-P2 |  |
| OS-A17-F6-P1 | **Game capture**: Records the last few minutes on a shortcut and streams with low overhead. | interface | medium | OS-A5-F8-P2, engine-media |  |
| OS-A17-F7-P1 | **Low-delay audio set-up**: A one-switch set-up for low-delay audio, tested with common audio interfaces. | integration | small | OS-A14-F8-P2 |  |

## OS-A18 AI, commands and automation

Headless by rule: every system action is a command and every state can be read by a query, used the same way by people, scripts, plugins and AI, with an MCP server any AI can drive under the person's approval, a registry of app MCP servers, agents kept in their own workspace, an optional on-device assistant, a shared on-device engine for text and speech recognition, routines and desktop plugins.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A18-F1 Run any system action as a command | Do anything the settings and system tools do through a typed command or a script, with a log and undo. | core | OS-A18-F1-P1 to P4 |
| OS-A18-F2 Let an AI help with the PC | Let any AI assistant the person chooses check and change the PC through MCP, approving each sensitive step. | next | OS-A18-F2-P1 to P6 |
| OS-A18-F3 Ask for help in plain words | Ask how to do something or to fix a problem, and get an answer or the fix, using an on-device model by default. | next | OS-A18-F3-P1 to P4 |
| OS-A18-F4 Recognise text and speech on the device | Have apps read text in images, turn speech into text and speak in natural voices, without sending anything away. | core | OS-A18-F4-P1 to P2 |
| OS-A18-F5 Automate routines | Set routines such as opening work apps when joining the work Wi-Fi, or running a clean-up every week. | later | OS-A18-F5-P1 to P2 |
| OS-A18-F6 Add plugins to the desktop | Add sandboxed plugins that extend the desktop, tray or file manager. | later | OS-A18-F6-P1 |

### OS-A18 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A18-F1-P1 | **System command layer**: Every setting and system action is an operation on the kit's command layer, shared by the interface, scripts, plugins and AI. | library | large | kit-ops, kit-core |  |
| OS-A18-F1-P2 | **Command catalogue**: A documented, searchable list of every system command, its inputs and the permission it needs. | data | medium | OS-A18-F1-P1 |  |
| OS-A18-F1-P3 | **Change log and undo**: Records every system change with who or what made it, and undoes settings changes. | library | medium | kit-ops, OS-A18-F1-P1 |  |
| OS-A18-F1-P4 | **System queries**: Read-only queries for state such as installed apps, battery, logs and settings, so people, scripts and agents can look without changing anything. | library | medium | OS-A18-F1-P1, kit-ops |  |
| OS-A18-F2-P1 | **System MCP server**: An MCP server over the system command layer, off until the person turns it on. | integration | medium | kit-mcp, OS-A18-F1-P1 |  |
| OS-A18-F2-P2 | **Approvals and limits for AI actions**: Applies the kit's approval steps and agent limits to system actions, asking the person before anything sensitive and logging every step. | interface | medium | OS-A18-F2-P1, KIT-A17-F2-P1, KIT-A17-F4-P1 |  |
| OS-A18-F2-P3 | **Hidden instruction guards on the PC**: Uses the kit's hidden instruction guards on everything an agent reads on the PC, such as files, windows and web pages. | library | small | OS-A18-F2-P1, KIT-A17-F5-P1 |  |
| OS-A18-F2-P4 | **Registry of app MCP servers**: One system list of every installed app's MCP server, with one consent screen for which agents may use which, as Windows now has with its on-device registry. | library | medium | OS-A18-F2-P1, kit-mcp, OS-A9-F2-P1 |  |
| OS-A18-F2-P5 | **Agent workspace**: Runs agents that drive the desktop in an isolated workspace or as a separate low-privilege user, so they never act with the person's full rights. | library | medium | OS-A18-F2-P1, OS-A13-F1-P1, OS-A12-F4-P1 |  |
| OS-A18-F2-P6 | **Approvals only a person can give**: Shows admin, payment and agent approvals on a secure screen that agents, scripts and synthetic input cannot press, while the person's own keyboard, switch access and voice control still can. | library | medium | OS-A13-F6-P1, OS-A18-F2-P2 |  |
| OS-A18-F3-P1 | **Assistant panel**: A panel from the taskbar to ask questions and approve suggested fixes, built on the kit's help assistant. | interface | medium | OS-A18-F2-P1, OS-A18-F3-P2, KIT-A17-F7-P1 |  |
| OS-A18-F3-P2 | **On-device assistant model**: Runs an open language model for the assistant on the shared on-device AI engine, with nothing leaving the PC. | library | large | OS-A18-F4-P2 |  |
| OS-A18-F3-P3 | **Model choice and licences**: Chooses on-device models whose licences allow shipping them, under the kit's AI tools policy, and lets people plug in a cloud AI of their choice. | research | medium | OS-A18-F3-P2, KIT-A17-F6-P1 |  |
| OS-A18-F3-P4 | **AI transparency duties**: Tells people they are talking to an AI and marks AI-made content, as Article 50 of the EU AI Act has required since 2 August 2026, and lists the assistant on the co-op's AI use register. | legal | small | KIT-A17-F9-P1, KIT-A17-F10-P1, OS-A18-F3-P1 |  |
| OS-A18-F4-P1 | **Text and speech recognition services**: Text recognition and speech recognition on the shared engine, used by screenshots, scanning, captions, dictation and voice control. | library | large | OS-A18-F4-P2 |  |
| OS-A18-F4-P2 | **Shared on-device AI engine**: Runs open models on the processor, graphics chip or AI chip for text recognition, speech recognition and text-to-speech, with nothing leaving the PC; it belongs in a shared engine so every Spare desktop app on Windows and macOS can use it too. | library | large |  |  |
| OS-A18-F5-P1 | **Routines engine**: Runs chains of system commands on triggers like time, network, device or battery. | library | medium | OS-A18-F1-P1 |  |
| OS-A18-F5-P2 | **Routines editor**: Builds routines from plain choices, with no code needed. | interface | medium | OS-A18-F5-P1 |  |
| OS-A18-F6-P1 | **Desktop plugin host**: Runs desktop plugins as sandboxed components with declared permissions. | integration | medium | kit-plugin, OS-A18-F1-P1 |  |

## OS-A19 Managing many machines

For offices, schools and co-ops: central sign-in, setting up many PCs at once with their network settings, rules and admin roles for every machine, update control, inventory, IT remote help, classrooms, exams and shared PCs, local admin passwords, joining existing Microsoft and Google networks, an office server, fleet security and local IT partners, all without paid licences.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A19-F1 Sign in with a work or school account on any PC | Sign in to any of the organisation's PCs with one work or school account. | next | OS-A19-F1-P1 to P4 |
| OS-A19-F2 Set up many PCs at once | Enrol new PCs and have them set themselves up with the right apps, settings and Wi-Fi. | next | OS-A19-F2-P1 to P3 |
| OS-A19-F3 Set rules for every PC | Set rules such as password strength, blocked USB drives, allowed apps and screen lock times for groups of PCs. | next | OS-A19-F3-P1 to P4 |
| OS-A19-F4 Control updates across PCs | Roll updates out to test groups first, pause them, and serve them from a local cache. | next | OS-A19-F4-P1 |
| OS-A19-F5 See every PC's state | See every PC's model, version, encryption, last update and health in one list and export it. | next | OS-A19-F5-P1 |
| OS-A19-F6 Help colleagues and reach PCs as IT staff | As IT staff, help a colleague on their screen under the organisation's rules, and connect to the organisation's PCs by remote desktop. | next | OS-A19-F6-P1 to P2 |
| OS-A19-F7 Run classrooms and shared PCs | Run lab and library PCs that reset after each person, single-app kiosks, and exam mode. | next | OS-A19-F7-P1 to P4 |
| OS-A19-F8 Keep local admin passwords safe | Have a different local admin password on each PC, changed automatically and stored in the directory. | next | OS-A19-F8-P1 |
| OS-A19-F9 Fit into an existing Microsoft network | Join existing Windows domains and cloud directories, use their file shares and printers, and pass their device rules. | next | OS-A19-F9-P1 to P2 |
| OS-A19-F10 Get help from a local IT partner | Hire a local IT firm trained and listed by the co-op to run Spare OS PCs. | later | OS-A19-F10-P1 |
| OS-A19-F11 Fit into Google Workspace | Sign in with a Google Workspace for Education or business account and manage PCs from it, as many schools do with Chromebooks. | next | OS-A19-F11-P1 |
| OS-A19-F12 Run the office server | Run file shares, printing, network addresses, backups and update caching for an office or school on one server, without a Windows Server licence. | next | OS-A19-F12-P1 to P2 |
| OS-A19-F13 Keep a fleet secure | Collect logs centrally, get alerts about suspicious activity, check each PC's health, and show the fleet meets a recognised security baseline. | next | OS-A19-F13-P1 to P3 |
| OS-A19-F14 Pick approved apps myself | As staff, choose and install apps the organisation has approved, without asking IT each time. | later | OS-A19-F14-P1 |

### OS-A19 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A19-F1-P1 | **Directory and sign-in research**: Compares open directory, sign-in and device management tools for offices and schools. | research | medium |  | [OS06](STATUS.md) |
| OS-A19-F1-P2 | **Directory service**: Ships the chosen open directory and sign-in service as an easy-to-run package for an organisation's own server, bringing people in through the kit's organisation directory sync, which Spare Office uses too. | integration | large | OS-A19-F1-P1, KIT-A1-F7-P1, KIT-A1-F7-P3 |  |
| OS-A19-F1-P3 | **Join a PC to a directory**: Joins a PC to the organisation's directory from first start or settings in a few steps. | interface | medium | OS-A19-F1-P2 |  |
| OS-A19-F1-P4 | **Recovery keys in the directory**: Stores each joined PC's disk recovery key in the organisation's directory, readable only by named IT staff, with every look logged. | integration | small | OS-A12-F1-P3, OS-A19-F1-P3 |  |
| OS-A19-F2-P1 | **Enrolment and set-up service**: Enrols PCs from first start or a network install and applies the organisation's set-up profile. | service | large | OS-A19-F1-P1, OS-A18-F1-P1 |  |
| OS-A19-F2-P2 | **Set-up profiles**: Documented files that list apps, settings, Wi-Fi and rules for each group of PCs. | data | medium | OS-A19-F2-P1 |  |
| OS-A19-F2-P3 | **Network and certificate profiles**: Pushes work Wi-Fi, proxy settings and trusted certificates to every enrolled PC from the console. | data | small | OS-A19-F2-P2, OS-A14-F1-P4, OS-A14-F1-P5 |  |
| OS-A19-F3-P1 | **Rules engine**: Applies and reports rules on each PC through the system command layer, even offline. | library | large | OS-A18-F1-P1 |  |
| OS-A19-F3-P2 | **Management console**: A web console to set rules, see PCs and act on them, with every admin action logged, plugged into the kit's organisation team management so device rules and Spare Office admin can share one console, which belongs in the kit. | interface | large | OS-A19-F3-P1, OS-A19-F2-P1, KIT-A1-F7-P2, KIT-A1-F5-P2 |  |
| OS-A19-F3-P3 | **Admin roles and IT scripts**: Role-based admin, such as teacher, IT staff and auditor, each limited to its own actions, with the same actions open to IT scripts through the command API and MCP. | library | medium | KIT-A1-F5-P2, kit-mcp, OS-A18-F1-P1, OS-A19-F3-P2 |  |
| OS-A19-F3-P4 | **Firmware settings from the console**: Sets start-up firmware options, such as boot order and firmware passwords, across PCs through the interfaces PC makers publish. | integration | medium | OS-A1-F4-P1, OS-A19-F3-P1 |  |
| OS-A19-F4-P1 | **Update rings and local cache**: Groups PCs into rings with set delays and caches updates on a local server. | service | medium | OS-A11-F1-P2 |  |
| OS-A19-F5-P1 | **Inventory and compliance reports**: Collects each PC's state and shows which ones break the organisation's rules. | service | medium | OS-A19-F2-P1 |  |
| OS-A19-F6-P1 | **Remote help for IT staff**: Lets the organisation's IT staff start the same consented, logged help sessions with colleagues, under the organisation's rules and admin roles. | integration | medium | OS-A20-F8-P1, OS-A19-F3-P1 |  |
| OS-A19-F6-P2 | **Remote desktop**: Open remote desktop server and client that also connect to Windows PCs. | integration | medium |  |  |
| OS-A19-F7-P1 | **Kiosk and shared PC modes**: Locks a PC to one app or resets it to a clean state at each sign-out. | library | medium | OS-A19-F3-P1 |  |
| OS-A19-F7-P2 | **Classroom tools**: Lets a teacher share their screen, send files and pause screens, with students told when it is on. | interface | medium | OS-A19-F6-P1 |  |
| OS-A19-F7-P3 | **Exam mode**: Locks the PC to an exam app or site for the length of an exam. | library | small | OS-A19-F7-P1 |  |
| OS-A19-F7-P4 | **Exam platforms that check the system**: Tracks exam and assessment platforms that check which operating system a device runs, as New Zealand's NZQA does, and works with each so exam mode meets them. | research | small | OS-A19-F7-P3, OS-A10-F6-P3 |  |
| OS-A19-F8-P1 | **Local admin password rotation**: Sets and rotates each PC's local admin password and saves it in the directory. | library | small | OS-A19-F3-P1, OS-A19-F1-P2 |  |
| OS-A19-F9-P1 | **Windows domain and cloud directory join**: Joins existing Windows domains and Microsoft cloud directories with open tools built from published protocols. | integration | medium | OS-A19-F1-P3 |  |
| OS-A19-F9-P2 | **Microsoft 365 device compliance**: Checks whether Microsoft 365 rules that only admit compliant devices will accept Spare OS PCs, and what an organisation can do if they don't. | research | medium | OS-A19-F9-P1 |  |
| OS-A19-F10-P1 | **IT partner programme**: How local IT firms are trained, listed and rated to support Spare OS for organisations. | research | medium | OS-A20-F2-P1 |  |
| OS-A19-F11-P1 | **Google Workspace sign-in and management**: Signs in to Spare OS PCs with Google Workspace accounts through Google's published sign-in standards, and takes settings from Google's admin tools where its terms allow. | integration | medium | OS-A19-F1-P3, OS-A13-F1-P1 |  |
| OS-A19-F12-P1 | **File and print serving**: File shares with permissions and quotas, and a print server with shared queues, from open servers that speak Windows' published protocols, installed beside the directory service. | integration | large | OS-A19-F1-P2, OS-A14-F3-P1, OS-A7-F4-P1 |  |
| OS-A19-F12-P2 | **Network services and backup target**: DNS and DHCP, a network backup target and the update cache in the same package, set up from one screen. | integration | medium | OS-A19-F12-P1, OS-A19-F4-P1, OS-A11-F4-P1 |  |
| OS-A19-F13-P1 | **Central logs and alerts**: Collects logs from every PC on the organisation's server and alerts IT staff to suspicious activity, using open detection rules. | service | large | OS-A19-F5-P1, OS-A6-F10-P2 |  |
| OS-A19-F13-P2 | **Device health checks**: Lets the console check, through each PC's security chip, that it started securely and has not been tampered with. | library | medium | OS-A12-F2-P1, OS-A12-F1-P2, OS-A19-F3-P2 |  |
| OS-A19-F13-P3 | **Hardening baselines**: Published security baselines for Spare OS, checked against government security manuals, such as the New Zealand Information Security Manual, and common benchmarks, that the rules engine can apply and report on. | data | medium | OS-A19-F3-P1 |  |
| OS-A19-F14-P1 | **Organisation app catalogue**: A private catalogue of the apps an organisation approves, shown in the store to its staff. | interface | medium | OS-A9-F1-P1, OS-A19-F3-P1 |  |

## OS-A20 Help, support and community

People need help switching: built-in guides that also work offline, a member support network, safe remote help from verified helpers, a community forum, one-click problem reports, complaints that get put right, a say on Spare OS, what it costs to run and how that is paid, release notes and status, and early test versions.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A20-F1 Get help inside Spare OS | Search plain guides inside Spare OS, including how to do familiar Windows tasks, and run fixers for common problems. | core | OS-A20-F1-P1 to P4 |
| OS-A20-F2 Get help from a person | Get help from a trained member by chat, call or in person, free for basics and at cost for businesses. | core | OS-A20-F2-P1 to P4 |
| OS-A20-F3 Ask the community | Ask and answer questions in a community forum linked from Spare OS. | next | OS-A20-F3-P1 |
| OS-A20-F4 Report a problem | Report a problem in one click, seeing exactly what diagnostic details will be sent before sending. | core | OS-A20-F4-P1 to P3 |
| OS-A20-F5 Have a say on Spare OS | Propose and vote on Spare OS defaults and priorities as a member. | next | OS-A20-F5-P1 to P2 |
| OS-A20-F6 See what Spare OS costs to run | See each month what the build servers, mirrors, test lab, signing and support cost. | core | OS-A20-F6-P1 |
| OS-A20-F7 Test new versions early | Join a test channel to try new versions early and send feedback. | next | OS-A20-F7-P1 |
| OS-A20-F8 Get help remotely, safely | Let a verified helper see or control the screen for a set task and time, after agreeing, with a record kept and an easy way to report a scam. | core | OS-A20-F8-P1 to P3 |
| OS-A20-F9 Know a helper who visits is safety-checked | Know that helpers who come to their home or work with vulnerable people have passed a safety check. | next | OS-A20-F9-P1 |
| OS-A20-F10 Know how Spare OS's running costs are paid | See who pays for the build servers, mirrors, test lab, signing, security team and support, since a free system has no jobs to charge a fee on. | core | OS-A20-F10-P1 to P2 |
| OS-A20-F11 Put things right when help goes wrong | Complain or open a dispute about a helper, paid business support, an IT partner or a store purchase, and know paid helpers work under the co-op's worker and safety rules. | core | OS-A20-F11-P1 to P2 |
| OS-A20-F12 Know what changed and what's down | Read plain release notes and known issues, and check one page when updates, mirrors or the store fail. | core | OS-A20-F12-P1 to P2 |

### OS-A20 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A20-F1-P1 | **Help app and guides**: Shows the kit's help centre guides for Spare OS in every supported language, written in short sentences with pictures. | interface | medium | OS-A16-F1-P1, KIT-A21-F1-P1 |  |
| OS-A20-F1-P2 | **Fixers for common problems**: Step-by-step checks that find and fix common Wi-Fi, sound, printer, screen and update problems. | library | medium | OS-A18-F1-P1 |  |
| OS-A20-F1-P3 | **Guides that work offline**: Keeps the main guides on the PC, in recovery mode and on the USB stick, so they work when Wi-Fi, start-up or the screen has failed. | data | small | OS-A20-F1-P1, OS-A11-F6-P1 |  |
| OS-A20-F1-P4 | **Help on a phone**: A phone-friendly help site with fixes for a PC that won't start or connect. | interface | small | KIT-A21-F1-P1 |  |
| OS-A20-F2-P1 | **Member support network**: Matches people who need help with trained member helpers, by language and place. | service | large | svc-messaging, svc-accounts, svc-matching |  |
| OS-A20-F2-P2 | **Helper training and badges**: The helper course and badge on the kit's training and certificates, with ratings from the people helpers help. | data | medium | svc-trust, KIT-A25-F2-P1 |  |
| OS-A20-F2-P3 | **Business support at cost**: Paid support plans for organisations priced at cost and shown on the ledger. | service | medium | svc-payments, svc-ledger |  |
| OS-A20-F2-P4 | **Phone and relay support**: Help by phone, and through a relay service (example: New Zealand Relay) for Deaf, hard of hearing and speech-impaired people, for when the PC itself is what's broken. | integration | small | KIT-A21-F4-P1 |  |
| OS-A20-F3-P1 | **Community forum**: Spare OS's part of the kit's member forum, signed in with the Spareware login. | integration | medium | svc-accounts, KIT-A21-F6-P1 |  |
| OS-A20-F4-P1 | **Diagnostics collector**: Gathers logs and system details for a problem, removes personal details and shows the result to the person. | library | medium | OS-A18-F1-P1 |  |
| OS-A20-F4-P2 | **Problem report form**: A short form that sends the report to the public tracker or a helper, with the person's consent. | interface | small | OS-A20-F4-P1 |  |
| OS-A20-F4-P3 | **Crash reports with consent**: Collects crash reports only when the person agrees, and groups them so fixes go where they matter, moving onto the kit's crash reports (KIT-A22-F9-P1) once the kit brings them forward. | service | medium | OS-A20-F4-P1 |  |
| OS-A20-F5-P1 | **Votes on defaults and priorities**: Links Spare OS proposals to the member voting service, where only people, never agents, can vote. | integration | small | svc-voting, KIT-A17-F9-P2 |  |
| OS-A20-F5-P2 | **Who votes on Spare OS**: Sets who votes on Spare OS (users who are members, helpers, contributors or all members), what can be voted on, such as defaults, store rules, priorities and the funding model, and what no vote can override, such as legal duties and security fixes. | legal | small | KIT-A12-F5-P1, KIT-A12-F5-P2 |  |
| OS-A20-F6-P1 | **Spare OS ledger entries**: Publishes the running costs of Spare OS on the open cost ledger each month. | data | small | svc-ledger | [K06](../kit/STATUS.md) |
| OS-A20-F7-P1 | **Test channel and feedback**: A test update channel with an in-app feedback button and the option to return to stable. | service | small | OS-A11-F1-P2, OS-A20-F4-P2 |  |
| OS-A20-F8-P1 | **Remote help sessions**: Screen sharing and control the person approves each time, limited to a scope and a time, with a visible banner, one-click stop and a log kept for the person. | integration | medium | OS-A18-F1-P1, OS-A12-F4-P2, OS-A20-F8-P2 |  |
| OS-A20-F8-P2 | **Verified helpers only**: Lets only helpers whose identity the kit has checked start a session, and shows their verified name and badge throughout. | service | medium | KIT-A2-F1-P2, KIT-A2-F1-P3, OS-A20-F2-P2 |  |
| OS-A20-F8-P3 | **Scam warning and report**: Warns before every session that the co-op never calls out of the blue and gives a one-click report, since built-in remote help is the main tool of tech-support scams, as in the 2024 ransomware attacks through Windows' Quick Assist. | interface | small | KIT-A10-F3-P1, OS-A20-F8-P1 |  |
| OS-A20-F9-P1 | **Safety checks for visiting helpers**: Requires a consented safety check through the kit before a helper visits homes or helps vulnerable people, and in-person help starts only once this is in place. | integration | small | KIT-A2-F9-P1, KIT-A2-F9-P2, OS-A20-F2-P1 |  |
| OS-A20-F10-P1 | **Funding model**: Compares ways to pay Spare OS's running costs, such as donations, business support plans, partner PC fees and a part of the co-op's shared costs, with no member dues, as a decision note for members. | research | medium | OS-A20-F6-P1, KIT-A13-F3-P1, KIT-A14-F1-P1 |  |
| OS-A20-F10-P2 | **Funding vote and ledger lines**: Puts the chosen funding model to a member vote and shows each funding source on the ledger beside the costs it pays. | integration | small | svc-voting, svc-ledger, KIT-A12-F5-P2, OS-A20-F10-P1 |  |
| OS-A20-F11-P1 | **Complaints and disputes about help**: Takes complaints and disputes about helpers, paid support, IT partners and store purchases through the kit's disputes, with a path to put right a broken PC or lost data. | service | medium | KIT-A11-F1-P1, KIT-A21-F2-P1, OS-A20-F2-P1 |  |
| OS-A20-F11-P2 | **Rules for paid helpers and IT partners**: Applies the kit's worker-status, health and safety and insurance rules to paid helpers and IT partners who take control of PCs or go into homes. | legal | medium | KIT-A23-F1-P1, KIT-A24-F1-P1, KIT-A14-F17-P1 |  |
| OS-A20-F12-P1 | **Release notes and known issues**: Plain-words release notes for every update and a list of known issues with workarounds. | data | small | OS-A11-F1-P2 |  |
| OS-A20-F12-P2 | **Status of updates, mirrors and store**: Shows the health of the update server, mirrors and store on the kit's status page. | integration | small | KIT-A21-F5-P1, OS-A11-F1-P2, OS-A9-F1-P3 |  |

## OS-A21 Legal duties and licences

What Spare OS must do by law and by licence wherever it is offered: licence notices and source for every shipped part, a policy for software that is not open source, product security law, age-assurance laws, app takedowns and platform law, privacy law, names and marks, encryption rules, consumer law, plain terms and a plan if the co-op stops. Accessibility law sits with accessibility (OS-A15-F8). Research only, not legal advice; each part needs a lawyer's check.

| Feature | What a person can do | Priority | Parts |
| --- | --- | --- | --- |
| OS-A21-F1 Get every part with its licence and source | See the licence and credit for every part of Spare OS, and get the source code for any part whose licence requires it. | core | OS-A21-F1-P1 to P5 |
| OS-A21-F3 Rely on Spare OS meeting product security law | Rely on Spare OS meeting product security law, such as the EU Cyber Resilience Act, whose reporting duties began in September 2026. | core | OS-A21-F3-P1 to P2 |
| OS-A21-F4 Meet age-assurance laws | Have the PC give apps only an age range where the law requires it, with nothing more shared. | core | OS-A21-F4-P1 to P2 |
| OS-A21-F5 Handle app takedowns fairly | Know that harmful or unlawful apps are removed under published rules, with notice, appeal and a public log. | core | OS-A21-F5-P1 |
| OS-A21-F6 Have my data protected by law | Know Spare OS follows privacy law in each country, including telling people about data breaches. | core | OS-A21-F6-P1 to P2 |
| OS-A21-F7 Know the names and look are our own | Trust that Spare OS uses its own name, logo, icons and design, and names other companies only to say what it replaces. | core | OS-A21-F7-P1 to P2 |
| OS-A21-F8 Download Spare OS from any country | Download Spare OS legally from anywhere encryption export rules allow, knowing where local law limits encryption. | core | OS-A21-F8-P1 |
| OS-A21-F9 Buy PCs and support with consumer rights | Have the usual consumer rights when buying a Spare OS PC or paid support. | next | OS-A21-F9-P1 to P2 |
| OS-A21-F10 Read terms I can understand | Read Spare OS's terms of use and the store's terms in plain words before agreeing. | core | OS-A21-F10-P1 to P2 |
| OS-A21-F11 Have the store, reviews and forum follow platform law | Know the store, its reviews and the forum handle notices, removals and complaints as platform law requires. | next | OS-A21-F11-P1 to P3 |
| OS-A21-F12 Know Spare OS carries on if the co-op stops | Know how updates, signing keys and mirrors would pass to another steward if the co-op or Spare OS stopped. | later | OS-A21-F12-P1 |

### OS-A21 parts

| ID | Part | Kind | Size | Depends on | Piece |
| --- | --- | --- | --- | --- | --- |
| OS-A21-F1-P1 | **Licence inventory and software bill of materials**: A machine-readable list of every shipped part, its version, licence and origin, made by the build line in the same form as the kit's software bills of materials. | data | medium | OS-A1-F1-P2, KIT-A22-F2-P3 |  |
| OS-A21-F1-P2 | **Source archive for every release**: Keeps the exact source of every shipped part for each release for as long as the licences require. | service | medium | OS-A1-F1-P2 |  |
| OS-A21-F1-P3 | **Licences screen**: Shows every part's licence and credit from the About page. | interface | small | OS-A21-F1-P1 |  |
| OS-A21-F1-P4 | **Licence policy for shipped software**: Writes down, for an owner's decision, where our code ends and separate programs begin: our own code stays under MPL-2.0, with the SDK and plugins under Apache-2.0, and for software that is not open source, such as NVIDIA's driver, device firmware files, the browser's streaming content protection module, game store apps and any Android store, which ship in the image, which are opt-in downloads the person chooses, and on what redistribution terms. | legal | medium | KIT-A16-F4-P1 |  |
| OS-A21-F1-P5 | **Labels for software that is not open source**: Marks every driver, module or app that is not open source wherever it is offered, with its maker and terms, before the person chooses it. | interface | small | OS-A21-F1-P4, OS-A9-F1-P1 |  |
| OS-A21-F3-P1 | **Product security law research**: Whether the co-op is a manufacturer or an open-source steward of Spare OS under the EU Cyber Resilience Act, its distributor or importer duties for other makers' commercial apps sold into the EU through the store from 11 December 2027, and the duties for support periods, reporting and bills of materials; the research belongs in the kit for every Spare app. | legal | medium | KIT-A14-F1-P1 |  |
| OS-A21-F3-P2 | **Reporting exploited flaws to authorities**: Reports actively exploited flaws and severe incidents to the right authority within the legal deadlines. | service | small | OS-A12-F8-P1, OS-A21-F3-P1 |  |
| OS-A21-F4-P1 | **Age-assurance law research**: Tracks laws that put age duties on operating systems or app stores, such as California's from 1 January 2027 and Texas's app store law that courts let take effect in 2026, including parent consent for purchases, and designs the lightest way to comply. | legal | medium |  |  |
| OS-A21-F4-P2 | **Age range service for apps**: A local service that tells permitted apps only the user's age range, never the birth date, and logs each request. | library | medium | OS-A21-F4-P1, OS-A13-F1-P1 |  |
| OS-A21-F5-P1 | **Store rules and takedown process**: Published store rules, legal takedown handling and appeals, with every decision in the trust service's public log. | legal | medium | svc-trust |  |
| OS-A21-F6-P1 | **Privacy law research**: Spare OS's duties under each country's privacy law, such as New Zealand's Privacy Act 2020 and the EU's data protection law, worked through with the kit's privacy officer. | legal | medium | KIT-A18-F14-P1 |  |
| OS-A21-F6-P2 | **Spare OS in the breach plan**: Adds Spare OS's own services, such as backups, sync, device finding and the store, to the kit's breach response. | legal | small | OS-A21-F6-P1, KIT-A18-F7-P1 |  |
| OS-A21-F7-P1 | **Name and look review**: Checks every release's names, icons and screens are our own work and not copies of another system. | legal | small | OS-A5-F1-P3 |  |
| OS-A21-F7-P2 | **Name and logo registration**: Registers the Spare OS name and logo in the main markets; the registration work belongs in the kit for every Spare name. | legal | small | KIT-A14-F1-P1 |  |
| OS-A21-F8-P1 | **Encryption export check**: Checks each country's export rules for publicly available encryption software, starting with New Zealand's, and laws in some countries that restrict importing or using encryption, which is on by default, and records the basis for shipping it; the research belongs in the kit for every Spare app. | legal | small |  |  |
| OS-A21-F9-P1 | **Consumer law research**: Spare OS's duties under each country's consumer and fair trading law, starting with New Zealand's, for pre-installed PCs and paid support, building on the kit's consumer law research. | legal | medium | KIT-A18-F8-P1 |  |
| OS-A21-F9-P2 | **Ecodesign rules for partner tablets**: Checks the EU ecodesign rules that require years of operating system updates for tablets, in case partners sell tablets with Spare OS. | legal | small | OS-A21-F9-P1 |  |
| OS-A21-F10-P1 | **Spare OS terms of use**: Plain-words terms for Spare OS and its services, including the warranty disclaimer that free software licences carry, shown in the installer and on the website. | legal | small | KIT-A18-F4-P1, KIT-A18-F4-P2 |  |
| OS-A21-F10-P2 | **Store terms for customers**: Plain-words terms for buying and installing apps from the store, including refunds and what the co-op checks. | legal | small | OS-A21-F10-P1, KIT-A18-F8-P1 |  |
| OS-A21-F11-P1 | **EU Digital Services Act duties**: Notice and action, statements of reasons, complaints and, where it applies, checks on the makers selling apps, as the EU Digital Services Act requires of hosts and marketplaces. | legal | medium | KIT-A10-F9-P1, KIT-A10-F8-P1, OS-A21-F5-P1 |  |
| OS-A21-F11-P2 | **Online harm duties**: Handles complaints about reviews and forum posts under each country's online harm law (example: New Zealand's Harmful Digital Communications Act), so the co-op keeps its protection as a host. | legal | small | KIT-A10-F9-P2 |  |
| OS-A21-F11-P3 | **Copyright notices for member content**: Takes copyright notices for member-given wallpapers and forum posts through the kit's notice handling. | service | small | KIT-A10-F21-P2, OS-A5-F9-P2 |  |
| OS-A21-F12-P1 | **Continuity plan**: A plan, held by the steward society, for handing the update service, signing keys and mirrors to another steward. | legal | small | KIT-A14-F15-P1, OS-A1-F1-P5, OS-A11-F1-P2 |  |

## Questions to ask when assessing

- Are the base system (OS01) and desktop (OS02) chosen on evidence, and which parts of this map change if the leans (KDE Plasma, image-based updates) change?
- In the demo, can a Windows user find the start menu, files, settings and Wi-Fi in the first minute without help?
- Is every icon, layout, sound and screen our own design, with nothing copied from Windows or any other system?
- Can someone try Spare OS from a USB stick and go back to Windows untouched, and is BitLocker checked before any partition is changed?
- Does the move from Windows bring files, browser data, email and Wi-Fi across without breaking any encryption or terms, and what happens to cloud-only files?
- Of the 50 Windows programs offices and schools use most, measured first in New Zealand, how many run, and is the compatibility list honest about the ones that don't?
- Can an update ever leave a PC unable to start, and how is rollback tested on real hardware each release?
- Who holds the signing keys and the Secure Boot signing agreement, and what is the plan if the firmware certificates change again?
- Is every data-sharing choice off by default, and could a sceptical member check exactly what is sent?
- Does every screen work with a screen reader and keyboard alone, and in the example country's languages (for New Zealand: te reo Māori)?
- Is the co-op a manufacturer or an open-source steward under EU product security law, and which duties (accessibility, age ranges, privacy, export) need a lawyer before the first release?
- Is every system action a command, so scripts and AI use the same path as people, and can an AI only act with the person's approval?
- What will Spare OS cost to run each month (build servers, mirrors, test lab, signing, support), and is it on the open cost ledger?
- Is the core set small enough for a first release, or should more move to next?
- Who helps people who get stuck, and can the member support network keep up if many people switch when old PCs lose Windows support?
- Does any part depend on another Spare program directly, breaking the rule that programs depend only on the kit and engines?
- For offices and schools, does the map replace directory and device-management licences fully, and which gaps are listed?
- Are media patents, Microsoft fonts and streaming content protection handled legally, with the basis written down?
- Which parts that are not open source (NVIDIA's driver, firmware files, streaming content protection, game stores, Android stores) ship in the image and which are opt-in, and has an owner signed that policy off against the README's legal rules?
- Could a scammer use remote help on a first-time switcher, and what stops them: verified helpers, scope and time limits, the scam warning and the log?
- Can a blind person unlock an encrypted Spare OS PC and finish install with no sighted help?
- Who pays Spare OS's running costs when there are no jobs to charge a fee on, and have members voted on it?
- Can students sit digital exams on a Spare OS laptop, starting with New Zealand's NCEA, and can school PCs get online behind school networks such as New Zealand's N4L?

## Sources checked on 2026-10-18 and 2026-10-19

Research only, not legal advice. Dates and duties in the legal area need a lawyer's check before anything ships.

- [Red Hat: Secure Boot signing certificates expiring in 2026](https://www.redhat.com/en/blog/expiration-secure-boot-signing-certificates-2026)
- [Hunton: California age assurance law for operating systems (AB 1043)](https://www.hunton.com/privacy-and-cybersecurity-law-blog/california-introduces-new-age-verification-requirements-for-software-applications)
- [Orrick: state age verification laws for app stores](https://www.orrick.com/de-DE/Insights/2025/07/Texas-and-Louisiana-Join-Growing-Trend-of-State-Age-Verification-Laws-for-App-Stores)
- [European Disability Forum: analysis of the European Accessibility Act](https://www.edf-feph.org/?p=788)
- [OpenSSF: Cyber Resilience Act stewards one-pager](https://policy.openssf.org/CRA/stewards-one-pager.html)
- [Noze: Cyber Resilience Act guidance after the Commission's clarifications](https://www.noze.it/en/insights/cra-commission-guidance/)
- [Pureinfotech: AI agents in the Windows 11 taskbar (April 2026)](https://pureinfotech.com/windows-11-ai-agent-support-taskbar-search/)
- [BleepingComputer: Windows 11 recovery features](https://www.bleepingcomputer.com/news/microsoft/windows-11-kb5062660-update-brings-new-windows-resilience-features/amp/)
- [Flathub forum: paid apps status](https://discourse.flathub.org/t/when-are-paid-applications-supported/7757)
- [Microsoft Security Blog: Storm-1811 misusing Quick Assist to deliver Black Basta ransomware (May 2024)](https://www.microsoft.com/en-us/security/blog/?p=134322)
- [NZQA: digital assessment device check](https://www.nzqa.govt.nz/ncea/ncea-exams-and-portfolios/external/device-readiness-check/)
- [NZQA: end-of-year student checklist (operating systems listed: Chrome OS, Windows, Mac OS)](https://www2.nzqa.govt.nz/assets/NCEA/External-assessment/end-of-year-student-checklist.pdf)
- [Morrison Foerster: Texas app store law takes effect after the Fifth Circuit stays the injunction](https://www.mofo.com/resources/insights/251111-texas-targets-app-stores-with-new-accountability-law)
- [Microsoft Learn: Artifact Signing quickstart (organisations in New Zealand are eligible)](https://learn.microsoft.com/azure/artifact-signing/quickstart)
- [Mozilla: retiring the Mozilla Location Service](https://discourse.mozilla.org/t/retiring-the-mozilla-location-service/128693)
- [The Update Framework](https://theupdateframework.io/)
- [EU AI Act, Article 50: transparency duties](https://artificialintelligenceact.eu/article/50/)
