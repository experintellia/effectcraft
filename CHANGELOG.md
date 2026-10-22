# Changelog

Releases of the webxdc fork (`experintellia/effectcraft`, branch `webxdc`). Each release attaches `effectcraft.xdc`: send it to a Delta Chat chat to make animated stickers there. Tags are the fork's own versions, independent of upstream's `Cargo.toml` version.

## v0.1.0 — 2026-10-22

First release of EffectCraft as a webxdc app, built on upstream EffectCraft 0.14.0 (`storytold/effectcraft` `26b4019`).

### Stickers
- File ▸ Export ▸ Telegram Sticker… exports the active composition as an animated sticker (`.tgs`), a video sticker (`.webm`, VP9 with alpha) or a 100×100 custom emoji. It checks Telegram's limits (512 px, 3 s, 64 KB / 256 KB, and what animated stickers may contain) and lists the problems before writing anything ([#2](https://github.com/experintellia/effectcraft/pull/2)).
- An Animated Sticker 512×512 template (Home ▸ Templates, File ▸ New ▸ New Project from Template…): a looping bounce made only of what `.tgs` allows, with the ball's colour in Essential Graphics ([#5](https://github.com/experintellia/effectcraft/pull/5), [#8](https://github.com/experintellia/effectcraft/pull/8)).

### In a chat
- Exports go to the chat: a sticker from File ▸ Export ▸ Telegram Sticker… (or a render) is sent to a chat you pick ([#10](https://github.com/experintellia/effectcraft/pull/10)).
- Packaged as `effectcraft.xdc`, with the wasm stored pre-gzipped for Delta Chat. Frames render in Web Workers when the messenger allows them, and on the page otherwise ([#3](https://github.com/experintellia/effectcraft/pull/3)).
- Commands a messenger can't run (saving to disk, revealing files, watch folders, scripts, Quit) are left out of the menus; the project lives in the app's storage ([#3](https://github.com/experintellia/effectcraft/pull/3), [#10](https://github.com/experintellia/effectcraft/pull/10)).
- Opening a project (or the auto-save after a crash) keeps its footage instead of marking it all missing ([#10](https://github.com/experintellia/effectcraft/pull/10)).
- The Home screen fits a phone's width ([#3](https://github.com/experintellia/effectcraft/pull/3)).

### Phones and tablets
- One tap on a text field or a value you type into (Opacity in the Timeline, the Project search…) opens the on-screen keyboard on iOS. Drags and long presses that start on a field still scrub values, scroll panels and open context menus; no double-tap zoom ([#7](https://github.com/experintellia/effectcraft/pull/7)).

### Loading
- A loading bar shows each startup stage and how long it took, stays up with the failing stage marked when something goes wrong, and Help ▸ About keeps the timings after the app has loaded, to find out what makes loading slow on a device ([#6](https://github.com/experintellia/effectcraft/pull/6), [#8](https://github.com/experintellia/effectcraft/pull/8)).

### Known limits
- Without WebGPU (Delta Chat desktop blocks it on some GPUs, e.g. ARM GPUs on Linux) previews render on the CPU and play slowly. The editor's docked panels are cramped at phone width.
