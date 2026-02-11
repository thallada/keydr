use rand::rngs::SmallRng;
use rand::Rng;

use crate::engine::filter::CharFilter;
use crate::generator::cache::{DiskCache, fetch_url};
use crate::generator::TextGenerator;

pub struct CodeSyntaxGenerator {
    rng: SmallRng,
    language: String,
    fetched_snippets: Vec<String>,
}

impl CodeSyntaxGenerator {
    pub fn new(rng: SmallRng, language: &str) -> Self {
        let mut generator = Self {
            rng,
            language: language.to_string(),
            fetched_snippets: Vec::new(),
        };
        generator.load_cached_snippets();
        generator
    }

    fn load_cached_snippets(&mut self) {
        if let Some(cache) = DiskCache::new("code_cache") {
            let key = format!("{}_snippets", self.language);
            if let Some(content) = cache.get(&key) {
                self.fetched_snippets = content
                    .split("\n---SNIPPET---\n")
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .collect();
            }
        }
    }

    fn try_fetch_code(&mut self) {
        let urls = match self.language.as_str() {
            "rust" => vec![
                "https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/sync/mutex.rs",
                "https://raw.githubusercontent.com/serde-rs/serde/master/serde/src/ser/mod.rs",
            ],
            "python" => vec![
                "https://raw.githubusercontent.com/python/cpython/main/Lib/json/encoder.py",
                "https://raw.githubusercontent.com/python/cpython/main/Lib/pathlib/__init__.py",
            ],
            "javascript" | "js" => vec![
                "https://raw.githubusercontent.com/lodash/lodash/main/src/chunk.ts",
                "https://raw.githubusercontent.com/expressjs/express/master/lib/router/index.js",
            ],
            "go" => vec![
                "https://raw.githubusercontent.com/golang/go/master/src/fmt/print.go",
            ],
            _ => vec![],
        };

        let cache = match DiskCache::new("code_cache") {
            Some(c) => c,
            None => return,
        };

        let key = format!("{}_snippets", self.language);
        if cache.get(&key).is_some() {
            return;
        }

        let mut all_snippets = Vec::new();
        for url in urls {
            if let Some(content) = fetch_url(url) {
                let snippets = extract_code_snippets(&content);
                all_snippets.extend(snippets);
            }
        }

        if !all_snippets.is_empty() {
            let combined = all_snippets.join("\n---SNIPPET---\n");
            cache.put(&key, &combined);
            self.fetched_snippets = all_snippets;
        }
    }

