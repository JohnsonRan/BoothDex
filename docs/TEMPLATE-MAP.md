# Template map

Every route and surface of the site, the Astryx template it is built from, and the rules it follows. The web
frontend (`web/`) is being rebuilt on Astryx with the neutral theme as the default (six more Astryx themes load
on demand, rule 7); phase 1 is the home page. Surfaces marked
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
   it with `padding={6}`, so titles and content share one left edge at every width. Exception: the admin
   console (`/admin/*`) has its own frame (page `shell-side-nav`) and pads header and content with 4, as its
   templates do (page `detail-page` header, page.tsx:169; page `settings` content, page.tsx:111). Its page
   title row is taller than the side nav's heading row (h1 against the nav's heading), so the two are not on
   one line; both rows are the templates' own.
6. Icons: all lucide (owner: 「全 lucide」), always through Astryx's `Icon`. The theme's semantic names
   (`search`, `funnel`, `error`, ...) first; the neutral registry maps all 29 of them to lucide, so components'
   internal glyphs (chevrons, close, check) are lucide too and none falls back to core's heroicons. Glyphs with
   no semantic name are registered once under `site:` (`lib/icons.tsx`, table below). No hand-drawn glyphs
   (the logo is the brand mark, not an icon).
7. Appearance: System (default), Light, Dark from the settings cookie; a head script sets `data-theme` and
   `lang` before first paint, and the root `Theme` gets the same mode from the server. The style is neutral (the
   default, built into every page) or one of six Astryx themes (`lib/settings-core.ts` `STYLES`: Stone, Matcha,
   Y2K, Gothic, Chocolate, Butter), each loaded only when chosen (`themes/index.tsx`; Gothic is dark-only). Pure
   black (a `Switch` under the styles) applies in dark mode only; the same script marks it as `data-black`.
8. This file lists every surface before it is built.
9. Astryx is MIT: `web/THIRD-PARTY-NOTICES.md` names each adapted template and carries the licence.

## Token spec (neutral defaults)

