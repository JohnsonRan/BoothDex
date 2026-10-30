//! In-memory Tantivy index. Two ways to match:
//! - words: Lindera (IPADIC) morphological tokens over title/tags/author/bio, BM25-ranked
//! - substring: character bigrams over title/tags, verified by `contains` so results are exact
//!   (covers partial names like "ミル" → "ミルティナ" that morphological tokens miss)

use std::collections::HashMap;

use anyhow::Result;
use lindera::{dictionary::load_dictionary, mode::Mode, segmenter::Segmenter};
use lindera_tantivy::tokenizer::LinderaTokenizer;
use roaring::RoaringBitmap;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, Query, QueryParser, TermQuery};
use tantivy::schema::{FAST, Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions};
use tantivy::tokenizer::{LowerCaser, NgramTokenizer, TextAnalyzer};
use tantivy::{Index, IndexReader, TantivyDocument, Term};

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
    /// per segment: tantivy doc id -> catalog doc index
    seg_docs: Vec<Vec<u32>>,
    /// lowercased title + tags, for exact substring verification
    haystack: Vec<String>,
    doc_count: usize,
}

fn text_field(tokenizer: &str, positions: bool) -> TextOptions {
    TextOptions::default().set_indexing_options(
        TextFieldIndexing::default()
            .set_tokenizer(tokenizer)
            .set_index_option(if positions { IndexRecordOption::WithFreqsAndPositions } else { IndexRecordOption::Basic }),
    )
}

fn bigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    chars.windows(2).map(|w| w.iter().collect()).collect()
}

impl TextIndex {
    pub fn build(docs: impl ExactSizeIterator<Item = TextDoc>) -> Result<Self> {
        let mut sb = Schema::builder();
        let doc = sb.add_u64_field("doc", FAST);
        let title = sb.add_text_field("title", text_field("ja", true));
        let tags = sb.add_text_field("tags", text_field("ja", true));
        let author = sb.add_text_field("author", text_field("ja", true));
        let bio = sb.add_text_field("bio", text_field("ja", true));
        let gram = sb.add_text_field("gram", text_field("bigram", false));
        let index = Index::create_in_ram(sb.build());

        let segmenter = Segmenter::new(Mode::Normal, load_dictionary("embedded://ipadic")?, None);
        index
            .tokenizers()
            .register("ja", TextAnalyzer::builder(LinderaTokenizer::from_segmenter(segmenter)).filter(LowerCaser).build());
        index
            .tokenizers()
            .register("bigram", TextAnalyzer::builder(NgramTokenizer::new(2, 2, false)?).filter(LowerCaser).build());

        let doc_count = docs.len();
        let mut haystack = Vec::with_capacity(doc_count);
        let mut writer = index.writer::<TantivyDocument>(512 * 1024 * 1024)?;
        for (i, d) in docs.enumerate() {
            let hay = format!("{}\n{}", d.title, d.tags).to_lowercase();
            let mut td = TantivyDocument::default();
            td.add_u64(doc, i as u64);
            td.add_text(title, &d.title);
            td.add_text(tags, &d.tags);
            td.add_text(author, &d.author);
            td.add_text(bio, &d.bio);
            td.add_text(gram, &hay);
            writer.add_document(td)?;
            haystack.push(hay);
        }
        writer.commit()?;
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

        let mut parser = QueryParser::for_index(&index, vec![title, tags, author, bio]);
        parser.set_conjunction_by_default();
        parser.set_field_boost(title, 3.0);
        parser.set_field_boost(tags, 2.0);
        parser.set_field_boost(author, 2.0);
        Ok(Self { reader, parser, gram, seg_docs, haystack, doc_count })
    }

    /// Matching catalog docs with a relevance score (substring-only hits get a small floor score).
    pub fn search(&self, q: &str) -> Result<(RoaringBitmap, HashMap<u32, f32>)> {
        let searcher = self.reader.searcher();
        let mut hits = RoaringBitmap::new();
        let mut scores = HashMap::new();

        let (query, _errors) = self.parser.parse_query_lenient(q);
        for (score, addr) in searcher.search(&query, &TopDocs::with_limit(self.doc_count.max(1)).order_by_score())? {
            let d = self.seg_docs[addr.segment_ord as usize][addr.doc_id as usize];
            hits.insert(d);
            scores.insert(d, score);
        }

        let needle = q.trim().to_lowercase();
        let grams = bigrams(&needle);
        if !grams.is_empty() {
            let gram_query = BooleanQuery::new(
                grams
                    .iter()
                    .map(|g| {
                        let tq: Box<dyn Query> = Box::new(TermQuery::new(Term::from_field_text(self.gram, g), IndexRecordOption::Basic));
                        (Occur::Must, tq)
                    })
                    .collect(),
            );
            for (_, addr) in searcher.search(&gram_query, &TopDocs::with_limit(self.doc_count.max(1)).order_by_score())? {
                let d = self.seg_docs[addr.segment_ord as usize][addr.doc_id as usize];
                if !hits.contains(d) && self.haystack[d as usize].contains(&needle) {
                    hits.insert(d);
                    scores.insert(d, 0.01);
                }
            }
        }
        Ok((hits, scores))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(title: &str, tags: &str) -> TextDoc {
        TextDoc { title: title.into(), tags: tags.into(), author: String::new(), bio: String::new() }
    }

    #[test]
    fn word_and_substring_matches() {
        let idx = TextIndex::build(
            vec![doc("ミルティナ用 ゴシックドレス", "衣装"), doc("Gothic Bone Dress", "VRChat"), doc("髪型セット", "hair")].into_iter(),
        )
        .unwrap();
        let (hits, scores) = idx.search("dress").unwrap();
        assert_eq!(hits.iter().collect::<Vec<_>>(), [1]);
        assert!(scores[&1] > 0.01);
        // partial katakana name only reachable through the bigram path
        let (hits, _) = idx.search("ミルティ").unwrap();
        assert_eq!(hits.iter().collect::<Vec<_>>(), [0]);
        let (hits, _) = idx.search("nothing-here").unwrap();
        assert!(hits.is_empty());
    }
}
