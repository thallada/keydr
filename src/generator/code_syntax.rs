use rand::Rng;
use rand::rngs::SmallRng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;
use crate::generator::cache::{DiskCache, fetch_url};

pub struct CodeSyntaxGenerator {
    rng: SmallRng,
    language: String,
    fetched_snippets: Vec<String>,
    last_source: String,
}

impl CodeSyntaxGenerator {
    pub fn new(rng: SmallRng, language: &str) -> Self {
        let mut generator = Self {
            rng,
            language: language.to_string(),
            fetched_snippets: Vec::new(),
            last_source: "Built-in snippets".to_string(),
        };
        generator.load_cached_snippets();
        generator
    }

    pub fn last_source(&self) -> &str {
        &self.last_source
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
            "go" => vec!["https://raw.githubusercontent.com/golang/go/master/src/fmt/print.go"],
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
            "fn main() {\n    println!(\"hello\");\n}",
            "let mut x = 0;\nx += 1;",
            "for i in 0..10 {\n    println!(\"{}\", i);\n}",
            "if x > 0 {\n    return true;\n}",
            "match val {\n    Some(x) => x,\n    None => 0,\n}",
            "struct Point {\n    x: f64,\n    y: f64,\n}",
            "impl Point {\n    fn new(x: f64, y: f64) -> Self {\n        Self { x, y }\n    }\n}",
            "let v: Vec<i32> = vec![1, 2, 3];",
            "fn add(a: i32, b: i32) -> i32 {\n    a + b\n}",
            "use std::collections::HashMap;",
            "pub fn process(input: &str) -> Result<String, Error> {\n    Ok(input.to_string())\n}",
            "let result = items\n    .iter()\n    .filter(|x| x > &0)\n    .map(|x| x * 2)\n    .collect::<Vec<_>>();",
            "enum Color {\n    Red,\n    Green,\n    Blue,\n}",
            "trait Display {\n    fn show(&self) -> String;\n}",
            "while let Some(item) = stack.pop() {\n    process(item);\n}",
            "#[derive(Debug, Clone)]\nstruct Config {\n    name: String,\n    value: i32,\n}",
            "let handle = std::thread::spawn(|| {\n    println!(\"thread\");\n});",
            "let mut map = HashMap::new();\nmap.insert(\"key\", 42);",
            "fn factorial(n: u64) -> u64 {\n    if n <= 1 {\n        1\n    } else {\n        n * factorial(n - 1)\n    }\n}",
            "impl Iterator for Counter {\n    type Item = u32;\n\n    fn next(&mut self) -> Option<Self::Item> {\n        None\n    }\n}",
            "async fn fetch(url: &str) -> Result<String> {\n    let body = reqwest::get(url)\n        .await?\n        .text()\n        .await?;\n    Ok(body)\n}",
            "let closure = |x: i32, y: i32| -> i32 {\n    x + y\n};",
            "#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn it_works() {\n        assert_eq!(2 + 2, 4);\n    }\n}",
            "pub struct Builder {\n    name: Option<String>,\n}\n\nimpl Builder {\n    pub fn name(mut self, n: &str) -> Self {\n        self.name = Some(n.into());\n        self\n    }\n}",
            "use std::sync::{Arc, Mutex};\nlet data = Arc::new(Mutex::new(vec![1, 2, 3]));",
            "if let Ok(value) = \"42\".parse::<i32>() {\n    println!(\"parsed: {}\", value);\n}",
            "fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {\n    if x.len() > y.len() {\n        x\n    } else {\n        y\n    }\n}",
            "type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;",
            "macro_rules! vec_of_strings {\n    ($($x:expr),*) => {\n        vec![$($x.to_string()),*]\n    };\n}",
            "let (tx, rx) = std::sync::mpsc::channel();\ntx.send(42).unwrap();",
        ]
    }

    fn python_snippets() -> Vec<&'static str> {
        vec![
            "def main():\n    print(\"hello\")",
            "for i in range(10):\n    print(i)",
            "if x > 0:\n    return True",
            "class Point:\n    def __init__(self, x, y):\n        self.x = x\n        self.y = y",
            "import os\npath = os.path.join(\"a\", \"b\")",
            "result = [\n    x * 2\n    for x in items\n    if x > 0\n]",
            "with open(\"file.txt\") as f:\n    data = f.read()",
            "def add(a: int, b: int) -> int:\n    return a + b",
            "try:\n    result = process(data)\nexcept ValueError as e:\n    print(e)",
            "from collections import defaultdict",
            "lambda x: x * 2 + 1",
            "dict_comp = {\n    k: v\n    for k, v in pairs.items()\n}",
            "async def fetch(url):\n    async with aiohttp.ClientSession() as session:\n        return await session.get(url)",
            "def fibonacci(n):\n    if n <= 1:\n        return n\n    return fibonacci(n-1) + fibonacci(n-2)",
            "@property\ndef name(self):\n    return self._name",
            "from dataclasses import dataclass\n\n@dataclass\nclass Config:\n    name: str\n    value: int = 0",
            "yield from range(10)",
            "sorted(\n    items,\n    key=lambda x: x.name,\n    reverse=True,\n)",
            "from typing import Optional, List, Dict",
            "with contextlib.suppress(FileNotFoundError):\n    os.remove(\"temp.txt\")",
            "class Meta(type):\n    def __new__(cls, name, bases, attrs):\n        return super().__new__(\n            cls, name, bases, attrs\n        )",
            "from functools import lru_cache\n\n@lru_cache(maxsize=128)\ndef expensive(n):\n    return sum(range(n))",
            "from pathlib import Path\nfiles = list(Path(\".\").glob(\"**/*.py\"))",
            "assert isinstance(result, dict), \\\n    f\"Expected dict, got {type(result)}\"",
            "values = {*set_a, *set_b}\nmerged = {**dict_a, **dict_b}",
        ]
    }

    fn javascript_snippets() -> Vec<&'static str> {
        vec![
            "const x = 42;\nconsole.log(x);",
            "function add(a, b) {\n    return a + b;\n}",
            "const arr = [1, 2, 3].map(\n    x => x * 2\n);",
            "if (x > 0) {\n    return true;\n}",
            "for (let i = 0; i < 10; i++) {\n    console.log(i);\n}",
            "class Point {\n    constructor(x, y) {\n        this.x = x;\n        this.y = y;\n    }\n}",
            "const { name, age } = person;",
            "async function fetch(url) {\n    const res = await get(url);\n    return res.json();\n}",
            "const obj = {\n    ...defaults,\n    ...overrides,\n};",
            "try {\n    parse(data);\n} catch (e) {\n    console.error(e);\n}",
            "export default function handler(req, res) {\n    res.send(\"ok\");\n}",
            "const result = items\n    .filter(x => x > 0)\n    .reduce((a, b) => a + b, 0);",
            "const promise = new Promise(\n    (resolve, reject) => {\n        setTimeout(resolve, 1000);\n    }\n);",
            "const [first, ...rest] = array;",
            "class EventEmitter {\n    constructor() {\n        this.listeners = new Map();\n    }\n}",
            "const proxy = new Proxy(target, {\n    get(obj, prop) {\n        return obj[prop];\n    }\n});",
            "for await (const chunk of stream) {\n    process(chunk);\n}",
            "const memoize = (fn) => {\n    const cache = new Map();\n    return (...args) => {\n        return cache.get(args) ?? fn(...args);\n    };\n};",
            "import { useState, useEffect } from 'react';\nconst [state, setState] = useState(null);",
            "const pipe = (...fns) => (x) =>\n    fns.reduce((v, f) => f(v), x);",
            "Object.entries(obj).forEach(\n    ([key, value]) => {\n        console.log(key, value);\n    }\n);",
            "const debounce = (fn, ms) => {\n    let timer;\n    return (...args) => {\n        clearTimeout(timer);\n        timer = setTimeout(\n            () => fn(...args),\n            ms\n        );\n    };\n};",
            "const observable = new Observable(\n    subscriber => {\n        subscriber.next(1);\n        subscriber.complete();\n    }\n);",
        ]
    }

    fn go_snippets() -> Vec<&'static str> {
        vec![
            "func main() {\n\tfmt.Println(\"hello\")\n}",
            "for i := 0; i < 10; i++ {\n\tfmt.Println(i)\n}",
            "if err != nil {\n\treturn err\n}",
            "type Point struct {\n\tX float64\n\tY float64\n}",
            "func add(a, b int) int {\n\treturn a + b\n}",
            "import \"fmt\"",
            "result := make([]int, 0, 10)",
            "switch val {\ncase 1:\n\treturn \"one\"\ndefault:\n\treturn \"other\"\n}",
            "go func() {\n\tch <- result\n}()",
            "defer file.Close()",
            "type Reader interface {\n\tRead(p []byte) (n int, err error)\n}",
            "ctx, cancel := context.WithTimeout(\n\tcontext.Background(),\n\ttime.Second,\n)",
            "var wg sync.WaitGroup\nwg.Add(1)\ngo func() {\n\tdefer wg.Done()\n}()",
            "func (p *Point) Distance() float64 {\n\treturn math.Sqrt(p.X*p.X + p.Y*p.Y)\n}",
            "select {\ncase msg := <-ch:\n\tprocess(msg)\ncase <-time.After(time.Second):\n\ttimeout()\n}",
            "json.NewEncoder(w).Encode(response)",
            "http.HandleFunc(\"/api\",\n\tfunc(w http.ResponseWriter, r *http.Request) {\n\t\tw.Write([]byte(\"ok\"))\n\t},\n)",
            "func Map[T, U any](s []T, f func(T) U) []U {\n\tr := make([]U, len(s))\n\tfor i, v := range s {\n\t\tr[i] = f(v)\n\t}\n\treturn r\n}",
            "var once sync.Once\nonce.Do(func() {\n\tinitialize()\n})",
            "buf := bytes.NewBuffer(nil)\nbuf.WriteString(\"hello\")",
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
        let mut used_fetched = false;

        let total_available = embedded.len() + self.fetched_snippets.len();

        while current_words < target_words {
            let idx = self.rng.gen_range(0..total_available.max(1));

            let snippet = if idx < embedded.len() {
                embedded[idx]
            } else if !self.fetched_snippets.is_empty() {
                let f_idx = (idx - embedded.len()) % self.fetched_snippets.len();
                used_fetched = true;
                &self.fetched_snippets[f_idx]
            } else {
                embedded[idx % embedded.len()]
            };

            current_words += snippet.split_whitespace().count();
            result.push(snippet.to_string());
        }

        self.last_source = if used_fetched {
            format!("GitHub source cache ({})", self.language)
        } else {
            format!("Built-in snippets ({})", self.language)
        };

        result.join("\n\n")
    }
}

/// Extract function-length snippets from raw source code, preserving whitespace.
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
                // Preserve original newlines and indentation
                let snippet = snippet_lines.join("\n");
                let char_count = snippet.chars().filter(|c| !c.is_whitespace()).count();
                // Require at least one newline (reject single-line snippets)
                let has_newline = snippet.contains('\n');
                if char_count >= 20 && snippet.len() <= 800 && has_newline {
                    snippets.push(snippet);
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
