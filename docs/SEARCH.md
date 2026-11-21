# Search behaviour

Reference for what a search does with the words you type. Parameters and response fields are in the [API reference](API.md); the jobs that prepare the data are in [Background jobs](WORKER.md).

## Search field

- The filter row holds the search field and one selector per filter. The filters that do not fit fold behind a "+N" button.
- The field suggests recent searches, avatars, items, shops and tags as you type. `Ctrl+K` (`Cmd+K` on macOS) focuses it.
- Each search word is its own token, removable on its own. Words read as an avatar or a category join one token, led by the avatar's picture or a category icon. An avatar's token shows the avatar's name.
- A word, or two words, of three or more characters that equal an avatar's slug, a name or an alias (クマリ for ひきくまりのクマリ) are read as that avatar, once it has items. A name two avatars with items share (Mio) is read as neither.

## Item links and numbers

A BOOTH item link (`booth.pm/<lang>/items/<id>` or `<shop>.booth.pm/items/<id>`), or an item number of five or more digits, puts that item first in whichever view is open.

- An avatar shows among the items.
- An item relisted as a new one shows although other searches leave it out.
- The filters and the adult setting still apply.
- A sort other than relevance (a search's default) places the item like any other result.
- Such a search is not sent to the AI provider and is not searched as translated tags.
- A number that is no item's id is searched as text.

## Text matching

- Japanese text search uses Lindera (IPADIC). Chinese titles are segmented with its Chinese dictionary (CC-CEDICT), so 水着 (swimsuit) does not find 水着色器 (water shader).
- Full-width and half-width forms match each other, and so do hiragana and katakana ("しなの" finds シナノ).
- Item descriptions are mostly Japanese and are not translated. Titles and BOOTH tags are the cross-language material.

## Typo handling

- A Latin query of up to three words that matches no text is tried again over titles, tags and author names. Each all-letter word of five or more letters may have one typo ("shinnano" finds Shinano). The results page says so.
- A word that no title, tag or author name has, one typo from the Latin name of exactly one avatar, is read as that avatar ("shinnano short hair" searches like "shinano short hair"). The results page then names that avatar and offers to search the words as typed.

## Tags and translations

- BOOTH tags also match by their English, Japanese, Korean and Chinese words in the tag dictionary, as whole words only. "裙子" finds the items tagged スカート; "hair" does not find a tag translated as "chair".
- A query of up to three words is also searched as up to three BOOTH tags that translate it. These tags are carried by the items whose titles match the query. Their own title matches largely overlap the query's among the items titled in the query's language, so "skirt" also finds the items tagged only スカート.
- Items found this way rank among the word matches at half weight. The results page names the tags and offers to search without them.
- Palette suggestions search these tags too. RSS feeds do not, so a feed's items do not move with the tags chosen to translate its query; its words still match the tag dictionary's words.

## Semantic search (optional)

Semantic search finds items by meaning across languages ("哥特裙" finds ゴスロリ dresses) and merges both rankings. It needs an embeddings model; see the [deployment guide](DEPLOY.md#semantic-search-optional).

- **Slow provider:** when the AI provider takes longer than 2.5 s, the results page shows the word matches with a note, and reads the search once more after 1 s. If the matches by meaning are still not ready, it says so and stops.
- **Provider failure:** when the provider fails, the search shows the word matches alone.
- The request that timed out keeps running and, when it succeeds, caches the query's vector, so the same search a moment later uses it.

## Related searches

A search that semantic search answered offers up to four related searches above the results. A search that found nothing offers them under "No items found" once the query has been embedded.

- They are BOOTH tags whose items are closest to the query in meaning (ミニスカート and long skirt for "skirt").
- Each is shown in the site's language when the tag dictionary has it.
- None repeats or translates the query or one of its words: "long hair" does not offer ロング, which reads "long".
- No two make the same search: wavy hair is left out after ウェーブヘア, which reads "wavy hair".
- Choosing one replaces the query's words and keeps the avatar names and the filters.
- The row reads only the embedding a search already stored. It never asks the AI provider, so there is none for a query without a stored vector, for an item link or number, or while the configured embeddings model is not the one the items were embedded with.

The exact exclusions are in the [API reference](API.md#related-searches).
