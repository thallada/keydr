use std::fs;
use std::path::PathBuf;

use rand::Rng;
use rand::rngs::SmallRng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;
use crate::generator::cache::fetch_url_bytes_with_progress;

const PASSAGES: &[&str] = &[
    "the quick brown fox jumps over the lazy dog and then runs across the field while the sun sets behind the distant hills",
    "it was the best of times it was the worst of times it was the age of wisdom it was the age of foolishness",
    "in the beginning there was nothing but darkness and then the light appeared slowly spreading across the vast empty space",
    "she walked along the narrow path through the forest listening to the birds singing in the trees above her head",
    "the old man sat on the bench watching the children play in the park while the autumn leaves fell softly around him",
    "there is nothing either good or bad but thinking makes it so for the mind is its own place and in itself can make a heaven of hell",
    "to be or not to be that is the question whether it is nobler in the mind to suffer the slings and arrows of outrageous fortune",
    "all that glitters is not gold and not all those who wander are lost for the old that is strong does not wither",
    "the river flowed quietly through the green valley and the mountains rose high on either side covered with trees and snow",
    "a long time ago in a land far away there lived a wise king who ruled his people with kindness and justice",
];

pub struct GutenbergBook {
    pub id: u32,
    pub key: &'static str,
    pub title: &'static str,
}

pub const GUTENBERG_BOOKS: &[GutenbergBook] = &[
    GutenbergBook {
        id: 1342,
        key: "pride_prejudice",
        title: "Pride and Prejudice",
    },
    GutenbergBook {
        id: 11,
        key: "alice_wonderland",
        title: "Alice in Wonderland",
    },
    GutenbergBook {
        id: 1661,
        key: "sherlock_holmes",
        title: "Sherlock Holmes",
    },
    GutenbergBook {
        id: 84,
        key: "frankenstein",
        title: "Frankenstein",
    },
    GutenbergBook {
        id: 2701,
        key: "moby_dick",
        title: "Moby Dick",
    },
    GutenbergBook {
        id: 98,
        key: "tale_two_cities",
        title: "A Tale of Two Cities",
    },
    GutenbergBook {
        id: 2554,
        key: "crime_punishment",
        title: "Crime and Punishment",
    },
];

pub fn passage_options() -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("all", "All (Built-in + all books)".to_string()),
        ("builtin", "Built-in passages only".to_string()),
    ];
    for book in GUTENBERG_BOOKS {
        out.push((book.key, format!("Book: {}", book.title)));
    }
    out
}

pub fn is_valid_passage_book(book: &str) -> bool {
    book == "all" || book == "builtin" || GUTENBERG_BOOKS.iter().any(|b| b.key == book)
}

pub fn uncached_books(cache_dir: &str) -> Vec<&'static GutenbergBook> {
    GUTENBERG_BOOKS
        .iter()
        .filter(|book| !cache_file(cache_dir, book.key).exists())
        .collect()
}

pub fn book_by_key(key: &str) -> Option<&'static GutenbergBook> {
    GUTENBERG_BOOKS.iter().find(|b| b.key == key)
}

pub fn is_book_cached(cache_dir: &str, key: &str) -> bool {
    cache_file(cache_dir, key).exists()
}

pub fn download_book_to_cache_with_progress<F>(
    cache_dir: &str,
    book: &GutenbergBook,
    mut on_progress: F,
) -> bool
where
    F: FnMut(u64, Option<u64>),
{
    let _ = fs::create_dir_all(cache_dir);
    let url = format!(
        "https://www.gutenberg.org/cache/epub/{}/pg{}.txt",
        book.id, book.id
    );
    if let Some(bytes) = fetch_url_bytes_with_progress(&url, |downloaded, total| {
        on_progress(downloaded, total);
    }) {
        return fs::write(cache_file(cache_dir, book.key), bytes).is_ok();
    }
    false
}

fn cache_file(cache_dir: &str, key: &str) -> PathBuf {
    PathBuf::from(cache_dir).join(format!("{key}.txt"))
}

pub struct PassageGenerator {
    fetched_passages: Vec<(String, String)>,
    rng: SmallRng,
    selection: String,
    cache_dir: String,
    paragraph_limit: usize,
    _downloads_enabled: bool,
    last_source: String,
}

impl PassageGenerator {
    pub fn new(
        rng: SmallRng,
        selection: &str,
        cache_dir: &str,
        paragraph_limit: usize,
        downloads_enabled: bool,
    ) -> Self {
        let selected = if is_valid_passage_book(selection) {
            selection.to_string()
        } else {
            "all".to_string()
        };
        let mut generator = Self {
            fetched_passages: Vec::new(),
            rng,
            selection: selected,
            cache_dir: cache_dir.to_string(),
            paragraph_limit,
            _downloads_enabled: downloads_enabled,
            last_source: "Built-in passage library".to_string(),
        };
        generator.load_cached_passages();
        generator
    }

    pub fn last_source(&self) -> &str {
        &self.last_source
    }

    fn load_cached_passages(&mut self) {
        let _ = fs::create_dir_all(&self.cache_dir);
        for book in relevant_books(&self.selection) {
            if let Ok(content) = fs::read_to_string(cache_file(&self.cache_dir, book.key)) {
                for para in extract_paragraphs(&content, self.paragraph_limit) {
                    self.fetched_passages.push((para, book.title.to_string()));
                }
            }
        }
    }
}

