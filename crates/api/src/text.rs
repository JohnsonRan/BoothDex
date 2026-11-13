//! Tantivy text index, kept on disk (memory-mapped) when a directory is given, otherwise in RAM. Three ways to match:
//! - words: Lindera (IPADIC) morphological tokens over title/tags/author/bio, BM25-ranked
//! - substring: character bigrams over title/tags with spaces and punctuation removed, verified per query word by
//!   `contains` (covers partial names like "ミル" → "ミルティナ", and "shorthair" / "short hair" / "short-hair")
//! - typos: a Latin query that matches nothing either way is tried once more allowing one typo per word (`typo_tolerant`)

use std::path::Path;

use anyhow::Result;
use lindera::{dictionary::load_dictionary, mode::Mode, segmenter::Segmenter};
use lindera_tantivy::tokenizer::LinderaTokenizer;
use roaring::RoaringBitmap;
use rustc_hash::FxHashMap;
use tantivy::collector::{Collector, DocSetCollector, SegmentCollector};
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{FAST, Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{LowerCaser, NgramTokenizer, SimpleTokenizer, TextAnalyzer, Token, TokenFilter, TokenStream, Tokenizer};
use unicode_normalization::UnicodeNormalization;
use tantivy::{DocAddress, DocId, Index, IndexReader, Score, SegmentOrdinal, SegmentReader, TantivyDocument, Term};

pub struct TextDoc {
    pub title: String,
    pub tags: String,
    pub author: String,
    pub bio: String,
}

pub struct TextIndex {
    reader: IndexReader,
    parser: QueryParser,
    gram: Field,
    /// title, tags and author: the word fields a typo-tolerant retry searches
    named: [Field; 3],
    /// per segment: tantivy doc id -> catalog doc index
    seg_docs: Vec<Vec<u32>>,
    /// title + tags, compacted (see `compact`), for substring verification
    haystack: Vec<String>,
}

struct ScoredDocs;

struct ScoredSegment {
    segment_ord: SegmentOrdinal,
    hits: Vec<(Score, DocAddress)>,
}

impl Collector for ScoredDocs {
    type Fruit = Vec<(Score, DocAddress)>;
    type Child = ScoredSegment;

    fn for_segment(&self, segment_ord: SegmentOrdinal, _reader: &SegmentReader) -> tantivy::Result<Self::Child> {
        Ok(ScoredSegment { segment_ord, hits: Vec::new() })
    }

    fn requires_scoring(&self) -> bool {
        true
    }

    fn merge_fruits(&self, fruits: Vec<Self::Fruit>) -> tantivy::Result<Self::Fruit> {
        Ok(fruits.into_iter().flatten().collect())
    }
}

impl SegmentCollector for ScoredSegment {
    type Fruit = Vec<(Score, DocAddress)>;

    fn collect(&mut self, doc_id: DocId, score: Score) {
        self.hits.push((score, DocAddress { segment_ord: self.segment_ord, doc_id }));
    }

    fn harvest(self) -> Self::Fruit {
        self.hits
    }
}

/// Token filter that writes hiragana as katakana (see `fold`).
#[derive(Clone)]
struct KanaFolder;

#[derive(Clone)]
struct KanaFolderFilter<T>(T);

struct KanaFolderStream<T>(T);

impl TokenFilter for KanaFolder {
    type Tokenizer<T: Tokenizer> = KanaFolderFilter<T>;

    fn transform<T: Tokenizer>(self, tokenizer: T) -> KanaFolderFilter<T> {
        KanaFolderFilter(tokenizer)
    }
}

impl<T: Tokenizer> Tokenizer for KanaFolderFilter<T> {
    type TokenStream<'a> = KanaFolderStream<T::TokenStream<'a>>;

    fn token_stream<'a>(&'a mut self, text: &'a str) -> Self::TokenStream<'a> {
        KanaFolderStream(self.0.token_stream(text))
    }
}

impl<T: TokenStream> TokenStream for KanaFolderStream<T> {
    fn advance(&mut self) -> bool {
        if !self.0.advance() {
            return false;
        }
        let text = &mut self.0.token_mut().text;
        if !text.is_ascii() {
            *text = text.chars().map(kana).collect();
        }
        true
    }

    fn token(&self) -> &Token {
        self.0.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.0.token_mut()
    }
}

fn text_field(tokenizer: &str, positions: bool) -> TextOptions {
    TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(tokenizer)
            .set_index_option(if positions { IndexRecordOption::WithFreqsAndPositions } else { IndexRecordOption::Basic }),
    )
}