    fn rust_snippets() -> Vec<&'static str> {
        vec![
            "fn main() { println!(\"hello\"); }",
            "let mut x = 0; x += 1;",
            "for i in 0..10 { println!(\"{}\", i); }",
            "if x > 0 { return true; }",
            "match val { Some(x) => x, None => 0 }",
            "struct Point { x: f64, y: f64 }",
            "impl Point { fn new(x: f64, y: f64) -> Self { Self { x, y } } }",
            "let v: Vec<i32> = vec![1, 2, 3];",
            "fn add(a: i32, b: i32) -> i32 { a + b }",
            "use std::collections::HashMap;",
            "pub fn process(input: &str) -> Result<String, Error> { Ok(input.to_string()) }",
            "let result = items.iter().filter(|x| x > &0).map(|x| x * 2).collect::<Vec<_>>();",
            "enum Color { Red, Green, Blue }",
            "trait Display { fn show(&self) -> String; }",
            "while let Some(item) = stack.pop() { process(item); }",
            "#[derive(Debug, Clone)] struct Config { name: String, value: i32 }",
            "let handle = std::thread::spawn(|| { println!(\"thread\"); });",
            "let mut map = HashMap::new(); map.insert(\"key\", 42);",
            "fn factorial(n: u64) -> u64 { if n <= 1 { 1 } else { n * factorial(n - 1) } }",
            "impl Iterator for Counter { type Item = u32; fn next(&mut self) -> Option<Self::Item> { None } }",
            "async fn fetch(url: &str) -> Result<String> { let body = reqwest::get(url).await?.text().await?; Ok(body) }",
            "let closure = |x: i32, y: i32| -> i32 { x + y };",
            "mod tests { use super::*; #[test] fn it_works() { assert_eq!(2 + 2, 4); } }",
            "pub struct Builder { name: Option<String> } impl Builder { pub fn name(mut self, n: &str) -> Self { self.name = Some(n.into()); self } }",
            "use std::sync::{Arc, Mutex}; let data = Arc::new(Mutex::new(vec![1, 2, 3]));",
            "if let Ok(value) = \"42\".parse::<i32>() { println!(\"parsed: {}\", value); }",
            "fn longest<'a>(x: &'a str, y: &'a str) -> &'a str { if x.len() > y.len() { x } else { y } }",
            "type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;",
            "macro_rules! vec_of_strings { ($($x:expr),*) => { vec![$($x.to_string()),*] }; }",
            "let (tx, rx) = std::sync::mpsc::channel(); tx.send(42).unwrap();",
        ]
    }

    fn python_snippets() -> Vec<&'static str> {
        vec![
            "def main(): print(\"hello\")",
            "for i in range(10): print(i)",
            "if x > 0: return True",
            "class Point: def __init__(self, x, y): self.x = x",
            "import os; path = os.path.join(\"a\", \"b\")",
            "result = [x * 2 for x in items if x > 0]",
            "with open(\"file.txt\") as f: data = f.read()",
            "def add(a: int, b: int) -> int: return a + b",
            "try: result = process(data) except ValueError as e: print(e)",
            "from collections import defaultdict",
            "lambda x: x * 2 + 1",
            "dict_comp = {k: v for k, v in pairs.items()}",
            "async def fetch(url): async with aiohttp.ClientSession() as session: return await session.get(url)",
            "def fibonacci(n): return n if n <= 1 else fibonacci(n-1) + fibonacci(n-2)",
            "@property def name(self): return self._name",
            "from dataclasses import dataclass; @dataclass class Config: name: str; value: int = 0",
            "yield from range(10)",
            "sorted(items, key=lambda x: x.name, reverse=True)",
            "from typing import Optional, List, Dict",
            "with contextlib.suppress(FileNotFoundError): os.remove(\"temp.txt\")",
            "class Meta(type): def __new__(cls, name, bases, attrs): return super().__new__(cls, name, bases, attrs)",
            "from functools import lru_cache; @lru_cache(maxsize=128) def expensive(n): return sum(range(n))",
            "from pathlib import Path; files = list(Path(\".\").glob(\"**/*.py\"))",
            "assert isinstance(result, dict), f\"Expected dict, got {type(result)}\"",
            "values = {*set_a, *set_b}; merged = {**dict_a, **dict_b}",
        ]
    }

    fn javascript_snippets() -> Vec<&'static str> {
        vec![
            "const x = 42; console.log(x);",
            "function add(a, b) { return a + b; }",
            "const arr = [1, 2, 3].map(x => x * 2);",
            "if (x > 0) { return true; }",
            "for (let i = 0; i < 10; i++) { console.log(i); }",
            "class Point { constructor(x, y) { this.x = x; this.y = y; } }",
            "const { name, age } = person;",
            "async function fetch(url) { const res = await get(url); return res.json(); }",
            "const obj = { ...defaults, ...overrides };",
            "try { parse(data); } catch (e) { console.error(e); }",
            "export default function handler(req, res) { res.send(\"ok\"); }",
            "const result = items.filter(x => x > 0).reduce((a, b) => a + b, 0);",
            "const promise = new Promise((resolve, reject) => { setTimeout(resolve, 1000); });",
            "const [first, ...rest] = array;",
            "class EventEmitter { constructor() { this.listeners = new Map(); } }",
            "const proxy = new Proxy(target, { get(obj, prop) { return obj[prop]; } });",
            "for await (const chunk of stream) { process(chunk); }",
            "const memoize = (fn) => { const cache = new Map(); return (...args) => cache.get(args) ?? fn(...args); };",
            "import { useState, useEffect } from 'react'; const [state, setState] = useState(null);",
            "const pipe = (...fns) => (x) => fns.reduce((v, f) => f(v), x);",
            "Object.entries(obj).forEach(([key, value]) => { console.log(key, value); });",
            "const debounce = (fn, ms) => { let timer; return (...args) => { clearTimeout(timer); timer = setTimeout(() => fn(...args), ms); }; };",
            "const observable = new Observable(subscriber => { subscriber.next(1); subscriber.complete(); });",
            "Symbol.iterator",
        ]
    }

    fn go_snippets() -> Vec<&'static str> {
        vec![
            "func main() { fmt.Println(\"hello\") }",
            "for i := 0; i < 10; i++ { fmt.Println(i) }",
            "if err != nil { return err }",
            "type Point struct { X float64; Y float64 }",
            "func add(a, b int) int { return a + b }",
            "import \"fmt\"",
            "result := make([]int, 0, 10)",
            "switch val { case 1: return \"one\" default: return \"other\" }",
            "go func() { ch <- result }()",
            "defer file.Close()",
            "type Reader interface { Read(p []byte) (n int, err error) }",
            "ctx, cancel := context.WithTimeout(context.Background(), time.Second)",
            "var wg sync.WaitGroup; wg.Add(1); go func() { defer wg.Done() }()",
            "func (p *Point) Distance() float64 { return math.Sqrt(p.X*p.X + p.Y*p.Y) }",
            "select { case msg := <-ch: process(msg) case <-time.After(time.Second): timeout() }",
            "json.NewEncoder(w).Encode(response)",
            "http.HandleFunc(\"/api\", func(w http.ResponseWriter, r *http.Request) { w.Write([]byte(\"ok\")) })",
            "func Map[T, U any](s []T, f func(T) U) []U { r := make([]U, len(s)); for i, v := range s { r[i] = f(v) }; return r }",
            "var once sync.Once; once.Do(func() { initialize() })",
            "buf := bytes.NewBuffer(nil); buf.WriteString(\"hello\")",
        ]
    }

    fn get_snippets(&self) -> Vec<&'static str> {
        match self.language.as_str() {
            "rust" => Self::rust_snippets(),
            "python" => Self::python_snippets(),
            "javascript" | "js" => Self::javascript_snippets(),
            "go" => Self::go_snippets(),
            _ => Self::rust_snippets(),
        }
    }
}

