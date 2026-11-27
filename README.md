# Lotus Download Manager

Lotus Download Manager is a free download manager for Windows and Android.
It uses segmented downloads, resume, pause and cancel, per-download and
global speed limits, and a single queue with status filters. A built-in
torrent client, browser integration, and tray support are on the roadmap.

## Getting it

Pre-built binaries are on the Releases page:

https://github.com/b0dr7/lotus-dm/releases

The website is at https://b0dr7.github.io/lotus-dm/ and links to the
same builds.

Windows: download LotusDM-windows-universal.exe (or the x64 / x86 build).
Android: download LotusDM-android-universal.apk (or the per-ABI APK).

## Requirements

- Windows 10 or 11.
- Android 6.0 or newer.

## Building from source

Desktop (Rust, eframe/egui):

    cd desktop
    cargo build --release

Android (Kotlin, Jetpack Compose):

    cd android
    ./gradlew assembleRelease

Both also build in GitHub Actions on every tag push.

## Browser extension

Firefox: install link on the website (listing in review).
Chromium: download the extension zip from the website and load it unpacked.
See the in-app Help panel for the full steps.

## Data and privacy

Downloads and settings are stored locally on your device. There is no
account, no telemetry, and no server in the middle. The browser extension
talks to the app over 127.0.0.1 only.

## Legal

Terms: TERMS.md
Privacy: PRIVACY.md
License: Apache License 2.0 (LICENSE)