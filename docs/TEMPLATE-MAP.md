# Template map

Every route and surface of the site, the Astryx template it is built from, and the rules it follows. The web
frontend (`web/`) is being rebuilt on Astryx with the neutral theme; phase 1 is the home page. Surfaces marked
*planned* do not exist on this branch yet.

Template names are the ids from `bunx astryx template --list` (page templates) or block names under
`node_modules/@astryxdesign/cli/assets/templates/blocks/components/`. Live previews:
`https://astryx.atmeta.com/templates?preview=<id>`.

## Owner rules (apply to every screen)

1. Official setup only: `@astryxdesign/core`, `@stylexjs/stylex`, `@astryxdesign/theme-neutral`,
   `@astryxdesign/cli`; `astryx init`, `astryx theme add neutral`, plain-CSS path (no Tailwind, no StyleX
   compiler: Astryx ships prebuilt CSS).
2. Every screen starts from an Astryx page template; parts it lacks come from template blocks. Layout comes
   from the template, features from the old site. A surface no template covers is listed as a gap, not invented.
3. Every UI element is an Astryx component; site components only compose them. No theme overrides except
   documented fixes (none so far).
4. Spacing and sizes are Astryx tokens (spacing steps on component props). Raw pixels only for structural
   widths (grid column floor, card width).
5. One page frame for every route: `SiteFrame` (AppShell + TopNav). Each page renders its own `Layout` inside
   it with `padding={6}`, so titles and content share one left edge at every width.
6. Icons: the theme's semantic names (`search`, `funnel`, `error`, ...) first, otherwise lucide-react (the icon
   set the neutral theme itself uses). No hand-drawn glyphs.
7. Appearance: System (default), Light, Dark from the settings cookie; a head script sets `data-theme` and
   `lang` before first paint, and the root `Theme` gets the same mode from the server.
8. This file lists every surface before it is built.
9. Astryx is MIT: `web/THIRD-PARTY-NOTICES.md` names each adapted template and carries the licence.

## Token spec (neutral defaults)

| Aspect | Value |
|---|---|
| Page inset | `Layout padding={6}` on header and content |
| Rhythm | `VStack gap={6}` between page sections, `gap={4}` within the control block, `gap={2}` between a tile's cover and text, `gap={1}` between text lines |
| Grid | `Grid columns={{minWidth: 160}} gap={4}`: two columns on a 390 px phone, six to seven at 1440 px |
| Control size | `lg` in the page control row (search, tabs, filter, sort), `sm` inside the filter panel |
| Type | neutral scale: page title `Heading level={1}` (2xl); tile title `Heading level={4}` (base) at outline level 2; shop `Text type="supporting"` (sm); price `Text type="body" weight="bold"` |
| Colour | neutral theme tokens only, light and dark via `light-dark()`; no overrides |
| Radius, elevation | component defaults |

## Surfaces

| Surface | Route / trigger | Astryx template | Status |
|---|---|---|---|
| Page frame | every route | page `shell-top-nav` (AppShell + TopNav, heading with logo) | phase 1 |
| Home grid | `/` | page `library` (Card Grid): header with title, search, filter tabs + sort row, card grid, empty state | phase 1 |
| Result tile | home grid | page `product-gallery`, `ProductCard` (square media, title, supporting line, price) | phase 1 |
| Search | home, top of content | page `library` search `TextInput` (block `TextInputSearch`) | phase 1 |
| Category / gender tabs, sort | home control row | page `library` (`ToggleButtonGroup` + `OverflowList`, `DropdownMenu`) | phase 1 |
| Filter panel | "Filters" button | block `PopoverFilterPanel` (`presentation="adaptive"`: popover on desktop, bottom sheet on phones) | phase 1 |
| Active filters | under the control row | block `TokenRemovable` | phase 1 |
| Avatar strip | home, items view | block `CarouselSnap` (`SelectableCard` per avatar) | phase 1 |
| Pagination | under the grid | `Pagination` component; no template covers paging (gap) | phase 1 |
| Empty, filtered-empty, error, API starting | instead of the grid | block `EmptyStateActions` | phase 1 |
| Item detail | `?item=<id>` | page `product-detail` inside a `Dialog` | planned |
| Command palette | Ctrl+K, search button | block `CommandPaletteAsyncSearch` | planned |
| Account menu, sign-in | TopNav end | `DropdownMenu` in TopNav `endContent`; sign-in page `login-card` in a `Dialog` | planned |
| Settings (appearance, language, currency, item text) | TopNav end | page `settings-dialog` | planned |
| Favourites, recently viewed | account menu | page `library` with the same frame (views of the home grid) | planned |
| Admin login | `/admin` signed out | page `login-card` | planned |
| Admin console | `/admin` | page `shell-side-nav` + page `table-page` (Searchable Table) | planned |
| AI settings | admin console | page `settings` (Settings Form) | planned |
| Reset password | admin console | page `form-two-column` in a `Dialog` | planned |

## Deviations from the templates in phase 1

Each is a prop value, not a style override:

- `AppShell height="auto"` and `Layout height="auto"`: the window scrolls instead of an inner region, so a
  phone's address bar collapses and the grid is one long page.
- Grid column floor 160 px instead of the template's 320 px: product tiles need two columns on a phone.
- Tile title `Heading level={4}` instead of level 2: level 2 is xl, too large for a 160 px tile; the outline
  level stays 2 through `accessibilityLevel`.
- The Card Grid template groups cards under per-category headings; BoothDex pages results instead, so there
  is one `Divider` above one grid.
- Product Gallery tiles have no badges; feature tags and NEW markers were dropped from tiles because
  `Badge` is for counts only (they belong in item detail).

## Remaining Tailwind or shadcn files

None. The rewrite removed Tailwind, shadcn and their dependencies.
