# Spare Keys: architecture map

*Stage: Idea.*

Spare Keys keeps every password, sign-in code and passkey you own in one place, free for everyone. It replaces paid password managers and separate authenticator apps (1Password, LastPass, Dashlane, Google Authenticator, Authy and similar). The one idea that makes it better: your vault is encrypted on your own device before it is stored anywhere, so nobody else, including Spareware, can ever read it, and you can take it with you in an open format at any time.

Signing in to Spareware itself is already planned in the shared kit (passkeys, backup codes, a list of your devices). Spare Keys is for all your other accounts.

## Goals

- **Passwords:** save, fill and generate strong passwords in browsers and apps on every device.
- **Authenticator codes:** the six-digit codes many sites ask for (TOTP and HOTP), with secure backup so losing a phone never locks you out.
- **Passkeys:** create, store, sync and use passkeys for any site that supports them, so people can move away from passwords.
- **Free for everyone, works offline**, with encrypted sync between your devices. The only cost is running the sync servers, shown on the open cost ledger.
- **Share safely:** share a login with family or a team without showing the password, and hand access to a trusted person in an emergency.
- **Leave any time:** export everything in an open, documented format, and import from the big password managers.

## What it uses from the shared kit

- Accounts and passkey sign-in, devices and recovery.
- Sync and storage, holding encrypted data only: the server stores data it cannot read.
- The command layer and open API, so scripts and AI agents can ask for a login only with your approval on each request.
- Languages, accessibility and the one design look.

## Its own parts

**Vault and encryption.** Everything is encrypted on the device with a key only you hold; the server only ever sees encrypted data. It uses standard, audited cryptography libraries and a well-reviewed design from the open projects below, never home-made cryptography. Discuss: which key derivation and recovery method gives the best mix of safety and help for people who forget their master password.

**Autofill.** Browser extensions and the phone systems' own published autofill and passkey interfaces, so it works everywhere people sign in.

**Authenticator codes.** Standard TOTP and HOTP (RFC 6238 and RFC 4226), scanning setup QR codes, with encrypted backup and sync.

**Passkeys.** A passkey provider following the published WebAuthn and FIDO standards, including the standard for moving passkeys between managers.

**Sharing and emergency access.** End-to-end encrypted sharing with people and teams, and a waiting period before a trusted person gets emergency access.

**Security health.** Warns about reused, weak or breached passwords, using privacy-preserving breach checks that never send your password.

## The industry's hard parts

- **Trust:** a password manager must be audited by outside security experts before anyone relies on it. Independent audits are a release requirement, not an extra.
- **Recovery without a back door:** helping people who lose their master password without giving anyone else a way in.
- **Platform rules:** autofill and passkey providers depend on what each operating system and browser allows.
- **Licences:** most strong open projects here are GPL or AGPL, so they are learn-from only for our core.

## Learn from (licences to confirm)

| Project | What it is | How we can use it |
| --- | --- | --- |
| Bitwarden | Open password manager with passkeys | Learn from only (GPL and AGPL parts) |
| KeePassXC and the KeePass format | Offline password manager and a widely used open file format | Learn from; read and write the KDBX format for import and export |
| Aegis, Ente Auth | Open authenticator apps | Learn from only (GPL, AGPL) |
| FIDO Alliance and W3C WebAuthn | Passkey standards | Follow the published standards |

## Open decisions

| Decision | Options | Lean |
| --- | --- | --- |
| Own app, or part of the account portal | Separate app; a section of the account portal | Separate app on the kit, usable without other Spareware apps |
| Vault format | KDBX-compatible; our own documented format | Research first |
| Recovery | Saved recovery kit; trusted contacts; none | Research first |
| First release | Authenticator codes first; full password manager first | Research first |
