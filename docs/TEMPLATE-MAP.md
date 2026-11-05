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
6. Icons: all lucide (owner: 「全 lucide」), always through Astryx's `Icon`. The theme's semantic names
   (`search`, `funnel`, `error`, ...) first; the neutral registry maps all 29 of them to lucide, so components'
   internal glyphs (chevrons, close, check) are lucide too and none falls back to core's heroicons. Glyphs with
   no semantic name are registered once under `site:` (`lib/icons.tsx`, table below). No hand-drawn glyphs
   (the logo is the brand mark, not an icon).
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
| Avatar strip | home, items view | block `CarouselSnap` (`SelectableCard` per avatar exactly as page `product-detail`'s image thumbnails, lines 117-150: square `AspectRatio` > `SelectableCard variant="transparent" padding={0}` filling it > image filling and covering; name as caption below); the first 14 cards until the strip is touched, focused or scrolled | phase 1 |
| Item detail | `?item=<id>`, tile click, palette item | page `product-detail` inside a `Dialog` (blocks `DialogAdaptivePresentation`, `DialogFullscreenDialog`, `DialogWithSubtitle`): gallery (block `CarouselSnap` hero, `SelectableCard` thumbnails, block `LightboxGallery` zoom), info column (SegmentedControl language tabs, display-2 title, rating row as likes, price row, `Badge` marks, `Banner` avatar fit, two lg actions, `CollapsibleGroup` sections with blocks `ListBasicList` and `MetadataListBasicMetadata`, `Token` chips), "More from" `Carousel` strip; full screen with a `LayoutFooter` bar below lg | phase 2 |
| Pagination | under the grid | `Pagination` component; no template covers paging (gap) | phase 1 |
| Empty, filtered-empty, error, API starting | instead of the grid | block `EmptyStateActions` | phase 1 |
| Loading, refresh | home, while the search answers | `Skeleton` silhouettes of the count and of 24 tiles (doona `apps/lite` loading rules); refresh: nothing for 150 ms, then a `Spinner size="sm"` beside the count held at least 300 ms, the new count and results fading in (`lib/use-delayed.ts`); failed refresh: inline `Banner` with retry above the kept results | phase 1 |
| Command palette | Ctrl/Cmd+K, top-bar magnifier | page `shell-nav` wiring + blocks `CommandPaletteAsyncSearch`, `CommandPaletteRichItems` | checkpoint 1 |
| Sign-in / register | TopNav end "Sign in" | page `login-card` in a `Dialog` (TabList for the two forms) | checkpoint 1 |
| Account menu | TopNav end, signed in | `DropdownMenu` with sections (block `DropdownMenuWithSections`): favourites, recently viewed, saved searches (block `DialogFormDialog`), admin console, settings, sign out | track D |
| Language, item text, currency, colour mode | TopNav end | doona lite `LanguageMenu` / `ColorModeToggle` pattern: ghost IconButton + tooltip, `DropdownMenu` radio groups; currency as a ghost DropdownMenu button | checkpoint 1 |
| Settings (appearance, language, currency, item text) | TopNav end, account menu | page `settings-dialog`; synced to the account when signed in | track D |
| Favourites, recently viewed | account menu | page `library` with the same frame (views of the home grid; recently viewed is read in the browser) | track D |
| Admin frame | `/admin/*` | page `shell-side-nav` (collapsible, resizable `SideNav`, two titled groups, footer: back to site, language, colour mode; phone drawer from `AppShell`) | track F |
| Admin sign-in, not an admin, unreachable | `/admin/*` before the probe succeeds | `EmptyState` inside the frame; sign-in opens the site's `login-card` dialog, sign-in only under the admin heading | track F |
| Admin overview | `/admin` | page `dashboard` (`MetricCard` grid, `Divider`, titled section of meters with `ProgressBar`); header `StatusDot` + text as page `incident-console` | track F |
| Crawler tasks | `/admin/jobs` | page `incident-console` list rows (`List` compact with dividers, status `Token`); stop and run options through blocks `AlertDialogAsyncAction` and `DialogFormDialog` | track F |
| Crawler history | `/admin/jobs/history` | page `incident-console` rows and `EmptyState`; kind and status `Selector`s in the URL; details in a `Dialog` with the inspector's `MetadataList` | track F |
| Admin settings (AI provider, reset password) | `/admin/settings` | page `settings` (two-column sections with `Divider`); key overwrite and reset behind `AlertDialog` | track F |

## Deviations from the templates in phase 1

Theme fix (rule 3), the only style override:

- Fonts. The neutral theme's `fonts.css`, imported by the generated `src/astryx-themes.ts`, `@import`s Figtree
  from Google Fonts: a render-blocking request chain (~850 ms in Lighthouse). `next.config.ts` aliases that
  import (`turbopack.resolveAlias`) to the empty `src/app/theme-fonts.css`; `app/layout.tsx` loads Figtree with
  `next/font/google` (self-hosted, preloaded, `font-display: swap`); `app/globals.css` sets
  `--font-family-body` and `--font-family-heading` to Figtree's variable, followed by the theme's own system
  stack (Chinese, Japanese and Korean use the system's fonts, the theme's supported fallback). The generated module stays exact, so `astryx doctor` keeps 0 failures; its
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
  `apps/lite`; replaced in checkpoint 1 (owner: the bar on every change was ugly) by the 150/300 ms rule and a
  small `Spinner` beside the count. The current results and the count stay until the next ones arrive.

Loading and motion (phase 1d, ported from doona `apps/lite` UI-RULES §2-§3), the only `style` props, all
structural or token-backed:

- Silhouettes (`catalog/silhouette.tsx`, `catalog/loading.tsx`): each text line is a box of
  `calc(var(--text-<type>-leading) * var(--text-<type>-size))` with a `Skeleton` of the font size inside; the
  badge row is `var(--spacing-5)` (Badge's height). The real tile keeps the same boxes: its title has
  `min-block-size: 2lh` (always two lines, as the clamp allows at most two) and its badge row stays, one badge
  tall, when the item has no badges. `tools/loading-layout.cjs` checks every box within 2 px.
- The refresh indicator's slot (`catalog/refresh.tsx`) is always laid out, 16 px square, beside the count; the
  `Spinner` is mounted only while shown, so nothing animates at rest. The count's `Swap` has `min-width: 6em`
  (the count silhouette's width).
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
- Tile: secondary line is the shop (icon + name) instead of a two-line description; the price row ends
  with the favourite button and is `--size-element-sm` tall in tile and silhouette alike. The shop icon is
  server markup (a 20 px round crop on `--color-background-muted`, Avatar `xsm`'s size and shape) instead of
  `Avatar`: Avatar is a client component, and with the per-tile favourite handler it put the home page over the
  380 KB JS budget (390,065 B).
  For the same reason the favourite button is a server-rendered `IconButton` (`data-fav="<id>"`): one document
  listener (`account/favourites.tsx`) toggles it through the session and sets `aria-pressed` on saved items,
  which fills the heart by CSS (the pressed icon of block `ToggleButtonShowcase`); notices load with the first.
- Top bar inset: the template leaves TopNav at its own 8 px while its content sits at 24 px. One `style`
  (`padding-inline: var(--spacing-4)`) puts the logo on the content's 24 px edge and the last borderless
  action's label within its own padding (~5 px) of the right edge (rule 5). All end actions stay ghost, size md, as the template's (lines 186-194).
- Icons are registered with `registerIcons()`, not `defineTheme({ extends: neutralTheme, icons })` as Astryx's
  dev warning suggests: extending the prebuilt theme makes it an unbuilt theme whose CSS `Theme` generates and
  injects at run time, on top of the shipped `theme.css`.
- Tile title: `weight="normal"` on the level-4 Heading, the weight the tiles were approved with at checkpoint 1.
- Avatar strip: `SelectableCard` is documented for multi-select; the strip is single-select (one avatar or
  all). Kept because it is the component with a selected state for media cards; ClickableCard has none.

## Icons

| Use | Name | lucide glyph |
|---|---|---|
| Quick search, search field | `search` (theme) | Search |
| Filters | `funnel` (theme) | Filter |
| Palette: recent search | `clock` (theme) | Clock |
| Favourite (tiles, item dialog; saved: filled by CSS on `aria-pressed`) | `site:favourite` | Heart |
| Language menu | `site:language` | Languages |
| Colour mode | `site:dark` / `site:light` | Moon / Sun |
| Avatar strip "All", avatar without image | `site:allAvatars` | LayoutGrid |
| Palette: tag row, clear recent | `site:tag`, `site:clear` | Hash, Trash2 |
| Component internals (chevrons, close, check, carousel arrows, pagination) | theme semantic names | ChevronDown/Left/Right, X, Check, ... |

The tag editor passes lucide `Plus`/`Minus` to `Icon` (filter area below). Sizes: `sm` in the top bar, tiles
and menus (as shell-top-nav), `lg` for the strip's "All" glyph.

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

## Deviations in phase 2 (item detail)

- Container: the template is a page; here it is a `Dialog` 1200 px wide (the template's `contentWidth`) and 92dvh
  tall (master), the fullscreen variant below 1024 px (master's lg). On phones the actions move to a
  `LayoutFooter` bar (price, favourite, Buy on Booth), as master's sticky bar.
- Header: `DialogHeader` names the shop (avatar as start content, host as subtitle, "All items from shop" as end
  content); the dialog's accessible name is the item title. Master had the shop block at the top of the info
  column.
- Hero: a snapping `Carousel` of square `AspectRatio` slides instead of one image, so phones swipe; each slide is a muted `ClickableCard` opening the Lightbox, the image
  `contain`ed (a product photo is not cropped; the template's hero is `cover`). Download (`DropdownMenu`) and the
  counter sit in a row under it, the counter between previous and next ghost `IconButton`s: the Carousel's own
  buttons live in the top layer and floated over the page behind the dialog when the hero scrolled, and no
  template shows a gallery in a dialog. Thumbnails, arrow buttons and sticky columns only from lg (master hid
  thumbnails on phones; a single column cannot stick); past about three rows the thumbnails scroll in place
  (master: max-h-40), and so does a long variation list from lg (phones show it whole).
- Sticky: the template's `stickyInfo` is put on both columns, so whichever is shorter stays in view.
- Description: inside the `CollapsibleGroup` (first section, open) instead of the template's large text above
  the options: Booth descriptions run to hundreds of lines and would push the actions off screen. It is clamped
  to 12 lines with Show more / Show less.
- Options: the template's glaze/finish `SegmentedControl`s become the item-text language tabs (Original and the
  translated languages); quantity and Add to Cart have no counterpart. Rating row: likes with the favourite
  glyph instead of stars.
- "More from {shop}": a `Carousel` of the avatar strip's card pattern with `ClickableCard` (it opens an item; no
  selection), under the two columns. From lg its heading row holds ghost scroll `IconButton`s with the
  Carousel's own labels, in place of the Carousel's top-layer buttons (they floated over the page when the
  dialog scrolled the strip away).
- Tiles: the root carries `data-item` and the title is a plain `<a href="?item=">` in server markup (keyboard
  focus, new tab on a modified click); one document listener in `item/item-host.tsx` opens the dialog, as
  `account/favourites.tsx` answers the favourite buttons. Astryx `Link` and `ClickableCard` are client components, which
  the server tiles avoid (TBT).
- `?item=` uses the browser history API directly (it integrates with the Next router), so opening an item does
  not render the page again on the server.


## Remaining Tailwind or shadcn files

None. The rewrite removed Tailwind, shadcn and their dependencies.
