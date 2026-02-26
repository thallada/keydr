use std::fs;

use rand::Rng;
use rand::rngs::SmallRng;

use crate::engine::filter::CharFilter;
use crate::generator::TextGenerator;
use crate::generator::cache::fetch_url_bytes_with_progress;

pub enum BlockStyle {
    Braces(&'static [&'static str]),
    Indentation(&'static [&'static str]),
    EndDelimited(&'static [&'static str]),
}

pub struct CodeLanguage {
    pub key: &'static str,
    pub display_name: &'static str,
    #[allow(dead_code)]
    pub extensions: &'static [&'static str],
    pub repos: &'static [CodeRepo],
    pub has_builtin: bool,
    pub block_style: BlockStyle,
}

pub struct CodeRepo {
    pub key: &'static str,
    pub urls: &'static [&'static str],
}

pub const CODE_LANGUAGES: &[CodeLanguage] = &[
    // === Built-in languages (has_builtin: true) ===
    CodeLanguage {
        key: "rust",
        display_name: "Rust",
        extensions: &[".rs"],
        repos: &[
            CodeRepo {
                key: "tokio",
                urls: &[
                    "https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/sync/mutex.rs",
                    "https://raw.githubusercontent.com/tokio-rs/tokio/master/tokio/src/net/tcp/stream.rs",
                ],
            },
            CodeRepo {
                key: "ripgrep",
                urls: &[
                    "https://raw.githubusercontent.com/BurntSushi/ripgrep/master/crates/regex/src/config.rs",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "fn ",
            "pub fn ",
            "async fn ",
            "pub async fn ",
            "impl ",
            "trait ",
            "struct ",
            "enum ",
            "macro_rules! ",
            "mod ",
            "const ",
            "static ",
            "type ",
            "pub struct ",
            "pub enum ",
            "pub trait ",
            "pub mod ",
            "pub const ",
            "pub static ",
            "pub type ",
        ]),
    },
    CodeLanguage {
        key: "python",
        display_name: "Python",
        extensions: &[".py", ".pyi"],
        repos: &[CodeRepo {
            key: "cpython",
            urls: &[
                "https://raw.githubusercontent.com/python/cpython/main/Lib/json/encoder.py",
                "https://raw.githubusercontent.com/python/cpython/main/Lib/pathlib/__init__.py",
            ],
        }],
        has_builtin: true,
        block_style: BlockStyle::Indentation(&["def ", "class ", "async def ", "@"]),
    },
    CodeLanguage {
        key: "javascript",
        display_name: "JavaScript",
        extensions: &[".js", ".mjs"],
        repos: &[CodeRepo {
            key: "node-stdlib",
            urls: &[
                "https://raw.githubusercontent.com/nodejs/node/main/lib/path.js",
                "https://raw.githubusercontent.com/nodejs/node/main/lib/url.js",
            ],
        }],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "function ",
            "async function ",
            "const ",
            "class ",
            "export function ",
            "export default function ",
            "let ",
            "export ",
        ]),
    },
    CodeLanguage {
        key: "go",
        display_name: "Go",
        extensions: &[".go"],
        repos: &[CodeRepo {
            key: "go-stdlib",
            urls: &["https://raw.githubusercontent.com/golang/go/master/src/fmt/print.go"],
        }],
        has_builtin: true,
        block_style: BlockStyle::Braces(&["func ", "type "]),
    },
    CodeLanguage {
        key: "typescript",
        display_name: "TypeScript",
        extensions: &[".ts", ".tsx"],
        repos: &[
            CodeRepo {
                key: "ts-node",
                urls: &["https://raw.githubusercontent.com/TypeStrong/ts-node/main/src/index.ts"],
            },
            CodeRepo {
                key: "deno-std",
                urls: &[
                    "https://raw.githubusercontent.com/denoland/std/main/path/posix/normalize.ts",
                    "https://raw.githubusercontent.com/denoland/std/main/fs/walk.ts",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "function ",
            "export function ",
            "async function ",
            "const ",
            "class ",
            "interface ",
            "type ",
            "export default function ",
            "let ",
            "export ",
        ]),
    },
    CodeLanguage {
        key: "java",
        display_name: "Java",
        extensions: &[".java"],
        repos: &[
            CodeRepo {
                key: "guava",
                urls: &[
                    "https://raw.githubusercontent.com/google/guava/master/guava/src/com/google/common/collect/ImmutableList.java",
                    "https://raw.githubusercontent.com/google/guava/master/guava/src/com/google/common/base/Preconditions.java",
                ],
            },
            CodeRepo {
                key: "gson",
                urls: &[
                    "https://raw.githubusercontent.com/google/gson/main/gson/src/main/java/com/google/gson/Gson.java",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "public ",
            "private ",
            "protected ",
            "static ",
            "class ",
            "interface ",
            "void ",
            "int ",
            "String ",
            "boolean ",
            "@",
            "abstract ",
            "final ",
        ]),
    },
    CodeLanguage {
        key: "c",
        display_name: "C",
        extensions: &[".c", ".h"],
        repos: &[
            CodeRepo {
                key: "redis",
                urls: &[
                    "https://raw.githubusercontent.com/redis/redis/unstable/src/server.c",
                    "https://raw.githubusercontent.com/redis/redis/unstable/src/networking.c",
                ],
            },
            CodeRepo {
                key: "jq",
                urls: &["https://raw.githubusercontent.com/jqlang/jq/master/src/builtin.c"],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "int ",
            "void ",
            "char ",
            "float ",
            "double ",
            "struct ",
            "unsigned ",
            "static ",
            "const ",
            "typedef ",
            "#define ",
            "enum ",
        ]),
    },
    CodeLanguage {
        key: "cpp",
        display_name: "C++",
        extensions: &[".cpp", ".hpp", ".cc", ".cxx"],
        repos: &[
            CodeRepo {
                key: "json",
                urls: &[
                    "https://raw.githubusercontent.com/nlohmann/json/develop/include/nlohmann/json.hpp",
                ],
            },
            CodeRepo {
                key: "fmt",
                urls: &["https://raw.githubusercontent.com/fmtlib/fmt/master/include/fmt/format.h"],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "int ",
            "void ",
            "char ",
            "auto ",
            "class ",
            "struct ",
            "template",
            "namespace ",
            "virtual ",
            "static ",
            "const ",
            "typedef ",
            "#define ",
            "enum ",
            "constexpr ",
        ]),
    },
    CodeLanguage {
        key: "ruby",
        display_name: "Ruby",
        extensions: &[".rb"],
        repos: &[
            CodeRepo {
                key: "rake",
                urls: &[
                    "https://raw.githubusercontent.com/ruby/rake/master/lib/rake/task.rb",
                    "https://raw.githubusercontent.com/ruby/rake/master/lib/rake/application.rb",
                ],
            },
            CodeRepo {
                key: "sinatra",
                urls: &[
                    "https://raw.githubusercontent.com/sinatra/sinatra/main/lib/sinatra/base.rb",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::EndDelimited(&[
            "def ",
            "class ",
            "module ",
            "attr_",
            "scope ",
            "describe ",
            "it ",
        ]),
    },
    CodeLanguage {
        key: "swift",
        display_name: "Swift",
        extensions: &[".swift"],
        repos: &[
            CodeRepo {
                key: "swift-algorithms",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift-algorithms/main/Sources/Algorithms/Chunked.swift",
                    "https://raw.githubusercontent.com/apple/swift-algorithms/main/Sources/Algorithms/Combinations.swift",
                ],
            },
            CodeRepo {
                key: "swift-nio",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift-nio/main/Sources/NIOCore/Channel.swift",
                    "https://raw.githubusercontent.com/apple/swift-nio/main/Sources/NIOCore/EventLoop.swift",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&[
            "func ",
            "class ",
            "struct ",
            "enum ",
            "protocol ",
            "var ",
            "let ",
            "init(",
            "deinit ",
            "extension ",
            "typealias ",
        ]),
    },
    CodeLanguage {
        key: "bash",
        display_name: "Bash",
        extensions: &[".sh", ".bash"],
        repos: &[
            CodeRepo {
                key: "nvm",
                urls: &["https://raw.githubusercontent.com/nvm-sh/nvm/master/nvm.sh"],
            },
            CodeRepo {
                key: "oh-my-zsh",
                urls: &[
                    "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/lib/functions.zsh",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Braces(&["function ", "if ", "for ", "while ", "case "]),
    },
    CodeLanguage {
        key: "lua",
        display_name: "Lua",
        extensions: &[".lua"],
        repos: &[
            CodeRepo {
                key: "kong",
                urls: &["https://raw.githubusercontent.com/Kong/kong/master/kong/init.lua"],
            },
            CodeRepo {
                key: "luarocks",
                urls: &[
                    "https://raw.githubusercontent.com/luarocks/luarocks/master/src/luarocks/core/cfg.lua",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::EndDelimited(&["function ", "local function "]),
    },
    // === Network-only languages (has_builtin: false) ===
    CodeLanguage {
        key: "kotlin",
        display_name: "Kotlin",
        extensions: &[".kt", ".kts"],
        repos: &[CodeRepo {
            key: "kotlinx-coroutines",
            urls: &[
                "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/master/kotlinx-coroutines-core/common/src/flow/Builders.kt",
                "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/master/kotlinx-coroutines-core/common/src/channels/Channel.kt",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "fun ",
            "class ",
            "object ",
            "interface ",
            "suspend fun ",
            "public ",
            "private ",
            "internal ",
            "override fun ",
            "open ",
            "data class ",
            "sealed ",
            "abstract ",
            "val ",
            "var ",
            "enum ",
            "annotation ",
            "typealias ",
        ]),
    },
    CodeLanguage {
        key: "scala",
        display_name: "Scala",
        extensions: &[".scala"],
        repos: &[CodeRepo {
            key: "scala-stdlib",
            urls: &[
                "https://raw.githubusercontent.com/scala/scala/2.13.x/src/library/scala/collection/immutable/List.scala",
                "https://raw.githubusercontent.com/scala/scala/2.13.x/src/library/scala/collection/mutable/HashMap.scala",
                "https://raw.githubusercontent.com/scala/scala/2.13.x/src/library/scala/Option.scala",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "def ",
            "class ",
            "object ",
            "trait ",
            "case class ",
            "val ",
            "var ",
            "type ",
            "implicit ",
            "given ",
            "extension ",
        ]),
    },
    CodeLanguage {
        key: "csharp",
        display_name: "C#",
        extensions: &[".cs"],
        repos: &[
            CodeRepo {
                key: "aspnetcore",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/aspnetcore/main/src/Http/Http.Abstractions/src/HttpContext.cs",
                ],
            },
            CodeRepo {
                key: "roslyn",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/roslyn/main/src/Compilers/CSharp/Portable/Syntax/SyntaxFactory.cs",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "public ",
            "private ",
            "protected ",
            "internal ",
            "static ",
            "class ",
            "interface ",
            "void ",
            "async ",
        ]),
    },
    CodeLanguage {
        key: "php",
        display_name: "PHP",
        extensions: &[".php"],
        repos: &[
            CodeRepo {
                key: "wordpress",
                urls: &[
                    "https://raw.githubusercontent.com/WordPress/WordPress/master/wp-includes/formatting.php",
                ],
            },
            CodeRepo {
                key: "symfony",
                urls: &[
                    "https://raw.githubusercontent.com/symfony/symfony/7.2/src/Symfony/Component/HttpFoundation/Request.php",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "function ",
            "public function ",
            "private function ",
            "protected function ",
            "class ",
            "interface ",
            "trait ",
            "enum ",
        ]),
    },
    CodeLanguage {
        key: "dart",
        display_name: "Dart",
        extensions: &[".dart"],
        repos: &[CodeRepo {
            key: "flutter",
            urls: &[
                "https://raw.githubusercontent.com/flutter/flutter/master/packages/flutter/lib/src/widgets/framework.dart",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "void ",
            "Future ",
            "Future<",
            "class ",
            "int ",
            "String ",
            "bool ",
            "static ",
            "factory ",
            "Widget ",
            "get ",
            "set ",
            "enum ",
            "typedef ",
            "extension ",
        ]),
    },
    CodeLanguage {
        key: "elixir",
        display_name: "Elixir",
        extensions: &[".ex", ".exs"],
        repos: &[
            CodeRepo {
                key: "phoenix",
                urls: &[
                    "https://raw.githubusercontent.com/phoenixframework/phoenix/main/lib/phoenix/router.ex",
                ],
            },
            CodeRepo {
                key: "elixir-lang",
                urls: &[
                    "https://raw.githubusercontent.com/elixir-lang/elixir/main/lib/elixir/lib/enum.ex",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::EndDelimited(&[
            "def ",
            "defp ",
            "defmodule ",
            "defmacro ",
            "defstruct",
            "defprotocol ",
            "defimpl ",
        ]),
    },
    CodeLanguage {
        key: "perl",
        display_name: "Perl",
        extensions: &[".pl", ".pm"],
        repos: &[CodeRepo {
            key: "mojolicious",
            urls: &["https://raw.githubusercontent.com/mojolicious/mojo/main/lib/Mojolicious.pm"],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&["sub "]),
    },
    CodeLanguage {
        key: "zig",
        display_name: "Zig",
        extensions: &[".zig"],
        repos: &[CodeRepo {
            key: "zig-stdlib",
            urls: &[
                "https://raw.githubusercontent.com/ziglang/zig/master/lib/std/mem.zig",
                "https://raw.githubusercontent.com/ziglang/zig/master/lib/std/fmt.zig",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "pub fn ",
            "fn ",
            "const ",
            "pub const ",
            "test ",
            "var ",
        ]),
    },
    CodeLanguage {
        key: "julia",
        display_name: "Julia",
        extensions: &[".jl"],
        repos: &[CodeRepo {
            key: "julia-stdlib",
            urls: &["https://raw.githubusercontent.com/JuliaLang/julia/master/base/array.jl"],
        }],
        has_builtin: false,
        block_style: BlockStyle::EndDelimited(&["function ", "macro "]),
    },
    CodeLanguage {
        key: "nim",
        display_name: "Nim",
        extensions: &[".nim"],
        repos: &[CodeRepo {
            key: "nim-stdlib",
            urls: &["https://raw.githubusercontent.com/nim-lang/Nim/devel/lib/pure/strutils.nim"],
        }],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["proc ", "func ", "method ", "type "]),
    },
    CodeLanguage {
        key: "ocaml",
        display_name: "OCaml",
        extensions: &[".ml", ".mli"],
        repos: &[CodeRepo {
            key: "ocaml-stdlib",
            urls: &["https://raw.githubusercontent.com/ocaml/ocaml/trunk/stdlib/list.ml"],
        }],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["let ", "type ", "module "]),
    },
    CodeLanguage {
        key: "haskell",
        display_name: "Haskell",
        extensions: &[".hs"],
        repos: &[
            CodeRepo {
                key: "aeson",
                urls: &[
                    "https://raw.githubusercontent.com/haskell/aeson/master/src/Data/Aeson/Types/Internal.hs",
                ],
            },
            CodeRepo {
                key: "xmonad",
                urls: &[
                    "https://raw.githubusercontent.com/xmonad/xmonad/master/src/XMonad/Operations.hs",
                ],
            },
        ],
        has_builtin: false,
        // Haskell: top-level declarations are indented blocks
        block_style: BlockStyle::Indentation(&[
            "data ",
            "type ",
            "class ",
            "instance ",
            "newtype ",
            "module ",
        ]),
    },
    CodeLanguage {
        key: "clojure",
        display_name: "Clojure",
        extensions: &[".clj", ".cljs"],
        repos: &[CodeRepo {
            key: "clojure-core",
            urls: &[
                "https://raw.githubusercontent.com/clojure/clojure/master/src/clj/clojure/core.clj",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["(defn ", "(defn- ", "(defmacro "]),
    },
    CodeLanguage {
        key: "r",
        display_name: "R",
        extensions: &[".r", ".R"],
        repos: &[CodeRepo {
            key: "shiny",
            urls: &[
                "https://raw.githubusercontent.com/rstudio/shiny/main/R/bootstrap.R",
                "https://raw.githubusercontent.com/rstudio/shiny/main/R/input-text.R",
            ],
        }],
        has_builtin: false,
        // R functions are defined as `name <- function(...)`. Since our extractor only
        // supports `starts_with`, we match roxygen doc blocks that precede functions.
        block_style: BlockStyle::Braces(&["#' "]),
    },
    CodeLanguage {
        key: "erlang",
        display_name: "Erlang",
        extensions: &[".erl"],
        repos: &[CodeRepo {
            key: "cowboy",
            urls: &[
                "https://raw.githubusercontent.com/ninenines/cowboy/master/src/cowboy_req.erl",
                "https://raw.githubusercontent.com/ninenines/cowboy/master/src/cowboy_http.erl",
            ],
        }],
        has_builtin: false,
        // Erlang: -spec and -record use braces for types/fields.
        // Erlang functions themselves don't use braces (they end with `.`),
        // so extraction is limited to type specs and records.
        block_style: BlockStyle::Braces(&["-spec ", "-record(", "-type ", "-callback "]),
    },
    CodeLanguage {
        key: "groovy",
        display_name: "Groovy",
        extensions: &[".groovy"],
        repos: &[CodeRepo {
            key: "nextflow",
            urls: &[
                "https://raw.githubusercontent.com/nextflow-io/nextflow/master/modules/nextflow/src/main/groovy/nextflow/processor/TaskProcessor.groovy",
                "https://raw.githubusercontent.com/nextflow-io/nextflow/master/modules/nextflow/src/main/groovy/nextflow/Session.groovy",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&["def ", "void ", "static ", "public ", "private "]),
    },
    CodeLanguage {
        key: "fsharp",
        display_name: "F#",
        extensions: &[".fs", ".fsx"],
        repos: &[CodeRepo {
            key: "fsharp-compiler",
            urls: &[
                "https://raw.githubusercontent.com/dotnet/fsharp/main/src/Compiler/Utilities/lib.fs",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["let ", "member ", "type ", "module "]),
    },
    CodeLanguage {
        key: "objective-c",
        display_name: "Objective-C",
        extensions: &[".m", ".h"],
        repos: &[CodeRepo {
            key: "afnetworking",
            urls: &[
                "https://raw.githubusercontent.com/AFNetworking/AFNetworking/master/AFNetworking/AFURLSessionManager.m",
            ],
        }],
        has_builtin: false,
        block_style: BlockStyle::Braces(&[
            "- (",
            "+ (",
            "- (void)",
            "- (id)",
            "- (BOOL)",
            "@interface ",
            "@implementation ",
            "@protocol ",
            "typedef ",
        ]),
    },
];

/// Returns list of (key, display_name) for language selection UI.
pub fn code_language_options() -> Vec<(&'static str, String)> {
    let mut options: Vec<(&'static str, String)> = CODE_LANGUAGES
        .iter()
        .map(|lang| (lang.key, lang.display_name.to_string()))
        .collect();
    options.sort_by_key(|(_, display)| display.to_lowercase());
    options.insert(0, ("all", "All (random)".to_string()));
    options
}

/// Look up a language by its key.
pub fn language_by_key(key: &str) -> Option<&'static CodeLanguage> {
    CODE_LANGUAGES.iter().find(|lang| lang.key == key)
}

/// Check if any cached snippet files exist for a language.
pub fn is_language_cached(cache_dir: &str, key: &str) -> bool {
    let dir = std::path::Path::new(cache_dir);
    if !dir.is_dir() {
        return false;
    }
    let prefix = format!("{}_", key);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&prefix) && name.ends_with(".txt") {
                if let Ok(meta) = entry.metadata() {
                    if meta.len() > 0 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Returns language keys that have either built-in snippets or cached content.
pub fn languages_with_content(cache_dir: &str) -> Vec<&'static str> {
    CODE_LANGUAGES
        .iter()
        .filter(|lang| lang.has_builtin || is_language_cached(cache_dir, lang.key))
        .map(|lang| lang.key)
        .collect()
}

/// Build a download queue of `(language_key, repo_index)` pairs for uncached repos.
/// When `lang_key` is `"all"`, queues all uncached repos across all languages.
pub fn build_code_download_queue(lang_key: &str, cache_dir: &str) -> Vec<(String, usize)> {
    let languages_to_download: Vec<&str> = if lang_key == "all" {
        CODE_LANGUAGES.iter().map(|l| l.key).collect()
    } else if language_by_key(lang_key).is_some() {
        vec![lang_key]
    } else {
        vec![]
    };

    let mut queue: Vec<(String, usize)> = Vec::new();
    for lk in &languages_to_download {
        if let Some(lang) = language_by_key(lk) {
            for (repo_idx, repo) in lang.repos.iter().enumerate() {
                let cache_path =
                    std::path::Path::new(cache_dir).join(format!("{}_{}.txt", lang.key, repo.key));
                if !cache_path.exists()
                    || std::fs::metadata(&cache_path)
                        .map(|m| m.len() == 0)
                        .unwrap_or(true)
                {
                    queue.push((lang.key.to_string(), repo_idx));
                }
            }
        }
    }
    queue
}

pub struct CodeSyntaxGenerator {
    rng: SmallRng,
    language: String,
    fetched_snippets: Vec<(String, String)>, // (snippet, repo_key)
    last_source: String,
}

impl CodeSyntaxGenerator {
    pub fn new(rng: SmallRng, language: &str, cache_dir: &str) -> Self {
        let mut generator = Self {
            rng,
            language: language.to_string(),
            fetched_snippets: Vec::new(),
            last_source: "Built-in snippets".to_string(),
        };
        generator.load_cached_snippets(cache_dir);
        generator
    }

    pub fn last_source(&self) -> &str {
        &self.last_source
    }

    fn load_cached_snippets(&mut self, cache_dir: &str) {
        let dir = std::path::Path::new(cache_dir);
        if !dir.is_dir() {
            return;
        }
        let prefix = format!("{}_", self.language);
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(&prefix) && name_str.ends_with(".txt") {
                    // Extract repo key from filename: {language}_{repo}.txt
                    let repo_key = name_str
                        .strip_prefix(&prefix)
                        .and_then(|s| s.strip_suffix(".txt"))
                        .unwrap_or("unknown")
                        .to_string();
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        let snippets: Vec<(String, String)> = content
                            .split("\n---SNIPPET---\n")
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| (s.to_string(), repo_key.clone()))
                            .collect();
                        self.fetched_snippets.extend(snippets);
                    }
                }
            }
        }
    }

    fn rust_snippets() -> Vec<&'static str> {
        vec![
            r#"fn main() {
    println!("hello");
}"#,
            r#"let mut x = 0;
x += 1;"#,
            r#"for i in 0..10 {
    println!("{}", i);
}"#,
            r#"if x > 0 {
    return true;
}"#,
            r#"match val {
    Some(x) => x,
    None => 0,
}"#,
            r#"struct Point {
    x: f64,
    y: f64,
}"#,
            r#"impl Point {
    fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}"#,
            "let v: Vec<i32> = vec![1, 2, 3];",
            r#"fn add(a: i32, b: i32) -> i32 {
    a + b
}"#,
            "use std::collections::HashMap;",
            r#"pub fn process(input: &str) -> Result<String, Error> {
    Ok(input.to_string())
}"#,
            r#"let result = items
    .iter()
    .filter(|x| x > &0)
    .map(|x| x * 2)
    .collect::<Vec<_>>();"#,
            r#"enum Color {
    Red,
    Green,
    Blue,
}"#,
            r#"trait Display {
    fn show(&self) -> String;
}"#,
            r#"while let Some(item) = stack.pop() {
    process(item);
}"#,
            r#"#[derive(Debug, Clone)]
struct Config {
    name: String,
    value: i32,
}"#,
            r#"let handle = std::thread::spawn(|| {
    println!("thread");
});"#,
            r#"let mut map = HashMap::new();
map.insert("key", 42);"#,
            r#"fn factorial(n: u64) -> u64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}"#,
            r#"impl Iterator for Counter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        None
    }
}"#,
            r#"async fn fetch(url: &str) -> Result<String> {
    let body = reqwest::get(url)
        .await?
        .text()
        .await?;
    Ok(body)
}"#,
            r#"let closure = |x: i32, y: i32| -> i32 {
    x + y
};"#,
            r#"#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}"#,
            r#"pub struct Builder {
    name: Option<String>,
}

impl Builder {
    pub fn name(mut self, n: &str) -> Self {
        self.name = Some(n.into());
        self
    }
}"#,
            r#"use std::sync::{Arc, Mutex};
let data = Arc::new(Mutex::new(vec![1, 2, 3]));"#,
            r#"if let Ok(value) = "42".parse::<i32>() {
    println!("parsed: {}", value);
}"#,
            r#"fn longest<'a>(x: &'a str, y: &'a str) -> &'a str {
    if x.len() > y.len() {
        x
    } else {
        y
    }
}"#,
            "type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;",
            r#"macro_rules! vec_of_strings {
    ($($x:expr),*) => {
        vec![$($x.to_string()),*]
    };
}"#,
            r#"let (tx, rx) = std::sync::mpsc::channel();
