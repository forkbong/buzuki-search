use std::mem;

use tantivy::tokenizer::BoxTokenStream;
use tantivy::tokenizer::{Token, TokenFilter, TokenStream};

impl TokenFilter for GreekLowerCaser {
    fn transform<'a>(&self, token_stream: BoxTokenStream<'a>) -> BoxTokenStream<'a> {
        BoxTokenStream::from(GreekLowerCaserTokenStream {
            tail: token_stream,
            buffer: String::with_capacity(100),
        })
    }
}

/// Token filter that lowercase terms.
#[derive(Clone)]
pub struct GreekLowerCaser;

pub struct GreekLowerCaserTokenStream<'a> {
    buffer: String,
    tail: BoxTokenStream<'a>,
}

/// Writes a lowercased version of text into output.
fn to_greek_lowercase_unicode(text: &mut str, output: &mut String) {
    output.clear();
    for c in text.chars() {
        for c in c.to_lowercase() {
            output.push(match c {
                'ά' => 'α',
                'έ' => 'ε',
                'ί' => 'ι',
                'ϊ' => 'ι',
                'ΐ' => 'ι',
                'ύ' => 'υ',
                'ϋ' => 'υ',
                'ΰ' => 'υ',
                'ή' => 'η',
                'ό' => 'ο',
                'ώ' => 'ω',
                'ς' => 'σ',
                c => c,
            });
        }
    }
}

impl<'a> TokenStream for GreekLowerCaserTokenStream<'a> {
    fn advance(&mut self) -> bool {
        if !self.tail.advance() {
            return false;
        }
        if self.token_mut().text.is_ascii() {
            // fast track for ascii.
            self.token_mut().text.make_ascii_lowercase();
        } else {
            to_greek_lowercase_unicode(&mut self.tail.token_mut().text, &mut self.buffer);
            mem::swap(&mut self.tail.token_mut().text, &mut self.buffer);
        }
        true
    }

    fn token(&self) -> &Token {
        self.tail.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.tail.token_mut()
    }
}
