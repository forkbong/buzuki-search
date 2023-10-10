use lazy_static::lazy_static;
use regex::Regex;

/// Return greek string in greeklish.
pub fn to_greeklish(string: &str) -> String {
    // We always replace spaces with underscores. We don't need that for searching, only for
    // storing the slug, but it also works for searching so we leave it like that for simplicity.
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^a-z_\n]").unwrap();
    }
    string
        .to_lowercase()
        .replace('ψ', "ps")
        .replace('ξ', "ks")
        .replace('θ', "th")
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
