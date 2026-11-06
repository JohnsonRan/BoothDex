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
| Grid | page `product-gallery` (page.tsx:149) at master's density (owner: 「太大了。為什麼一頁只有4張圖了」): `minWidth` 160 (10rem, `lib/grid.ts`), `gap={4}`: two columns on a 390 px phone (the floor is also at most half the row less one gap), six at 1280-1440 px, nine at 1920 px (115 % root) |
| Density | `html { font-size: 115% }` in `globals.css` is the one knob (110 / 115 / 125 %): every rem token follows it, and `--size-element-sm/md/lg` are restated as 1.75 / 2 / 2.25rem so controls scale with their text |
| Control size | `sm` for the whole toolbar row, as the table-filter template: search, every filter clause, presets, sort, save and saved views |
| Type | neutral scale: page title `Heading level={1}` (2xl); tile title `Heading level={4}` (base) at outline level 2; shop `Text type="supporting"` (sm); price `Text type="body" weight="bold"` |
| Colour | neutral theme tokens only, light and dark via `light-dark()`; no overrides |
| Radius, elevation | component defaults |

## Surfaces

| Surface | Route / trigger | Astryx template | Status |
|---|---|---|---|
| Page frame | every route | page `shell-top-nav` (AppShell + TopNav, heading with master's logo; end actions: language menu, currency menu, colour mode, account) | checkpoint 1 |
| Home grid | `/` | header: page `table-filter` (`LayoutHeader hasDivider` > indeterminate `ProgressBar` > `Section` > title row + toolbar row, page.tsx:4335-4500); body: page `library` (Card Grid) card grid, empty state | phase 1 |
| Result tile | home grid | page `product-gallery`, `ProductCard` (square media; title, secondary line = shop icon and name, price `large` bold with a favourite `IconButton`); NEW and feature tags: `Badge` row under the price (block `BadgeCategoryTags`). The server renders the first 24 tiles; the rest of the page's 100 are drawn in the browser near the viewport, a tile silhouette holding their place | phase 1 |
| Search | home, leading the filter row | page `table-filter` search `TextInput size="sm"` (:3419-3430, `searchSlot`); Enter or a pause applies `q`, Ctrl/Cmd+K focuses it | phase 1 |
| Filter clauses, sort, saved searches | the toolbar row | page `table-filter` `filterBar` (:3408-3494): one closed trigger per filter that doubles as its chip, folding from the end into "+N" (`FilterOverflow`, :2034-2121: `Popover`; `BottomSheet` on a compact surface), then the readout "{n} items • Clear all". End cluster: sort `DropdownMenu`, save (`IconButton`, `AdaptiveSavedViewForm` :2122), saved-views toggle swapping the row for `savedViewsBar` (:3495) | checkpoint 1 |
| Avatar strip | home, items view | block `CarouselSnap` (`SelectableCard` per avatar exactly as page `product-detail`'s image thumbnails, lines 117-150: square `AspectRatio` > `SelectableCard variant="transparent" padding={0}` filling it > image filling and covering; name as caption below); the first 14 cards until the strip is touched, focused or scrolled | phase 1 |
| Item detail | `?item=<id>`, tile click | page `product-detail` inside a `Dialog` (blocks `DialogAdaptivePresentation`, `DialogFullscreenDialog`, `DialogWithSubtitle`): gallery (block `CarouselSnap` hero, `SelectableCard` thumbnails, block `LightboxGallery` zoom), info column (SegmentedControl language tabs, display-2 title, rating row as likes, price row, `Badge` marks, `Banner` avatar fit, two lg actions, `CollapsibleGroup` sections with blocks `ListBasicList` and `MetadataListBasicMetadata`, `Token` chips), "More from" `Carousel` strip; full screen with a `LayoutFooter` bar below lg | phase 2 |
| Pagination | under the grid | `Pagination` component; no template covers paging (gap) | phase 1 |
| Empty, filtered-empty, error, API starting | instead of the grid | block `EmptyStateActions` | phase 1 |
| Loading, refresh | home, while the search answers | `Skeleton` silhouettes of the count and of 24 tiles (doona `apps/lite` loading rules); every image box holds a `Skeleton` under its image until the image paints; refresh: nothing for 150 ms, then the template's indeterminate `ProgressBar` along the header's top edge (:4341-4347) held at least 300 ms, the new count and results fading in (`lib/use-delayed.ts`); failed refresh: inline `Banner` with retry above the kept results | phase 1 |
| Sign-in / register | TopNav end "Sign in" | page `login-card` in a `Dialog` (TabList for the two forms) | checkpoint 1 |
| Account menu | TopNav end, signed in | ghost `IconButton` with the user-circle glyph (template `shell-side-nav` account row, page.tsx:180), tooltip the display name; `DropdownMenu` with a header (display name, email) and two groups: favourites, recently viewed / settings, admin console (admins only), sign out | track D |
| Language, currency, colour mode | TopNav end | doona lite `LanguageMenu` / `ColorModeToggle` pattern: ghost IconButton + tooltip, `DropdownMenu` radio group (interface language only); currency as a ghost DropdownMenu button of the same height | checkpoint 1 |
| Settings (appearance, language, currency, item text) | TopNav end, account menu | page `settings-dialog`; synced to the account when signed in; item text defaults to the interface language | track D |
| Favourites, recently viewed | account menu | page `library` with the same frame (views of the home grid; recently viewed is read in the browser) | track D |
| Admin frame | `/admin/*` | page `shell-side-nav` (collapsible, resizable `SideNav`, two titled groups, footer: back to site, language, colour mode; phone drawer from `AppShell`) | track F |
| Admin sign-in, not an admin, unreachable | `/admin/*` before the probe succeeds | `EmptyState` inside the frame; sign-in opens the site's `login-card` dialog, sign-in only under the admin heading | track F |
| Admin overview | `/admin` | page `dashboard-scorecard`: titled bands (catalogue, crawler, system) 40 px apart, each a tight tile grid (gap 8 px, four across, two on a phone) with the daily `TrendChart` cards under the tiles they explain (recharts, loaded on first use, admin only); usage tiles keep page `dashboard`'s `ProgressBar` (warning from 80 %, error from 90 %); tiles use padding 16 px, not 20, for density; header `StatusDot` + text as page `incident-console` | track F |
| Crawler tasks | `/admin/jobs` | header as page `detail-page` (title row, then `TabList size="lg" isFullBleed` on the header divider); page `incident-console` list rows (`List` compact with dividers, status `Token`); stop and run options through blocks `AlertDialogAsyncAction` and `DialogFormDialog` | track F |
| Crawler history | `/admin/jobs/history` | the tasks page's header and tabs; page `incident-console` rows and `EmptyState`; kind and status `Selector`s in the URL; details in a `Dialog` with the inspector's `MetadataList` | track F |
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
- `AppShell mobileNav={false}`: the bar has no drawer content, and Astryx's phone bar (chosen by a media query
  in the browser only) remounted the end actions after hydration, dropping the clicks made before it.
- Grid column floor 160 px instead of the template's 320 px: product tiles need two columns on a phone.
- Search slot floor one control tall (`--size-element-sm`) instead of the template's resting 120 px: a phone's row
  (335 px at 390) cannot seat the field, the "+N" trigger and the readout with "Clear all" at the 115 % root, so the
  field gives down to its icon before the readout passes the page inset; it still widens to 240 px on focus where
  the row has the room.
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
- Logo: master's SVG mark at 1.75rem (28 px at a 16 px root) instead of the template's `NavIcon` (the mark is itself a filled shape). On
  phones (< 640 px) the wordmark is visually hidden by a documented rule in `globals.css` (`.site-heading`), as on
  master: TopNavHeading has no prop for it, and five end actions do not fit beside a truncated name at 390 px.
- End actions: no magnifier or palette; the site search is the home page's filter row (Filter area below).
- Currency: a ghost `DropdownMenu` button showing only the code ("JPY", no chevron) instead of an icon (owner: the
  coin icon was not understood); its accessible name and tooltip give the localised name, the rows symbol, code
  and name.
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

## Deviations in the beautification batch

- Density: the root font size (115 %) scales the page as the owner's 110-125 % browser zoom did; the Astryx
  control heights are px in `astryx.css`, so `globals.css` restates them in rem. Viewport breakpoints stay px.
  Grid's `columns={{minWidth}}` takes px only, so `lib/grid.ts` passes the same track list with a rem floor as
  the `style` Grid merges (results grid, item dialog columns, gallery thumbnails).
- Results grid: 24 server tiles (whole rows at 2 to 4 columns; at 5 columns the fifth row ends early, below the
  first screen): whole rows at 5 too would take 60 tiles, 19 KB of HTML over the size budget.
- Image skeletons: each image is a `SkeletonImage` (`catalog/silhouette.tsx`), an `img` over a `Skeleton` in one
  grid cell, server markup with no handler; the image is positioned so it paints above the animated Skeleton.
- Top bar account: both variants are server markup in `[data-account-slot]` wrappers (`display: contents`), and
  `globals.css` shows one by `<html data-account>`, set before the first paint from `lib/session-hint.ts`.
- Avatar cards: `padding={0.5}` (the selection ring's width) and an image radius of `--radius-container` minus
  that padding, instead of product-detail's padding 0, so no band of card shows between ring and image.
- Tab icon: `app/icon.svg` is the logo's mark in black, white under `prefers-color-scheme: dark` inside the SVG.

## Icons

| Use | Name | lucide glyph |
|---|---|---|
| Search field, suggestion "Search “q”" row | `search` (theme) | Search |
| Filters | `funnel` (theme) | Filter |
| Suggestions: recent search (remove: `close`) | `clock` (theme) | Clock |
| Favourite (tiles, item dialog; saved: filled by CSS on `aria-pressed`) | `site:favourite` | Heart |
| Language menu | `site:language` | Languages |
| Account menu (signed in) | `site:account` | CircleUser |
| Colour mode | `site:dark` / `site:light` | Moon / Sun |
| Avatar strip "All", avatar without image | `site:allAvatars` | LayoutGrid |
| Suggestions: tag row, clear recent | `site:tag`, `site:clear` | Hash, Trash2 |
| Component internals (chevrons, close, check, carousel arrows, pagination) | theme semantic names | ChevronDown/Left/Right, X, Check, ... |

The tag editor passes lucide `Plus`/`Minus` to `Icon` (filter area below). Sizes: `sm` in the top bar, tiles
and menus (as shell-top-nav), `lg` for the strip's "All" glyph.

## Filter area (checkpoint 1, track A)

The header is the Filterable Table template's (`web/templates/pages/table-filter/page.tsx`): its default filter row,
not its PowerSearch mode. A set clause is its own chip, the row folds into "+N" instead of wrapping, and phones get
the folded set in a BottomSheet. The site search leads the row; there is no separate search field, tab row or
command palette. The search is a `Typeahead` (block TypeaheadSearchField) carrying the old quick search's
suggestions: recent searches on focus, then `/api/palette`'s groups as `TypeaheadItem` rows with a leading Avatar
(the component's own example). Not 1:1: the applied query shows as the Typeahead's token (its value), and the list is
36rem wide and up to 60vh tall instead of the field's width and 300 px (`app/filter-row.css`).
With a text query, the readout's count is a link that opens the search trace (how the search was answered and the
time of each step) in a `Popover` laid out as block PopoverKeyboardShortcuts; master showed it as a chip beside the
count, which the readout's reserved width has no room for.

| Master filter | Clause | Not 1:1 |
|---|---|---|
| View: Items / Avatar bodies | preset `ToggleButton` "Avatar bodies" (hidden in favourites) | a toggle, not a two-way switch |
| Category, smart counts (items view) | `Selector`, counts as the option description | - |
| Gender (avatar view) | `Selector` in the category's place | - |
| Avatar | `Selector` with search (items view) | the avatar strip still sets it too |
| Content rating | `Selector` (All ages / Everything) with `hasClear`; All ages is the unset default | - |
| Known avatars only (avatar view) | preset `ToggleButton` | - |
| Price: free, presets, min/max, swap | `ComplexSelector` (the template's Quote shell) with `CheckboxInput`, `RadioList`, two `TextInput`s, Clear / Apply | a preset is cleared by choosing "Any" (RadioList); no × on the trigger (ComplexSelector has no `hasClear`, as the template's Quote) |
| Published range | `Selector` with `hasClear` | - |
| Features with counts | `MultiSelector`; counts as the option description | - |
| Include / exclude tags, search, counts | `ComplexSelector` whose editor is the tag picker; staged, Clear / Apply; the trigger shows display names | - |
| Shop (the item dialog's "All items from shop") | `Selector` holding the one shop, × removes it | master showed it as a token |
| Active-filter tokens | gone: each set clause is its chip; the query is in the search field | - |
| Mobile drawer | the "+N" BottomSheet (`height="hug"`); on a compact surface every clause folds (`minVisibleItems={0}`) | - |
| Sort | `DropdownMenu`, ghost `sm`, in the end cluster | - |
| Saved searches (account menu dialog) | the template's saved views: save `IconButton` (bookmark-plus) opens `AdaptiveSavedViewForm` (name starts as the heading, at most 100 characters); the saved-views `ToggleButton` swaps the row for the bar of chips; the pencil on the active chip opens the edit form (rename, Copy RSS, delete). Signed out, both open sign-in; disabled with no filter; not offered in the favourites or recent views | delete has no confirmation step, as the template's edit form |

Set clauses read as the template's triggers: a selector "field is value" (`renderValue`, :3250-3252), a
multi-selector its first value and a count (:3315-3319), the price as the Quote range (`quoteLabel`, :2493-2499,
with ≥ / ≤ as `OPERATOR_LABELS`). Chinese and Japanese write "is" as a full-width colon, Korean as ": ", the
form those interfaces use for a field and its value. The saved search's summary uses the same words
(`catalog/clause-text.ts`).

Deviations, all prop values or token-backed inline styles:

- The header `Section` keeps `padding={6}` (24 px) where the template has 16 px, so the title and the grid
  share one left edge (rule 5).
- The template's StyleX rules (`filterChrome`, `filterSurface`, `filterFill`, slot and readout sizing) are
  `style`/`controlStyle` objects of the same tokens (`catalog/filter-style.ts`); the ones that need a container
  query or `:focus-within` (`toolbarContainer`, `toolbarPrimary`, `searchSlot`) are classes in
  `app/filter-row.css`. The site has no StyleX compiler.
- The fold's Popover moves focus in only when opened by keyboard (`hasAutoFocus`); a click leaves focus on the
  trigger. Clause popups keep their own focus handling.
- `BottomSheet`, the price and tag editors and the saved-views code load after the page (`lib/lazy`; the
  saved-views code through `next/dynamic`).

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
