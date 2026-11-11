# Third-party notices

The site's screens are built from Astryx page templates and template blocks (`@astryxdesign/cli`, under
`assets/templates/`). Files adapted from them name the template in their header comment:

| File                                                                | Astryx template                                                                     |
| ------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `src/components/frame/site-frame.tsx`                               | page `shell-top-nav` (Top Nav)                                                      |
| `src/components/frame/site-frame.tsx` (Ctrl/Cmd+K)                  | page `shell-nav`                                                                    |
| `src/components/frame/palette.tsx`                                  | blocks `CommandPaletteAsyncSearch`, `CommandPaletteRichItems`                       |
| `src/components/frame/sign-in.tsx`                                  | page `login-card`                                                                   |
| `src/components/account/toaster.tsx`                                | block `useToastHookUsage`                                                           |
| `src/components/account/menu.tsx`                                   | block `DropdownMenuWithSections`                                                    |
| `src/components/account/saved-searches.tsx`                         | block `DialogFormDialog`                                                            |
| `src/components/account/delete-saved.tsx`                           | block `PopoverConfirmAction`                                                        |
| `src/components/settings/*`                                         | page `settings-dialog`                                                              |
| `src/components/legal/legal-screen.tsx`                             | page `documentation-technical`                                                      |
| `templates/**` (verbatim copies)                                    | the pages and blocks listed in `templates/README.md`                                |
| `src/components/catalog/home-screen.tsx`                            | page `library` (Card Grid)                                                          |
| `src/components/catalog/toolbar.tsx`                                | page `library` (Card Grid), control row                                             |
| `src/components/catalog/item-tile.tsx`                              | page `product-gallery` (Product Gallery), `ProductCard`                             |
| `src/components/catalog/item-tile.tsx` (tile badges)                | block `BadgeCategoryTags`                                                           |
| `src/components/catalog/filter-bar.tsx`, `filter-style.ts`          | page `table-filter`, block `ToolbarTableFilter`                                     |
| `src/components/catalog/filter-clauses.tsx`                         | page `table-filter` (clause controls)                                               |
| `src/components/catalog/price-clause.tsx`, `price-editor.tsx`       | page `table-filter` (Quote), block `PopoverFilterPanel`                             |
| `src/components/catalog/tag-clause.tsx`                             | page `table-filter` (Quote), block `PopoverFilterPanel`                             |
| `src/components/catalog/avatar-strip.tsx`                           | block `CarouselSnap`                                                                |
| `src/components/catalog/tag-picker.tsx`                             | blocks `TokenRemovable`, `PopoverFilterPanel`                                       |
| `src/components/catalog/result-states.tsx`                          | block `EmptyStateActions`                                                           |
| `src/components/catalog/viewed-overlay.tsx`                         | block `OverlayBottomStrip`                                                          |
| `src/components/item/*`                                             | page `product-detail` (Product Detail)                                              |
| `src/components/item/item-dialog.tsx`                               | blocks `DialogAdaptivePresentation`, `DialogFullscreenDialog`, `DialogWithSubtitle` |
| `src/components/item/item-gallery.tsx`                              | blocks `CarouselSnap`, `LightboxGallery`                                            |
| `src/components/item/item-sections.tsx`                             | blocks `ListBasicList`, `MetadataListBasicMetadata`                                 |
| `src/components/item/more-from.tsx`                                 | block `CarouselSnap`                                                                |
| `src/components/admin/admin-shell.tsx`                              | page `shell-side-nav`                                                               |
| `src/components/admin/page-frame.tsx`                               | pages `incident-console`, `settings` (header)                                       |
| `src/components/admin/overview.tsx`                                 | page `dashboard`                                                                    |
| `src/components/admin/tasks.tsx`                                    | page `incident-console` (list), block `AlertDialogAsyncAction`                      |
| `src/components/admin/job-*.tsx`, `task-row.tsx`, `live-status.tsx` | page `incident-console`; block `DialogScrollingContent` (job log)                   |
| `src/components/admin/metric-card.tsx`                              | page `dashboard`                                                                    |
| `src/components/admin/settings-page.tsx`                            | page `settings`                                                                     |
| `src/components/admin/run-dialog.tsx`                               | block `DialogFormDialog`                                                            |

Astryx is distributed under the MIT License:

```
MIT License

Copyright (c) 2026 Meta Platforms, Inc.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
