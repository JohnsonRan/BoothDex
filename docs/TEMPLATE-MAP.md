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
   compiler: Astryx ships prebuilt CSS). The source build (`@astryxdesign/build`, `withAstryx` + `@astryx;`) was
   measured on 2026-11-06 and not adopted: under Turbopack it emits the same stylesheet (240,691 B, 40.0 KB gz,
   against 241,014 B / 39.9 KB gz prebuilt); under webpack 189,870 B / 38.6 KB gz, still every rule of the package
   (both compile every source file of `@astryxdesign/*`, not only the imported ones), at the cost of Turbopack and
   `next/font`.
2. Every screen starts from an Astryx page template; parts it lacks come from template blocks. Layout comes
   from the template, features from the old site. A surface no template covers is listed as a gap, not invented.
3. Every UI element is an Astryx component; site components only compose them. No theme overrides except
   documented fixes (listed under Deviations).
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
| Control size | `lg` (36 px) for the whole control block: search, tabs, sort and every filter clause; `sm` for the buttons inside a clause's editor |
| Type | neutral scale: page title `Heading level={1}` (2xl); tile title `Heading level={4}` (base) at outline level 2; shop `Text type="supporting"` (sm); price `Text type="body" weight="bold"` |
| Colour | neutral theme tokens only, light and dark via `light-dark()`; no overrides |
| Radius, elevation | component defaults |

## Surfaces

