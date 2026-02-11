use rand::rngs::SmallRng;
use rand::Rng;

use crate::engine::filter::CharFilter;
use crate::generator::cache::{DiskCache, fetch_url};
use crate::generator::TextGenerator;

const PASSAGES: &[&str] = &[
    // Classic literature & speeches
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
    "the rain fell steadily on the roof making a soft drumming sound that filled the room and made everything feel calm",
    "she opened the door and stepped outside into the cool morning air breathing deeply as the first light of dawn appeared",
    "he picked up the book and began to read turning the pages slowly as the story drew him deeper and deeper into its world",
    "the stars shone brightly in the clear night sky and the moon cast a silver light over the sleeping town below",
    "they gathered around the fire telling stories and laughing while the wind howled outside and the snow piled up against the door",
    // Pride and Prejudice
    "it is a truth universally acknowledged that a single man in possession of a good fortune must be in want of a wife",
    "there is a stubbornness about me that never can bear to be frightened at the will of others my courage always rises at every attempt to intimidate me",
    "i could easily forgive his pride if he had not mortified mine but vanity not love has been my folly",
    // Alice in Wonderland
    "alice was beginning to get very tired of sitting by her sister on the bank and of having nothing to do",
    "who in the world am i that is the great puzzle she said as she looked around the strange room with wonder",
    "but i dont want to go among mad people alice remarked oh you cant help that said the cat were all mad here",
    // Great Gatsby
    "in my younger and more vulnerable years my father gave me some advice that i have been turning over in my mind ever since",
    "so we beat on boats against the current borne back ceaselessly into the past dreaming of that green light",
    // Sherlock Holmes
    "when you have eliminated the impossible whatever remains however improbable must be the truth my dear watson",
    "the world is full of obvious things which nobody by any chance ever observes but which are perfectly visible",
    // Moby Dick
    "call me ishmael some years ago having little or no money in my purse and nothing particular to interest me on shore",
    "it is not down on any map because true places never are and the voyage was long and the sea was deep",
    // 1984
    "it was a bright cold day in april and the clocks were striking thirteen winston smith his chin nuzzled into his breast",
    "who controls the past controls the future and who controls the present controls the past said the voice from the screen",
    // Walden
    "i went to the woods because i wished to live deliberately to front only the essential facts of life",
    "the mass of men lead lives of quiet desperation and go to the grave with the song still in them",
    // Science & philosophy
    "the only way to do great work is to love what you do and if you have not found it yet keep looking and do not settle",
    "imagination is more important than knowledge for while knowledge defines all we currently know imagination points to what we might discover",
    "the important thing is not to stop questioning for curiosity has its own reason for existing in this wonderful universe",
    "we are all in the gutter but some of us are looking at the stars and dreaming of worlds beyond our own",
    "the greatest glory in living lies not in never falling but in rising every time we fall and trying once more",
    // Nature & observation
    "the autumn wind scattered golden leaves across the garden as the last rays of sunlight painted the clouds in shades of orange and pink",
    "deep in the forest where the ancient trees stood tall and silent a small stream wound its way through moss covered stones",
    "the ocean stretched endlessly before them its surface catching the light of the setting sun in a thousand shimmering reflections",
    "morning mist hung low over the meadow as the first birds began their chorus and dew drops sparkled like diamonds on every blade of grass",
    "the mountain peak stood above the clouds its snow covered summit glowing pink and gold in the light of the early morning sun",
    // Everyday wisdom
    "the best time to plant a tree was twenty years ago and the second best time is now so do not wait any longer to begin",
    "a journey of a thousand miles begins with a single step and every great achievement started with the decision to try",
    "the more that you read the more things you will know and the more that you learn the more places you will go",
    "in three words i can sum up everything i have learned about life it goes on and so must we with hope",
    "happiness is not something ready made it comes from your own actions and your choices shape the life you live",
    "do not go where the path may lead but go instead where there is no path and leave a trail for others to follow",
    "success is not final failure is not fatal it is the courage to continue that counts in the end",
    "be yourself because everyone else is already taken and the world needs what only you can bring to it",
    "life is what happens when you are busy making other plans so enjoy the journey along the way",
    "the secret of getting ahead is getting started and the secret of getting started is breaking your tasks into small steps",
];

