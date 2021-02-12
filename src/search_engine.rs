use std::collections::HashMap;

use tantivy::collector::TopDocs;
use tantivy::doc;
use tantivy::query::QueryParser;
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED};
use tantivy::tokenizer::{
    Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, TextAnalyzer,
};
use tantivy::Index;
use tantivy::IndexReader;
use tantivy::ReloadPolicy;

use tempfile::tempdir;

use crate::greek_lower_caser::GreekLowerCaser;
use crate::song::Song;
use crate::tokenizer::NgramTokenizer;
use crate::utils::to_greeklish;

fn get_options(tokenizer: &str) -> TextOptions {
    let text_field_indexing = TextFieldIndexing::default()
        .set_tokenizer(tokenizer)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);

    TextOptions::default().set_indexing_options(text_field_indexing)
}

#[derive(Clone)]
pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    full_query_parser: QueryParser,
    ngram_query_parser: QueryParser,
    schema: Schema,
}

impl SearchEngine {
    pub fn new(song_dir: &str) -> tantivy::Result<SearchEngine> {
        // Build tokenizers
        let greek_ngram_tokenizer = TextAnalyzer::from(NgramTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(GreekLowerCaser);

        let english_ngram_tokenizer = TextAnalyzer::from(NgramTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let greek_simple_tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(GreekLowerCaser);

        let english_simple_tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let greek_stem_tokenizer = TextAnalyzer::from(SimpleTokenizer)
            .filter(RemoveLongFilter::limit(40))
            .filter(GreekLowerCaser)
            .filter(Stemmer::new(Language::Greek));

        // Build schema
        let mut schema_builder = Schema::builder();

        // Full word fields
        let name = schema_builder.add_text_field("name", get_options("el_simple") | STORED);
        let slug = schema_builder.add_text_field("slug", get_options("en_simple") | STORED);
        let body = schema_builder.add_text_field("body", get_options("el_simple"));
        let body_greeklish =
            schema_builder.add_text_field("body_greeklish", get_options("en_simple"));

        // Ngram fields
        let ngram_name = schema_builder.add_text_field("ngram_name", get_options("el_ngram"));
        let ngram_slug = schema_builder.add_text_field("ngram_slug", get_options("en_ngram"));
        let ngram_body = schema_builder.add_text_field("ngram_body", get_options("el_ngram"));
        let ngram_body_greeklish =
            schema_builder.add_text_field("body_greeklish", get_options("en_ngram"));

        // Stemmed fields
        let stemmed_name = schema_builder.add_text_field("stemmed_name", get_options("el_stem"));
        let stemmed_body = schema_builder.add_text_field("stemmed_body", get_options("el_stem"));

        // Keyword fields
        let url = schema_builder.add_text_field("url", STORED);

        let schema = schema_builder.build();

        // Build index
        let index_path = tempdir()?;

        let index = Index::create_in_dir(&index_path, schema)?;

        let manager = index.tokenizers();
        manager.register("el_ngram", greek_ngram_tokenizer);
        manager.register("en_ngram", english_ngram_tokenizer);
        manager.register("el_simple", greek_simple_tokenizer);
        manager.register("en_simple", english_simple_tokenizer);
        manager.register("el_stem", greek_stem_tokenizer);

        let mut index_writer = index.writer(50_000_000)?;

        let mut indexed_artists: Vec<String> = vec![];

        for path in std::fs::read_dir(song_dir).unwrap() {
            let filename = path.unwrap().path();
            let song = Song::from_path(&filename)?;

            // On songs, we tokenize the name and body with both the simple
            // and the stemmed tokenizer. This results in including stemmed
            // results, but giving a higher score to full word results.
            index_writer.add_document(doc!(
                name => song.name.as_str(),
                slug => song.slug.as_str(),
                body => song.body.as_str(),
                body_greeklish => song.body_greeklish.as_str(),
                ngram_name => song.name.as_str(),
                ngram_slug => song.slug.as_str(),
                ngram_body => song.body.as_str(),
                ngram_body_greeklish => song.body_greeklish.as_str(),
                stemmed_name => song.name.as_str(),
                stemmed_body => song.body.as_str(),
                url => format!("/songs/{}/", song.slug.as_str()),
            ));

            if !indexed_artists.contains(&song.artist) {
                index_writer.add_document(doc!(
                    name => song.artist.as_str(),
                    slug => song.artist_slug.as_str(),
                    ngram_name => song.artist.as_str(),
                    ngram_slug => song.artist_slug.as_str(),
                    url => format!("/artists/{}/", song.artist_slug.as_str()),
                ));
                indexed_artists.push(song.artist);
            }
        }

        for &scale in &[
            "Ματζόρε",
            "Ραστ",
            "Φυσικό Μινόρε",
            "Αρμονικό Μινόρε",
            "Χιτζάζ",
            "Χιτζαζκάρ",
            "Πειραιώτικο",
            "Ουσάκ",
            "Καρσιγάρ",
            "Σαμπάχ",
            "Νικρίζ",
            "Νιαβέντ",
            "Χουζάμ",
            "Σεγκιάχ",
            "Σουζινάκ",
            "Κιουρντί",
        ] {
            let scale_slug = to_greeklish(scale);
            index_writer.add_document(doc!(
                name => scale,
                slug => scale_slug.as_str(),
                ngram_name => scale,
                ngram_slug => scale_slug.as_str(),
                url => format!("/scales/{}/", scale_slug.as_str()),
            ));
        }

        index_writer.commit()?;

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual) // OnCommit?
            .try_into()?;

        let mut full_query_parser = QueryParser::for_index(
            &index,
            vec![name, slug, body, body_greeklish, stemmed_name, stemmed_body],
        );
        full_query_parser.set_conjunction_by_default();

        let mut ngram_query_parser = QueryParser::for_index(
            &index,
            vec![ngram_name, ngram_slug, ngram_body, ngram_body_greeklish],
        );
        ngram_query_parser.set_conjunction_by_default();

        let schema = index.schema();

        Ok(SearchEngine {
            index,
            reader,
            full_query_parser,
            ngram_query_parser,
            schema,
        })
    }

    pub fn search(&self, query: &str, full: bool) -> tantivy::Result<String> {
        let searcher = self.reader.searcher();
        let (query_parser, limit) = if full {
            (&self.full_query_parser, 1000)
        } else {
            (&self.ngram_query_parser, 15)
        };
        let query = query_parser.parse_query(query)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;
        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc = searcher.doc(doc_address)?;
            let mut entry = HashMap::new();
            for field_value in retrieved_doc.field_values() {
                let field_name = self.schema.get_field_name(field_value.field());
                let value = field_value.value().text().unwrap();
                entry.insert(field_name.to_string(), value.to_string());
            }
            results.push(entry);
        }
        Ok(serde_json::to_string(&results)?)
    }
}