tx.send(42).unwrap();"#,
        ]
    }

    fn python_snippets() -> Vec<&'static str> {
        vec![
            r#"def main():
    print("hello")"#,
            r#"for i in range(10):
    print(i)"#,
            r#"if x > 0:
    return True"#,
            r#"class Point:
    def __init__(self, x, y):
        self.x = x
        self.y = y"#,
            r#"import os
path = os.path.join("a", "b")"#,
            r#"result = [
    x * 2
    for x in items
    if x > 0
]"#,
            r#"with open("file.txt") as f:
    data = f.read()"#,
            r#"def add(a: int, b: int) -> int:
    return a + b"#,
            r#"try:
    result = process(data)
except ValueError as e:
    print(e)"#,
            "from collections import defaultdict",
            "lambda x: x * 2 + 1",
            r#"dict_comp = {
    k: v
    for k, v in pairs.items()
}"#,
            r#"async def fetch(url):
    async with aiohttp.ClientSession() as session:
        return await session.get(url)"#,
            r#"def fibonacci(n):
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)"#,
            r#"@property
def name(self):
    return self._name"#,
            r#"from dataclasses import dataclass

@dataclass
class Config:
    name: str
    value: int = 0"#,
            "yield from range(10)",
            r#"sorted(
    items,
    key=lambda x: x.name,
    reverse=True,
)"#,
            "from typing import Optional, List, Dict",
            r#"with contextlib.suppress(FileNotFoundError):
    os.remove("temp.txt")"#,
            r#"class Meta(type):
    def __new__(cls, name, bases, attrs):
        return super().__new__(
            cls, name, bases, attrs
        )"#,
            r#"from functools import lru_cache