impl TextGenerator for PassageGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused: Option<char>,
        word_count: usize,
    ) -> String {
        let use_builtin = self.selection == "all" || self.selection == "builtin";
        let total = (if use_builtin { PASSAGES.len() } else { 0 }) + self.fetched_passages.len();

        if total == 0 {
            let idx = self.rng.gen_range(0..PASSAGES.len());
            self.last_source = "Built-in passage library (fallback)".to_string();
            return normalize_keyboard_text(PASSAGES[idx]);
        }
        let idx = self.rng.gen_range(0..total);
        if use_builtin && idx < PASSAGES.len() {
            self.last_source = "Built-in passage library".to_string();
            return fit_to_word_target(&normalize_keyboard_text(PASSAGES[idx]), word_count);
        }

        let fetched_idx = if use_builtin {
            idx - PASSAGES.len()
        } else {
            idx
        };
        let (text, source) = &self.fetched_passages[fetched_idx % self.fetched_passages.len()];
        self.last_source = format!("Project Gutenberg ({source})");
        fit_to_word_target(text, word_count)
    }
}

fn relevant_books(selection: &str) -> Vec<&'static GutenbergBook> {
    if selection == "all" || selection == "builtin" {
        return GUTENBERG_BOOKS.iter().collect();
    }
    GUTENBERG_BOOKS
        .iter()
        .filter(|book| book.key == selection)
        .collect()
}

fn extract_paragraphs(text: &str, limit: usize) -> Vec<String> {
    const MIN_WORDS: usize = 12;
    const MAX_WORDS: usize = 42;

    let mut paragraphs = Vec::new();
    let start_markers = ["*** START OF", "***START OF"];
    let end_markers = ["*** END OF", "***END OF"];

    let content_start = start_markers
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .map(|pos| text[pos..].find('\n').map(|nl| pos + nl + 1).unwrap_or(pos))
        .unwrap_or(0);
    let content_end = end_markers
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());
    let normalized = normalize_keyboard_text(
        &text[content_start..content_end]
            .replace("\r\n", "\n")
            .replace('\r', "\n"),
    );

    for para in normalized.split("\n\n") {
        let raw = para.trim_matches('\n');
        if raw.is_empty() {
            continue;
        }

        let has_letters = raw.chars().any(|c| c.is_alphabetic());
        let has_only_supported_controls = raw
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\t');
        let word_count = raw.split_whitespace().count();
        if !has_letters || !has_only_supported_controls || word_count < MIN_WORDS {
            continue;
        }

        if word_count <= MAX_WORDS {
            paragraphs.push(raw.to_string());
        } else {
            paragraphs.extend(split_into_sentence_chunks(raw, MIN_WORDS, MAX_WORDS));
        }
    }

    if limit > 0 {
        paragraphs.truncate(limit);
    }
    paragraphs
}

fn normalize_keyboard_text(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{201B}' | '\u{2032}' => '\'',
            '\u{201C}' | '\u{201D}' | '\u{201F}' | '\u{2033}' => '"',
            '\u{2013}' | '\u{2014}' | '\u{2015}' | '\u{2212}' => '-',
            '\u{2026}' => '.',
            '\u{00A0}' => ' ',
            _ => c,
        })
        .collect()
}

fn fit_to_word_target(text: &str, target_words: usize) -> String {
    if target_words == 0 {
        return text.to_string();
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return text.to_string();
    }
    // Keep passages slightly longer than target at most.
    let keep = target_words.saturating_mul(6) / 5;
    if words.len() <= keep.max(1) {
        return text.to_string();
    }
    words[..keep.max(1)].join(" ")
}

fn split_into_sentence_chunks(text: &str, min_words: usize, max_words: usize) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut start = 0usize;
    for (idx, ch) in text.char_indices() {
        if matches!(ch, '.' | '!' | '?') {
            let end = idx + ch.len_utf8();
            let s = text[start..end].trim();
            if !s.is_empty() {
                sentences.push(s.to_string());
            }
            start = end;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        sentences.push(tail.to_string());
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_words = 0usize;

    for sentence in sentences {
        let w = sentence.split_whitespace().count();
        if w == 0 {
            continue;
        }
        if w > max_words {
            if current_words >= min_words {
                chunks.push(current.trim().to_string());
            }
            current.clear();
            current_words = 0;
            chunks.extend(split_long_by_words(&sentence, min_words, max_words));
            continue;
        }

        if current_words == 0 {
            current = sentence;
            current_words = w;
        } else if current_words + w <= max_words {
            current.push(' ');
            current.push_str(&sentence);
            current_words += w;
        } else {
            if current_words >= min_words {
                chunks.push(current.trim().to_string());
            }
            current = sentence;
            current_words = w;
        }
    }

    if current_words >= min_words {
        chunks.push(current.trim().to_string());
    }

    chunks
}

fn split_long_by_words(sentence: &str, min_words: usize, max_words: usize) -> Vec<String> {
    let words: Vec<&str> = sentence.split_whitespace().collect();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < words.len() {
        let end = (i + max_words).min(words.len());
        let chunk = words[i..end].join(" ");
        if chunk.split_whitespace().count() >= min_words {
            out.push(chunk);
        } else if let Some(last) = out.last_mut() {
            last.push(' ');
            last.push_str(&chunk);
        }
        i = end;
    }
    out
}
