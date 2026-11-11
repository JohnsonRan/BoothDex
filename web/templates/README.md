# Astryx templates, copied as is

Verbatim copies of the Astryx (`@astryxdesign/cli` 0.7.8, MIT) page templates and blocks the site is built
from. They are the reference each adapted file in `src/` is diffed against; they are not compiled (excluded
from `tsconfig.json`, Biome and knip) because they use the StyleX compiler and Heroicons, which the site does
not. Do not edit them: copy a newer version over them when Astryx is upgraded.

| Copy | Adapted in |
|---|---|
| `pages/shell-top-nav/page.tsx` | `src/components/frame/site-frame.tsx` |
| `pages/shell-nav/page.tsx` (CommandPalette wiring, Ctrl+K) | `src/components/frame/palette.tsx` |
| `blocks/CommandPaletteAsyncSearch.tsx` | `src/components/frame/palette.tsx` |
| `pages/library/page.tsx` (header, search, tabs, sort) | `src/components/catalog/home-screen.tsx`, `search-box.tsx`, `toolbar.tsx` |
| `pages/product-gallery/page.tsx` (ProductCard, grid) | `src/components/catalog/item-tile.tsx`, `results-grid.tsx` |
| `pages/login-card/page.tsx` (sign-in card, providers dropped) | `src/components/frame/sign-in.tsx` |
| `blocks/useToastHookUsage.tsx` | `src/components/account/toaster.tsx` |
| `pages/table-filter/page.tsx` (filter row: clause triggers, `FilterOverflow`, readout) | `src/components/catalog/filter-bar.tsx`, `filter-clauses.tsx` |
| `blocks/ToolbarTableFilter.tsx` (the same filter row as a block) | `src/components/catalog/filter-bar.tsx` |
| `blocks/PopoverFilterPanel.tsx` (Apply / Reset footer) | `src/components/catalog/price-editor.tsx`, `tag-picker.tsx` |
| `blocks/PowerSearchPresetFilters.tsx` (considered, not used: see docs/TEMPLATE-MAP.md) | - |
| `pages/product-detail/page.tsx` (gallery, info column, collapsibles) | `src/components/item/*` |
| `blocks/DialogAdaptivePresentation.tsx`, `DialogFullscreenDialog.tsx`, `DialogWithSubtitle.tsx` | `src/components/item/item-dialog.tsx` |
| `blocks/LightboxGallery.tsx` (zoom) | `src/components/item/item-gallery.tsx` |
| `blocks/CarouselSnap.tsx` (gallery swipe, "More from" strip) | `src/components/item/item-gallery.tsx`, `more-from.tsx` |
| `blocks/ListBasicList.tsx` (variations), `MetadataListBasicMetadata.tsx` (info) | `src/components/item/item-sections.tsx` |
| `pages/documentation-technical/page.tsx` (reading column, headings, disc lists) | `src/components/legal/legal-screen.tsx` |