@lru_cache(maxsize=128)
def expensive(n):
    return sum(range(n))"#,
            r#"from pathlib import Path
files = list(Path(".").glob("**/*.py"))"#,
            r#"assert isinstance(result, dict), \
    f"Expected dict, got {type(result)}""#,
            r#"values = {*set_a, *set_b}
merged = {**dict_a, **dict_b}"#,
        ]
    }

    fn javascript_snippets() -> Vec<&'static str> {
        vec![
            r#"const x = 42;
console.log(x);"#,
            r#"function add(a, b) {
    return a + b;
}"#,
            r#"const arr = [1, 2, 3].map(
    x => x * 2
);"#,
            r#"if (x > 0) {
    return true;
}"#,
            r#"for (let i = 0; i < 10; i++) {
    console.log(i);
}"#,
            r#"class Point {
    constructor(x, y) {
        this.x = x;
        this.y = y;
    }
}"#,
            "const { name, age } = person;",
            r#"async function fetch(url) {
    const res = await get(url);
    return res.json();
}"#,
            r#"const obj = {
    ...defaults,
    ...overrides,
};"#,
            r#"try {
    parse(data);
} catch (e) {
    console.error(e);
}"#,
            r#"export default function handler(req, res) {
    res.send("ok");
}"#,
            r#"const result = items
    .filter(x => x > 0)
    .reduce((a, b) => a + b, 0);"#,
            r#"const promise = new Promise(
    (resolve, reject) => {
        setTimeout(resolve, 1000);
    }
);"#,
            "const [first, ...rest] = array;",
            r#"class EventEmitter {
    constructor() {
        this.listeners = new Map();
    }
}"#,
            r#"const proxy = new Proxy(target, {
    get(obj, prop) {
        return obj[prop];
    }
});"#,
            r#"for await (const chunk of stream) {
    process(chunk);
}"#,
            r#"const memoize = (fn) => {
    const cache = new Map();
    return (...args) => {
        return cache.get(args) ?? fn(...args);
    };
};"#,
            r#"import { useState, useEffect } from 'react';
