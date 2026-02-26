use criterion::{Criterion, black_box, criterion_group, criterion_main};

use keydr::engine::key_stats::KeyStatsStore;
use keydr::engine::ngram_stats::{
    BigramKey, BigramStatsStore, TrigramStatsStore, extract_ngram_events,
};
use keydr::session::result::KeyTime;

fn make_keystrokes(count: usize) -> Vec<KeyTime> {
    let chars = ['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'];
    (0..count)
        .map(|i| KeyTime {
            key: chars[i % chars.len()],
            time_ms: 200.0 + (i % 50) as f64,
            correct: i % 7 != 0, // ~14% error rate
        })
        .collect()
}

fn bench_extraction(c: &mut Criterion) {
    let keystrokes = make_keystrokes(500);

    c.bench_function("extract_ngram_events (500 keystrokes)", |b| {
        b.iter(|| extract_ngram_events(black_box(&keystrokes), 800.0))
    });
}

fn bench_update(c: &mut Criterion) {
    let keystrokes = make_keystrokes(500);
    let (bigram_events, _) = extract_ngram_events(&keystrokes, 800.0);

    c.bench_function("bigram_stats update (400 events)", |b| {
        b.iter(|| {
            let mut store = BigramStatsStore::default();
            for ev in bigram_events.iter().take(400) {
                store.update(
                    black_box(ev.key.clone()),
                    black_box(ev.total_time_ms),
                    black_box(ev.correct),
                    black_box(ev.has_hesitation),
                    0,
                );
            }
            store
        })
    });
}

fn bench_focus_selection(c: &mut Criterion) {
    // Use a-z + A-Z + 0-9 = 62 chars for up to 3844 unique bigrams
    let all_chars: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();

    let mut bigram_stats = BigramStatsStore::default();
    let mut char_stats = KeyStatsStore::default();

    for &ch in &all_chars {
        let stat = char_stats.stats.entry(ch).or_default();
        stat.confidence = 0.8;
        stat.filtered_time_ms = 430.0;
        stat.sample_count = 50;
        stat.total_count = 50;
        stat.error_count = 3;
    }

    let mut count: usize = 0;
    for &a in &all_chars {
        for &b in &all_chars {
            if bigram_stats.stats.len() >= 3000 {
                break;
            }
            let key = BigramKey([a, b]);
            let stat = bigram_stats.stats.entry(key).or_default();
            stat.confidence = 0.5 + (count % 50) as f64 * 0.01;
            stat.sample_count = 25 + count % 30;
            stat.error_count = 5 + count % 10;
            stat.redundancy_streak = if count % 3 == 0 { 3 } else { 1 };
            count += 1;
        }
    }
    assert_eq!(bigram_stats.stats.len(), 3000);

    let unlocked: Vec<char> = all_chars;

    c.bench_function("weakest_bigram (3K entries)", |b| {
        b.iter(|| bigram_stats.weakest_bigram(black_box(&char_stats), black_box(&unlocked)))
    });
}

fn bench_history_replay(c: &mut Criterion) {
    // Build 500 drills of ~300 keystrokes each
    let drills: Vec<Vec<KeyTime>> = (0..500).map(|_| make_keystrokes(300)).collect();

    c.bench_function("history replay (500 drills x 300 keystrokes)", |b| {
        b.iter(|| {
            let mut bigram_stats = BigramStatsStore::default();
            let mut trigram_stats = TrigramStatsStore::default();
            let mut key_stats = KeyStatsStore::default();

            for (drill_idx, keystrokes) in drills.iter().enumerate() {
                let (bigram_events, trigram_events) = extract_ngram_events(keystrokes, 800.0);

                for kt in keystrokes {
                    if kt.correct {
                        let stat = key_stats.stats.entry(kt.key).or_default();
                        stat.total_count += 1;
                    } else {
                        key_stats.update_key_error(kt.key);
                    }
                }

                for ev in &bigram_events {
                    bigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_idx as u32,
                    );
                }
                for ev in &trigram_events {
                    trigram_stats.update(
                        ev.key.clone(),
                        ev.total_time_ms,
                        ev.correct,
                        ev.has_hesitation,
                        drill_idx as u32,
                    );
                }
            }

            (bigram_stats, trigram_stats, key_stats)
        })
    });
}

criterion_group!(
    benches,
    bench_extraction,
    bench_update,
    bench_focus_selection,
    bench_history_replay,
);
criterion_main!(benches);
