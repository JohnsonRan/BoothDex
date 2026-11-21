# API reference

The public HTTP API of `api`, reached through `web` under `/api/`. What a search does with its words is in [Search behaviour](SEARCH.md).

## Endpoints

| Endpoint | Takes |
|---|---|
| `/api/search` | A VRCatalogue-compatible query string: `v q sort dir page pageSize adult avatars authors tags extags btags exbtags sc scp bc if free pmin pmax range from to asup gender fields`, plus the BoothDex parameters below |
| `/api/avatars`, `/api/authors`, `/api/tags`, `/api/booth-tags` | `?q=&offset=&limit=` |
| `/api/categories` | none |
| `/api/items` | `?ids=` |
| `/api/palette` | `?q=` |
| `/api/related` | The search query string |
| `/api/rss` | The search query string |

## Differences from VRCatalogue

- Feature facet counts respect the selected category.
- Trending is likes decayed by age.
- `range` also accepts `7d`, `30d` and `90d`.

## Parameters added by BoothDex

| Parameter | Effect |
|---|---|
| `fields=card` | Returns compact card records: `bio`, `variations`, `booth_tags`, `linked_avatars` and `categories` emptied, translations keeping only titles, `booth_images` and `media` at most 2. The default response is unchanged; fetch full records through `/api/items?ids=` |
| `without=` | Groups of filter names, `+` between groups and `,` within one, at most 8, such as `without=sc+pmin,pmax`. A search that finds nothing then also returns `widerCounts`: what it finds without each group, in order |
| `xl=0` | Leaves out the BOOTH tags that translate the query (`expandedWith`); the words still match the tag dictionary's words |
| `typo=0` | Reads no word as a misspelt avatar; a name spelt right still counts |
| `hl` | On `/api/related` only: `en`, `ja`, `ko` or `zh`; see below |

## Search response fields

| Field | Meaning |
|---|---|
| `typoTolerant` | True when the query's words matched nothing as typed and the typo retry found the results, or when a misspelt word was read as an avatar |
| `nameTypo` | True when a misspelt word was read as an avatar; that avatar's entry in `recognized` has `typo` true |
| `expandedWith` | The BOOTH tags that translate the query and were searched too, listed when items only they found are among the results |
| `semantic` | See below |

`/api/palette` searches the translating tags too. `/api/rss` leaves them out, so a feed's items do not move with the tags chosen to translate its query; its words still match the tag dictionary's words.

### `semantic`

| Value | When |
|---|---|
| `used` | Matches by meaning were merged with the word matches |
| `off` | No embeddings model is configured, or the search has no text query |
| `skipped` | No embedding was needed: `sem=0`, a query that only names avatars, or a `q` that is a BOOTH item link or an item's number of five or more digits. That item ranks first by relevance in any view, relisted or not, and the query is not embedded |
| `pending` | The AI provider is still embedding the query after the search's 2.5 s wait. The request runs on and, when it succeeds, caches the vector, so the same search a moment later uses it |
| `unavailable` | The provider failed, an embedding rate limit refused the request, or every concurrent provider request slot stayed busy. The search answers by keywords alone |

## Related searches

`/api/related` takes the same query string and returns up to four related searches in `searches`. Each has a `label` and the `q` that searches it, with the query's avatar names kept.

They are BOOTH tags whose items' mean embedding is nearest to the query's. These tags are left out:

- tags that respell or translate the query, as the tag dictionary or the expansion reads it;
- tags that repeat one of the query's words, or that the tag dictionary translates as one (ロング, which reads "long", for "long hair");
- tags that make the same search as a nearer one: their labels respell each other, or one tag respells the other or one of its dictionary values (wavy hair after ウェーブヘア, which reads "wavy hair");
- tags that name an avatar;
- tags that come from a few shops or from more than 5% of the items;
- tags that seldom go with the query's own matches;
- tags that find nothing under the same filters, adult setting and view.

With `hl`, `label` is the tag in that language when the tag dictionary has it. `q` uses that label when it has two or more letters or digits and its words find items, and the tag otherwise: one character, such as 长 for ロング, matches only where text segmentation leaves it a word of its own.

The endpoint reads only the embedding a search already stored for the query and never asks the AI provider. It returns none for a query without a stored embedding, for an item link or number, and while the configured embeddings model is not the one the items were embedded with.