#[cfg(test)]
mod tests {
    use tantivy::tokenizer::{Language, LowerCaser, SimpleTokenizer, Stemmer, TextAnalyzer};

    use crate::greek_lower_caser::GreekLowerCaser;
    use crate::tokenizer::NgramTokenizer;

    #[test]
    fn test_simple_tokenizer() {
        let text = "Έλα τι λέει";
        let mut tokens = vec![];
        let mut token_stream = TextAnalyzer::from(SimpleTokenizer)
            .filter(GreekLowerCaser)
            .token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(tokens, vec!["ελα", "τι", "λεει"]);
    }

    #[test]
    fn test_greek_ngram_tokenizer() {
        let text = "Έλα τι λέει";
        let mut tokens = vec![];
        let mut token_stream = TextAnalyzer::from(NgramTokenizer)
            .filter(GreekLowerCaser)
            .token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(
            tokens,
            vec!["ε", "ελ", "ελα", "τ", "τι", "λ", "λε", "λεε", "λεει"]
        );
    }

    #[test]
    fn test_english_ngram_tokenizer() {
        let text = "Whazup";
        let mut tokens = vec![];
        let mut token_stream = TextAnalyzer::from(NgramTokenizer)
            .filter(LowerCaser)
            .token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(tokens, vec!["w", "wh", "wha", "whaz", "whazu", "whazup"]);
    }

    #[test]
    fn test_greek_stemmer_tokenizer() {
        let text = "Εφουμέρναμε ένα βράδυ";
        let mut tokens = vec![];
        let mut token_stream = TextAnalyzer::from(SimpleTokenizer)
            .filter(Stemmer::new(Language::Greek))
            .token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(tokens, vec!["εφουμερν", "εν", "βραδ"]);
    }
}
