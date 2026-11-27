# Publishing the Firefox build on addons.mozilla.org

Everything here is done by you, on your own Mozilla account. The repository only
supplies the unsigned archive:
`https://github.com/b0dr7/lotus-dm/releases/latest/download/LotusDM-extension-firefox.xpi`
(folder: `extension/firefox/`, manifest is MV2 with the id `lotus-dm@b0dr7.github.io`).

## 1. Account

1. Go to https://addons.mozilla.org and sign in (or create the account).
2. Open the developer hub: https://addons.mozilla.org/developers/
3. Accept the developer agreement once. No payment is needed; add-ons are free
   to list and sign.

## 2. Submit

1. Developer Hub -> **Submit a New Add-on**.
2. Choose **On this site** (listed) if you want the public listing and the
   one-click install button on the Lotus site. Choose **On your own** (unlisted)
   only if you just want a signed file to hand out yourself - unlisted builds
   never appear in search and are signed the same way.
3. Upload `LotusDM-extension-firefox.xpi` from the Releases page.
4. Wait for the automatic validation. Warnings are usually fine, errors are not.
   Typical warnings for this add-on: none expected; if you touched the manifest,
   read the message and fix the field it names.
5. Fill in the listing:
   - Name: `Lotus Download Manager`
   - Summary (max 250 chars): "Send links from Firefox to the Lotus Download
     Manager queue. Requires the Windows app running on the same machine."
   - Category: **Download Management**
   - Tags: download manager, download
   - License: match the repository licence (or "All Rights Reserved" if the repo
     has no licence file yet)
   - Homepage: https://b0dr7.github.io/lotus-dm/
   - Support email or the GitHub issues URL.
   - Privacy policy: https://b0dr7.github.io/lotus-dm/privacy/
6. Screenshots are optional but the review goes faster with one 1280x800 shot of
   the popup and one of the right-click menu.
7. Submit for review.

## 3. After approval

1. The hub shows the public listing URL -
   `https://addons.mozilla.org/firefox/addon/<slug>/`. Send that link back so the
   site button and the app's setup text can point at it.
2. Install it once yourself from that page to confirm the one-click flow, then
   click the toolbar icon with the app running: the badge must read **connected**.

## 4. Updates

1. Bump `version` in `extension/firefox/manifest.json` (for example `1.0.0` ->
   `1.0.1`), commit, tag a new release.
2. Wait for the release workflow, then in the hub open the add-on and use
   **Upload New Version**, pick the new `.xpi`, add release notes, submit.
3. AMO signs and publishes; Firefox updates installed copies automatically
   within a day.

## Notes

- Firefox refuses unsigned extensions. Never point the site's install button at
  the raw `.xpi` from GitHub - it only installs in Developer Edition or through
  `about:debugging` temporary loading.
- Keep the `browser_specific_settings.gecko.id` unchanged between versions, or
  AMO treats the update as a different add-on.
- `browser_specific_settings.gecko.data_collection_permissions` must stay in
  the manifest. It is `{"required": ["none"]}`, which is accurate: the extension
  talks to 127.0.0.1 and sends nothing to any server. Only change it if that
  ever stops being true.
- Icon sizes in `icons` must match the real pixel size of each file. The build
  takes them straight out of `desktop/assets/lotus.ico`, which has no 96px
  frame, so there is no 96 entry - do not add one pointing at the 128 file.
- The Chromium build is not involved here; it is distributed as a zip the user
  loads unpacked (see `extensions/index.html` on the site).
