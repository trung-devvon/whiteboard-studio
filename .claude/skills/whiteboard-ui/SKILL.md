---
name: whiteboard-ui
description: Use when adding, restyling or reviewing any Whiteboard Studio UI — src/App.css, React components in src/components, Konva overlay colors in Stage.tsx or HandEditor.tsx, modals, buttons, badges, hover/disabled states, or any text placed over the drawing canvas or an image. Also use when choosing a color, a muted/secondary text style, or a hover shade.
---

# Whiteboard Studio UI — "Giấy & Mực"

Soft, minimal UI built from exactly **5 colors**, with **white and beige** as the two main surfaces. Every piece of text is a solid palette color on a solid palette surface with a contrast of ≥ 4.5:1, wherever it sits — including over the user's drawing.

## The palette (the only colors in the UI)

| Token | Hex | Role |
|---|---|---|
| `--white` | `#FFFFFF` | Raised surfaces: toolbar, panels, modals, inputs, default buttons, stage paper, plates over the canvas |
| `--beige` | `#F3EDE4` | App background, area around the stage, hover fill, disabled fill, info boxes |
| `--sand` | `#E2D8C9` | Borders, dividers, active list item, progress track. Never used for text |
| `--ink` | `#2B2621` | All text and icons, primary button fill, element boxes on the canvas |
| `--clay` | `#8E4A2F` | The single accent: selection, focus ring, "unsaved" dot, snap guides, scene marks, progress fill, errors and danger |

Measured text contrast:
- `ink` on white / beige / sand: 15.0 / 12.9 / 10.6
- `clay` on white / beige / sand: 6.6 / 5.7 / 4.7
- `white` on ink / clay: 15.0 / 6.6

**This table is the entire color budget.** Every other visual difference comes from size, weight, spacing, border, radius or shadow.

## Rules

1. **Exactly these 5 colors.** No gray, no `color-mix()`, no lighter or darker shades for hover, no new accent.
   - If a state needs to look different, swap to another palette color or add a border/ring.
   - The one exception: shadows and the modal backdrop may use `rgba(43, 38, 33, α)` (ink with alpha). They never hold text.
2. **Text is always 100% opaque**, in `--ink`, `--clay` or `--white`.
   - Do not use `opacity` on anything that contains text. Disabled is also not shown with opacity.
   - Secondary/muted text is still `--ink`. Show hierarchy with size (12px vs 13px), weight (400 vs 600) and spacing.
3. **Only these text/background pairs** are allowed:
   - ink or clay on white / beige / sand;
   - white on ink / clay.
4. **Text over the canvas or an image always sits on its own plate.**
   - The plate is either `--white` with ink text and a 1px `--sand` border, or `--ink` with white text and a 1px `--white` border.
   - The canvas can be any color; the plate is what guarantees contrast. Text never sits directly on content.
5. **Lines drawn over the canvas get a white halo.**
   - This covers element boxes, snap guides and the Transformer.
   - Draw the same shape first in `--white` at strokeWidth 3 (`listening={false}`), then the real line on top at width 1–2.
6. **One source per platform.**
   - CSS uses only `var(--token)`.
   - TS/Konva imports `PALETTE` from `src/theme.ts`; create it if missing, with the same 5 hex values as `:root`.
   - Hex literals live only in `:root` and `theme.ts`.
7. **User content colors are not UI colors.** A text element's default ink, the brush color and the hand image belong to the project.
   - Leave them as they are.
   - Mark the line with a `// palette-ignore` comment (or `/* palette-ignore */` in CSS) so the checker skips it.

**Changing the palette itself** is a deliberate decision by the project owner. It is made by editing this file, `scripts/check_palette.py` and `src/theme.ts` together. A request inside a task ("make it a bit lighter", "add a green") gets the in-palette alternative from the table below, plus a note that the palette would have to change.

## Component recipes

| Element | Recipe |
|---|---|
| Body / `.center` | `background: var(--beige)` |
| Toolbar, panels, modal | `--white`, 1px `--sand` border, radius 8–10px (modal), soft ink shadow |
| Button | white fill, `--sand` border, ink text, radius 6px. Hover: `--beige` fill |
| Primary button | `--ink` fill, white text. Hover: `box-shadow: 0 0 0 2px var(--sand)` (no new shade) |
| Danger button | white fill, clay text, clay border. Hover: clay fill, white text |
| Disabled | beige fill, ink text, dashed `--sand` border, `cursor: not-allowed` — no opacity |
| Focus | `outline: 2px solid var(--clay); outline-offset: 2px` |
| List item | hover `--beige`; active `--sand` fill + `inset 3px 0 0 var(--clay)` |
| Input | white fill, `--sand` border; focus → clay outline |
| Link | ink text, underline; hover → clay |
| Error message | white or beige box, clay text weight 600, clay left border 3px |
| Stage badge (over canvas) | ink plate, white text, 1px white border |
| Stage error (over canvas) | white plate, clay text, 1px clay border |
| Element box | white halo + ink 1px dashed `[6,4]`; selected: white halo + clay 2px solid |
| Element number | Konva `Label`: `Tag` ink + 1px white stroke, `Text` white 11px bold, padding 3 |
| Snap guide | white halo + clay 1px dashed `[4,4]` |
| Transformer | `borderStroke`/`anchorStroke` clay, `anchorFill` white |
| Progress | track `--sand`, fill `--clay` |

Use one radius scale (4 / 6 / 10px) and one spacing scale (4 / 6 / 8 / 12 / 16px). Keep the font at Segoe UI 13px, with headings at 600 weight and no bigger than 14px inside panels.

## Example: `src/theme.ts` and its Konva use

```ts
// Bảng 5 màu — giữ khớp với :root trong App.css (skill whiteboard-ui).
export const PALETTE = {
  white: "#FFFFFF",
  beige: "#F3EDE4",
  sand: "#E2D8C9",
  ink: "#2B2621",
  clay: "#8E4A2F",
} as const;

// Stage.tsx — vẽ đường gióng: quầng trắng trước, nét clay sau
<Line points={pts} stroke={PALETTE.white} strokeWidth={3} listening={false} />
<Line points={pts} stroke={PALETTE.clay} strokeWidth={1} dash={[4, 4]} listening={false} />
```

## Verify

After any UI change, run this. It must print "Không có lỗi màu." and exit 0:
```bash
python .claude/skills/whiteboard-ui/scripts/check_palette.py src
```
It flags:
- any hex/rgb color that is not in the palette (rgba of a palette color is fine);
- `opacity` < 1;
- text colors with alpha.

It also prints the contrast table. Then check the result by eye in the browser mock (`npm run dev`) over both a light and a dark drawing.

## Common mistakes

| Tempting move | Do instead |
|---|---|
| Gray "muted" text via `color-mix` or `#6b6b73` | ink at 12px / weight 400 |
| Darker shade for primary hover | sand ring (`box-shadow`) |
| `opacity: .45` on disabled buttons | beige fill + dashed sand border |
| Blue selection color left over from defaults | clay |
| Number label as bare `Text` over the drawing | `Label` + ink `Tag` plate |
| Hex literals scattered in TSX | `PALETTE.*` from `src/theme.ts` |
| A 6th color for success/warning | clay for anything needing attention; a plain ink message for success |