const [state, setState] = useState(null);"#,
            r#"const pipe = (...fns) => (x) =>
    fns.reduce((v, f) => f(v), x);"#,
            r#"Object.entries(obj).forEach(
    ([key, value]) => {
        console.log(key, value);
    }
);"#,
            r#"const debounce = (fn, ms) => {
    let timer;
    return (...args) => {
        clearTimeout(timer);
        timer = setTimeout(
            () => fn(...args),
            ms
        );
    };
};"#,
            r#"const observable = new Observable(
    subscriber => {
        subscriber.next(1);
        subscriber.complete();
    }
);"#,
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

    fn typescript_snippets() -> Vec<&'static str> {
        vec![
            r#"interface User {
    id: number;
    name: string;
    email: string;
}"#,
            r#"type Result<T> = {
    data: T;
    error: string | null;
};"#,
            r#"function identity<T>(arg: T): T {
    return arg;
}"#,
            r#"export async function fetchData<T>(
    url: string
): Promise<T> {
    const res = await fetch(url);
    return res.json() as T;
}"#,
            r#"class Stack<T> {
    private items: T[] = [];

    push(item: T): void {
        this.items.push(item);
    }

    pop(): T | undefined {
        return this.items.pop();
    }
}"#,
            r#"const enum Direction {
    Up = "UP",
    Down = "DOWN",
    Left = "LEFT",
    Right = "RIGHT",
}"#,
            r#"type EventHandler<T> = (
    event: T
) => void;"#,
            r#"export function createStore<S>(
    initialState: S
) {
    let state = initialState;
    return {
        getState: () => state,
        setState: (next: S) => {
            state = next;
        },
    };
}"#,
            r#"interface Repository<T> {
    findById(id: string): Promise<T | null>;
    save(entity: T): Promise<void>;
    delete(id: string): Promise<boolean>;
}"#,
            r#"const guard = (
    value: unknown
): value is string => {
    return typeof value === "string";
};"#,
            r#"type DeepPartial<T> = {
    [P in keyof T]?: T[P] extends object
        ? DeepPartial<T[P]>
        : T[P];
};"#,
            r#"export function debounce<T extends (...args: any[]) => void>(
    fn: T,
    delay: number
): T {
    let timer: ReturnType<typeof setTimeout>;
    return ((...args: any[]) => {
        clearTimeout(timer);
        timer = setTimeout(() => fn(...args), delay);
    }) as T;
}"#,
        ]
    }

    fn java_snippets() -> Vec<&'static str> {
        vec![
            r#"public class Main {
    public static void main(String[] args) {
        System.out.println("hello");
    }
}"#,
            r#"public int add(int a, int b) {
    return a + b;
}"#,
            r#"public class Stack<T> {
    private List<T> items = new ArrayList<>();

    public void push(T item) {
        items.add(item);
    }

    public T pop() {
        return items.remove(items.size() - 1);
    }
}"#,
            r#"public interface Repository<T> {
    Optional<T> findById(String id);
    void save(T entity);
    boolean delete(String id);
}"#,
            r#"List<String> result = items.stream()
    .filter(s -> s.length() > 3)
    .map(String::toUpperCase)
    .collect(Collectors.toList());"#,
            r#"try {
    BufferedReader reader = new BufferedReader(
        new FileReader("data.txt")
    );
    String line = reader.readLine();
} catch (IOException e) {
    e.printStackTrace();
}"#,
            r#"@Override
public boolean equals(Object obj) {
    if (this == obj) return true;
    if (!(obj instanceof Point)) return false;
    Point other = (Point) obj;
    return x == other.x && y == other.y;
}"#,
            r#"public static <T extends Comparable<T>> T max(
    T a, T b
) {
    return a.compareTo(b) >= 0 ? a : b;
}"#,
            r#"Map<String, Integer> counts = new HashMap<>();
for (String word : words) {
    counts.merge(word, 1, Integer::sum);
}"#,
            r#"public record Point(double x, double y) {
    public double distance() {
        return Math.sqrt(x * x + y * y);
    }
}"#,
            r#"CompletableFuture<String> future =
    CompletableFuture.supplyAsync(() -> {
        return fetchData();
    }).thenApply(data -> {
        return process(data);
    });"#,
            r#"private final Lock lock = new ReentrantLock();

public void update(String value) {
    lock.lock();
    try {
        this.data = value;
    } finally {
        lock.unlock();
    }
}"#,
        ]
    }

    fn c_snippets() -> Vec<&'static str> {
        vec![
            r#"int main(int argc, char *argv[]) {
    printf("hello\n");
    return 0;
}"#,
            r#"struct Point {
    double x;
    double y;
};"#,
            r#"int *create_array(int size) {
    int *arr = malloc(size * sizeof(int));
    if (arr == NULL) {
        return NULL;
    }
    memset(arr, 0, size * sizeof(int));
    return arr;
}"#,
            r#"void swap(int *a, int *b) {
    int temp = *a;
    *a = *b;
    *b = temp;
}"#,
            r#"typedef struct Node {
    int data;
    struct Node *next;
} Node;"#,
            r#"char *str_dup(const char *src) {
    size_t len = strlen(src) + 1;
    char *dst = malloc(len);
    if (dst != NULL) {
        memcpy(dst, src, len);
    }
    return dst;
}"#,
            r#"void free_list(Node *head) {
    Node *current = head;
    while (current != NULL) {
        Node *next = current->next;
        free(current);
        current = next;
    }
}"#,
            r#"int binary_search(
    int *arr, int size, int target
) {
    int low = 0, high = size - 1;
    while (low <= high) {
        int mid = low + (high - low) / 2;
        if (arr[mid] == target) return mid;
        if (arr[mid] < target) low = mid + 1;
        else high = mid - 1;
    }
    return -1;
}"#,
            r#"static int compare(
    const void *a, const void *b
) {
    return (*(int *)a - *(int *)b);
}"#,
            r#"FILE *fp = fopen("data.txt", "r");
if (fp == NULL) {
    perror("fopen");
    return 1;
}
fclose(fp);"#,
            r#"#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define MIN(a, b) ((a) < (b) ? (a) : (b))"#,
            r#"void print_array(
    const int *arr, size_t len
) {
    for (size_t i = 0; i < len; i++) {
        printf("%d ", arr[i]);
    }
    printf("\n");
}"#,
        ]
    }

    fn cpp_snippets() -> Vec<&'static str> {
        vec![
            r#"class Vector {
public:
    Vector(double x, double y)
        : x_(x), y_(y) {}

    double length() const {
        return std::sqrt(x_ * x_ + y_ * y_);
    }

private:
    double x_, y_;
};"#,
            r#"template <typename T>
T max_value(T a, T b) {
    return (a > b) ? a : b;
}"#,
            r#"auto ptr = std::make_unique<Widget>();
ptr->update();
auto shared = std::make_shared<Config>();"#,
            r#"std::vector<int> nums = {3, 1, 4, 1, 5};
std::sort(nums.begin(), nums.end());
auto it = std::find(
    nums.begin(), nums.end(), 4
);"#,
            r#"class Shape {
public:
    virtual double area() const = 0;
    virtual ~Shape() = default;
};"#,
            r#"template <typename Container>
void print_all(const Container& c) {
    for (const auto& item : c) {
        std::cout << item << " ";
    }
    std::cout << std::endl;
}"#,
            r#"std::map<std::string, int> counts;
for (const auto& word : words) {
    counts[word]++;
}"#,
            r#"namespace utils {
    std::string trim(const std::string& s) {
        auto start = s.find_first_not_of(" \t");
        auto end = s.find_last_not_of(" \t");
        return s.substr(start, end - start + 1);
    }
}"#,
            r#"auto future = std::async(
    std::launch::async,
    []() { return compute(); }
);
auto result = future.get();"#,
            r#"class Singleton {
public:
    static Singleton& instance() {
        static Singleton s;
        return s;
    }
    Singleton(const Singleton&) = delete;
    Singleton& operator=(const Singleton&) = delete;
};"#,
            r#"try {
    auto data = parse(input);
} catch (const std::exception& e) {
    std::cerr << e.what() << std::endl;
}"#,
            r#"template <typename T>
