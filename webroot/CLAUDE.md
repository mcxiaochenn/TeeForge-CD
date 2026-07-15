# TeeForge WebUI

Astro-based WebUI for TeeForge KernelSU module.

## Development

```bash
cd webroot
npm install
npm run dev        # dev server
npm run build      # build to module/webroot/
```

Use background mode for dev server:
```
astro dev --background
```

Manage: `astro dev stop`, `astro dev status`, `astro dev logs`.

## Architecture

### Stack
- **Astro 7.x** — static site generator, outputs to `module/webroot/`
- **kernelsu** npm — JS bridge to KernelSU's root shell (`exec`, `spawn`, `toast`)
- **No framework** — vanilla Astro components + plain CSS

### File Structure
```
src/
├── i18n/               # Translation dictionaries (source, copied to public/)
├── styles/
│   ├── global.css      # Reset, layout, dialog, button base styles
│   ├── theme-material.css  # Material 3 tokens (light + dark)
│   └── theme-cyber.css     # Cyberpunk tokens (always dark)
├── components/
│   ├── Header.astro    # Logo + language/theme selectors
│   ├── StatusCard.astro    # Device info (root, version, arch, keybox)
│   ├── ActionCard.astro    # Reusable button card
│   └── LogDialog.astro     # Modal dialog for streaming command output
├── layouts/
│   └── Base.astro      # HTML skeleton, theme init, i18n runtime
└── pages/
    └── index.astro     # Main page, composes all components
```

### Key Patterns

**Theme switching**: `<html data-theme="material|cyber" data-mode="auto|light|dark">`
- CSS variables drive all colors/radii/shadows
- Theme init runs in `<head>` before paint (prevents flash)
- User choice saved to `localStorage('tf-theme')`

**i18n**: JSON dictionaries in `public/i18n/{lang}.json`
- `window.__tf` API: `t(key)`, `setLang(code)`, `applyI18n()`
- DOM elements marked with `data-i18n="key"` attribute
- Auto-detect via `navigator.language`, user override saved to `localStorage('tf-lang')`

**Command execution**: `ksu.spawn()` for streaming, `ksu.exec()` for one-shot
- Module path hardcoded: `/data/adb/modules/teeforge_cd/teeforge`
- Streaming: `child.stdout.on('data')` → append to LogDialog
- Exit handling: `child.on('exit', code)` → success/fail indicator

**External resources**: All via `cdn.jsdelivr.net`
- `@fontsource-variable/inter` (Material 3)
- `@fontsource-variable/jetbrains-mono` (Cyberpunk)

### Build
- `npm run build` → outputs to `../module/webroot/`
- `module/webroot/` is gitignored (build artifact)
- i18n JSON files in `public/i18n/` are copied as-is to output

## Documentation

- Astro: https://docs.astro.build
- KernelSU WebUI: https://kernelsu.org/zh_CN/guide/module-webui.html
- KernelSU JS API: https://www.npmjs.com/package/kernelsu
