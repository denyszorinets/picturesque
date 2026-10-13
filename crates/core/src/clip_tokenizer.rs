//! CLIP's text tokenizer: lowercased byte-level BPE with 77-token
//! sequences, as in OpenAI's `simple_tokenizer.py`.
//!
//! The vocabulary is rebuilt from the merge list alone, like the
//! original: the 256 byte symbols, the same with an end-of-word mark,
//! one entry per merge, then the start and end tokens.

use std::collections::HashMap;

/// Tokens per sequence.
pub const CONTEXT_LENGTH: usize = 77;
const END_OF_WORD: &str = "</w>";

/// Encodes text for the CLIP text encoder.
#[derive(Debug, Clone)]
pub struct ClipTokenizer {
    /// Byte → the printable character standing for it.
    byte_chars: [char; 256],
    ranks: HashMap<(String, String), usize>,
    vocab: HashMap<String, i32>,
    start: i32,
    end: i32,
}

/// OpenAI's `bytes_to_unicode`: printable bytes stand for themselves, the
/// rest map to characters from U+0100 on.
fn byte_chars() -> [char; 256] {
    let printable =
        |b: u32| (33..=126).contains(&b) || (161..=172).contains(&b) || (174..=255).contains(&b);
    let mut chars = ['\0'; 256];
    let mut next = 256;
    for b in 0..256u32 {
        chars[b as usize] = if printable(b) {
            char::from_u32(b).expect("latin-1")
        } else {
            next += 1;
            char::from_u32(next - 1).expect("valid code point")
        };
    }
    chars
}

impl ClipTokenizer {
    /// From the contents of `merges.txt` (a `#version` line, then one
    /// space-separated pair per line). CLIP uses the first 48,894 merges.
    pub fn from_merges(text: &str) -> Option<Self> {
        let byte_chars = byte_chars();
        let merges: Vec<(String, String)> = text
            .lines()
            .filter(|l| !l.starts_with("#version") && !l.trim().is_empty())
            .take(49_152 - 256 - 2)
            .map(|l| {
                let (a, b) = l.split_once(' ')?;
                Some((a.to_owned(), b.to_owned()))
            })
            .collect::<Option<_>>()?;
        if merges.is_empty() {
            return None;
        }
        // In `bytes_to_unicode` order: printable bytes, then the others.
        let mut symbols: Vec<char> = byte_chars.to_vec();
        symbols.sort_by_key(|c| u32::from(*c) >= 256);
        let mut entries: Vec<String> = symbols.iter().map(char::to_string).collect();
        entries.extend(symbols.iter().map(|c| format!("{c}{END_OF_WORD}")));
        entries.extend(merges.iter().map(|(a, b)| format!("{a}{b}")));
        entries.push("<|startoftext|>".into());
        entries.push("<|endoftext|>".into());
        let vocab: HashMap<String, i32> = entries
            .into_iter()
            .enumerate()
            .map(|(i, e)| (e, i as i32))
            .collect();
        Some(Self {
            byte_chars,
            start: vocab["<|startoftext|>"],
            end: vocab["<|endoftext|>"],
            ranks: merges
                .into_iter()
                .enumerate()
                .map(|(i, m)| (m, i))
                .collect(),
            vocab,
        })
    }

    /// Token ids of `text`: start token, the text's tokens, end token,
    /// zero padding; cut to [`CONTEXT_LENGTH`] keeping the end token.
    pub fn encode(&self, text: &str) -> Vec<i32> {
        let cleaned = text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let mut ids = vec![self.start];
        for word in split_words(&cleaned) {
            let symbols: String = word.bytes().map(|b| self.byte_chars[b as usize]).collect();
            ids.extend(self.bpe(&symbols).iter().filter_map(|t| self.vocab.get(t)));
        }
        ids.truncate(CONTEXT_LENGTH - 1);
        ids.push(self.end);
        ids.resize(CONTEXT_LENGTH, 0);
        ids
    }

    /// Merges the symbols of one word, lowest-ranked pair first.
    fn bpe(&self, word: &str) -> Vec<String> {
        let mut parts: Vec<String> = word.chars().map(String::from).collect();
        if let Some(last) = parts.last_mut() {
            last.push_str(END_OF_WORD);
        }
        while parts.len() > 1 {
            let best = parts
                .windows(2)
                .enumerate()
                .filter_map(|(i, pair)| {
                    self.ranks
                        .get(&(pair[0].clone(), pair[1].clone()))
                        .map(|rank| (*rank, i))
                })
                .min();
            let Some((_, i)) = best else { break };
            let (a, b) = (parts[i].clone(), parts[i + 1].clone());
            // Merge every occurrence of the pair, left to right.
            let mut merged = Vec::with_capacity(parts.len());
            let mut j = 0;
            while j < parts.len() {
                if j + 1 < parts.len() && parts[j] == a && parts[j + 1] == b {
                    merged.push(format!("{a}{b}"));
                    j += 2;
                } else {
                    merged.push(parts[j].clone());
                    j += 1;
                }
            }
            parts = merged;
        }
        parts
    }
}