class Stack {
    std::vector<T> data_;
public:
    void push(const T& val) {
        data_.push_back(val);
    }
    T pop() {
        T top = data_.back();
        data_.pop_back();
        return top;
    }
};"#,
        ]
    }

    fn ruby_snippets() -> Vec<&'static str> {
        vec![
            "class Animal\n  attr_reader :name, :age\n\n  def initialize(name, age)\n    @name = name\n    @age = age\n  end\nend",
            "def fibonacci(n)\n  return n if n <= 1\n  fibonacci(n - 1) + fibonacci(n - 2)\nend",
            "numbers = [1, 2, 3, 4, 5]\nresult = numbers\n  .select { |n| n.even? }\n  .map { |n| n * 2 }",
            "class Stack\n  def initialize\n    @data = []\n  end\n\n  def push(item)\n    @data.push(item)\n  end\n\n  def pop\n    @data.pop\n  end\nend",
            "File.open(\"data.txt\", \"r\") do |f|\n  f.each_line do |line|\n    puts line.strip\n  end\nend",
            "module Serializable\n  def to_json\n    instance_variables.each_with_object({}) do |var, hash|\n      hash[var.to_s.delete(\"@\")] = instance_variable_get(var)\n    end.to_json\n  end\nend",
            "begin\n  result = parse(data)\nrescue ArgumentError => e\n  puts \"Error: #{e.message}\"\nensure\n  cleanup\nend",
            "double = ->(x) { x * 2 }\ntriple = proc { |x| x * 3 }\nputs double.call(5)\nputs triple.call(5)",
            "class Config\n  def self.load(path)\n    YAML.load_file(path)\n  end\n\n  def self.defaults\n    { timeout: 30, retries: 3 }\n  end\nend",
            "hash = { name: \"Alice\", age: 30 }\nhash.each do |key, value|\n  puts \"#{key}: #{value}\"\nend",
            "def with_retry(attempts: 3)\n  attempts.times do |i|\n    begin\n      return yield\n    rescue StandardError => e\n      raise if i == attempts - 1\n    end\n  end\nend",
            "class Logger\n  def initialize(output = $stdout)\n    @output = output\n  end\n\n  def info(msg)\n    @output.puts \"[INFO] #{msg}\"\n  end\n\n  def error(msg)\n    @output.puts \"[ERROR] #{msg}\"\n  end\nend",
        ]
    }

    fn swift_snippets() -> Vec<&'static str> {
        vec![
            r#"struct Point {
    var x: Double
    var y: Double

    func distance(to other: Point) -> Double {
        let dx = x - other.x
        let dy = y - other.y
        return (dx * dx + dy * dy).squareRoot()
    }
}"#,
            r#"enum Result<T> {
    case success(T)
    case failure(Error)
}"#,
            r#"func fetchData(
    from url: URL,
    completion: @escaping (Data?) -> Void
) {
    URLSession.shared.dataTask(with: url) {
        data, _, _ in
        completion(data)
    }.resume()
}"#,
            r#"protocol Drawable {
    func draw()
    var bounds: CGRect { get }
}"#,
            r#"class ViewModel: ObservableObject {
    @Published var items: [String] = []

    func loadItems() {
        items = ["one", "two", "three"]
    }
}"#,
            r#"guard let value = optionalValue else {
    return nil
}
let result = process(value)"#,
            r#"let numbers = [1, 2, 3, 4, 5]
let doubled = numbers
    .filter { $0 > 2 }
    .map { $0 * 2 }"#,
            r#"extension Array where Element: Comparable {
    func sorted() -> [Element] {
        return self.sorted(by: <)
    }
}"#,
            r#"struct Config: Codable {
    let name: String
    let timeout: Int
    let retries: Int

    static let defaults = Config(
        name: "default",
        timeout: 30,
        retries: 3
    )
}"#,
            r#"func retry<T>(
    attempts: Int,
    task: () throws -> T
) rethrows -> T {
    for i in 0..<attempts {
        do {
            return try task()
        } catch where i < attempts - 1 {
            continue
        }
    }
    return try task()
}"#,
            r#"class Cache<Key: Hashable, Value> {
    private var storage: [Key: Value] = [:]

    func get(_ key: Key) -> Value? {
        return storage[key]
    }

    func set(_ key: Key, value: Value) {
        storage[key] = value
    }
}"#,
            r#"enum NetworkError: Error {
    case badURL
    case timeout
    case serverError(Int)

    var description: String {
        switch self {
        case .badURL: return "Invalid URL"
        case .timeout: return "Request timed out"
        case .serverError(let code):
            return "Server error: \(code)"
        }
    }
}"#,
        ]
    }

    fn bash_snippets() -> Vec<&'static str> {
        vec![
            "#!/bin/bash\nset -euo pipefail\nIFS=$'\\n\\t'",
            "function log() {\n  local level=\"$1\"\n  local msg=\"$2\"\n  echo \"[$level] $(date '+%Y-%m-%d %H:%M:%S') $msg\"\n}",
            "for file in *.txt; do\n  if [ -f \"$file\" ]; then\n    wc -l \"$file\"\n  fi\ndone",
            "count=0\nwhile read -r line; do\n  count=$((count + 1))\n  echo \"$count: $line\"\ndone < input.txt",
            "function check_deps() {\n  local deps=(\"git\" \"curl\" \"jq\")\n  for cmd in \"${deps[@]}\"; do\n    if ! command -v \"$cmd\" &>/dev/null; then\n      echo \"Missing: $cmd\"\n      exit 1\n    fi\n  done\n}",
            "case \"$1\" in\n  start)\n    echo \"Starting...\"\n    ;;\n  stop)\n    echo \"Stopping...\"\n    ;;\n  *)\n    echo \"Usage: $0 {start|stop}\"\n    exit 1\n    ;;\nesac",
            "readonly CONFIG_DIR=\"${HOME}/.config/myapp\"\nreadonly DATA_DIR=\"${HOME}/.local/share/myapp\"\nmkdir -p \"$CONFIG_DIR\" \"$DATA_DIR\"",
            "function cleanup() {\n  rm -rf \"$TMPDIR\"\n  echo \"Cleaned up temp files\"\n}\ntrap cleanup EXIT",
            "if [ -z \"${API_KEY:-}\" ]; then\n  echo \"Error: API_KEY not set\" >&2\n  exit 1\nfi",
            "function retry() {\n  local attempts=\"$1\"\n  shift\n  local count=0\n  until \"$@\"; do\n    count=$((count + 1))\n    if [ \"$count\" -ge \"$attempts\" ]; then\n      return 1\n    fi\n    sleep 1\n  done\n}",
            "declare -A colors\ncolors[red]=\"#ff0000\"\ncolors[green]=\"#00ff00\"\ncolors[blue]=\"#0000ff\"\nfor key in \"${!colors[@]}\"; do\n  echo \"$key: ${colors[$key]}\"\ndone",
            "find . -name \"*.log\" -mtime +7 -print0 |\n  xargs -0 rm -f\necho \"Old log files removed\"",
        ]
    }

    fn lua_snippets() -> Vec<&'static str> {
        vec![
            "local function greet(name)\n  print(\"Hello, \" .. name)\nend",
            "local config = {\n  host = \"localhost\",\n  port = 8080,\n  debug = false,\n}",
            "function factorial(n)\n  if n <= 1 then\n    return 1\n  end\n  return n * factorial(n - 1)\nend",
            "local mt = {\n  __index = function(t, k)\n    return rawget(t, k) or 0\n  end,\n  __tostring = function(t)\n    return table.concat(t, \", \")\n  end,\n}",
            "local function map(tbl, fn)\n  local result = {}\n  for i, v in ipairs(tbl) do\n    result[i] = fn(v)\n  end\n  return result\nend",
            "local function read_file(path)\n  local f = io.open(path, \"r\")\n  if not f then\n    return nil, \"cannot open file\"\n  end\n  local content = f:read(\"*a\")\n  f:close()\n  return content\nend",
            "local Class = {}\nClass.__index = Class\n\nfunction Class:new(name)\n  local instance = setmetatable({}, self)\n  instance.name = name\n  return instance\nend",
            "local function filter(tbl, pred)\n  local result = {}\n  for _, v in ipairs(tbl) do\n    if pred(v) then\n      table.insert(result, v)\n    end\n  end\n  return result\nend",
            "local function memoize(fn)\n  local cache = {}\n  return function(...)\n    local key = table.concat({...}, \",\")\n    if cache[key] == nil then\n      cache[key] = fn(...)\n    end\n    return cache[key]\n  end\nend",
            "for i = 1, 10 do\n  if i % 2 == 0 then\n    print(i .. \" is even\")\n  else\n    print(i .. \" is odd\")\n  end\nend",
            "local function merge(a, b)\n  local result = {}\n  for k, v in pairs(a) do\n    result[k] = v\n  end\n  for k, v in pairs(b) do\n    result[k] = v\n  end\n  return result\nend",
            "local function try_catch(fn, handler)\n  local ok, err = pcall(fn)\n  if not ok then\n    handler(err)\n  end\nend",
        ]
    }

    fn get_snippets(&self) -> Vec<&'static str> {
        match self.language.as_str() {
            "rust" => Self::rust_snippets(),
            "python" => Self::python_snippets(),
            "javascript" | "js" => Self::javascript_snippets(),
            "go" => Self::go_snippets(),
            "typescript" | "ts" => Self::typescript_snippets(),
            "java" => Self::java_snippets(),
            "c" => Self::c_snippets(),
            "cpp" | "c++" => Self::cpp_snippets(),
            "ruby" => Self::ruby_snippets(),
            "swift" => Self::swift_snippets(),
            "bash" => Self::bash_snippets(),
            "lua" => Self::lua_snippets(),
            _ => Self::rust_snippets(),
        }
    }
}

