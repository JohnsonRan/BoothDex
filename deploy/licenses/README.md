# Dictionary licences

The `api` binary embeds two dictionaries through Lindera. Its image copies this folder to
`/usr/share/doc/boothdex-api/`, because both licences require their notices to travel with every copy.

| Dictionary | Version | Used for | Licence |
| --- | --- | --- | --- |
| IPADIC (mecab-ipadic), Nara Institute of Science and Technology | 2.7.0-20250920 | segmenting Japanese text | [mecab-ipadic.COPYING](mecab-ipadic.COPYING), copied byte for byte from the archive |
| CC-CEDICT-MeCab, converted from [CC-CEDICT](https://www.mdbg.net/chinese/dictionary?page=cedict) | 0.1.0-20200409 | segmenting Chinese titles | [CC-CEDICT-MeCab.LICENSE](CC-CEDICT-MeCab.LICENSE): [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) |

Lindera's build converts each dictionary into its own binary format and embeds it unchanged otherwise.
