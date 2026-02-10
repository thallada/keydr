use rand::rngs::SmallRng;
use rand::Rng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;

pub struct CodeSyntaxGenerator {
    rng: SmallRng,
    language: String,
}

impl CodeSyntaxGenerator {
    pub fn new(rng: SmallRng, language: &str) -> Self {
        Self {
            rng,
            language: language.to_string(),
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
        let snippets = self.get_snippets();
        let mut result = Vec::new();
        let target_words = word_count;
        let mut current_words = 0;

        while current_words < target_words {
            let idx = self.rng.gen_range(0..snippets.len());
            let snippet = snippets[idx];
            current_words += snippet.split_whitespace().count();
            result.push(snippet);
        }

        result.join(" ")
    }
}
