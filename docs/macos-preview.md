# Kairos for Apple Silicon

The preview targets Apple Silicon Macs running macOS 12 or newer. The macOS build in GitHub Actions uploads a DMG named **kairos-macos-arm64-preview**. The preview uses an ad hoc signature for testing. A public macOS release requires Developer ID signing and Apple notarization.

The Mac panel uses a solid background and offers Normal and Always on Top modes. Quick Add, Search, and Settings use Command in place of Control. Global shortcuts use Command+Option+Shift with the keys listed in **Settings → Behavior**.

## Desktop acceptance

- Drag Kairos into Applications and open it. Check that a second launch or clicking the Dock icon reopens the existing panel.
- Add and confirm a task, close the panel, and reopen the app. Verify the task and window settings persist.
- Check panel position, resizing, Normal and Always on Top, and restoring mouse interaction from the menu bar.
- Try global shortcuts from another app and check that enabling and disabling Launch at login takes effect on the next login.
- Schedule a reminder. Check the sound, the notification buttons, and delivery after sleep and wake. Leave Kairos running in the menu bar for these checks.
- Choose a backup folder, export a snapshot, and restore it in a test profile.
- If a second display is available, move the panel to it and check its position after disconnecting and reconnecting the display.

## Local build on a Mac

```sh
npm ci
APPLE_SIGNING_IDENTITY=- npm run tauri build -- --ci --target aarch64-apple-darwin
```
