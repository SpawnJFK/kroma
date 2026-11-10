# Kroma design rules

- Neutral dark UI (panel #323234, canvas surround #262628), one accent blue (#3884FF) for selection and active tool only.
- Canvas is the hero: panels stay quiet, no gradients, no decorative color.
- Tool icons are vector drawings in `icons.rs` on a 24-unit grid, 1.6 unit stroke. No emoji, no icon fonts.
- Labels use Photoshop vocabulary so users transfer muscle memory (Opacity, Flow, Fill, Feather, Tolerance, Contiguous).
- Dialogs: title, controls, then OK / Cancel bottom-left. Enter = OK, Esc = Cancel. Anything that changes pixels previews live.
- Status bar: zoom, document size, cursor position + color under cursor, then a one-line hint for the active tool.
- Every command is reachable three ways: menu, shortcut (where Photoshop has one), command palette.
