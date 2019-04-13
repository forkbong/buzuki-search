use std::collections::HashMap;

use tantivy::collector::TopDocs;
use tantivy::doc;
use tantivy::query::QueryParser;
use tantivy::schema::{IndexRecordOption, Schema, TextFieldIndexing, TextOptions, STORED};
use tantivy::tokenizer::{
    Language, LowerCaser, RemoveLongFilter, SimpleTokenizer, Stemmer, Tokenizer,
};
use tantivy::Index;
use tantivy::IndexReader;
use tantivy::ReloadPolicy;

use lazy_static::lazy_static;
use log::error;
use regex::Regex;
use tempfile::tempdir;

use crate::greek_lower_caser::GreekLowerCaser;
use crate::tokenizer::NgramTokenizer;

fn get_options(tokenizer: &str) -> TextOptions {
    let text_field_indexing = TextFieldIndexing::default()
        .set_tokenizer(tokenizer)
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);

    TextOptions::default().set_indexing_options(text_field_indexing)
}

/// Remove lines that contain only chords and symbols and trim unneeded characters.
fn strip_metadata(string: &str) -> String {
    // We are interested in Greek lyrics so we can skip every line that only contains ASCII.
    lazy_static! {
        static ref RE: Regex = Regex::new(r"^[[:ascii:]]*$").unwrap();
    }

    let lines: Vec<&str> = string
        .split('\n')
        .filter(|line| !RE.is_match(line))
        .map(|line| {
            // Trim any symbols that indicate lyric repetition (e.g. "| 2x")
            line.trim_end_matches(|c: char| c == ' ' || c == '|' || c.is_ascii_digit() || c == 'x')
        })
        .collect();

    // Remove dashes that represent syllable repetition
    lines.join("\n").chars().filter(|&c| c != '-').collect()
}

/// Return greek string in greeklish.
fn to_greeklish(string: &str) -> String {
    // We always replace spaces with underscores. We don't need that for searching, only for
    // storing the slug, but it also works for searching so we leave it like that for simplicity.
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^a-z_\n]").unwrap();
    }
    string
        .to_lowercase()
        .replace("ψ", "ps")
        .replace("ξ", "ks")
        .replace("θ", "th")
        .replace("ου", "ou")
        .replace("ού", "ou")
        .replace("αυ", "au")
        .replace("αύ", "au")
        .replace("ευ", "eu")
        .replace("εύ", "eu")
        .chars()
        .map(|c| match c {
            'α' | 'ά' => 'a',
            'β' => 'v',
            'γ' => 'g',
            'δ' => 'd',
            'ε' | 'έ' => 'e',
            'ζ' => 'z',
            'η' | 'ή' => 'i',
            'ι' | 'ί' | 'ϊ' | 'ΐ' => 'i',
            'κ' => 'k',
            'λ' => 'l',
            'μ' => 'm',
            'ν' => 'n',
            'ο' | 'ό' => 'o',
            'π' => 'p',
            'ρ' => 'r',
            'σ' | 'ς' => 's',
            'τ' => 't',
            'υ' | 'ύ' => 'y',
            'φ' => 'f',
            'χ' => 'x',
            'ω' | 'ώ' => 'o',
            ' ' => '_',
            x => x,
        })
        .filter(|&c| !RE.is_match(c.to_string().as_str()))
        .collect()
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
        let greek_ngram_tokenizer = NgramTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(GreekLowerCaser);

        let english_ngram_tokenizer = NgramTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let greek_simple_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(GreekLowerCaser);

        let english_simple_tokenizer = SimpleTokenizer
            .filter(RemoveLongFilter::limit(40))
            .filter(LowerCaser);

        let greek_stem_tokenizer = SimpleTokenizer
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

            let contents = std::fs::read_to_string(filename.clone())?;
            let mut parts = contents.splitn(4, "\n\n");
            let head = parts.next().unwrap();
            let _song_scale = parts.next().unwrap();
            let _song_rhythm = parts.next().unwrap();
            let song_body = parts.next().unwrap();

            let head_parts: Vec<&str> = head.split('\n').collect();
            let (song_name, song_artist) = match head_parts[..] {
                [song_name, song_artist, _song_url] => (song_name, song_artist),
                _ => {
                    error!("Invalid song format");
                    std::process::exit(1);
                }
            };

            // On songs, we tokenize the name and body with both the simple
            // and the stemmed tokenizer. This results in including stemmed
            // results, but giving a higher score to full word results.
            let song_slug = to_greeklish(song_name);
            let song_body = strip_metadata(song_body);
            let song_body_greeklish = to_greeklish(song_body.as_str());
            index_writer.add_document(doc!(
                name => song_name,
                slug => song_slug.as_str(),
                body => song_body.as_str(),
                body_greeklish => song_body_greeklish.as_str(),
                ngram_name => song_name,
                ngram_slug => song_slug.as_str(),
                ngram_body => song_body.as_str(),
                ngram_body_greeklish => song_body_greeklish.as_str(),
                stemmed_name => song_name,
                stemmed_body => song_body.as_str(),
                url => format!("/songs/{}/", song_slug),
            ));

            if !indexed_artists.contains(&String::from(song_artist)) {
                let song_artist_slug = to_greeklish(song_artist);
                index_writer.add_document(doc!(
                    name => song_artist,
                    slug => song_artist_slug.clone(),
                    ngram_name => song_artist,
                    ngram_slug => song_artist_slug.clone(),
                    url => format!("/artists/{}/", song_artist_slug),
                ));
                indexed_artists.push(String::from(song_artist));
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
                slug => scale_slug.clone(),
                ngram_name => scale,
                ngram_slug => scale_slug.clone(),
                url => format!("/scales/{}/", scale_slug),
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
    use tantivy::tokenizer::{Language, SimpleTokenizer, Stemmer, TokenStream, Tokenizer};

    use crate::greek_lower_caser::GreekLowerCaser;
    use crate::tokenizer::NgramTokenizer;

    #[test]
    fn test_simple_tokenizer() {
        let text = "Έλα τι λέει";
        let mut tokens = vec![];
        let mut token_stream = SimpleTokenizer.filter(GreekLowerCaser).token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(tokens, vec!["ελα", "τι", "λεει"]);
    }

    #[test]
    fn test_custom_tokenizer() {
        let text = "Έλα τι λέει";
        let mut tokens = vec![];
        let mut token_stream = NgramTokenizer.filter(GreekLowerCaser).token_stream(text);
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
    fn test_greek_stemmer_tokenizer() {
        let text = "Εφουμέρναμε ένα βράδυ";
        let mut tokens = vec![];
        let mut token_stream = SimpleTokenizer
            .filter(Stemmer::new(Language::Greek))
            .token_stream(text);
        while token_stream.advance() {
            let token_text = token_stream.token().text.clone();
            tokens.push(token_text);
        }
        assert_eq!(tokens, vec!["εφουμερν", "εν", "βραδ"]);
    }
}