| Surface | Route / trigger | Astryx template | Status |
|---|---|---|---|
| Page frame | every route | page `shell-top-nav` (AppShell + TopNav, heading with master's logo; end actions: magnifier, language menu, currency menu, colour mode, account) | checkpoint 1 |
| Home grid | `/` | page `library` (Card Grid): header with title, search, filter tabs + sort row, card grid, empty state | phase 1 |
| Result tile | home grid | page `product-gallery`, `ProductCard` (square media; title, secondary line = shop icon and name, price `large` bold with a favourite `IconButton`); NEW and feature tags: `Badge` row under the price (block `BadgeCategoryTags`). The server renders the first 24 tiles; the rest of the page's 100 are drawn in the browser near the viewport, a tile silhouette holding their place | phase 1 |
| Search | home, top of content | page `library` search `TextInput` (block `TextInputSearch`) | phase 1 |
| Category / gender tabs, sort | home control row | page `library` (`ToggleButtonGroup` + `OverflowList`, `DropdownMenu`); a folded selected tab names the overflow menu | phase 1 |
| Filter clauses | under the tabs | page `table-filter` filter row (block `ToolbarTableFilter`): one closed trigger per filter that doubles as its chip, folding from the end into "+N" (`Popover`; `BottomSheet` on phones), then Clear all. Editors: the template's Quote popup with block `PopoverFilterPanel`'s staged Apply | checkpoint 1 |
| Avatar strip | home, items view | block `CarouselSnap` (`SelectableCard` per avatar, media only as the product-gallery card's media slot, name as caption below); the first 14 cards until the strip is touched, focused or scrolled | phase 1 |
| Pagination | under the grid | `Pagination` component; no template covers paging (gap) | phase 1 |
| Empty, filtered-empty, error, API starting | instead of the grid | block `EmptyStateActions` | phase 1 |
| Loading, refresh | home, while the search answers | `Skeleton` silhouettes of the count and of 24 tiles (doona `apps/lite` loading rules); refresh: indeterminate `ProgressBar` under the header, failed refresh: inline `Banner` with retry above the kept results | phase 1 |
| Item detail | `?item=<id>` | page `product-detail` inside a `Dialog` | planned |
| Command palette | Ctrl/Cmd+K, top-bar magnifier | page `shell-nav` wiring + blocks `CommandPaletteAsyncSearch`, `CommandPaletteRichItems` | checkpoint 1 |
| Sign-in / register | TopNav end "Sign in" | page `login-card` in a `Dialog` (TabList for the two forms) | checkpoint 1 |
| Account menu | TopNav end, signed in | `DropdownMenu` (now: name + Sign out; full menu planned) | partial |
| Language, item text, currency, colour mode | TopNav end | doona lite `LanguageMenu` / `ColorModeToggle` pattern: ghost IconButton + tooltip, `DropdownMenu` radio groups; currency as a ghost DropdownMenu button | checkpoint 1 |
| Settings (appearance, language, currency, item text) | TopNav end | page `settings-dialog` | planned |
| Favourites, recently viewed | account menu | page `library` with the same frame (views of the home grid) | planned |
| Admin login | `/admin` signed out | page `login-card` | planned |
| Admin console | `/admin` | page `shell-side-nav` + page `table-page` (Searchable Table) | planned |
| AI settings | admin console | page `settings` (Settings Form) | planned |
| Reset password | admin console | page `form-two-column` in a `Dialog` | planned |

## Deviations from the templates in phase 1

Theme fix (rule 3), the only style override:

- Fonts. The neutral theme's `fonts.css`, imported by the generated `src/astryx-themes.ts`, `@import`s Figtree
  from Google Fonts: a render-blocking request chain (~850 ms in Lighthouse). `next.config.ts` aliases that
  import (`turbopack.resolveAlias`) to the empty `src/app/theme-fonts.css`; `app/layout.tsx` loads Figtree with
  `next/font/google` (self-hosted, preloaded, `font-display: swap`); `app/globals.css` sets
  `--font-family-body` and `--font-family-heading` to Figtree's variable, then MiSans for Chinese and Japanese
  (loaded after the first covers are painted, as on master: `components/font-loader.tsx`, `lib/fonts.ts`), then
  the theme's own system stack. The generated module stays exact, so `astryx doctor` keeps 0 failures; its
  `theme-fonts` warning ("External font imports prevent proof for neutral: Figtree") is expected, since doctor
  reads the package file, not the alias. Editing the generated module instead fails two doctor checks
  (`theme-module`, `theme-fonts`), and `astryx theme add neutral` rewrites it with the import back, so the alias
  is the stable form: nothing needs re-applying after `theme add` or `theme use`. If a theme release renames
  `fonts.css`, update the alias key.

The rest are prop values, not style overrides:

- `AppShell height="auto"` and `Layout height="auto"`: the window scrolls instead of an inner region, so a
  phone's address bar collapses and the grid is one long page.
- Grid column floor 160 px instead of the template's 320 px: product tiles need two columns on a phone.
- Tile title `Heading level={4}` instead of level 2: level 2 is xl, too large for a 160 px tile; the outline
  level stays 2 through `accessibilityLevel`.
- The Card Grid template groups cards under per-category headings; BoothDex pages results instead, so there
  is one `Divider` above one grid.
- Product Gallery tiles have no badges; BoothDex adds NEW and up to two feature tags as a `Badge` row under
  the price (an `HStack` as in the Badge Category Tags block), only on tiles that have one. NEW is
  `variant="blue"` (as the Item With Media block's "New"), feature tags the default neutral. Master showed them
  over the cover; the Overlay Bottom Strip block does the same with `Overlay`, but Overlay is a client component
  and ~70 of them (one per marked tile on the first page) raised median Lighthouse TBT from 1669 to 2969 ms
  (5 runs each), so the row stays server-rendered. `Badge` is not for counts only: `astryx component Badge`
  documents status labels and category tags (blocks `BadgeStatusLabels`, `BadgeCategoryTags`); the
  `astryx init` rule "Badge = counts only" is shorthand for "not for metadata".
- The tile has no hover second image: `Card` has no media swap, and `Overlay showOn="hover"` keeps its
  content mounted (visibility is CSS), so every first-screen tile would download its second image, and it is a
  client component per tile (the TBT cost above). Listed for
  the owner.
- Results are not dimmed while a navigation is pending (no Astryx component exposes a pending state for a
  region). Owner ruling 2026-11-06: an indeterminate `ProgressBar` under the header instead, as in doona's
  `apps/lite`; the current results and the count stay until the next ones arrive.

Loading and motion (phase 1d, ported from doona `apps/lite` UI-RULES §2-§3), the only `style` props, all
structural or token-backed:

- Silhouettes (`catalog/silhouette.tsx`, `catalog/loading.tsx`): each text line is a box of
  `calc(var(--text-<type>-leading) * var(--text-<type>-size))` with a `Skeleton` of the font size inside; the
  badge row is `var(--spacing-5)` (Badge's height). The real tile keeps the same boxes: its title has
  `min-block-size: 2lh` (always two lines, as the clamp allows at most two) and its badge row stays, one badge
  tall, when the item has no badges. `tools/loading-layout.cjs` checks every box within 2 px.
- The refresh `ProgressBar` sits in a zero-height `VStack` (`position: relative`), so showing it moves nothing.
- The entrance observer (`catalog/arrival.tsx`) is a `display: contents` `VStack`.
- Toolbar: the tabs' `StackItem` has `min-width: 0` and the Filters and sort buttons are `StackItem
  size="static"`, so measuring the tabs' overflow on hydration no longer moves the buttons.
- Motion values live in `lib/motion.ts` (Astryx `--ease-standard`). Only content that replaces a silhouette the
  visitor saw enters; the server-rendered first screen (the LCP cover and the title) is never animated.

## Deviations in checkpoint 1 (frame and home)

Templates copied as is to `web/templates/` (not compiled); each adapted file names its template.

- Frame: no centre navigation (BoothDex has no sections; the template's mega menus are dropped). `contentPadding`
  is not set and the template's 1100 px content cap is not used: the home page is the full-width library layout,
  whose own `Layout padding={6}` gives the inset (rule 5).
- Logo: master's SVG mark at 28 px instead of the template's `NavIcon` (the mark is itself a filled shape). On
  phones (< 640 px) the wordmark is visually hidden by a documented rule in `globals.css` (`.site-heading`), as on
  master: TopNavHeading has no prop for it, and five end actions do not fit beside a truncated name at 390 px.
- End actions: the template's magnifier opens the palette instead of an inline search; the catalogue query keeps
  the library template's full-width TextInput in the page. Both record recent searches in one list.
- Currency: a ghost `DropdownMenu` button showing symbol and code ("¥ JPY") instead of an icon (owner: the coin
  icon was not understood); rows show symbol, code and the localised name.
- Palette: Astryx renders it as a centred dialog on phones too (master: full screen with a back arrow); Back,
  Esc and the backdrop close it. Removing a single recent search is not offered: palette rows may not contain
  buttons (CommandPalette rule); "Clear recent searches" stays. Picking an item opens it on Booth until the item
  dialog exists.
- Sign-in: the login card's centring frame and Card are dropped inside the Dialog (as the reference board
  notes); no provider buttons or divider (no third-party sign-in). Tabs (TabList) switch sign-in and register.
- Tile: secondary line is the shop (Avatar `xsm` + name) instead of a two-line description; the price row ends
  with the favourite button and is `--size-element-sm` tall in tile and silhouette alike.
- Avatar strip: `SelectableCard` is documented for multi-select; the strip is single-select (one avatar or
  all). Kept because it is the component with a selected state for media cards; ClickableCard has none.

## Filter area (checkpoint 1, track A)

Chosen structure: the Filterable Table template's default filter row, not its PowerSearch mode. The template
itself keeps PowerSearch behind a toggle as the expert query bar (typed `field operator value` tokens); its
default for a large filter set is the row of clause triggers, where a set clause is its own chip, so the
tokens and the controls are one control at one height, nothing wraps (the row folds into "+N"), and phones
get the folded set in a BottomSheet. The library template's search field and tabs stay above it (approved).
Copies: `web/templates/pages/table-filter/page.tsx`, `blocks/ToolbarTableFilter.tsx`,
`blocks/PopoverFilterPanel.tsx`; `blocks/PowerSearchPresetFilters.tsx` kept as the rejected alternative.

| Master filter | Clause | Not 1:1 |
|---|---|---|
| View: Items / Avatar bodies | preset `ToggleButton` "Avatar bodies" (hidden in favourites) | a toggle, not a two-way switch |
| Content rating | `Selector` (All ages / All) with `hasClear`; All ages is the unset default | - |
| Known avatars only (avatar view) | preset `ToggleButton`, pressed by default | - |
| Gender (avatar view) | the tabs row, as before | - |
| Category, smart counts | the tabs row; counts in each tab's tooltip and in the folded tabs' menu rows | counts are not printed on visible tabs: they arrive after the tabs and would widen them (layout shift) |
| Price: free, presets, min/max, swap | `ComplexSelector` (the template's Quote shell) with `CheckboxInput`, `RadioList`, two `TextInput`s, Clear / Apply | a preset is cleared by choosing "Any", not by clicking it again (RadioList); no × on the trigger (ComplexSelector has no `hasClear`, as the template's Quote) |
| Published range | `Selector` with `hasClear` | - |
| Features with counts | `MultiSelector`; counts as the option description | - |
| Include / exclude tags, search, counts | `ComplexSelector` whose editor is the tag picker; staged, Clear / Apply | the trigger shows tag names as stored (Japanese), not the display names |
| Shop (palette pick) | `Selector` holding the one shop, × removes it | master showed it as a token |
| Active-filter tokens | gone: each set clause is its chip; query, avatar and category are shown by the search field, the strip and the tabs | - |
| Mobile drawer, Clear, "Show {n} items" | the "+N" BottomSheet (`height="hug"`) ends with Clear and "Show {n} items" | on phones the row keeps whatever fits before "+N" |
| Sort | unchanged (library `DropdownMenu`) | - |

Deviations, all prop values or token-backed inline styles:

- Size `lg` instead of the template's `sm`: one size with the library row above (measured 36 px for search,
  tabs, sort, presets, selectors, "+N" at 1440 and 390).
- The template's StyleX rules (`filterChrome`, `filterSurface`, `filterFill`, the slot and readout sizing) are
  `style`/`controlStyle` objects of the same tokens (`catalog/filter-style.ts`): the site has no StyleX
  compiler. Popup editors are 300 px (the template's `quotePopover`). `READOUT_RESERVE` is 88 px (only Clear all; the count stays in the page header).
- The fold's Popover moves focus in only when opened by keyboard (`hasAutoFocus`); a click leaves focus on the
  trigger, so the first clause is not drawn focused. Clause popups keep their own focus handling.
- `BottomSheet` and the price and tag editors load on first open (`next/dynamic`), as the old filter panel did.
- Icons: all through the theme registry (`chevronDown`, `close`, `search`, lucide in the neutral theme);
  include/exclude in the tag editor are lucide `Plus`/`Minus` passed to `Icon`; checkmarks are CSS, no SVG.

## Remaining Tailwind or shadcn files

None. The rewrite removed Tailwind, shadcn and their dependencies.