impl TextGenerator for CodeSyntaxGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused: Option<char>,
        word_count: usize,
    ) -> String {
        // Try to fetch from GitHub on first use
        if self.fetched_snippets.is_empty() {
            self.try_fetch_code();
        }

        let embedded = self.get_snippets();
        let mut result = Vec::new();
        let target_words = word_count;
        let mut current_words = 0;

        let total_available = embedded.len() + self.fetched_snippets.len();

        while current_words < target_words {
            let idx = self.rng.gen_range(0..total_available.max(1));

            let snippet = if idx < embedded.len() {
                embedded[idx]
            } else if !self.fetched_snippets.is_empty() {
                let f_idx = (idx - embedded.len()) % self.fetched_snippets.len();
                &self.fetched_snippets[f_idx]
            } else {
                embedded[idx % embedded.len()]
            };

            current_words += snippet.split_whitespace().count();
            result.push(snippet.to_string());
        }

        result.join(" ")
    }
}

/// Extract function-length snippets from raw source code
fn extract_code_snippets(source: &str) -> Vec<String> {
    let mut snippets = Vec::new();
    let lines: Vec<&str> = source.lines().collect();

    let mut i = 0;
    while i < lines.len() {
        // Look for function/method starts
        let line = lines[i].trim();
        let is_func_start = line.starts_with("fn ")
            || line.starts_with("pub fn ")
            || line.starts_with("def ")
            || line.starts_with("func ")
            || line.starts_with("function ")
            || line.starts_with("async fn ")
            || line.starts_with("pub async fn ");

        if is_func_start {
            let mut snippet_lines = Vec::new();
            let mut depth = 0i32;
            let mut j = i;

            while j < lines.len() && snippet_lines.len() < 30 {
                let l = lines[j];
                snippet_lines.push(l);

                depth += l.chars().filter(|&c| c == '{' || c == '(').count() as i32;
                depth -= l.chars().filter(|&c| c == '}' || c == ')').count() as i32;

                if depth <= 0 && j > i {
                    break;
                }
                j += 1;
            }

            if snippet_lines.len() >= 3 && snippet_lines.len() <= 30 {
                let snippet = snippet_lines.join(" ");
                // Normalize whitespace
                let normalized: String = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
                if normalized.len() >= 20 && normalized.len() <= 500 {
                    snippets.push(normalized);
                }
            }

            i = j + 1;
        } else {
            i += 1;
        }
    }

    snippets.truncate(50);
    snippets
}