impl TextGenerator for CodeSyntaxGenerator {
    fn generate(
        &mut self,
        _filter: &CharFilter,
        _focused_char: Option<char>,
        _focused_bigram: Option<[char; 2]>,
        word_count: usize,
    ) -> String {
        let embedded = self.get_snippets();
        let target_words = word_count.max(1);
        let mut candidates: Vec<(bool, usize)> = Vec::new(); // (is_fetched, idx)
        let min_units = (target_words / 3).max(4);

        for (i, snippet) in embedded.iter().enumerate() {
            if approx_token_count(snippet) >= min_units {
                candidates.push((false, i));
            }
        }
        for (i, (snippet, _)) in self.fetched_snippets.iter().enumerate() {
            if approx_token_count(snippet) >= min_units {
                candidates.push((true, i));
            }
        }

        // If everything is short, fall back to all snippets.
        if candidates.is_empty() {
            for (i, _) in embedded.iter().enumerate() {
                candidates.push((false, i));
            }
            for (i, _) in self.fetched_snippets.iter().enumerate() {
                candidates.push((true, i));
            }
        }
        if candidates.is_empty() {
            return String::new();
        }

        let pick = self.rng.gen_range(0..candidates.len());
        let (is_fetched, idx) = candidates[pick];

        let display_name = CODE_LANGUAGES
            .iter()
            .find(|l| l.key == self.language)
            .map(|l| l.display_name)
            .unwrap_or(&self.language);

        let (selected, repo_key) = if is_fetched {
            let (snippet, repo) = self
                .fetched_snippets
                .get(idx)
                .map(|(s, r)| (s.as_str(), Some(r.as_str())))
                .unwrap_or((embedded[0], None));
            (snippet, repo)
        } else {
            (embedded.get(idx).copied().unwrap_or(embedded[0]), None)
        };
        let text = fit_snippet_to_target(selected, target_words);

        self.last_source = if let Some(repo) = repo_key {
            format!("{} \u{b7} {}", display_name, repo)
        } else {
            format!("{} \u{b7} built-in", display_name)
        };

        text
    }
}

fn approx_token_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn fit_snippet_to_target(snippet: &str, target_units: usize) -> String {
    let max_units = target_units
        .saturating_mul(3)
        .saturating_div(2)
        .max(target_units);
    if approx_token_count(snippet) <= max_units {
        return snippet.to_string();
    }

    let mut out_lines: Vec<&str> = Vec::new();
    let mut units = 0usize;
    for line in snippet.lines() {
        out_lines.push(line);
        units = units.saturating_add(approx_token_count(line));
        if units >= target_units && out_lines.len() >= 2 {
            break;
        }
    }

    if out_lines.is_empty() {
        snippet.to_string()
    } else {
        out_lines.join("\n")
    }
}

/// Download code from a repo and save extracted snippets to cache.
pub fn download_code_repo_to_cache_with_progress<F>(
    cache_dir: &str,
    language_key: &str,
    repo: &CodeRepo,
    block_style: &BlockStyle,
    snippets_limit: usize,
    mut on_progress: F,
) -> bool
where
    F: FnMut(u64, Option<u64>),
{
    if let Err(_) = fs::create_dir_all(cache_dir) {
        return false;
    }

    let mut all_snippets = Vec::new();

    for url in repo.urls {
        let bytes = fetch_url_bytes_with_progress(url, &mut on_progress);
        if let Some(bytes) = bytes {
            if let Ok(content) = String::from_utf8(bytes) {
                let snippets = extract_code_snippets(&content, block_style);
                all_snippets.extend(snippets);
            }
        }
    }

    if all_snippets.is_empty() {
        return false;
    }

    all_snippets.truncate(snippets_limit);

    let cache_path =
        std::path::Path::new(cache_dir).join(format!("{}_{}.txt", language_key, repo.key));
    let combined = all_snippets.join("\n---SNIPPET---\n");
    fs::write(cache_path, combined).is_ok()
}

/// Extract function-length snippets from raw source code, preserving whitespace.
/// Uses the given `BlockStyle` to determine how to find and delimit code blocks.
/// When keyword-based extraction yields fewer than 20 snippets, runs a structural
/// fallback pass to capture blocks by structure (brace depth, indentation, etc.).
pub fn extract_code_snippets(source: &str, block_style: &BlockStyle) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();

    let mut snippets = keyword_extract(&lines, block_style);

    if snippets.len() < 20 {
        let structural = structural_extract(&lines, block_style);
        for s in structural {
            if !snippets.contains(&s) {
                snippets.push(s);
            }
        }
    }

    snippets.truncate(200);
    snippets
}

/// Check if a snippet is "noise" (import-only, single-statement body, etc.)
fn is_noise_snippet(snippet: &str) -> bool {
    let meaningful_lines: Vec<&str> = snippet
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.is_empty()
                && !t.starts_with("//")
                && !t.starts_with('#')
                && !t.starts_with("/*")
                && !t.starts_with('*')
                && !t.starts_with("*/")
        })
        .collect();

    if meaningful_lines.is_empty() {
        return true;
    }

    // Reject if first meaningful line is just `{` or `}`
    let first = meaningful_lines[0].trim();
    if first == "{" || first == "}" {
        return true;
    }

    // Reject if body consists entirely of import/use/require/include statements
    let import_prefixes = [
        "import ",
        "from ",
        "use ",
        "require",
        "#include",
        "using ",
        "package ",
        "module ",
        "extern crate ",
    ];
    let body_lines: Vec<&str> = meaningful_lines.iter().skip(1).copied().collect();
    if !body_lines.is_empty()
        && body_lines.iter().all(|l| {
            let t = l.trim();
            import_prefixes.iter().any(|p| t.starts_with(p)) || t == "{" || t == "}"
        })
    {
        return true;
    }

    // Reject single-statement body (only 1 non-blank body line after opening)
    let non_blank_body: Vec<&str> = snippet
        .lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty() && l.trim() != "}" && l.trim() != "end")
        .collect();
    if non_blank_body.len() <= 1 && snippet.lines().count() <= 3 {
        return true;
    }

    false
}

/// Validate a candidate snippet for quality.
fn is_valid_snippet(snippet: &str) -> bool {
    let line_count = snippet.lines().count();
    if line_count < 3 || line_count > 30 {
        return false;
    }
    let char_count = snippet.chars().filter(|c| !c.is_whitespace()).count();
    if char_count < 20 || snippet.len() > 800 {
        return false;
    }
    if !snippet.contains('\n') {
        return false;
    }
    !is_noise_snippet(snippet)
}

/// Keyword-based extraction (the original algorithm).
fn keyword_extract(lines: &[&str], block_style: &BlockStyle) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let trimmed = lines[i].trim();

        match block_style {
            BlockStyle::Braces(patterns) => {
                if patterns.iter().any(|p| trimmed.starts_with(p)) {
                    let mut snippet_lines = Vec::new();
                    let mut depth = 0i32;
                    let mut j = i;

                    while j < lines.len() && snippet_lines.len() < 30 {
                        let l = lines[j];
                        snippet_lines.push(l);
                        depth += l.chars().filter(|&c| c == '{').count() as i32;
                        depth -= l.chars().filter(|&c| c == '}').count() as i32;
                        if depth <= 0 && j > i {
                            break;
                        }
                        j += 1;
                    }

                    let snippet = snippet_lines.join("\n");
                    if is_valid_snippet(&snippet) {
                        snippets.push(snippet);
                    }
                    i = j + 1;
                } else {
                    i += 1;
                }
            }
            BlockStyle::Indentation(patterns) => {
                if patterns.iter().any(|p| trimmed.starts_with(p)) {
                    let base_indent = lines[i].len() - lines[i].trim_start().len();
                    let mut snippet_lines = vec![lines[i]];
                    let mut j = i + 1;

                    while j < lines.len() && snippet_lines.len() < 30 {
                        let l = lines[j];
                        if l.trim().is_empty() {
                            snippet_lines.push(l);
                            j += 1;
                            continue;
                        }
                        let indent = l.len() - l.trim_start().len();
                        if indent > base_indent {
                            snippet_lines.push(l);
                            j += 1;
                        } else {
                            break;
                        }
                    }

                    while snippet_lines.last().map_or(false, |l| l.trim().is_empty()) {
                        snippet_lines.pop();
                    }

                    let snippet = snippet_lines.join("\n");
                    if is_valid_snippet(&snippet) {
                        snippets.push(snippet);
                    }
                    i = j;
                } else {
                    i += 1;
                }
            }
            BlockStyle::EndDelimited(patterns) => {
                if patterns.iter().any(|p| trimmed.starts_with(p)) {
                    let base_indent = lines[i].len() - lines[i].trim_start().len();
                    let mut snippet_lines = vec![lines[i]];
                    let mut j = i + 1;

                    while j < lines.len() && snippet_lines.len() < 30 {
                        let l = lines[j];
                        snippet_lines.push(l);
                        let l_trimmed = l.trim();
                        let l_indent = l.len() - l.trim_start().len();
                        if l_trimmed == "end" && l_indent <= base_indent {
                            break;
                        }
                        j += 1;
                    }

                    let snippet = snippet_lines.join("\n");
                    if is_valid_snippet(&snippet) {
                        snippets.push(snippet);
                    }
                    i = j + 1;
                } else {
                    i += 1;
                }
            }
        }
    }

    snippets
}