/// Bump when what goes into the index changes, so indexes already on disk are rebuilt.
/// 3: tantivy 0.27 and Lindera 6 (dictionary and tokenizer versions changed).
/// 4: NFKC and kana folding (`fold`).
const FORMAT: u32 = 4;

/// Words shorter than this must match exactly in a typo-tolerant retry: one edit in "hair" reaches "hare" and "pair".
const TYPO_MIN: usize = 5;

/// A typo-tolerant retry takes queries of at most this many words: each fuzzy word costs a scan of the term
/// dictionary and a bitset per field, so a long query that misses on one word would pay for all the others.
const TYPO_MAX_WORDS: usize = 3;

/// Hiragana as katakana; every other character unchanged.
fn kana(c: char) -> char {
    match c {
        '\u{3041}'..='\u{3096}' | '\u{309d}'..='\u{309e}' => char::from_u32(c as u32 + 0x60).unwrap_or(c),
        _ => c,
    }
}

/// The form search compares text in: NFKC (full-width Latin and half-width kana become the ordinary forms),
/// lowercase, and hiragana as katakana, so "ＤＲＥＳＳ", "ﾐﾙﾃｨﾅ" and "しなの" find "dress", "ミルティナ" and "シナノ".
pub(crate) fn fold(s: &str) -> String {
    fold_width(s).chars().map(kana).collect()
}

/// NFKC and lowercase only, with kana kept as written: model names (`Lookup`), where reading "ココア" as the
/// model ここあ would turn an ordinary word into a compatibility filter (owner, 2026-11-13).
pub(crate) fn fold_width(s: &str) -> String {
    s.nfkc().flat_map(char::to_lowercase).collect()
}

/// Text handed to Lindera: NFKC only. Kana are folded per token afterwards (`KanaFolder`), because the
/// dictionary segments hiragana words and particles differently from katakana.
fn nfkc(s: &str) -> String {
    s.nfkc().collect()
}

/// Folded letters and digits only: "Short-Hair" and "short hair" both become "shorthair".
fn compact(s: &str) -> String {
    fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// Adjacent Latin words joined ("Short Hair" -> "shorthair"), indexed so a query written as one word finds text
/// written as two. The other direction ("short hair" finding "ShortHair") is covered by the substring path.
fn joined_pairs(texts: &[&str]) -> String {
    let mut out = String::new();
    for t in texts {
        let words: Vec<String> = t
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| w.chars().any(|c| c.is_ascii_alphabetic()))
            .map(str::to_ascii_lowercase)
            .collect();
        for w in words.windows(2) {
            out.push_str(&w[0]);
            out.push_str(&w[1]);
            out.push(' ');
        }
    }
    out
}

/// Separators users type between words ("short-hair", "a/b") mean a word break, not query syntax
/// (a leading "-" would otherwise exclude the word).
fn query_words(q: &str) -> String {
    nfkc(q).chars().map(|c| if c.is_alphanumeric() || c == '"' { c } else { ' ' }).collect()
}

fn bigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| w.iter().collect()).collect()
}

