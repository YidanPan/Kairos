# Kairos for Apple Silicon

The preview targets Apple Silicon Macs running macOS 12 or newer. The macOS build in GitHub Actions uploads a DMG named **kairos-macos-arm64-preview**. The preview uses an ad hoc signature for testing. A public macOS release requires Developer ID signing and Apple notarization.

The Mac panel offers Normal and Always on Top modes. Choose Solid, Light, Medium, or Transparent under **Settings → Appearance → Transparency**. The translucent presets use the macOS window backdrop and the selected theme's tint. Quick Add, Search, and Settings use Command in place of Control. Global shortcuts use Command+Option+Shift with the keys listed in **Settings → Behavior**.

## Desktop acceptance

- Drag Kairos into Applications and open it. Check that a second launch or clicking the Dock icon reopens the existing panel.
- Add and confirm a task, close the panel, and reopen the app. Verify the task and window settings persist.
- Check panel position, resizing, Normal and Always on Top, and restoring mouse interaction from the menu bar.
- Place the panel over a patterned background. Switch between the four transparency presets in both themes and verify the chosen setting survives relaunch.
- Open a task with an unset deadline, then choose a date and leave its time unset. Verify the form identifies both empty states, the native pickers are usable, and a reminder uses 23:59 when the deadline time is unset.
- Try global shortcuts from another app and check that enabling and disabling Launch at login takes effect on the next login.
- Schedule a reminder. Check the sound, the notification buttons, and delivery after sleep and wake. Leave Kairos running in the menu bar for these checks.
- Choose a backup folder, export a snapshot, and restore it in a test profile.
- If a second display is available, move the panel to it and check its position after disconnecting and reconnecting the display.

## Local build on a Mac

```sh
npm ci
APPLE_SIGNING_IDENTITY=- npm run tauri build -- --ci --target aarch64-apple-darwin
```
