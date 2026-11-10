# maybe

ideas and potential features to look at later if needed.

### display change listener (`WM_DISPLAYCHANGE`)
hook `WM_DISPLAYCHANGE` (0x007E) so the active gamma ramp automatically re-applies when an external monitor is plugged in, unplugged, resolution changes, or after waking from sleep (windows graphics drivers like to reset gamma ramps back to 6500k on display events). also resizes the transparent dimmer overlay to match the new virtual screen bounds.

### global hotkeys
add `@tauri-apps/plugin-global-shortcut` so warmth and brightness can be adjusted without opening the window:
- `ctrl + alt + up/down`: adjust color temperature
- `ctrl + alt + left/right`: adjust brightness
- `ctrl + alt + p`: pause / resume filter

### hardware ddc/ci dimming
use windows `dxva2.dll` (`SetVCPFeature` code 0x10) to control physical monitor backlight over i2c for supported hdmi/displayport screens. saves power and preserves contrast on external monitors down to ~30%, then hands off to amber's software overlay for sub-hardware dimming.