impl TextIndex {
    /// Builds the index, or reuses one already on disk for exactly these docs.
    /// `dir`: index root; text content and format select a reusable subdirectory. `None` keeps it in RAM.
    /// An on-disk index is memory-mapped, so its pages live in the OS page cache instead of the heap,
    /// and a restart with an unchanged catalogue skips re-indexing.
    pub fn build(docs: impl ExactSizeIterator<Item = TextDoc>, dir: Option<&Path>, fingerprint: u64) -> Result<Self> {
        let mut sb = Schema::builder();
        let doc = sb.add_u64_field("doc", FAST);
        let title = sb.add_text_field("title", text_field("ja", true));
        let tags = sb.add_text_field("tags", text_field("ja", true));
        let author = sb.add_text_field("author", text_field("ja", true));
        let bio = sb.add_text_field("bio", text_field("ja", true));
        let gram = sb.add_text_field("gram", text_field("bigram", false));
        let pairs = sb.add_text_field("pairs", text_field("simple", false));
        let schema = sb.build();
        let doc_count = docs.len();
        let index_dir = dir.map(|d| d.join(format!("text-v{FORMAT}-{fingerprint:016x}")));
        // the marker records doc count and content fingerprint, so a half-written or stale index is never reused
        let marker = index_dir.as_ref().map(|d| d.join("READY"));
        let expected_marker = format!("v{FORMAT}:{doc_count}:{fingerprint:016x}");
        let reuse = marker.as_ref().is_some_and(|m| std::fs::read_to_string(m).ok().as_deref() == Some(&expected_marker));
        let index = match index_dir.as_deref() {
            Some(d) if reuse => Index::open_in_dir(d)?,
            Some(d) => {
                let _ = std::fs::remove_dir_all(d);
                std::fs::create_dir_all(d)?;
                Index::create_in_dir(d, schema)?
            }
            None => Index::create_in_ram(schema),
        };

        let segmenter = Segmenter::new(Mode::Normal, load_dictionary("embedded://ipadic")?, None);
        index
            .tokenizers()
            .register("ja", TextAnalyzer::builder(LinderaTokenizer::from_segmenter(segmenter)).filter(LowerCaser).filter(KanaFolder).build());
        index
            .tokenizers()
            .register("bigram", TextAnalyzer::builder(NgramTokenizer::new(2, 2, false)?).filter(LowerCaser).build());
        index.tokenizers().register("simple", TextAnalyzer::builder(SimpleTokenizer::default()).filter(LowerCaser).build());

        let mut haystack = Vec::with_capacity(doc_count);
        if reuse {
            haystack.extend(docs.map(|d| compact(&format!("{} {}", d.title, d.tags))));
        } else {
            // a smaller indexing heap lowers peak memory during (re)builds
            let mut writer = index.writer::<TantivyDocument>(192 * 1024 * 1024)?;
            for (i, d) in docs.enumerate() {
                let hay = compact(&format!("{} {}", d.title, d.tags));
                let mut td = TantivyDocument::default();
                td.add_u64(doc, i as u64);
                let (t, g, b) = (nfkc(&d.title), nfkc(&d.tags), nfkc(&d.bio));
                td.add_text(title, &t);
                td.add_text(tags, &g);
                td.add_text(author, nfkc(&d.author));
                td.add_text(bio, &b);
                td.add_text(gram, &hay);
                td.add_text(pairs, joined_pairs(&[&t, &g, &b]));
                writer.add_document(td)?;
                haystack.push(hay);
            }
            writer.commit()?;
            writer.wait_merging_threads()?;
            if let Some(m) = &marker {
                std::fs::write(m, &expected_marker)?;
            }
        }
        let reader = index.reader()?;
        let searcher = reader.searcher();
        let seg_docs = searcher
            .segment_readers()
            .iter()
            .map(|seg| {
                let col = seg.fast_fields().u64("doc")?;
                Ok((0..seg.max_doc()).map(|d| col.first(d).unwrap_or(0) as u32).collect())
            })
            .collect::<Result<_>>()?;

        let mut parser = QueryParser::for_index(&index, vec![title, tags, author, bio, pairs]);
        parser.set_conjunction_by_default();
        parser.set_field_boost(title, 3.0);
        parser.set_field_boost(tags, 2.0);
        parser.set_field_boost(author, 2.0);
        // Only remove our own obsolete indexes, after the replacement is ready. Mapped files on Windows
        // may remain until the old snapshot drops; cleanup retries on the next reload.
        if let (Some(root), Some(current)) = (dir, &index_dir)
            && let Ok(entries) = std::fs::read_dir(root)
        {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let owned = name.starts_with("text-v") || name.strip_prefix('v').is_some_and(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()));
                if owned && entry.path() != *current {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
        Ok(Self { reader, parser, gram, named: [title, tags, author], seg_docs, haystack })
    }

    /// Matching catalog docs with a relevance score (substring-only hits get a small floor score).
    pub fn search(&self, q: &str) -> Result<(RoaringBitmap, FxHashMap<u32, f32>)> {
        let searcher = self.reader.searcher();
        let mut hits = RoaringBitmap::new();
        let mut scores = FxHashMap::default();

        let (query, _errors) = self.parser.parse_query_lenient(&query_words(q));
        for (score, addr) in searcher.search(&query, &ScoredDocs)? {
            let d = self.seg_docs[addr.segment_ord as usize][addr.doc_id as usize];
            hits.insert(d);
            scores.insert(d, score);
        }

        // every query word must appear in the compacted title + tags; also tried with all words joined, so
        // "short hair" finds "ShortHair" and "shorthair" finds "Short Hair"
        let words: Vec<String> = query_words(q).split_whitespace().map(compact).filter(|w| !w.is_empty()).collect();
        let mut needles = vec![words.clone()];
        // With no single-character word, the joined hits are already a subset of the per-word hits.
        if words.len() > 1 && words.iter().any(|w| w.chars().count() < 2) {
            needles.push(vec![words.concat()]);
        }
        for needles in needles {
            // single characters have no bigram; they would match nearly everything anyway
            if needles.is_empty() || needles.iter().any(|w| w.chars().count() < 2) {
                continue;
            }
            let mut grams: Vec<String> = needles.iter().flat_map(|w| bigrams(w)).collect();
            grams.sort();
            grams.dedup();
            let gram_query = BooleanQuery::new(
                grams
                    .iter()
                    .map(|g| {
                        let tq: Box<dyn Query> = Box::new(TermQuery::new(Term::from_field_text(self.gram, g), IndexRecordOption::Basic));
                        (Occur::Must, tq)
                    })
                    .collect(),
            );
            for addr in searcher.search(&gram_query, &DocSetCollector)? {
                let d = self.seg_docs[addr.segment_ord as usize][addr.doc_id as usize];
                let hay = &self.haystack[d as usize];
                if !hits.contains(d) && needles.iter().all(|w| hay.contains(w.as_str())) {
                    hits.insert(d);
                    scores.insert(d, 0.01);
                }
            }
        }
        if hits.is_empty() {
            for (score, addr) in self.typo_tolerant(&searcher, &words)? {
                let d = self.seg_docs[addr.segment_ord as usize][addr.doc_id as usize];
                hits.insert(d);
                scores.insert(d, score);
            }
        }
        Ok((hits, scores))
    }

    /// Matches for a query that found nothing, allowing one typo (a letter added, dropped, changed, or two swapped)
    /// in each word of at least `TYPO_MIN` letters: "shinnano" finds "Shinano". Shorter words, and words with digits
    /// ("20277" is not "2077"), must match exactly. Only for ASCII queries of at most `TYPO_MAX_WORDS` words: in
    /// kana or kanji one changed character is usually another word.
    fn typo_tolerant(&self, searcher: &tantivy::Searcher, words: &[String]) -> Result<Vec<(Score, DocAddress)>> {
        let typo = |w: &String| w.len() >= TYPO_MIN && w.bytes().all(|b| b.is_ascii_alphabetic());
        if words.len() > TYPO_MAX_WORDS || !words.iter().all(|w| w.is_ascii()) || !words.iter().any(typo) {
            return Ok(Vec::new());
        }
        let word = |w: &String| -> Box<dyn Query> {
            let fields = self.named.iter().map(|&f| {
                let term = Term::from_field_text(f, w);
                let q: Box<dyn Query> = if typo(w) {
                    Box::new(FuzzyTermQuery::new(term, 1, true))
                } else {
                    Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs))
                };
                (Occur::Should, q)
            });
            Box::new(BooleanQuery::new(fields.collect()))
        };
        let query = BooleanQuery::new(words.iter().map(|w| (Occur::Must, word(w))).collect());
        Ok(searcher.search(&query, &ScoredDocs)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(title: &str, tags: &str) -> TextDoc {
        TextDoc { title: title.into(), tags: tags.into(), author: String::new(), bio: String::new() }
    }

    fn sample() -> std::vec::IntoIter<TextDoc> {
        vec![doc("ミルティナ用 ゴシックドレス", "衣装"), doc("Gothic Bone Dress", "VRChat"), doc("髪型セット", "hair")].into_iter()
    }

    fn fingerprint(docs: &[TextDoc]) -> u64 {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        docs.len().hash(&mut hasher);
        for d in docs {
            d.title.hash(&mut hasher);
            d.tags.hash(&mut hasher);
            d.author.hash(&mut hasher);
            d.bio.hash(&mut hasher);
        }
        hasher.finish()
    }

    #[test]
    fn on_disk_index_is_reused_only_when_complete() {
        let dir = std::env::temp_dir().join(format!("textidx-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let s1 = sample().collect::<Vec<_>>();
        let fp1 = fingerprint(&s1);
        drop(TextIndex::build(s1.into_iter(), Some(&dir), fp1).unwrap());
        let index_dir = dir.join(format!("text-v{FORMAT}-{fp1:016x}"));
        assert_eq!(std::fs::read_to_string(index_dir.join("READY")).unwrap(), format!("v{FORMAT}:3:{fp1:016x}"));
        std::fs::write(index_dir.join("reused"), "keep").unwrap();
        std::fs::create_dir(dir.join("unrelated")).unwrap();
        // reopened from disk: same results
        let reopened = TextIndex::build(sample(), Some(&dir), fp1).unwrap();
        assert!(index_dir.join("reused").exists());
        assert_eq!(reopened.search("dress").unwrap().0.iter().collect::<Vec<_>>(), [1]);
        assert_eq!(reopened.search("ミルティ").unwrap().0.iter().collect::<Vec<_>>(), [0]);
        // same doc count (3), but changed content and fingerprint forces a rebuild
        let s_changed = vec![
            doc("ミルティナ用 ゴシックドレス", "衣装"),
            doc("Summer Beach Outfit", "VRChat"), // changed from "Gothic Bone Dress"
            doc("髪型セット", "hair"),
        ];
        let fp_changed = fingerprint(&s_changed);
        let changed = TextIndex::build(s_changed.into_iter(), Some(&dir), fp_changed).unwrap();
        assert_eq!(changed.search("beach").unwrap().0.iter().collect::<Vec<_>>(), [1]);
        assert_eq!(changed.search("bone").unwrap().0.iter().collect::<Vec<_>>(), Vec::<u32>::new());
        assert_eq!(reopened.search("bone").unwrap().0.iter().collect::<Vec<_>>(), [1]);
        assert!(dir.join("unrelated").exists());
        drop(reopened);
        drop(changed);
        // a different doc count forces a rebuild instead of reusing a mismatched index
        let s_two = vec![doc("Gothic Bone Dress", "VRChat"), doc("x", "y")];
        let fp_two = fingerprint(&s_two);
        let two = TextIndex::build(s_two.into_iter(), Some(&dir), fp_two).unwrap();
        assert_eq!(two.search("dress").unwrap().0.iter().collect::<Vec<_>>(), [0]);
        drop(two);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn word_and_substring_matches() {
        let idx = TextIndex::build(sample(), None, 0).unwrap();
        let (hits, scores) = idx.search("dress").unwrap();
        assert_eq!(hits.iter().collect::<Vec<_>>(), [1]);
        assert!(scores[&1] > 0.01);
        // partial katakana name only reachable through the bigram path
        let (hits, _) = idx.search("ミルティ").unwrap();
        assert_eq!(hits.iter().collect::<Vec<_>>(), [0]);
        let (hits, _) = idx.search("nothing-here").unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn single_character_words_keep_the_joined_substring_fallback() {
        let docs = vec![doc("AB accessory", ""), doc("XZ set", ""), doc("ShortHair", ""), doc("ショートヘア", "")];
        let idx = TextIndex::build(docs.into_iter(), None, 0).unwrap();
        for (query, expected) in [("a b", vec![0]), ("x z", vec![1]), ("s h", vec![2]), ("シ ョート", vec![3]), ("short hair", vec![2])] {
            assert_eq!(idx.search(query).unwrap().0.iter().collect::<Vec<_>>(), expected, "query {query:?}");
        }
    }

    #[test]
    fn width_and_kana_variants_match_the_same_items() {
        let docs = vec![doc("ミルティナ用 ゴシックドレス", "衣装"), doc("Gothic Bone Dress", "VRChat"), doc("しなの用 ショートヘア", ""), doc("ＦＵＬＬ Ｗｉｄｔｈ Hat", "")];
        let idx = TextIndex::build(docs.into_iter(), None, 0).unwrap();
        let ids = |q: &str| idx.search(q).unwrap().0.iter().collect::<Vec<_>>();
        for (q, expected) in [
            ("ＤＲＥＳＳ", vec![1]),
            ("full width", vec![3]),
            ("ﾐﾙﾃｨﾅ", vec![0]),
            ("みるてぃな", vec![0]),
            ("みるてぃ", vec![0]),
            ("シナノ", vec![2]),
            ("しょーとへあ", vec![2]),
        ] {
            assert_eq!(ids(q), expected, "query {q:?}");
        }
    }

    #[test]
    fn author_and_bio_words_ignore_width_and_kana() {
        // author and bio have no substring path: only their words can match
        let bio = |title: &str, author: &str, bio: &str| TextDoc { title: title.into(), tags: String::new(), author: author.into(), bio: bio.into() };
        let docs = vec![bio("衣装", "", "しなの対応"), bio("帽子", "", "ＲＥＡＤＭＥ included"), bio("髪型", "ＳＡＫＵＲＡ工房", ""), bio("靴", "", "Works on Kikyo")];
        let idx = TextIndex::build(docs.into_iter(), None, 0).unwrap();
        let ids = |q: &str| idx.search(q).unwrap().0.iter().collect::<Vec<_>>();
        for (q, expected) in [("シナノ", vec![0]), ("readme", vec![1]), ("sakura", vec![2]), ("ＫＩＫＹＯ", vec![3])] {
            assert_eq!(ids(q), expected, "query {q:?}");
        }
    }

    #[test]
    fn a_query_finding_nothing_allows_one_typo_per_long_latin_word() {
        let docs = vec![
            doc("Shinano Short Hair Ribbon", ""),
            doc("Manuka Dress", ""),
            doc("Rusk Dragon Wings", ""),
            doc("ミルティナ用 ゴシックドレス", ""),
            doc("Press Kit", ""),
            doc("Cyberpunk 2077 Jacket", ""),
        ];
        let idx = TextIndex::build(docs.into_iter(), None, 0).unwrap();
        let ids = |q: &str| idx.search(q).unwrap().0.iter().collect::<Vec<_>>();
        for (q, expected) in [
            ("shinnano", vec![0]),
            ("manuak", vec![1]),
            ("dragn wings", vec![2]),
            ("wings drgon", vec![2]),
            // a word under 5 letters must still match exactly
            ("hari", vec![]),
            ("rusk dragn", vec![2]),
            ("rusc dragn", vec![]),
            // only when nothing matched: "dress" does not also bring "press"
            ("dress", vec![1]),
            ("xyzzyq", vec![]),
            // numbers are not words with typos: 20277 is not 2077
            ("20277", vec![]),
            ("manuka 20277", vec![]),
            // at most three words are retried, which bounds its cost
            ("shinnano short hair", vec![0]),
            ("shinnano short hair ribbon", vec![]),
            // other scripts get no typo allowance
            ("ミルティノ", vec![]),
        ] {
            assert_eq!(ids(q), expected, "query {q:?}");
        }
    }

    #[test]
    fn fold_unifies_width_case_and_kana() {
        assert_eq!(fold("ＡＢＣ ｱｲｳ あいう ゔ"), "abc アイウ アイウ ヴ");
        assert_eq!(fold("ヷ・ー～"), "ヷ・ー~");
    }

    #[test]
    fn spacing_and_hyphens_do_not_matter() {
        let docs = vec![
            doc("Shinano Short Hair", "VRChat"),
            doc("ShortHair for しなの", "Shinano"),
            doc("short-hair set", "shinano"),
            doc("Long Hair", "Shinano"),
            doc("Shinano Shirt", "hair accessory"),
            TextDoc { title: "Hair cutting tool".into(), tags: "tool".into(), author: String::new(), bio: "Works on Shinano, makes short hair.".into() },
        ];
        let idx = TextIndex::build(docs.into_iter(), None, 0).unwrap();
        let ids = |q: &str| idx.search(q).unwrap().0.iter().collect::<Vec<_>>();
        for q in ["shinano short hair", "shinano shorthair", "shinano short-hair", "Shinano  Short/Hair"] {
            assert_eq!(ids(q), [0, 1, 2, 5], "query {q:?}");
        }
        // a hyphen inside a word is a separator, not an exclusion
        assert_eq!(ids("short-hair"), [0, 1, 2, 5]);
        assert!(joined_pairs(&["Short Hair 2.0 for しなの"]).contains("shorthair"));
    }
}