/// Splits like CLIP's pattern: contractions (`'s`, `'t`, `'re`, `'ve`,
/// `'m`, `'ll`, `'d`), runs of letters, single digits, and runs of other
/// non-space characters.
fn split_words(text: &str) -> Vec<&str> {
    const CONTRACTIONS: [&str; 7] = ["'s", "'t", "'re", "'ve", "'m", "'ll", "'d"];
    let other = |c: char| !c.is_whitespace() && !c.is_alphabetic() && !c.is_numeric();
    let mut words = Vec::new();
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        let len = if let Some(k) = CONTRACTIONS.iter().find(|k| rest.starts_with(**k)) {
            k.len()
        } else if c.is_alphabetic() {
            rest.find(|c: char| !c.is_alphabetic())
                .unwrap_or(rest.len())
        } else if c.is_numeric() {
            c.len_utf8()
        } else if c.is_whitespace() {
            rest = &rest[c.len_utf8()..];
            continue;
        } else {
            rest.find(|c: char| !other(c)).unwrap_or(rest.len())
        };
        words.push(&rest[..len]);
        rest = &rest[len..];
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Merges that build "dog" and "dogs": d o → do, do g</w> → dog</w>,
    /// do g → dog, dog s</w> → dogs</w>.
    const MERGES: &str = "#version: 0.2\nd o\ndo g</w>\ndo g\ndog s</w>\n";

    fn id(t: &ClipTokenizer, token: &str) -> i32 {
        t.vocab[token]
    }

    #[test]
    fn rebuilds_the_vocabulary_from_merges() {
        let t = ClipTokenizer::from_merges(MERGES).unwrap();
        // 256 bytes, 256 end-of-word bytes, 4 merges, 2 special tokens.
        assert_eq!(t.vocab.len(), 518);
        assert_eq!(id(&t, "!"), 0);
        assert_eq!(id(&t, "a"), 64);
        assert_eq!(id(&t, "a</w>"), 256 + 64);
        // Non-printable bytes come after the 188 printable ones.
        assert_eq!(id(&t, "Ā"), 188);
        assert_eq!(id(&t, "do"), 512);
        assert_eq!((t.start, t.end), (516, 517));
        // Space is not printable: it stands as U+0120.
        assert_eq!(t.byte_chars[b' ' as usize], 'Ġ');
        assert!(ClipTokenizer::from_merges("#version: 0.2\n").is_none());
    }

    #[test]
    fn encodes_words_with_merges() {
        let t = ClipTokenizer::from_merges(MERGES).unwrap();
        let ids = t.encode("  Dogs DOG  dot ");
        assert_eq!(ids.len(), CONTEXT_LENGTH);
        assert_eq!(
            &ids[..7],
            &[
                t.start,
                id(&t, "dogs</w>"),
                id(&t, "dog</w>"),
                id(&t, "do"),
                id(&t, "t</w>"),
                t.end,
                0
            ]
        );
    }

    #[test]
    fn splits_like_clips_pattern() {
        assert_eq!(
            split_words("it's 2024!! café, l'été"),
            [
                "it", "'s", "2", "0", "2", "4", "!!", "café", ",", "l", "'", "été"
            ]
        );
        assert_eq!(split_words("we'll"), ["we", "'ll"]);
        assert!(split_words("   ").is_empty());
    }

    #[test]
    fn long_text_keeps_the_end_token() {
        let t = ClipTokenizer::from_merges(MERGES).unwrap();
        let ids = t.encode(&"a ".repeat(200));
        assert_eq!(ids.len(), CONTEXT_LENGTH);
        assert_eq!(ids[CONTEXT_LENGTH - 1], t.end);
        assert!(
            ids[1..CONTEXT_LENGTH - 1]
                .iter()
                .all(|&i| i == id(&t, "a</w>"))
        );
    }

    /// Against the real merges: known ids from OpenAI's tokenizer.
    #[test]
    #[ignore = "needs CLIP's merges.txt (PICTURESQUE_CLIP_MERGES)"]
    fn matches_openai_ids() {
        let path = std::env::var("PICTURESQUE_CLIP_MERGES").expect("PICTURESQUE_CLIP_MERGES");
        let t = ClipTokenizer::from_merges(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(t.vocab.len(), 49_408);
        assert_eq!(
            &t.encode("a photo of a dog")[..7],
            &[49406, 320, 1125, 539, 320, 1929, 49407]
        );
    }
}