/// Gutenberg book IDs for popular public domain works
const GUTENBERG_IDS: &[(u32, &str)] = &[
    (1342, "pride_and_prejudice"),
    (11, "alice_in_wonderland"),
    (1661, "sherlock_holmes"),
    (84, "frankenstein"),
    (1952, "yellow_wallpaper"),
    (2701, "moby_dick"),
    (74, "tom_sawyer"),
    (345, "dracula"),
    (1232, "prince"),
    (76, "huckleberry_finn"),
    (5200, "metamorphosis"),
    (2542, "aesop_fables"),
    (174, "dorian_gray"),
    (98, "tale_two_cities"),
    (1080, "modest_proposal"),
    (219, "heart_of_darkness"),
    (4300, "ulysses"),
    (28054, "brothers_karamazov"),
    (2554, "crime_and_punishment"),
    (55, "oz"),
];

pub struct PassageGenerator {
    current_idx: usize,
    fetched_passages: Vec<String>,
    rng: SmallRng,
}

impl PassageGenerator {
    pub fn new(rng: SmallRng) -> Self {
        let mut generator = Self {
            current_idx: 0,
            fetched_passages: Vec::new(),
            rng,
        };
        generator.load_cached_passages();
        generator
    }

    fn load_cached_passages(&mut self) {
        if let Some(cache) = DiskCache::new("passages") {
            for &(_, name) in GUTENBERG_IDS {
                if let Some(content) = cache.get(name) {
                    let paragraphs = extract_paragraphs(&content);
                    self.fetched_passages.extend(paragraphs);
                }
            }
        }
    }

    fn try_fetch_gutenberg(&mut self) {
        let cache = match DiskCache::new("passages") {
            Some(c) => c,
            None => return,
        };

        // Pick a random book that we haven't cached yet
        let uncached: Vec<(u32, &str)> = GUTENBERG_IDS
            .iter()
            .filter(|(_, name)| cache.get(name).is_none())
            .copied()
            .collect();

        if uncached.is_empty() {
            return;
        }

        let idx = self.rng.gen_range(0..uncached.len());
        let (book_id, name) = uncached[idx];
        let url = format!("https://www.gutenberg.org/cache/epub/{book_id}/pg{book_id}.txt");

        if let Some(content) = fetch_url(&url) {
            cache.put(name, &content);
            let paragraphs = extract_paragraphs(&content);
            self.fetched_passages.extend(paragraphs);
        }
    }
}

impl TextGenerator for PassageGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused: Option<char>,
        _word_count: usize,
    ) -> String {
        // Try to fetch a new Gutenberg book in the background (first few calls)
        if self.fetched_passages.len() < 50 && self.current_idx < 3 {
            self.try_fetch_gutenberg();
        }

        let total_passages = PASSAGES.len() + self.fetched_passages.len();

        if total_passages == 0 {
            self.current_idx += 1;
            return PASSAGES[0].to_string();
        }

        // Mix embedded and fetched passages
        let idx = self.current_idx % total_passages;
        self.current_idx += 1;

        if idx < PASSAGES.len() {
            PASSAGES[idx].to_string()
        } else {
            let fetched_idx = idx - PASSAGES.len();
            self.fetched_passages[fetched_idx % self.fetched_passages.len()].clone()
        }
    }
}

/// Extract readable paragraphs from Gutenberg text, skipping header/footer
fn extract_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();

    // Find the start of actual content (after Gutenberg header)
    let start_markers = ["*** START OF", "***START OF"];
    let end_markers = ["*** END OF", "***END OF"];

    let content_start = start_markers
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .map(|pos| {
            // Find the end of the header line
            text[pos..].find('\n').map(|nl| pos + nl + 1).unwrap_or(pos)
        })
        .unwrap_or(0);

    let content_end = end_markers
        .iter()
        .filter_map(|marker| text.find(marker))
        .min()
        .unwrap_or(text.len());

    let content = &text[content_start..content_end];

    // Split into paragraphs (double newline separated)
    for para in content.split("\r\n\r\n").chain(content.split("\n\n")) {
        let cleaned: String = para
            .lines()
            .map(|l| l.trim())
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace() || c.is_ascii_punctuation())
            .collect::<String>()
            .to_lowercase();

        let word_count = cleaned.split_whitespace().count();
        if word_count >= 15 && word_count <= 60 {
            // Keep only the alpha/space portions for typing
            let typing_text: String = cleaned
                .chars()
                .filter(|c| c.is_ascii_lowercase() || *c == ' ')
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");

            if typing_text.split_whitespace().count() >= 10 {
                paragraphs.push(typing_text);
            }
        }
    }

    // Take at most 100 paragraphs per book
    paragraphs.truncate(100);
    paragraphs
}
