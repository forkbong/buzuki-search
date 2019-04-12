use std::str::CharIndices;

use tantivy::tokenizer::{Token, TokenStream, Tokenizer};

/// Tokenize the text by splitting on whitespace and punctuation and finding all ngrams for each
/// word. Based on the built-in tantivy SimpleTokenizer.
#[derive(Clone)]
pub struct NgramTokenizer;

pub struct SimpleTokenStream<'a> {
    text: &'a str,
    chars: CharIndices<'a>,
    token: Token,

    offset_from: usize,
    first: bool,
    last: bool,
}

impl<'a> Tokenizer<'a> for NgramTokenizer {
    type TokenStreamImpl = SimpleTokenStream<'a>;

    fn token_stream(&self, text: &'a str) -> Self::TokenStreamImpl {
        SimpleTokenStream {
            text: text.trim(),
            chars: text.char_indices(),
            token: Token::default(),

            offset_from: 0,
            first: true,
            last: false,
        }
    }
}

impl<'a> TokenStream for SimpleTokenStream<'a> {
    fn advance(&mut self) -> bool {
        self.token.text.clear();
        self.token.position = self.token.position.wrapping_add(1);

        if self.last {
            return false;
        }

        loop {
            match self.chars.next() {
                Some((offset, c)) => {
                    if self.first {
                        if !c.is_alphanumeric() {
                            continue;
                        }
                        self.offset_from = offset;
                        self.first = false;
                        continue;
                    }

                    self.token.offset_from = self.offset_from;
                    self.token.offset_to = offset;
                    self.token
                        .text
                        .push_str(&self.text[self.offset_from..offset]);

                    if !c.is_alphanumeric() {
                        self.first = true;
                    }

                    return true;
                }
                None => {
                    self.last = true;

                    let offset_to = self.text.len();

                    self.token.offset_from = self.offset_from;
                    self.token.offset_to = offset_to;
                    self.token
                        .text
                        .push_str(&self.text[self.offset_from..offset_to]);

                    return true;
                }
            }
        }
    }

    fn token(&self) -> &Token {
        &self.token
    }

    fn token_mut(&mut self) -> &mut Token {
        &mut self.token
    }
}
