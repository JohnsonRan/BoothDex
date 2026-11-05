# Third-party notices

The site's screens are built from Astryx page templates and template blocks (`@astryxdesign/cli`, under
`assets/templates/`). Files adapted from them name the template in their header comment:

| File                                                          | Astryx template                                         |
| ------------------------------------------------------------- | ------------------------------------------------------- |
| `src/components/site-frame.tsx`                               | page `shell-top-nav` (Top Nav)                          |
| `src/components/catalog/home-screen.tsx`                      | page `library` (Card Grid)                              |
| `src/components/catalog/toolbar.tsx`                          | page `library` (Card Grid), control row                 |
| `src/components/catalog/results-grid.tsx`                     | page `product-gallery` (Product Gallery), `ProductCard` |
| `src/components/catalog/filter-panel.tsx`                     | block `PopoverFilterPanel`                              |
| `src/components/catalog/avatar-strip.tsx`                     | block `CarouselSnap`                                    |
| `src/components/catalog/active-filters.tsx`, `tag-picker.tsx` | block `TokenRemovable`                                  |
| `src/components/catalog/result-states.tsx`                    | block `EmptyStateActions`                               |

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