/// Structural fallback: extract code blocks by structure when keywords don't
/// find enough. Captures anonymous functions, nested blocks, and other constructs.
fn structural_extract(lines: &[&str], block_style: &BlockStyle) -> Vec<String> {
    match block_style {
        BlockStyle::Braces(_) => structural_extract_braces(lines),
        BlockStyle::Indentation(_) => structural_extract_indent(lines),
        BlockStyle::EndDelimited(_) => structural_extract_end(lines),
    }
}

/// Structural extraction for brace-delimited languages.
/// Scans for lines containing `{` where brace depth transitions from low levels,
/// captures until depth returns.
fn structural_extract_braces(lines: &[&str]) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut global_depth = 0i32;
    let mut i = 0;

    while i < lines.len() {
        let l = lines[i];
        let opens = l.chars().filter(|&c| c == '{').count() as i32;
        let closes = l.chars().filter(|&c| c == '}').count() as i32;
        let new_depth = global_depth + opens - closes;

        // Detect transition from depth 0→1 or 1→2 (entering a new block)
        if opens > 0 && (global_depth == 0 || global_depth == 1) && new_depth > global_depth {
            let start_depth = global_depth;
            let mut snippet_lines = Vec::new();
            let mut depth = global_depth;
            let mut j = i;

            while j < lines.len() && snippet_lines.len() < 30 {
                let sl = lines[j];
                snippet_lines.push(sl);
                depth += sl.chars().filter(|&c| c == '{').count() as i32;
                depth -= sl.chars().filter(|&c| c == '}').count() as i32;
                if depth <= start_depth && j > i {
                    break;
                }
                j += 1;
            }

            let snippet = snippet_lines.join("\n");
            if is_valid_snippet(&snippet) {
                snippets.push(snippet);
            }
            // Continue from after the block
            global_depth = depth;
            i = j + 1;
        } else {
            global_depth = new_depth;
            i += 1;
        }
    }

    snippets
}

/// Structural extraction for indentation-based languages.
/// Captures top-level non-blank lines followed by indented blocks.
fn structural_extract_indent(lines: &[&str]) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }

        let base_indent = l.len() - l.trim_start().len();
        // Only consider top-level or near-top-level lines (indent 0 or 4)
        if base_indent > 4 {
            i += 1;
            continue;
        }

        // Check if next non-blank line is indented more
        let mut has_body = false;
        let mut peek = i + 1;
        while peek < lines.len() {
            if lines[peek].trim().is_empty() {
                peek += 1;
                continue;
            }
            let peek_indent = lines[peek].len() - lines[peek].trim_start().len();
            has_body = peek_indent > base_indent;
            break;
        }

        if !has_body {
            i += 1;
            continue;
        }

        let mut snippet_lines = vec![lines[i]];
        let mut j = i + 1;

        while j < lines.len() && snippet_lines.len() < 30 {
            let sl = lines[j];
            if sl.trim().is_empty() {
                snippet_lines.push(sl);
                j += 1;
                continue;
            }
            let indent = sl.len() - sl.trim_start().len();
            if indent > base_indent {
                snippet_lines.push(sl);
                j += 1;
            } else {
                break;
            }
        }

        while snippet_lines
            .last()
            .map_or(false, |sl| sl.trim().is_empty())
        {
            snippet_lines.pop();
        }

        let snippet = snippet_lines.join("\n");
        if is_valid_snippet(&snippet) {
            snippets.push(snippet);
        }
        i = j;
    }

    snippets
}

