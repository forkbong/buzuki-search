use std::mem;

use tantivy::tokenizer::{Token, TokenFilter, TokenStream};

/// Token filter that lowercase terms.
#[derive(Clone)]
pub struct GreekLowerCaser;

impl<TailTokenStream> TokenFilter<TailTokenStream> for GreekLowerCaser
where
    TailTokenStream: TokenStream,
{
    type ResultTokenStream = GreekLowerCaserTokenStream<TailTokenStream>;

    fn transform(&self, token_stream: TailTokenStream) -> Self::ResultTokenStream {
        GreekLowerCaserTokenStream::wrap(token_stream)
    }
}

pub struct GreekLowerCaserTokenStream<TailTokenStream> {
    buffer: String,
    tail: TailTokenStream,
}

/// Writes a lowercased version of text into output.
fn to_greek_lowercase_unicode(text: &mut String, output: &mut String) {
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

impl<TailTokenStream> TokenStream for GreekLowerCaserTokenStream<TailTokenStream>
where
    TailTokenStream: TokenStream,
{
    fn token(&self) -> &Token {
        self.tail.token()
    }

    fn token_mut(&mut self) -> &mut Token {
        self.tail.token_mut()
    }

    fn advance(&mut self) -> bool {
        if self.tail.advance() {
            if self.token_mut().text.is_ascii() {
                // fast track for ascii.
                self.token_mut().text.make_ascii_lowercase();
            } else {
                to_greek_lowercase_unicode(&mut self.tail.token_mut().text, &mut self.buffer);

                mem::swap(&mut self.tail.token_mut().text, &mut self.buffer);
            }
            true
        } else {
            false
        }
    }
}

impl<TailTokenStream> GreekLowerCaserTokenStream<TailTokenStream>
where
    TailTokenStream: TokenStream,
{
    fn wrap(tail: TailTokenStream) -> GreekLowerCaserTokenStream<TailTokenStream> {
        GreekLowerCaserTokenStream {
            tail,
            buffer: String::with_capacity(100),
        }
    }
}