| Aspect | Value |
|---|---|
| Page inset | `Layout padding={6}` on header and content |
| Rhythm | `VStack gap={6}` between page sections, `gap={4}` within the control block, `gap={2}` between a tile's cover and text, `gap={1}` between text lines |
| Grid | page `product-gallery` (page.tsx:149) at master's density (owner: 「太大了。為什麼一頁只有4張圖了」): `minWidth` 160 (10rem, `lib/grid.ts`), `gap={4}`: two columns on a 390 px phone (the floor is also at most half the row less one gap), six at 1280-1440 px, nine at 1920 px (115 % root) |
| Density | `html { font-size: 115% }` in `globals.css`, only above 768 px (the template's compact-surface width, page.tsx:1129), is the one knob (110 / 115 / 125 %): every rem token follows it, a phone stays at 100 % as the template is drawn, and `--size-element-sm/md/lg` are restated as 1.75 / 2 / 2.25rem so controls scale with their text |
| Control size | `sm` for the whole toolbar row, as the table-filter template: search, every filter clause, presets, sort, save and saved views |
| Type | neutral scale: page title `Heading level={1}` (2xl); tile title `Heading level={4}` (base) at outline level 2; shop `Text type="body" color="secondary"` and price `Text type="large" weight="bold"`, as product-gallery's description and price (page.tsx:106-109) |
| Colour | the chosen theme's tokens, light and dark via `light-dark()`; the one override is pure black (Deviations after the style audit) |
| Radius, elevation | component defaults |

## Surfaces

| Surface | Route / trigger | Astryx template | Status |
|---|---|---|---|
| Page frame | every route | page `shell-top-nav` (AppShell + TopNav, heading with master's logo; end actions: language menu, currency menu, colour mode, account) | checkpoint 1 |
| Home grid | `/` | header: page `table-filter` (`LayoutHeader` > `Section` > title row, page.tsx:4335-4500); its toolbar row is the filter band, a sticky `Section` first in a `LayoutContent isScrollable={false}` (LayoutContent.d.ts:28-33, example :70-75; style object as page `product-detail` `stickyInfo` :26-30, opaque as page `dashboard-alert-rail` `stickyHeader` :895-900) carrying the header's divider and the indeterminate `ProgressBar` on it; body: page `library` (Card Grid) card grid, empty state | phase 1 |
| Result tile | home grid | page `product-gallery`, `ProductCard` (square media; title, secondary line = shop icon and name, price `large` bold with a favourite `IconButton`); NEW and feature tags: `Badge` row under the price (block `BadgeCategoryTags`). The server renders the first 24 tiles; the rest of the page's 100 are drawn in the browser near the viewport, a tile silhouette holding their place | phase 1 |
| Filter sidebar (default) | home, wide screens; Settings > Appearance "Filters at the top" moves the filters back to the row (`filters` setting, synced to the account) | **gap**: no page template has a filter sidebar. Composed from blocks: `LayoutPanel` in the page `Layout`'s start slot (block `LayoutPanelNavigation`, `astryx docs layout/side-panels`), the fields a compact `CollapsibleGroup` with dividers, every section open (block `CollapsibleGroupAccordion`), choices as `RadioList` / `CheckboxList` with result counts in `endContent` (block `CheckboxListWithEndContent`); models and tags keep their pickers; the search, the count and the sort stay in the toolbar; at 1024 px and below the filter row keeps everything. Listed after it was built (rule 8 not followed); see the deviations below | under review |
| Search | home, leading the filter row | page `table-filter` search `TextInput size="sm"` (:3419-3430, `searchSlot`); Enter or a pause applies `q`, Ctrl/Cmd+K focuses it | phase 1 |
| Filter clauses, sort, saved searches | the toolbar row | page `table-filter` `filterBar` (:3408-3494): one closed trigger per filter that doubles as its chip, folding from the end into "+N" (`FilterOverflow`, :2034-2121: `Popover`; `BottomSheet` on a compact surface), then the readout "{n} items • Clear all". End cluster: sort `DropdownMenu`, save (`IconButton`, `AdaptiveSavedViewForm` :2122), saved-views toggle swapping the row for `savedViewsBar` (:3495), then the avatar strip's toggle in the items view (see deviations) | checkpoint 1 |
| Avatar strip | home, items view | block `CarouselSnap` (`SelectableCard` per avatar exactly as page `product-detail`'s image thumbnails, lines 117-150: square `AspectRatio` > `SelectableCard variant="transparent" padding={0}` filling it > image filling and covering; name as caption below); the first 14 cards until the strip is touched, focused or scrolled. Its label line as block `CarouselSnap` (:22-27: bold body label above the carousel, gap 3); the cards take the whole row; its toggle is in the toolbar's end cluster (see deviations). Shrunk with the scroll under the filter toolbar: the same strip as a row of chips, each the table-filter person cell (page `table-filter` :2862-2867: `HStack gap={2} vAlign="center"`, an `Avatar` and its name), the avatar the card's own image clipped round (`AspectRatio shape="ellipse"`) at Avatar's 32 px size (`Avatar` `size` scale), the name a supporting `Text` as the caption, the selection ring as wide as the `SelectableCard`'s own (2 px, SelectableCard.tsx:175-178), the selected chip a capsule in the selected row's fill (`Item` :418-420 `--color-accent-muted`, `Token`'s `--radius-full` :194) with its name semibold as a selected `TopNavItem` (:66-68); the "all" card's `site:allAvatars` glyph at 1rem in its chip; the prev/next buttons gone (they sit on the top layer above the band); open state and aria-expanded/aria-controls of block `CollapsibleHookUsage` (:14-42); see deviations | phase 1 |
| Item detail | `?item=<id>`, tile click | page `product-detail` inside a `Dialog` (blocks `DialogAdaptivePresentation`, `DialogFullscreenDialog`, `DialogWithSubtitle`): gallery (block `CarouselSnap` hero, `SelectableCard` thumbnails, block `LightboxGallery` zoom), info column (SegmentedControl language tabs, display-2 title, rating row as likes, price row, `Badge` marks, `Banner` avatar fit, two lg actions, `CollapsibleGroup` sections with blocks `ListBasicList` and `MetadataListBasicMetadata`, `Token` chips), "More from" `Carousel` strip; full screen with a `LayoutFooter` bar below lg | phase 2 |
| Endless results | the grid, page by page | page `table-filter` `useInfiniteBatches` (:1160-1237) with `/api/search` pages for its timer, sentinel (:2356-2376, margin two screens), "All N loaded" line (:4517-4538); silhouette tiles while a page loads, inline retry Banner on failure (in a page's own place for a page above); a start past page 1 puts the pages before it above it as boxes of the estimated height (at most 40 at once, more as the reader nears their top), each read when in view; the URL names the page in view (`replaceState`), Back and reload return to the same tile; hidden prev/next links for crawlers (`components/catalog/result-pages.tsx`) | phase 1 |
| Empty, filtered-empty, error, API starting | instead of the grid | block `EmptyStateActions` | phase 1 |
| Loading, refresh | home, while the search answers | `Skeleton` silhouettes of the count and of 24 tiles (doona `apps/lite` loading rules); every image box holds a `Skeleton` under its image until the image paints; refresh: nothing for 150 ms, then the template's indeterminate `ProgressBar` (:4341-4347) as a 2 px square line on the filter band's bottom edge (doona `apps/lite` src/ui.tsx:131-137, at its header's edge; restyled by its theme hooks for this instance only) held at least 300 ms, the new count and results fading in (`lib/use-delayed.ts`); failed refresh: inline `Banner` with retry above the kept results | phase 1 |
| Sign-in / register | TopNav end "Sign in" | page `login-card` in a `Dialog` (TabList for the two forms; sign-in only, without the tabs, while registration is closed) | checkpoint 1 |
| Account menu | TopNav end, signed in | ghost `IconButton` with the user-circle glyph (template `shell-side-nav` account row, page.tsx:180), tooltip the display name; `DropdownMenu` with a header (display name, email) and two groups: favourites, recently viewed / settings, admin console (admins only), sign out | track D |
| Language, currency, colour mode | TopNav end | doona lite `LanguageMenu` / `ColorModeToggle` pattern: ghost IconButton + tooltip, `DropdownMenu` radio group (interface language only); currency as a ghost DropdownMenu button of the same height | checkpoint 1 |
| Settings (appearance, language, currency, item text, devices) | TopNav end, account menu | page `settings-dialog` (appearance: colour mode, style cards, pure black `Switch`); synced to the account when signed in; item text defaults to the interface language; signed in, an Account group with Devices: the account's sessions as the card's rows, a secondary Sign out on each but this device, which carries a `Token`; and Your data: the template's "Delete my account" row (page.tsx:1953-1958, destructive sm `Button`) opening block `DialogFormDialog` with the password field, Cancel and a destructive Delete account | track D |
| Favourites, recently viewed | account menu | page `library` with the same frame (views of the home grid; recently viewed is read in the browser) | track D |
| Privacy policy, terms of use | `/privacy`, `/terms`; Settings > About rows; the register form's consent line (page `login-card` terms line, page.tsx:196-209) | page `documentation-technical` in the site frame: a reading column (`contentWidth` 960, page inset 6), `Heading level={1}` with a supporting "Last updated" line, then per section a `Divider` and a level-2 `Heading` over body `Text` paragraphs and disc `List`s; the cookie list a compact `Table`. The template's outline panel, phone selector, assistant card and code blocks are not used. Texts are server-only (`lib/legal-<lang>.ts`), the operator and contact from `OPERATOR_NAME` / `CONTACT_EMAIL` at runtime; inline links are underlined (neutral's accent is the text colour). From Settings > About and the register form the same body opens in a dialog over them (block `DialogScrollingContent` without its footer, 960 px wide, settings' height cap; "Last updated" as the `DialogHeader` subtitle, block `DialogWithSubtitle`); the body is one client component (`legal-body.tsx`) for both, and the dialog gets its texts as plain data from a server action, never as elements (a route's client manifest need not list the components they name); the terms' privacy link switches the dialog, a modified click opens the page | track G |
| Admin frame | `/admin/*` | page `shell-side-nav` (collapsible, resizable `SideNav`, two titled groups, footer: back to site, language, colour mode; phone drawer from `AppShell`) | track F |
| Admin sign-in, not an admin, unreachable | `/admin/*` before the probe succeeds | `EmptyState` (or the checking `Spinner`) in place of the page's content, under the page's own title and tabs; sign-in opens the site's `login-card` dialog, sign-in only under the admin heading | track F |
| Admin overview | `/admin` | page `dashboard-scorecard`: titled bands 40 px apart in the order an operator reads them (status, in a fixed order with attention shown by each tile's status dot and the index rebuild in its heading; activity, the last 24 hours then the daily trends; catalogue; system), each a tight tile grid (gap 8 px, up to four across, two on a phone, a short band shares its row); one `TrendChart` card for every trend: a 2 px line per series with a wash fading downward, horizontal grid only, five value ticks (three on a phone), legend of short line keys above the plot, one tooltip per day, plot 14 rem (12 rem on a phone), recharts loaded on first use, admin only; usage tiles keep page `dashboard`'s `ProgressBar` (warning from 80 %, error from 90 %); tiles and chart cards use padding 16 px, not 20, for density; header `StatusDot` + text as page `incident-console` | track F |
| Crawler tasks | `/admin/jobs` | header as page `detail-page` (title row, then `TabList size="lg" isFullBleed` on the header divider); page `incident-console` list rows (`List` compact with dividers, `edge="outset"` so the text sits on the title's line, status `Token`; schedule, one small secondary Run or Stop and the `MoreMenu` in the row's end slot, under the description on a phone); stop and run options through blocks `AlertDialogAsyncAction` and `DialogFormDialog` | track F |
| Crawler history | `/admin/jobs/history` | the tasks page's header and tabs; page `incident-console` rows (`edge="outset"`) and `EmptyState`; kind and status `Selector`s in the URL; details in a `Dialog` with the inspector's `MetadataList` | track F |
| Users | `/admin/users` | the crawler history's layout: page `table-filter` search `TextInput size="sm"` over page `incident-console` rows (`edge="outset"`, admin and disabled `Token`s, relative last sign-in) and `EmptyState`, `Pagination` under the rows, search and page in the URL; details in a `Dialog` with the inspector's `MetadataList`, the sessions as list rows with a ghost Sign out, then Disable or Enable and Reset password (both behind `AlertDialog`; the new password in a success `Banner`) | track F |
| Admin settings (AI provider, registration) | `/admin/settings` | page `settings` (two-column sections with `Divider`); key overwrite behind `AlertDialog`; registration a `Switch` saved when flipped | track F |

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
- Fold trigger place 3rem instead of the template's 44 px (slot floor and unfolded padding, `filter-bar.tsx`,
  `filter-row.css`): "+8" is 19.8 px wide on a phone, so 44 px clipped it to "+…".
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
- Avatar strip folding: no template shrinks a strip with the scroll. The strip stays one Carousel Snap strip (owner
  2026-11-09: 「跟著捲動連續縮小」): the band sticks higher by the strip's shrink with its toolbar pinned inside (two nested
  `position: sticky`), so the strip goes on under the toolbar while each card morphs into its chip (owner 2026-11-09:
  「頭像配名字在旁邊」), then stays over the results as a row the height of the toolbar's controls. Each card's square moves to
  its chip's place (measured once in `avatar-strip.tsx`, set as CSS variables) and scales to 32 px about its corner,
  while the chip's round face fades in over it: the same image in an `AspectRatio` `shape="ellipse"`
  (AspectRatio.tsx:68-78), with no ring, the "all" glyph at 1rem; the card's square corners and its selection ring
  fade out behind it, and a selected chip is its filled capsule and semibold name only, as Astryx marks a selected
  Token or item (owner 2026-11-09: the 2 px ring round the chip's avatar read as a heavy black circle). Each part shows only where it overlaps nothing (owner 2026-11-09:
  「收起的时候好难看」): the caption is gone by a quarter of the shrink, the square turns round over 70-85 % when the
  card is about half its size, and the chip's name (the selected chip's capsule with it) fades in from 90 %, once its
  avatar has moved clear of it. A filter change made with the strip shrunk keeps it shrunk: the window goes to where
  the new results start under the band, not to the page's top (owner: 「收起的时候切换会自动展开？」). The shrink is a scroll-driven CSS
  animation of transform and opacity only (app/globals.css `strip-*`; paused at a progress set from the scroll where
  scroll timelines are missing; under reduced motion none runs and the strip is whole or shrunk), so the compositor
  runs it with the scroll, the flow never changes and nothing below moves. Shrunk and in use (a pointer, a touch,
  focus or a wheel on it) the chips are laid out for real (`data-chips`: each tile as wide as its chip, drawn as the
  morph left it), so the row scrolls and snaps chip by chip; leaving rest, the chip nearest the row's start becomes
  the first card in view. The toggle follows block `CollapsibleHookUsage` (its own trigger and content on
  `useCollapsible`'s state shape) with plain state: open, the band sticks at the top bar's edge and the strip glides
  down to full size along the morph, over the results (lib/motion `glide`, Astryx's `--ease-standard` in,
  `--ease-exit` and `--duration-exit` out); at full size with the page it folds by scrolling to where the scroll
  shrinks it. The Carousel's prev/next buttons sit on the top layer, above the band, so they go as the shrink starts.
- Boxes for tiles not drawn (`result-pages.tsx`, pages not read and pages given up far from view) paint the tile
  silhouette through a CSS mask measured once from a hidden `TileSilhouette` (`silhouette.tsx` `silhouetteMask`),
  in the Skeleton's own static look (`--color-skeleton` at its resting 0.25 opacity, as it shows under reduced
  motion; core `src/Skeleton/Skeleton.tsx:81`, `:91`, `:108`): a Skeleton per tile there would be thousands of
  running layers, and an empty box showed white when scrolled into fast.
- Filter band's sticky top: `--_app-shell-header-height`, the AppShell's private variable (AppShell.js: set on its
  root from the header's measured height, by a ResizeObserver in an effect, so only once it hydrates; not in its
  `.d.ts`). The band sticks under the top bar with it, as the AppShell's own sticky parts do, and a start past
  page 1 (`lib/deep-start.ts`, before hydration) sets the same value on the same element first. Re-check it on an
  Astryx update.
- Avatar strip: `SelectableCard` is documented for multi-select; the strip is single-select (one avatar or
  all). Kept because it is the component with a selected state for media cards; ClickableCard has none.
- Strip toggle in the toolbar (`strip-toggle.tsx`; owner 2026-11-09: 「要不试试看直接放上门的收藏那里」): block
  `CollapsibleHookUsage` puts its trigger above its content; here the trigger is last in the filter toolbar's end
  cluster, after the saved-views toggle, so it stays in one place while the strip shrinks under the toolbar or
  opens over the results, and never sits beside the Carousel's buttons or over a card. It is an icon button as its
  neighbours, page `table-filter`'s toolbar actions (page.tsx:4439-4446 save, :4450-4468 saved views; toolbarEnd
  :1398-1400): ghost `sm` `IconButton`, its name ("Collapse avatars" / "Expand avatars") as label and tooltip, the
  chevron of Collapsible's trigger anatomy turned up to fold, `aria-expanded` and `aria-controls` naming the cards.
- Readout "Page N" (`result-pages.tsx` `PageInView`): from page 2 on the readout reads "Page N · {n} items", N
  the page in view (the address's). No template has it. `.readout-line` (`app/filter-row.css`) puts it on the
  side away from where the count is set (before it at the row's end, after it on a phone's own line), so the
  count never moves when it appears.
- Clause list clip (`app/filter-row.css` `.filter-clauses`): 1 px of block padding cancelled by a -1 px margin,
  so the clip reaches 1 px past the controls; flush with them, the sticky band's sub-pixel offset (the top bar is
  55.17 px tall) cut a control's top or bottom border.
- Astryx class and DOM hooks outside the `.d.ts` files, to re-check on an Astryx update:
  - `.astryx-app-shell` and `.astryx-app-shell-header` (`lib/deep-start.ts`): the deep start sets the shell's
    header height variable on the shell before it hydrates (above).
  - `.astryx-skeleton` (`silhouette.tsx` `silhouetteMask`, `app/globals.css` image skeletons): the boxes' mask is
    measured from a silhouette's Skeletons, and a painted image's Skeleton is dropped.
  - The Carousel's top-layer `[popover]` (its prev/next buttons) under `[data-strip]` (`app/globals.css`
    `strip-*`): faded and hidden as the strip shrinks.
  - The Carousel's scroll area `:scope > [role="group"]` (`avatar-strip.tsx`): the vertical wheel scrolls it by
    the same `scrollBy` the Carousel's Shift + wheel uses.
  - `.astryx-progress-bar-track` and `-fill` (`app/filter-row.css` `.band-progress`): the refresh progress as a
    2 px square-ended line on the band's divider instead of the 8 px pill on a muted track.

## Deviations in the beautification batch

- Density: the root font size (115 %, above 768 px only; phones stay at 100 %) scales the page as the owner's 110-125 % browser zoom did; the Astryx
  control heights are px in `astryx.css`, so `globals.css` restates them in rem. Viewport breakpoints stay px.
  Grid's `columns={{minWidth}}` takes px only, so `lib/grid.ts` passes the same track list with a rem floor as
  the `style` Grid merges (results grid, item dialog columns, gallery thumbnails).
- Results grid: 24 server tiles (whole rows at 2 to 4 columns; at 5 columns the fifth row ends early, below the
  first screen): whole rows at 5 too would take 60 tiles, 19 KB of HTML over the size budget.
- Image skeletons: each image is a `SkeletonImage` (`catalog/silhouette.tsx`), an `img` over a `Skeleton` in one
  grid cell, server markup with no handler; the image is positioned so it paints above the animated Skeleton.
  Once the image is decoded, one document listener in the head marks it `data-painted` and `globals.css` drops
  the Skeleton under it: each running Skeleton under a painted image was a compositor layer, about 300 on a
  screen of results, and layerizing them took most of every scrolled frame. A new `src` is a new `img` (keyed).
- Top bar account: both variants are server markup in `[data-account-slot]` wrappers (`display: contents`), and
  `globals.css` shows one by `<html data-account>`, set before the first paint from `lib/session-hint.ts`.
- Avatar cards and the item gallery's thumbnails: `padding={0.5}` (the selection ring's width) and an image
  radius of `--radius-container` minus that padding, instead of product-detail's padding 0, so no band of card
  shows between ring and image.
- Tab icon: `app/icon.svg` is the logo's mark in black, white under `prefers-color-scheme: dark` inside the SVG.
- Scrollbars: `.astryx-dropdown-menu`, a Selector popup's listbox and a dialog's body take `scrollbar-width: none`
  in `globals.css` (the filter sidebar too, in `app/filter-row.css`), the item card's thumbnails and variations the same inline (the classic Windows bar crowded
  narrow menus and the card's rounded edge); they still scroll by wheel, touch and keyboard. The account menu
  passes `menuMaxHeight` so its rows fit whole in themes with tall rows (Matcha: 55 px).

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
| Save this search, saved views (pressed: filled by `fill`) | `site:save`, `site:saved` / `site:savedFilled` | BookmarkPlus, Bookmark |
| Saved views: edit; saved-search form: feed | `site:edit`, `site:feed` (`lib/icons-on-demand.tsx`) | Pencil, Rss |
| Tag editor: include, exclude | `site:add`, `site:remove` (`lib/icons-on-demand.tsx`) | Plus, Minus |
| Item dialog: download, "More from" link | `site:download`, `site:arrowRight` (`item/item-icons.tsx`) | Download, ArrowRight |
| Search token of a word read as a category | `site:category` (`lib/icons-on-demand.tsx`) | Shapes |
| Settings: Devices panel and its rows | `site:devices` (`lib/icons-on-demand.tsx`) | MonitorSmartphone |
| Component internals (chevrons, close, check, carousel arrows, pagination) | theme semantic names | ChevronDown/Left/Right, X, Check, ... |

Sizes: `sm` in the top bar, tiles and menus (as shell-top-nav), `lg` for the strip's "All" glyph.

## Filter area (checkpoint 1, track A)

The header is the Filterable Table template's (`web/templates/pages/table-filter/page.tsx`): its default filter row,
not its PowerSearch mode. A set clause is its own chip, the row folds into "+N" instead of wrapping, and phones get
the folded set in a BottomSheet. The site search leads the row; there is no separate search field, tab row or
command palette. The search is a `Tokenizer` with block TypeaheadSearchField's search icon and suggestions
(`catalog/search-suggest.tsx`): recent searches on focus, then `/api/palette`'s groups as `TypeaheadItem` rows with
a leading Avatar (the component's own example). Each word of the applied query is its own token, as on Mercari
(a quoted phrase stays one): its × or Backspace searches the remaining words, Enter adds the typed words, a tag
suggestion adds its name to them and a recent search replaces them. The words the search read as an avatar or a
smart category (the API's `recognized`) join one token, led by the avatar's picture or the `site:category` glyph,
its tooltip naming the reading ("Avatar: Mao", "Categories: Hair · Short"); removing it removes all its words. Not
1:1: the list is 36rem wide and up to 60vh tall instead of the field's width and 300 px (`app/filter-row.css`).
With a text query, the readout's count is a link reading "{n} · {ms}" (the search's latency) that opens the search
trace (how the search was answered and the time of each step) in a `Popover` laid out as block
PopoverKeyboardShortcuts; master showed it as a chip beside the count, which the readout's reserved width has no
room for.

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
- The template's StyleX rules (`filterChrome`, `filterSurface`, `filterFill`) are `style`/`controlStyle` objects
  of the same tokens (`catalog/filter-style.ts`); the ones that need a container query or `:focus-within`
  (`toolbarContainer`, `toolbarPrimary`, `searchSlot`, `filterRow`, slot and readout sizing) are classes in
  `app/filter-row.css`. The site has no StyleX compiler.
- The readout is the slot's sibling, pulled back over the reserve, instead of its child: where a phone's toolbar
  cannot hold the search, "+N" and the reserve (a toolbar below 25.5rem), it takes a line of its own under the row
  (owner's request), and the search and "+N" keep their places.
- The favourites and recent views' actions start after the title below the 860 px toolbar width, where the end
  cluster drops below, instead of sitting on the far side of a wrapped line.
- The fold's Popover moves focus in only when opened by keyboard (`hasAutoFocus`); a click leaves focus on the
  trigger. Clause popups keep their own focus handling.
- `BottomSheet`, the price and tag editors and the saved-views code load after the page (`lib/lazy`; the
  saved-views code through `next/dynamic`).

## Deviations in phase 2 (item detail)

- Container: the template is a page; here it is a `Dialog` 1200 px wide (the template's `contentWidth`) and 92dvh
  tall (master), the fullscreen variant below 1024 px (master's lg). On phones the actions move to a
  `LayoutFooter` bar (price, favourite, Buy on Booth), as master's sticky bar.
- Inset: the dialog blocks' default (`LayoutContent` without padding, block DialogFullscreenDialog.tsx:71), so
  header, content and the phone footer share one edge; the template's page padding 6 set the body inside them.
- Title: `display-2` from lg; on a phone the page title's `Heading level={1}`, as display-2 took three or four
  lines of a 335 px column.
- Header: `DialogHeader` names the shop (avatar as start content; subtitle: the host, then "All items from shop"
  as an inline `Link`, which DialogHeader allows in the subtitle, so it reads as part of the shop and not as
  chrome beside the close button); the dialog's accessible name is the item title. Master had the shop block at
  the top of the info column.
- Hero: a snapping `Carousel` of square `AspectRatio` slides instead of one image, so phones swipe; each slide is a muted `ClickableCard` opening the Lightbox, the image
  `contain`ed (a product photo is not cropped; the template's hero is `cover`). Download (`DropdownMenu`) and the
  counter sit in a row under it, the counter between previous and next ghost `IconButton`s: the Carousel's own
  buttons live in the top layer and floated over the page behind the dialog when the hero scrolled, and no
  template shows a gallery in a dialog. Thumbnails, arrow buttons and sticky columns only from lg (master hid
  thumbnails on phones; a single column cannot stick); past about three rows the thumbnails scroll in place
  (master: max-h-40), and so does a long variation list from lg (phones show it whole).
- Sticky: the template's `stickyInfo` is put on both columns, so whichever is shorter stays in view.
- Description: inside the `CollapsibleGroup` (first section; every section starts closed, so the item's facts
  above come first) instead of the template's large text above the options: Booth descriptions run to hundreds of lines and would push the actions off screen. It is clamped
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


## Deviations in the style audit

- Settings theme cards: `columns={{minWidth: 80, max: 3}}` instead of settings-dialog's 96 (page.tsx:1179), so
  the three cards stay on one row in a phone's panel (264 px at 360, 294 px at 390).
- Height caps: DropdownMenu's 300 px cap is lifted to `menuMaxHeight` 480 px (`lib/menu-height.ts`) on the
  language, sort, download and account menus, whose rem rows outgrow it at the page scale or in a roomier theme;
  the viewport still bounds them.
- Dialog height: the item dialog takes settings' cap, `min(50rem, calc(100dvh - 2rem))` (`lib/dialog-height.ts`),
  instead of Dialog's default 75dvh, so on a short window it is as tall as settings (owner, 2026-11-09).
- Search tokens: the Tokenizer's cap of a token's wrapper at the field's width is lifted (`app/filter-row.css`),
  so a token in a phone's narrow field keeps its words and the field scrolls them sideways.
- Matcha and Y2K popups: a Selector's 300 px list cap and a ComplexSelector's 480 px panel cap grow by the themes'
  1.5 times spacing (`themes/matcha.css`, `themes/y2k.css`), so an 8-row list and the price panel show whole.

Checked and kept as the template:

- Settings dialog close button: on the content pane's inset (`LayoutContent padding={4}`), as settings-dialog
  (page.tsx:2364-2380), so it sits inside the card's edge.
- A folded preset toggle ("Avatar bodies") among the folded selectors: a full-width `ToggleButton` with a centred
  label, as table-filter's `overflowItem` (page.tsx:1375) stretches its button-backed presets.
- "API starting": a `Spinner size="xl"` against the empty and error states' `Icon size="lg"`; it is a loading
  state, not an empty one.

## Deviations after the style audit

- Search field width (`app/filter-row.css`): with a query it grows with its tokens (`--search-width`, reckoned in
  `catalog/search-box.tsx` so the plain field before the tokens load is as wide) from 7.5rem up to 20rem, past
  which the last words fold into "+N more"; focused, 6rem more for the typed words, up to 28rem. The tokens stay
  on the row's one line at `--size-element-sm`, scrolling sideways (the Tokenizer wraps them). A marked token
  reserves 1rem for its picture, so the field keeps its width when the search answers; marks apply only to the
  query they answered. The list's 36rem rule skips `.astryx-tooltip`, so a token's tooltip keeps its own size.
- Pure black (`app/globals.css`), the one colour override (rule 3): under `html[data-black]` in dark mode, pinned
  or followed from the system, the surface and body backgrounds are restated as `#000` and card, muted and
  popover as `#111`, unlayered so it wins over every theme's layer. Synced to the account as `black`.
- iOS Safari 26 ignores `theme-color` and tints its bars from the page's background: `<html>` takes
  `--color-background-surface`, the viewport is `viewport-fit=cover`, and `<body>` pads by the safe-area insets.
- Avatar strip: no prev/next buttons on a touch screen (`(hover: none) and (pointer: coarse)`, `hasButtons`); it
  swipes, and the buttons' always-open fixed top-layer popover made Safari 26 paint its toolbar solid.

## Deviations in the filter sidebar and viewed marks

- Viewed veil (`app/globals.css` `.viewed-cover`): Overlay's scrims are `dark` (`--color-overlay`) or `light`
  (white at 60 % in every mode); neither follows the mode, and the light one fogged covers white on a dark page. The
  viewed mark passes `scrim={false}` and the veil is the page surface (`--color-background-surface`) at 60 %.
- Filter sidebar (`catalog/filter-rail.tsx`): the `LayoutPanel` has no divider, and beside it the band's toolbar
  draws no lower edge over the avatar strip (`--band-edge`, `app/filter-row.css`): the panel's divider met the
  toolbar's and the shrunk strip's edges in a grid of lines (owner). Sticky under the top bar by a `style` object, as the filter band
  (`position: sticky`, the window's height, `align-self: flex-start`); a `CheckboxList`'s items lose their List
  inline padding so they start where the `RadioList`s do (`app/filter-row.css`, one class above core's four
  `:not(#\#)`); shown only above 1024 px (`NO_RAIL_QUERY`, as `docs layout/responsive` recommends for a side
  panel), the row keeping the filters below that.

## Inline styles kept (rule check, 2026-11-11)

`web/AGENTS.md` asks for component props, then token-backed utilities, and no `style` objects or raw `<div>`/`<span>`.
This site compiles neither StyleX nor Tailwind, so `xstyle` and utility classes are not available. Where Astryx has a
prop, the prop is used. Where it has none, a `style` object stays, holding tokens or values computed at run time, never
a raw colour. Each one is converted when its file is next changed and a prop covers it (owner). 72 remain:

- Positioning: the sticky filter band, item columns and settings tabs; the refresh bar and the boxes' probe
  (`absolute`).
- Showing and hiding without moving anything: `visibility` on silhouettes and the folded filter readout, `display:
  contents` wrappers, the item dialog's subtitle while it waits.
- Flex and grid escapes: `flexShrink`, `flex` and `minWidth: 0` outside a `StackItem`, `gridArea`, a title's
  `min-block-size: 2lh`.
- Scrolling: the saved views row, the gallery's thumbnails and the variations list (`overflow`, no scroll bar,
  `overscrollBehavior`).
- Token colours: the theme previews in Settings, the admin chart's legend dots, the search trace's failed steps, the
  pressed chips.
- Values known only at run time: the results grid's masks and the heights of boxes not drawn (`result-pages.tsx`), the
  strip's custom properties (`band.tsx`, `avatar-strip.tsx`), rotations.

Raw elements kept: the tile's shop icon (above), `SkeletonImage`'s wrapper (server markup that needs no script), the
account slots `globals.css` shows by `data-account`, and the item dialog's waiting subtitle.

## Remaining Tailwind or shadcn files

None. The rewrite removed Tailwind, shadcn and their dependencies.