/// Structural extraction for end-delimited languages (Ruby, Lua, Elixir).
/// Captures top-level lines followed by body ending with `end`.
fn structural_extract_end(lines: &[&str]) -> Vec<String> {
    let mut snippets = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let l = lines[i];
        if l.trim().is_empty() {
            i += 1;
            continue;
        }

        let base_indent = l.len() - l.trim_start().len();
        // Only consider top-level or near-top-level lines
        if base_indent > 4 {
            i += 1;
            continue;
        }

        // Look ahead for a matching `end` at same or lesser indent
        let mut snippet_lines = vec![lines[i]];
        let mut j = i + 1;
        let mut found_end = false;

        while j < lines.len() && snippet_lines.len() < 30 {
            let sl = lines[j];
            snippet_lines.push(sl);
            let sl_trimmed = sl.trim();
            let sl_indent = sl.len() - sl.trim_start().len();
            if sl_trimmed == "end" && sl_indent <= base_indent {
                found_end = true;
                break;
            }
            j += 1;
        }

        if found_end {
            let snippet = snippet_lines.join("\n");
            if is_valid_snippet(&snippet) {
                snippets.push(snippet);
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }

    snippets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_string_snippets_preserved() {
        // Verify rust snippet content is correct after raw string conversion
        let snippets = CodeSyntaxGenerator::rust_snippets();
        let main_snippet = snippets[0];
        assert!(main_snippet.contains("fn main()"));
        assert!(main_snippet.contains("println!"));
        assert!(main_snippet.contains('\n'));
        assert_eq!(main_snippet.matches('{').count(), 1);
        assert_eq!(main_snippet.matches('}').count(), 1);

        // Verify Python indentation preserved
        let py_snippets = CodeSyntaxGenerator::python_snippets();
        let class_snippet = py_snippets[3]; // class Point
        assert!(class_snippet.contains("class Point:"));
        assert!(class_snippet.contains("    def __init__"));
        assert!(class_snippet.contains("        self.x = x"));

        // Verify Go tabs preserved
        let go_snippets = CodeSyntaxGenerator::go_snippets();
        let main_go = go_snippets[0];
        assert!(main_go.contains('\t'));
        assert!(main_go.contains("fmt.Println"));

        // Verify JavaScript content
        let js_snippets = CodeSyntaxGenerator::javascript_snippets();
        assert!(js_snippets[1].contains("function add"));
    }

    #[test]
    fn test_snippet_counts_unchanged() {
        assert_eq!(CodeSyntaxGenerator::rust_snippets().len(), 30);
        assert_eq!(CodeSyntaxGenerator::python_snippets().len(), 25);
        assert_eq!(CodeSyntaxGenerator::javascript_snippets().len(), 23);
        assert_eq!(CodeSyntaxGenerator::go_snippets().len(), 20);
        assert_eq!(CodeSyntaxGenerator::typescript_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::java_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::c_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::cpp_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::ruby_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::swift_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::bash_snippets().len(), 12);
        assert_eq!(CodeSyntaxGenerator::lua_snippets().len(), 12);
    }

    #[test]
    fn test_languages_with_content_includes_builtin() {
        let langs = languages_with_content("/nonexistent/path");
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
        assert!(langs.contains(&"javascript"));
        assert!(langs.contains(&"go"));
        assert!(langs.contains(&"typescript"));
        assert!(langs.contains(&"java"));
        assert!(langs.contains(&"c"));
        assert!(langs.contains(&"cpp"));
        assert!(langs.contains(&"ruby"));
        assert!(langs.contains(&"swift"));
        assert!(langs.contains(&"bash"));
        assert!(langs.contains(&"lua"));
        // Network-only languages should NOT appear without cache
        assert!(!langs.contains(&"kotlin"));
        assert!(!langs.contains(&"scala"));
    }

    #[test]
    fn test_code_language_options() {
        let options = code_language_options();
        assert!(options.iter().any(|(k, _)| *k == "rust"));
        assert!(options.iter().any(|(k, _)| *k == "all"));
        assert_eq!(options.first().unwrap().0, "all");
        assert_eq!(options.first().unwrap().1, "All (random)");
    }

    #[test]
    fn test_code_language_options_sorted_after_all() {
        let options = code_language_options();
        assert!(!options.is_empty());
        assert_eq!(options[0].0, "all");
        for i in 1..options.len().saturating_sub(1) {
            let a = options[i].1.to_lowercase();
            let b = options[i + 1].1.to_lowercase();
            assert!(
                a <= b,
                "Language options are not sorted at index {i}: '{}' > '{}'",
                options[i].1,
                options[i + 1].1
            );
        }
    }

    #[test]
    fn test_language_by_key() {
        assert!(language_by_key("rust").is_some());
        assert_eq!(language_by_key("rust").unwrap().display_name, "Rust");
        assert!(language_by_key("nonexistent").is_none());
    }

    #[test]
    fn test_is_language_cached_empty_dir() {
        assert!(!is_language_cached("/nonexistent/path", "rust"));
    }

    #[test]
    fn test_config_code_language_options_valid() {
        let options = code_language_options();
        let keys: Vec<&str> = options.iter().map(|(k, _)| *k).collect();
        // All CODE_LANGUAGES keys should appear
        for lang in CODE_LANGUAGES {
            assert!(keys.contains(&lang.key), "Missing key: {}", lang.key);
        }
    }

    #[test]
    fn test_build_download_queue_single_language() {
        // With a nonexistent cache dir, all repos should be queued
        let queue = build_code_download_queue("rust", "/nonexistent/cache/dir");
        let rust_lang = language_by_key("rust").unwrap();
        assert_eq!(queue.len(), rust_lang.repos.len());
        for (lang_key, _) in &queue {
            assert_eq!(lang_key, "rust");
        }
    }

    #[test]
    fn test_build_download_queue_all_languages() {
        let queue = build_code_download_queue("all", "/nonexistent/cache/dir");
        // Should include repos from every language
        let total_repos: usize = CODE_LANGUAGES.iter().map(|l| l.repos.len()).sum();
        assert_eq!(queue.len(), total_repos);
        // Should include items from multiple languages
        let unique_langs: std::collections::HashSet<&str> =
            queue.iter().map(|(k, _)| k.as_str()).collect();
        assert!(unique_langs.len() > 1);
    }

    #[test]
    fn test_build_download_queue_invalid_language() {
        let queue = build_code_download_queue("nonexistent_lang", "/nonexistent/cache/dir");
        assert!(queue.is_empty());
    }

    #[test]
    fn test_build_download_queue_skips_cached() {
        // Create a temp dir with a cached file for one rust repo
        let tmp = std::env::temp_dir().join("keydr_test_queue_cache");
        let _ = fs::create_dir_all(&tmp);
        let rust_lang = language_by_key("rust").unwrap();
        let first_repo = &rust_lang.repos[0];
        let cache_file = tmp.join(format!("rust_{}.txt", first_repo.key));
        fs::write(&cache_file, "some cached content").unwrap();

        let queue = build_code_download_queue("rust", tmp.to_str().unwrap());
        // Should NOT include the cached repo
        assert!(
            !queue.iter().any(|(_, idx)| *idx == 0),
            "Cached repo should be skipped"
        );
        // Should still include other uncached repos
        assert_eq!(queue.len(), rust_lang.repos.len() - 1);

        // Cleanup
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_build_download_queue_empty_cache_file_not_skipped() {
        // An empty cache file should still be queued (treated as uncached)
        let tmp = std::env::temp_dir().join("keydr_test_queue_empty");
        let _ = fs::create_dir_all(&tmp);
        let rust_lang = language_by_key("rust").unwrap();
        let first_repo = &rust_lang.repos[0];
        let cache_file = tmp.join(format!("rust_{}.txt", first_repo.key));
        fs::write(&cache_file, "").unwrap();

        let queue = build_code_download_queue("rust", tmp.to_str().unwrap());
        // Empty file should still be in queue
        assert_eq!(queue.len(), rust_lang.repos.len());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_extract_braces_style() {
        let source = r#"fn hello() {
    println!("hello");
    println!("world");
}

fn other() {
    let x = 1;
    let y = 2;
}
"#;
        let style = BlockStyle::Braces(&["fn "]);
        let snippets = extract_code_snippets(source, &style);
        assert_eq!(snippets.len(), 2);
        assert!(snippets[0].contains("hello"));
        assert!(snippets[1].contains("other"));
    }

    #[test]
    fn test_extract_indentation_style() {
        let source = r#"def greet(name):
    msg = "Hello, " + name
    print(msg)
    return msg

x = 42

def add(a, b):
    result = a + b
    return result
"#;
        let style = BlockStyle::Indentation(&["def "]);
        let snippets = extract_code_snippets(source, &style);
        assert_eq!(snippets.len(), 2);
        assert!(snippets[0].contains("greet"));
        assert!(snippets[1].contains("add"));
    }

    #[test]
    fn test_extract_end_delimited_style() {
        let source = r#"def fibonacci(n)
  return n if n <= 1
  fibonacci(n - 1) + fibonacci(n - 2)
end

def hello
  puts "hello"
  puts "world"
end
"#;
        let style = BlockStyle::EndDelimited(&["def "]);
        let snippets = extract_code_snippets(source, &style);
        assert_eq!(snippets.len(), 2);
        assert!(snippets[0].contains("fibonacci"));
        assert!(snippets[1].contains("hello"));
    }

    #[test]
    fn test_extract_rejects_short_snippets() {
        let source = r#"fn a() {
    x
}
"#;
        let style = BlockStyle::Braces(&["fn "]);
        let snippets = extract_code_snippets(source, &style);
        // 3 lines but < 20 non-whitespace chars
        assert_eq!(snippets.len(), 0);
    }

    #[test]
    fn test_extract_indentation_with_blank_lines() {
        let source = r#"def complex():
    x = 1

    y = 2

    return x + y + 42 + 100

z = 99
"#;
        let style = BlockStyle::Indentation(&["def "]);
        let snippets = extract_code_snippets(source, &style);
        assert_eq!(snippets.len(), 1);
        assert!(snippets[0].contains("x = 1"));
        assert!(snippets[0].contains("y = 2"));
        assert!(snippets[0].contains("return"));
    }

    #[test]
    fn test_total_language_count() {
        // 12 built-in + 18 network-only = 30
        assert_eq!(CODE_LANGUAGES.len(), 30);
        let builtin_count = CODE_LANGUAGES.iter().filter(|l| l.has_builtin).count();
        assert_eq!(builtin_count, 12);
        let network_count = CODE_LANGUAGES.iter().filter(|l| !l.has_builtin).count();
        assert_eq!(network_count, 18);
    }

    #[test]
    fn test_fit_snippet_to_target_trims_large_snippet() {
        let snippet = "line one words here\nline two words here\nline three words here\nline four words here\nline five words here";
        let fitted = fit_snippet_to_target(snippet, 6);
        assert!(approx_token_count(&fitted) <= 9); // 1.5x target
        assert!(fitted.lines().count() >= 2);
    }

    /// Fetches every repo URL for all languages, runs extraction, and prints
    /// a summary with example snippets. Run with:
    ///   cargo test --features network test_verify_repo_urls -- --ignored --nocapture
    #[test]
    #[ignore]
    fn test_verify_repo_urls() {
        use crate::generator::cache::fetch_url;

        let mut total_ok = 0usize;
        let mut total_fail = 0usize;
        let mut langs_with_no_snippets: Vec<&str> = Vec::new();

        for lang in CODE_LANGUAGES {
            println!("\n{}", "=".repeat(60));
            println!("Language: {} ({})", lang.display_name, lang.key);
            println!("  Built-in: {}", lang.has_builtin);
            println!("  Repos: {}", lang.repos.len());

            let mut lang_total_snippets = 0usize;

            for repo in lang.repos {
                println!("\n  Repo: {}", repo.key);

                for url in repo.urls {
                    let short_url = if url.len() > 80 {
                        format!("{}...", &url[..77])
                    } else {
                        url.to_string()
                    };

                    match fetch_url(url) {
                        Some(content) => {
                            let lines = content.lines().count();
                            let bytes = content.len();
                            println!("    OK  {short_url}");
                            println!("        ({lines} lines, {bytes} bytes)");
                            total_ok += 1;

                            let snippets = extract_code_snippets(&content, &lang.block_style);
                            println!("        Extracted {} snippets", snippets.len());
                            lang_total_snippets += snippets.len();

                            // Show first 2 snippets (truncated)
                            for (si, snippet) in snippets.iter().take(2).enumerate() {
                                let preview: String =
                                    snippet.lines().take(5).collect::<Vec<_>>().join("\n");
                                let suffix = if snippet.lines().count() > 5 {
                                    "\n            ..."
                                } else {
                                    ""
                                };
                                let indented: String = preview
                                    .lines()
                                    .map(|l| format!("            {l}"))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                println!(
                                    "        --- snippet {} ---\n{}{}",
                                    si + 1,
                                    indented,
                                    suffix,
                                );
                            }
                        }
                        None => {
                            println!("    FAIL {short_url}");
                            total_fail += 1;
                        }
                    }
                }
            }

            println!(
                "\n  TOTAL for {}: {} snippets",
                lang.key, lang_total_snippets
            );
            if lang_total_snippets == 0 && !lang.repos.is_empty() {
                langs_with_no_snippets.push(lang.key);
            }
        }

        println!("\n{}", "=".repeat(60));
        println!("SUMMARY");
        println!("  URLs fetched OK:   {total_ok}");
        println!("  URLs failed:       {total_fail}");
        println!(
            "  Languages with 0 extracted snippets: {:?}",
            langs_with_no_snippets
        );

        if total_fail > 0 {
            println!("\nWARNING: {total_fail} URL(s) failed to fetch");
        }
        if !langs_with_no_snippets.is_empty() {
            println!(
                "\nWARNING: {} language(s) produced 0 snippets from downloads",
                langs_with_no_snippets.len()
            );
        }
    }

    #[test]
    fn test_all_languages_have_extraction_patterns() {
        for lang in CODE_LANGUAGES {
            let pattern_count = match &lang.block_style {
                BlockStyle::Braces(pats) => pats.len(),
                BlockStyle::Indentation(pats) => pats.len(),
                BlockStyle::EndDelimited(pats) => pats.len(),
            };
            assert!(
                pattern_count > 0,
                "Language '{}' has empty extraction patterns — downloads will never yield snippets",
                lang.key
            );
        }
    }
}
