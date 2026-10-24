# GLYPH security

**English** · [Russian](SECURITY.ru.md)

GLYPH is a local secret generator, not a password manager. It does not store
values on disk, does not sync and does not accept secrets from outside. The
threat model is the local machine: another user, a clipboard manager, someone
looking at the screen, a memory dump or malware.

## Guarantees

- Randomness comes only from the OS through `getrandom` (Windows CSPRNG).
  Indexes use rejection sampling without bias; shuffling is Fisher–Yates. A
  failure of the random source at any stage aborts generation; there is no weak
  fallback PRNG.
- Secrets do not implement `Debug` or `Serialize` and never reach widget names,
  status lines, tooltips or URLs. Metadata (mode, length, entropy, hidden count)
  is not a secret.
- Primary secret buffers are zeroized on history eviction, on session clearing
  and on a normal process exit.
- GLYPH makes no network requests and creates no files with secrets, history or
  logs; it has no telemetry and no update check. The only saved setting is the
  interface language in `HKCU\Software\GLYPH` (`en`/`ru`); it contains no
  secrets, history or generation parameters. The only external action is
  opening the fixed `https://t.me/rercon` in the browser on an explicit click.
  The address is a constant: no secret, length or entropy figure reaches it.
- Clipboard writes set the Windows `CanIncludeInClipboardHistory` and
  `CanUploadToCloudClipboard` markers, and clearing happens only while the
  clipboard sequence number still belongs to GLYPH. A later copy from another
  application is never erased.
- Passwords show a conservative lower-bound entropy estimate. Hidden zero-width
  characters are not included. Tokens use 8 bits per byte.
- `Cargo.lock` pins dependencies; CI checks known vulnerabilities with
  `cargo audit`, formatting, Clippy, tests and the PE artifact. The version in
  `Cargo.toml`, `icons/icon.rc` and the manifest cannot drift unnoticed.

## Limitations and residual risks

- **Copies in memory.** `zeroize` covers GLYPH's primary buffers. The text also
  exists in egui's layout cache, in the graphics stack and in the clipboard
  heap, and may remain in the pagefile. This is not protection against memory
  dumps, sleep mode or privileged malware.
- **The clipboard timer is best effort.** After 30 seconds GLYPH tries to clear
  the clipboard, retrying for up to 3 seconds. If the clipboard stays busy, the
  application stops tracking it and shows a warning: the secret may remain until
  the next copy from another application. Killing the process, a `panic` and a
  power loss skip the clear (`panic = "abort"` bypasses `Drop`).
- **Third-party clipboard managers.** The history/cloud opt-out markers are a
  Windows recommendation; other programs may ignore them and keep their own
  copy. GLYPH cannot delete foreign copies.
- **Ownership heuristic.** If a clipboard manager changes the sequence number
  without replacing the text, GLYPH treats the clipboard as foreign and leaves
  the secret. This is a deliberate choice to never erase someone else's data.
- **Session.** Up to 80 recent values intentionally stay in memory until the
  program exits. Masking hides them only on screen; a screenshot, a screen
  reader or a memory dump bypasses it. **Clear** releases the values but does
  not guarantee wiping every copy in the graphics stack.
- **Pixel mask.** The number of columns equals the visible length (length is
  already shown in the metadata); the pattern is built only from cell indexes
  and time, never from the characters themselves. The settle animation is a
  visual signal of a new value, not a strength indicator; it can only be
  disabled together with masking (the **show** button).
- **Hidden characters** are a compatibility feature, not protection. Sites,
  terminals and password managers may strip U+200B/C/D; the number actually
  inserted is shown, but the recipient may not preserve it.
- **Entropy estimate** assumes independent uniform draws. It does not account
  for weaknesses of a particular site, password reuse, leaks, phishing or human
  patterns. It is not a guarantee of strength.
- **Local adversary.** An administrator, malware, a keylogger, a process
  injector, a debugger or a screen-capture tool can read the secret around every
  GLYPH measure. The application does not elevate privileges or install hooks.
- **No code signing or auto-update.** Compare the release SHA-256 with the one in
  the release notes. Updates are never downloaded or installed automatically.
- **Build and toolchain trust.** Rust 1.99 MSVC and the pinned `Cargo.lock` are
  used; byte-for-byte reproducibility is not guaranteed. The PE check confirms
  the format, subsystem, static CRT and manifest, but not the absence of
  backdoors in the compiler or dependencies.

## Reporting a vulnerability

Write to the author on [Telegram](https://t.me/rercon). Include the GLYPH
version, reproduction steps and the expected/actual behaviour. Do not attach
real passwords, tokens, Fernet keys or clipboard contents — use synthetic
values. Please do not publish an exploitable detail before it is fixed.

The latest published release of the 0.1.x line is supported.
