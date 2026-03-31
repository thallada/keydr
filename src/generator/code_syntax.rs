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
                    "https://raw.githubusercontent.com/tokio-rs/tokio/6752f50154025a1aa2c231b643cdb78bb4c3892f/tokio/src/sync/mutex.rs",
                    "https://raw.githubusercontent.com/tokio-rs/tokio/6752f50154025a1aa2c231b643cdb78bb4c3892f/tokio/src/net/tcp/stream.rs",
                    "https://raw.githubusercontent.com/tokio-rs/tokio/6752f50154025a1aa2c231b643cdb78bb4c3892f/tokio/src/sync/mpsc/chan.rs",
                ],
            },
            CodeRepo {
                key: "ripgrep",
                urls: &[
                    "https://raw.githubusercontent.com/BurntSushi/ripgrep/4519153e5e461527f4bca45b042fff45c4ec6fb9/crates/regex/src/config.rs",
                    "https://raw.githubusercontent.com/BurntSushi/ripgrep/4519153e5e461527f4bca45b042fff45c4ec6fb9/crates/searcher/src/searcher/mod.rs",
                    "https://raw.githubusercontent.com/BurntSushi/ripgrep/4519153e5e461527f4bca45b042fff45c4ec6fb9/crates/globset/src/lib.rs",
                ],
            },
            CodeRepo {
                key: "serde",
                urls: &[
                    "https://raw.githubusercontent.com/serde-rs/serde/fa7da4a93567ed347ad0735c28e439fca688ef26/serde_core/src/ser/mod.rs",
                    "https://raw.githubusercontent.com/serde-rs/serde/fa7da4a93567ed347ad0735c28e439fca688ef26/serde_core/src/de/mod.rs",
                    "https://raw.githubusercontent.com/serde-rs/serde/fa7da4a93567ed347ad0735c28e439fca688ef26/serde_core/src/macros.rs",
                ],
            },
            CodeRepo {
                key: "axum",
                urls: &[
                    "https://raw.githubusercontent.com/tokio-rs/axum/441216428893d13544f12722b54dcaaadd47135a/axum/src/routing/mod.rs",
                    "https://raw.githubusercontent.com/tokio-rs/axum/441216428893d13544f12722b54dcaaadd47135a/axum/src/extract/state.rs",
                    "https://raw.githubusercontent.com/tokio-rs/axum/441216428893d13544f12722b54dcaaadd47135a/axum/src/routing/method_routing.rs",
                ],
            },
            CodeRepo {
                key: "bevy",
                urls: &[
                    "https://raw.githubusercontent.com/bevyengine/bevy/84be6ac40c88eb7c5ba0243e3d6902058225c6b6/crates/bevy_ecs/src/world/mod.rs",
                    "https://raw.githubusercontent.com/bevyengine/bevy/84be6ac40c88eb7c5ba0243e3d6902058225c6b6/crates/bevy_ecs/src/query/state.rs",
                    "https://raw.githubusercontent.com/bevyengine/bevy/84be6ac40c88eb7c5ba0243e3d6902058225c6b6/crates/bevy_ecs/src/system/system_param.rs",
                ],
            },
            CodeRepo {
                key: "clap",
                urls: &[
                    "https://raw.githubusercontent.com/clap-rs/clap/f0d30d961d26f8fb636b33242256fca73a717f77/clap_builder/src/builder/command.rs",
                    "https://raw.githubusercontent.com/clap-rs/clap/f0d30d961d26f8fb636b33242256fca73a717f77/clap_builder/src/parser/parser.rs",
                ],
            },
            CodeRepo {
                key: "tokio-runtime",
                urls: &[
                    "https://raw.githubusercontent.com/tokio-rs/tokio/6752f50154025a1aa2c231b643cdb78bb4c3892f/tokio/src/runtime/scheduler/multi_thread/worker.rs",
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
        repos: &[
            CodeRepo {
                key: "cpython",
                urls: &[
                    "https://raw.githubusercontent.com/python/cpython/70d1b08a4bb52652094c3eb69e36223ecd8b8075/Lib/json/encoder.py",
                    "https://raw.githubusercontent.com/python/cpython/70d1b08a4bb52652094c3eb69e36223ecd8b8075/Lib/pathlib/__init__.py",
                ],
            },
            CodeRepo {
                key: "django",
                urls: &[
                    "https://raw.githubusercontent.com/django/django/afa026cd80a2388255a137a274568aef09f9fee7/django/db/models/query.py",
                    "https://raw.githubusercontent.com/django/django/afa026cd80a2388255a137a274568aef09f9fee7/django/http/request.py",
                ],
            },
            CodeRepo {
                key: "flask",
                urls: &[
                    "https://raw.githubusercontent.com/pallets/flask/7ef2946fb5151b745df30201b8c27790cac53875/src/flask/app.py",
                    "https://raw.githubusercontent.com/pallets/flask/7ef2946fb5151b745df30201b8c27790cac53875/src/flask/helpers.py",
                    "https://raw.githubusercontent.com/pallets/flask/7ef2946fb5151b745df30201b8c27790cac53875/src/flask/ctx.py",
                ],
            },
            CodeRepo {
                key: "requests",
                urls: &[
                    "https://raw.githubusercontent.com/psf/requests/6360477c52303c9445b45fa8744b02d05a2f0905/src/requests/models.py",
                    "https://raw.githubusercontent.com/psf/requests/6360477c52303c9445b45fa8744b02d05a2f0905/src/requests/sessions.py",
                ],
            },
            CodeRepo {
                key: "fastapi",
                urls: &[
                    "https://raw.githubusercontent.com/fastapi/fastapi/d128a7089a645466b789e32097de125a3b0f8979/fastapi/routing.py",
                    "https://raw.githubusercontent.com/fastapi/fastapi/d128a7089a645466b789e32097de125a3b0f8979/fastapi/applications.py",
                ],
            },
            CodeRepo {
                key: "black",
                urls: &[
                    "https://raw.githubusercontent.com/psf/black/e079b7e100d1e181d4ee860ee4512bf3326f32c3/src/black/linegen.py",
                    "https://raw.githubusercontent.com/psf/black/e079b7e100d1e181d4ee860ee4512bf3326f32c3/src/black/parsing.py",
                ],
            },
            CodeRepo {
                key: "cpython-collections",
                urls: &[
                    "https://raw.githubusercontent.com/python/cpython/70d1b08a4bb52652094c3eb69e36223ecd8b8075/Lib/collections/__init__.py",
                ],
            },
        ],
        has_builtin: true,
        block_style: BlockStyle::Indentation(&["def ", "class ", "async def ", "@"]),
    },
    CodeLanguage {
        key: "javascript",
        display_name: "JavaScript",
        extensions: &[".js", ".mjs"],
        repos: &[
            CodeRepo {
                key: "node-stdlib",
                urls: &[
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/path.js",
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/url.js",
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/fs.js",
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/events.js",
                ],
            },
            CodeRepo {
                key: "express",
                urls: &[
                    "https://raw.githubusercontent.com/expressjs/express/6c4249feec8ab40631817c8e7001baf2ed022224/lib/application.js",
                    "https://raw.githubusercontent.com/expressjs/express/6c4249feec8ab40631817c8e7001baf2ed022224/lib/response.js",
                    "https://raw.githubusercontent.com/expressjs/express/6c4249feec8ab40631817c8e7001baf2ed022224/lib/request.js",
                ],
            },
            CodeRepo {
                key: "webpack",
                urls: &[
                    "https://raw.githubusercontent.com/webpack/webpack/9fb3b03ae1f7e66e6c6815561de5830c1821af5c/lib/Compiler.js",
                    "https://raw.githubusercontent.com/webpack/webpack/9fb3b03ae1f7e66e6c6815561de5830c1821af5c/lib/NormalModule.js",
                ],
            },
            CodeRepo {
                key: "three-js",
                urls: &[
                    "https://raw.githubusercontent.com/mrdoob/three.js/85f77d7f2ad12ce724aa5624facb11b23233281d/src/math/Vector3.js",
                    "https://raw.githubusercontent.com/mrdoob/three.js/85f77d7f2ad12ce724aa5624facb11b23233281d/src/math/Matrix4.js",
                    "https://raw.githubusercontent.com/mrdoob/three.js/85f77d7f2ad12ce724aa5624facb11b23233281d/src/math/Quaternion.js",
                ],
            },
            CodeRepo {
                key: "node-streams",
                urls: &[
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/internal/streams/readable.js",
                    "https://raw.githubusercontent.com/nodejs/node/bf452bb0af57b859f2f907329ef4c2a71f09b5e7/lib/internal/streams/writable.js",
                ],
            },
        ],
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
        repos: &[
            CodeRepo {
                key: "go-stdlib",
                urls: &[
                    "https://raw.githubusercontent.com/golang/go/1f8aff4386ca8be6ae9b9553205d113884c4a8ee/src/fmt/print.go",
                    "https://raw.githubusercontent.com/golang/go/1f8aff4386ca8be6ae9b9553205d113884c4a8ee/src/fmt/format.go",
                    "https://raw.githubusercontent.com/golang/go/1f8aff4386ca8be6ae9b9553205d113884c4a8ee/src/net/http/server.go",
                ],
            },
            CodeRepo {
                key: "gin",
                urls: &[
                    "https://raw.githubusercontent.com/gin-gonic/gin/d3ffc9985281dcf4d3bef604cce4e662b1a327a6/gin.go",
                    "https://raw.githubusercontent.com/gin-gonic/gin/d3ffc9985281dcf4d3bef604cce4e662b1a327a6/context.go",
                    "https://raw.githubusercontent.com/gin-gonic/gin/d3ffc9985281dcf4d3bef604cce4e662b1a327a6/routergroup.go",
                ],
            },
            CodeRepo {
                key: "hugo",
                urls: &[
                    "https://raw.githubusercontent.com/gohugoio/hugo/df520e315087210e069050a873fb5e208659af91/hugolib/page.go",
                    "https://raw.githubusercontent.com/gohugoio/hugo/df520e315087210e069050a873fb5e208659af91/hugolib/site.go",
                ],
            },
            CodeRepo {
                key: "prometheus",
                urls: &[
                    "https://raw.githubusercontent.com/prometheus/prometheus/ced39f8a74dda22d4a87fc8a2d1d71c446826bb1/model/labels/labels_common.go",
                    "https://raw.githubusercontent.com/prometheus/prometheus/ced39f8a74dda22d4a87fc8a2d1d71c446826bb1/model/labels/matcher.go",
                    "https://raw.githubusercontent.com/prometheus/prometheus/ced39f8a74dda22d4a87fc8a2d1d71c446826bb1/tsdb/head.go",
                ],
            },
            CodeRepo {
                key: "kubernetes",
                urls: &[
                    "https://raw.githubusercontent.com/kubernetes/kubernetes/610490d1e13f20aaf76ca4092afcf32ba07b5b34/pkg/scheduler/schedule_one.go",
                    "https://raw.githubusercontent.com/kubernetes/kubernetes/610490d1e13f20aaf76ca4092afcf32ba07b5b34/pkg/scheduler/scheduler.go",
                ],
            },
        ],
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
                urls: &[
                    "https://raw.githubusercontent.com/TypeStrong/ts-node/ddb05ef23be92a90c3ecac5a0220435c65ebbd2a/src/index.ts",
                ],
            },
            CodeRepo {
                key: "deno-std",
                urls: &[
                    "https://raw.githubusercontent.com/denoland/std/1cd63ca60af2e63d51162918b2c061c6f841f9ee/fs/walk.ts",
                    "https://raw.githubusercontent.com/denoland/std/1cd63ca60af2e63d51162918b2c061c6f841f9ee/fs/copy.ts",
                ],
            },
            CodeRepo {
                key: "typeorm",
                urls: &[
                    "https://raw.githubusercontent.com/typeorm/typeorm/c6f0aa5119fe7c5ab5b602ae8bfbf798278ce443/src/query-builder/QueryBuilder.ts",
                    "https://raw.githubusercontent.com/typeorm/typeorm/c6f0aa5119fe7c5ab5b602ae8bfbf798278ce443/src/query-builder/SelectQueryBuilder.ts",
                    "https://raw.githubusercontent.com/typeorm/typeorm/c6f0aa5119fe7c5ab5b602ae8bfbf798278ce443/src/entity-manager/EntityManager.ts",
                ],
            },
            CodeRepo {
                key: "zod",
                urls: &[
                    "https://raw.githubusercontent.com/colinhacks/zod/c7805073fef5b6b8857307c3d4b3597a70613bc2/packages/zod/src/v4/classic/schemas.ts",
                ],
            },
            CodeRepo {
                key: "prisma",
                urls: &[
                    "https://raw.githubusercontent.com/prisma/prisma/5fece0a97ca3f7a05a7ae6691d49728d19b795a4/packages/client/src/runtime/core/model/applyModel.ts",
                    "https://raw.githubusercontent.com/prisma/prisma/5fece0a97ca3f7a05a7ae6691d49728d19b795a4/packages/client/src/runtime/core/jsonProtocol/serializeJsonQuery.ts",
                ],
            },
            CodeRepo {
                key: "typescript-compiler",
                urls: &[
                    "https://raw.githubusercontent.com/microsoft/TypeScript/71586adc7bec6b358809b5301ad2c6f0d7703174/src/compiler/utilities.ts",
                    "https://raw.githubusercontent.com/microsoft/TypeScript/71586adc7bec6b358809b5301ad2c6f0d7703174/src/compiler/checker.ts",
                ],
            },
            CodeRepo {
                key: "vscode",
                urls: &[
                    "https://raw.githubusercontent.com/microsoft/vscode/b27242af3a1749bb0223224e87ec8432e825b91f/src/vs/editor/common/model/textModel.ts",
                    "https://raw.githubusercontent.com/microsoft/vscode/b27242af3a1749bb0223224e87ec8432e825b91f/src/vs/base/common/strings.ts",
                ],
            },
            CodeRepo {
                key: "angular",
                urls: &[
                    "https://raw.githubusercontent.com/angular/angular/c9502999cfd5063e8af32424b99496ae9906b04a/packages/core/src/render3/instructions/shared.ts",
                    "https://raw.githubusercontent.com/angular/angular/c9502999cfd5063e8af32424b99496ae9906b04a/packages/core/src/di/injector.ts",
                ],
            },
            CodeRepo {
                key: "nestjs",
                urls: &[
                    "https://raw.githubusercontent.com/nestjs/nest/f7d4460f0b34bd4a70be4552c3ca9e11eaecdb8c/packages/core/injector/container.ts",
                    "https://raw.githubusercontent.com/nestjs/nest/f7d4460f0b34bd4a70be4552c3ca9e11eaecdb8c/packages/core/router/router-explorer.ts",
                ],
            },
            CodeRepo {
                key: "react-router",
                urls: &[
                    "https://raw.githubusercontent.com/remix-run/react-router/7b21212c8044e7a541f101b62dc2496911fbc2d2/packages/react-router/lib/router/router.ts",
                ],
            },
            CodeRepo {
                key: "lexical",
                urls: &[
                    "https://raw.githubusercontent.com/facebook/lexical/84c9e0d66f494a992e8de2320b261295a56f1688/packages/lexical/src/LexicalNode.ts",
                    "https://raw.githubusercontent.com/facebook/lexical/84c9e0d66f494a992e8de2320b261295a56f1688/packages/lexical/src/LexicalEditor.ts",
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
                    "https://raw.githubusercontent.com/google/guava/816eaa1d8b0ab1f5d16b671337dbb447e3845ce0/guava/src/com/google/common/collect/ImmutableList.java",
                    "https://raw.githubusercontent.com/google/guava/816eaa1d8b0ab1f5d16b671337dbb447e3845ce0/guava/src/com/google/common/base/Preconditions.java",
                ],
            },
            CodeRepo {
                key: "gson",
                urls: &[
                    "https://raw.githubusercontent.com/google/gson/1fa9b7a0a994b006b3be00e2df9de778e71e6807/gson/src/main/java/com/google/gson/Gson.java",
                    "https://raw.githubusercontent.com/google/gson/1fa9b7a0a994b006b3be00e2df9de778e71e6807/gson/src/main/java/com/google/gson/stream/JsonReader.java",
                ],
            },
            CodeRepo {
                key: "spring-framework",
                urls: &[
                    "https://raw.githubusercontent.com/spring-projects/spring-framework/e7fdbb8339b3d5ee8e61a4d232d1d499a816a61b/spring-context/src/main/java/org/springframework/context/annotation/ConfigurationClassParser.java",
                    "https://raw.githubusercontent.com/spring-projects/spring-framework/e7fdbb8339b3d5ee8e61a4d232d1d499a816a61b/spring-beans/src/main/java/org/springframework/beans/factory/support/DefaultListableBeanFactory.java",
                ],
            },
            CodeRepo {
                key: "elasticsearch",
                urls: &[
                    "https://raw.githubusercontent.com/elastic/elasticsearch/31afffb42a14f1838474b431ea06ff281a7cd96e/server/src/main/java/org/elasticsearch/index/query/QueryBuilders.java",
                ],
            },
            CodeRepo {
                key: "junit5",
                urls: &[
                    "https://raw.githubusercontent.com/junit-team/junit5/596ca97324f3b820895ae84b64a5cfcd3446fe5a/junit-jupiter-api/src/main/java/org/junit/jupiter/api/Assertions.java",
                ],
            },
            CodeRepo {
                key: "commons-lang",
                urls: &[
                    "https://raw.githubusercontent.com/apache/commons-lang/4d52dd2e873bf9152d5132256d7a8c2b4e975bd9/src/main/java/org/apache/commons/lang3/StringUtils.java",
                ],
            },
            CodeRepo {
                key: "commons-lang-math",
                urls: &[
                    "https://raw.githubusercontent.com/apache/commons-lang/4d52dd2e873bf9152d5132256d7a8c2b4e975bd9/src/main/java/org/apache/commons/lang3/math/NumberUtils.java",
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
                    "https://raw.githubusercontent.com/redis/redis/a6a27f56f2df1a9da3d46aea1fca6e33a89e3f61/src/server.c",
                    "https://raw.githubusercontent.com/redis/redis/a6a27f56f2df1a9da3d46aea1fca6e33a89e3f61/src/networking.c",
                ],
            },
            CodeRepo {
                key: "jq",
                urls: &["https://raw.githubusercontent.com/jqlang/jq/cec6b0f34603edc5cd12db1f63dacdf547b4bb4a/src/builtin.c"],
            },
            CodeRepo {
                key: "sqlite",
                urls: &[
                    "https://raw.githubusercontent.com/sqlite/sqlite/f92d107242c8aed695bc2823e41d63afd69c84f7/src/where.c",
                    "https://raw.githubusercontent.com/sqlite/sqlite/f92d107242c8aed695bc2823e41d63afd69c84f7/src/select.c",
                ],
            },
            CodeRepo {
                key: "curl",
                urls: &[
                    "https://raw.githubusercontent.com/curl/curl/b9690e9cd14188a5f6ab994cfea98f33447c487e/lib/url.c",
                    "https://raw.githubusercontent.com/curl/curl/b9690e9cd14188a5f6ab994cfea98f33447c487e/lib/transfer.c",
                ],
            },
            CodeRepo {
                key: "git",
                urls: &[
                    "https://raw.githubusercontent.com/git/git/270e10ad6dda3379ea0da7efd11e4fbf2cd7a325/diff.c",
                    "https://raw.githubusercontent.com/git/git/270e10ad6dda3379ea0da7efd11e4fbf2cd7a325/commit.c",
                ],
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
                    "https://raw.githubusercontent.com/nlohmann/json/9a737481aed085fd289f82dff1fa8c3c66627a7e/include/nlohmann/json.hpp",
                ],
            },
            CodeRepo {
                key: "fmt",
                urls: &["https://raw.githubusercontent.com/fmtlib/fmt/cdb8dc76d936a12aacc20b6d283d7c24ee4307fe/include/fmt/format.h"],
            },
            CodeRepo {
                key: "abseil",
                urls: &[
                    "https://raw.githubusercontent.com/abseil/abseil-cpp/4ff7ff9ee91464682e2916af9ae0d62357001dc4/absl/strings/str_cat.cc",
                    "https://raw.githubusercontent.com/abseil/abseil-cpp/4ff7ff9ee91464682e2916af9ae0d62357001dc4/absl/container/flat_hash_map.h",
                ],
            },
            CodeRepo {
                key: "grpc",
                urls: &[
                    "https://raw.githubusercontent.com/grpc/grpc/a2f0c2965a3f644a24fcb1c40d7fe1706e65ef93/src/cpp/server/server_builder.cc",
                ],
            },
            CodeRepo {
                key: "imgui",
                urls: &[
                    "https://raw.githubusercontent.com/ocornut/imgui/689f837afae1f8673c7eebaaee8927350f3e1080/imgui_widgets.cpp",
                    "https://raw.githubusercontent.com/ocornut/imgui/689f837afae1f8673c7eebaaee8927350f3e1080/imgui.cpp",
                ],
            },
            CodeRepo {
                key: "opencv",
                urls: &[
                    "https://raw.githubusercontent.com/opencv/opencv/2027a3399076b099930fc8eb2721d8c028fdabc0/modules/core/src/matrix.cpp",
                    "https://raw.githubusercontent.com/opencv/opencv/2027a3399076b099930fc8eb2721d8c028fdabc0/modules/core/src/algorithm.cpp",
                ],
            },
            CodeRepo {
                key: "protobuf",
                urls: &[
                    "https://raw.githubusercontent.com/protocolbuffers/protobuf/b6e522885b3b69562677ba146be979d414f0b6c3/src/google/protobuf/descriptor.cc",
                ],
            },
            CodeRepo {
                key: "electron",
                urls: &[
                    "https://raw.githubusercontent.com/electron/electron/e0bd4ffc39d6bd563d7094905f579c8d79643b4c/shell/browser/api/electron_api_web_contents.cc",
                ],
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
                    "https://raw.githubusercontent.com/ruby/rake/83e40e3d5aef8289a097d0c0259958a40f866555/lib/rake/task.rb",
                    "https://raw.githubusercontent.com/ruby/rake/83e40e3d5aef8289a097d0c0259958a40f866555/lib/rake/application.rb",
                ],
            },
            CodeRepo {
                key: "sinatra",
                urls: &[
                    "https://raw.githubusercontent.com/sinatra/sinatra/f891dd2b6f4911e356600efe6c3b82af97d262c6/lib/sinatra/base.rb",
                ],
            },
            CodeRepo {
                key: "rails",
                urls: &[
                    "https://raw.githubusercontent.com/rails/rails/2ea08c89169c05bb2087cd67314bcff23116e597/activerecord/lib/active_record/relation/query_methods.rb",
                    "https://raw.githubusercontent.com/rails/rails/2ea08c89169c05bb2087cd67314bcff23116e597/actionpack/lib/action_controller/metal/strong_parameters.rb",
                ],
            },
            CodeRepo {
                key: "jekyll",
                urls: &[
                    "https://raw.githubusercontent.com/jekyll/jekyll/ff0d4dd78d939d8596f5ded57f3b2b321eb66b5a/lib/jekyll/site.rb",
                    "https://raw.githubusercontent.com/jekyll/jekyll/ff0d4dd78d939d8596f5ded57f3b2b321eb66b5a/lib/jekyll/document.rb",
                ],
            },
            CodeRepo {
                key: "devise",
                urls: &[
                    "https://raw.githubusercontent.com/heartcombo/devise/5e3a8bf3a01cc556185dbde47ecf3bb20c41b150/lib/devise/models/authenticatable.rb",
                ],
            },
            CodeRepo {
                key: "rails-routing",
                urls: &[
                    "https://raw.githubusercontent.com/rails/rails/2ea08c89169c05bb2087cd67314bcff23116e597/actionpack/lib/action_dispatch/routing/mapper.rb",
                    "https://raw.githubusercontent.com/rails/rails/2ea08c89169c05bb2087cd67314bcff23116e597/activesupport/lib/active_support/core_ext/string/inflections.rb",
                ],
            },
            CodeRepo {
                key: "ruby-net-http",
                urls: &[
                    "https://raw.githubusercontent.com/ruby/ruby/c6bee053a2bb07fd5187f88b9312d58244ba390b/lib/net/http.rb",
                    "https://raw.githubusercontent.com/ruby/ruby/c6bee053a2bb07fd5187f88b9312d58244ba390b/lib/uri/generic.rb",
                ],
            },
            CodeRepo {
                key: "rails-activerecord",
                urls: &[
                    "https://raw.githubusercontent.com/rails/rails/2ea08c89169c05bb2087cd67314bcff23116e597/activerecord/lib/active_record/base.rb",
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
                    "https://raw.githubusercontent.com/apple/swift-algorithms/0b4376902cfc3901c496a79f2cb70f1ffd583fff/Sources/Algorithms/Chunked.swift",
                    "https://raw.githubusercontent.com/apple/swift-algorithms/0b4376902cfc3901c496a79f2cb70f1ffd583fff/Sources/Algorithms/Combinations.swift",
                    "https://raw.githubusercontent.com/apple/swift-algorithms/0b4376902cfc3901c496a79f2cb70f1ffd583fff/Sources/Algorithms/Permutations.swift",
                ],
            },
            CodeRepo {
                key: "swift-nio",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift-nio/558f24a4647193b5a0e2104031b71c55d31ff83a/Sources/NIOCore/Channel.swift",
                    "https://raw.githubusercontent.com/apple/swift-nio/558f24a4647193b5a0e2104031b71c55d31ff83a/Sources/NIOCore/EventLoop.swift",
                ],
            },
            CodeRepo {
                key: "vapor",
                urls: &[
                    "https://raw.githubusercontent.com/vapor/vapor/ff88583e10f02aa47be49e632d5179f89e855e03/Sources/Vapor/Application.swift",
                    "https://raw.githubusercontent.com/vapor/vapor/ff88583e10f02aa47be49e632d5179f89e855e03/Sources/Vapor/Routing/RoutesBuilder.swift",
                ],
            },
            CodeRepo {
                key: "alamofire",
                urls: &[
                    "https://raw.githubusercontent.com/Alamofire/Alamofire/36f1747e31305e0cfda27864091318950c66a5b1/Source/Core/Session.swift",
                    "https://raw.githubusercontent.com/Alamofire/Alamofire/36f1747e31305e0cfda27864091318950c66a5b1/Source/Core/Request.swift",
                    "https://raw.githubusercontent.com/Alamofire/Alamofire/36f1747e31305e0cfda27864091318950c66a5b1/Source/Core/Response.swift",
                ],
            },
            CodeRepo {
                key: "swift-collections",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift-collections/63bfbed01a39126550b0f1ac87ac48027697831a/Sources/OrderedCollections/OrderedDictionary/OrderedDictionary.swift",
                ],
            },
            CodeRepo {
                key: "swift-stdlib-array",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Array.swift",
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/String.swift",
                ],
            },
            CodeRepo {
                key: "swift-stdlib-collections",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Dictionary.swift",
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Set.swift",
                ],
            },
            CodeRepo {
                key: "swift-stdlib-types",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Result.swift",
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Optional.swift",
                    "https://raw.githubusercontent.com/apple/swift/a891406502855a598d830c806b4fc1887766f6cb/stdlib/public/core/Sequence.swift",
                ],
            },
            CodeRepo {
                key: "swift-nio-buffer",
                urls: &[
                    "https://raw.githubusercontent.com/apple/swift-nio/558f24a4647193b5a0e2104031b71c55d31ff83a/Sources/NIOCore/ByteBuffer-core.swift",
                    "https://raw.githubusercontent.com/apple/swift-nio/558f24a4647193b5a0e2104031b71c55d31ff83a/Sources/NIOPosix/SocketChannel.swift",
                ],
            },
            CodeRepo {
                key: "rxswift",
                urls: &[
                    "https://raw.githubusercontent.com/ReactiveX/RxSwift/132aea4f236ccadc51590b38af0357a331d51fa2/RxSwift/Observables/Merge.swift",
                ],
            },
            CodeRepo {
                key: "kingfisher",
                urls: &[
                    "https://raw.githubusercontent.com/onevcat/Kingfisher/79a6be40f52b228415af9b73126cdeeb5970bf1e/Sources/General/KingfisherManager.swift",
                ],
            },
            CodeRepo {
                key: "swiftformat",
                urls: &[
                    "https://raw.githubusercontent.com/nicklockwood/SwiftFormat/c8e50ff2cfc2eab46246c072a9ae25ab656c6ec3/Sources/SwiftFormat.swift",
                ],
            },
            CodeRepo {
                key: "tca",
                urls: &[
                    "https://raw.githubusercontent.com/pointfreeco/swift-composable-architecture/ce8ee57840b2b46cd6b5cf06ed0526dd1d690126/Sources/ComposableArchitecture/Store.swift",
                ],
            },
            CodeRepo {
                key: "snapkit",
                urls: &[
                    "https://raw.githubusercontent.com/SnapKit/SnapKit/19f59a63f0faac287f4e59986959859d81ec851c/Sources/ConstraintMaker.swift",
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
                urls: &["https://raw.githubusercontent.com/nvm-sh/nvm/001ea8cac1eb61c8f0e29889ea05ab0af69546d8/nvm.sh"],
            },
            CodeRepo {
                key: "oh-my-zsh",
                urls: &[
                    "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/9e2c1548c3dfeefd055e1c6606f66657093ae928/lib/functions.zsh",
                ],
            },
            CodeRepo {
                key: "asdf",
                urls: &[
                    "https://raw.githubusercontent.com/asdf-vm/asdf/9d5f08b100fce9617a4e734a8a67eec63552c345/lib/functions/installs.bash",
                    "https://raw.githubusercontent.com/asdf-vm/asdf/9d5f08b100fce9617a4e734a8a67eec63552c345/lib/functions/versions.bash",
                ],
            },
            CodeRepo {
                key: "omz-diagnostics",
                urls: &[
                    "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/9e2c1548c3dfeefd055e1c6606f66657093ae928/lib/diagnostics.zsh",
                ],
            },
            CodeRepo {
                key: "rbenv",
                urls: &[
                    "https://raw.githubusercontent.com/rbenv/rbenv/23c3041695feb811c18dbb270096c7956f7c377d/libexec/rbenv-init",
                ],
            },
            CodeRepo {
                key: "neofetch",
                urls: &[
                    "https://raw.githubusercontent.com/dylanaraps/neofetch/ccd5d9f52609bbdcd5d8fa78c4fdb0f12954125f/neofetch",
                ],
            },
            CodeRepo {
                key: "n",
                urls: &[
                    "https://raw.githubusercontent.com/tj/n/f52d2172f12cd76f0efe9524690723f52ab74f40/bin/n",
                ],
            },
            CodeRepo {
                key: "acme-sh",
                urls: &[
                    "https://raw.githubusercontent.com/acmesh-official/acme.sh/5d158b164028b240e0710a8d7a0ce4835a0ba1be/acme.sh",
                ],
            },
            CodeRepo {
                key: "dehydrated",
                urls: &[
                    "https://raw.githubusercontent.com/dehydrated-io/dehydrated/7ea8aaab5c257cb2c4b980f2f73597369a44d503/dehydrated",
                ],
            },
            CodeRepo {
                key: "git-completion",
                urls: &[
                    "https://raw.githubusercontent.com/git/git/270e10ad6dda3379ea0da7efd11e4fbf2cd7a325/contrib/completion/git-completion.bash",
                ],
            },
            CodeRepo {
                key: "fzf",
                urls: &[
                    "https://raw.githubusercontent.com/junegunn/fzf/cc16a97a40a16efecbed5bd7fb322010c033e503/shell/completion.bash",
                    "https://raw.githubusercontent.com/junegunn/fzf/cc16a97a40a16efecbed5bd7fb322010c033e503/shell/key-bindings.bash",
                ],
            },
            CodeRepo {
                key: "bash-my-aws",
                urls: &[
                    "https://raw.githubusercontent.com/bash-my-aws/bash-my-aws/775ff9362c62670dcb44d6976d176c305e03d279/lib/instance-functions",
                ],
            },
            CodeRepo {
                key: "p10k",
                urls: &[
                    "https://raw.githubusercontent.com/romkatv/powerlevel10k/604f19a9eaa18e76db2e60b8d446d5f879065f90/internal/p10k.zsh",
                ],
            },
            CodeRepo {
                key: "liquidprompt",
                urls: &[
                    "https://raw.githubusercontent.com/liquidprompt/liquidprompt/a4f6b8d8c90b3eaa33d13dfd1093062ab9c4b30c/liquidprompt",
                ],
            },
            CodeRepo {
                key: "omz-git",
                urls: &[
                    "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/9e2c1548c3dfeefd055e1c6606f66657093ae928/lib/git.zsh",
                    "https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/9e2c1548c3dfeefd055e1c6606f66657093ae928/lib/cli.zsh",
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
                urls: &["https://raw.githubusercontent.com/Kong/kong/58f2daa56b90615f78d5953229936192cd1128e9/kong/init.lua"],
            },
            CodeRepo {
                key: "luarocks",
                urls: &[
                    "https://raw.githubusercontent.com/luarocks/luarocks/6b0a7f7f8770f5d21730a5f2fa8fcbc695687c43/src/luarocks/core/cfg.lua",
                ],
            },
            CodeRepo {
                key: "neovim",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/lsp/client.lua",
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/treesitter/query.lua",
                ],
            },
            CodeRepo {
                key: "openresty",
                urls: &[
                    "https://raw.githubusercontent.com/openresty/lua-resty-core/04c564acc22c90f9650fe42fec74879ae4b656a6/lib/resty/core/request.lua",
                ],
            },
            CodeRepo {
                key: "neovim-lsp-handlers",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/lsp/handlers.lua",
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/lsp/util.lua",
                ],
            },
            CodeRepo {
                key: "neovim-treesitter",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/treesitter/languagetree.lua",
                ],
            },
            CodeRepo {
                key: "plenary",
                urls: &[
                    "https://raw.githubusercontent.com/nvim-lua/plenary.nvim/b9fd5226c2f76c951fc8ed5923d85e4de065e509/lua/plenary/path.lua",
                    "https://raw.githubusercontent.com/nvim-lua/plenary.nvim/b9fd5226c2f76c951fc8ed5923d85e4de065e509/lua/plenary/job.lua",
                ],
            },
            CodeRepo {
                key: "telescope",
                urls: &[
                    "https://raw.githubusercontent.com/nvim-telescope/telescope.nvim/e6cdb4dc528c5dc4ca8da86e83ef4e3c84b0729c/lua/telescope/pickers.lua",
                    "https://raw.githubusercontent.com/nvim-telescope/telescope.nvim/e6cdb4dc528c5dc4ca8da86e83ef4e3c84b0729c/lua/telescope/builtin/__files.lua",
                ],
            },
            CodeRepo {
                key: "awesome-wm",
                urls: &[
                    "https://raw.githubusercontent.com/awesomeWM/awesome/496008691facf3d962c487588741db3ec9654c52/lib/awful/layout/init.lua",
                    "https://raw.githubusercontent.com/awesomeWM/awesome/496008691facf3d962c487588741db3ec9654c52/lib/awful/widget/taglist.lua",
                ],
            },
            CodeRepo {
                key: "neovim-diagnostic",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/diagnostic.lua",
                ],
            },
            CodeRepo {
                key: "neovim-lsp-buf",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/lsp/buf.lua",
                ],
            },
            CodeRepo {
                key: "neovim-snippet",
                urls: &[
                    "https://raw.githubusercontent.com/neovim/neovim/da58fe8fd27510bb68db91b7d67264737e2279a2/runtime/lua/vim/snippet.lua",
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
        repos: &[
            CodeRepo {
                key: "kotlinx-coroutines",
                urls: &[
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/flow/Builders.kt",
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/channels/Channel.kt",
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/flow/SharedFlow.kt",
                ],
            },
            CodeRepo {
                key: "ktor",
                urls: &[
                    "https://raw.githubusercontent.com/ktorio/ktor/1567ffe13d1dd131fc2449579dabe6d1167726a8/ktor-server/ktor-server-core/common/src/io/ktor/server/routing/RoutingBuilder.kt",
                    "https://raw.githubusercontent.com/ktorio/ktor/1567ffe13d1dd131fc2449579dabe6d1167726a8/ktor-server/ktor-server-core/common/src/io/ktor/server/routing/RoutingNode.kt",
                    "https://raw.githubusercontent.com/ktorio/ktor/1567ffe13d1dd131fc2449579dabe6d1167726a8/ktor-client/ktor-client-core/common/src/io/ktor/client/HttpClient.kt",
                ],
            },
            CodeRepo {
                key: "okhttp",
                urls: &[
                    "https://raw.githubusercontent.com/square/okhttp/99791c205d84bb4cacc433da99d418e93e1fccce/okhttp/src/commonJvmAndroid/kotlin/okhttp3/OkHttpClient.kt",
                    "https://raw.githubusercontent.com/square/okhttp/99791c205d84bb4cacc433da99d418e93e1fccce/okhttp/src/commonJvmAndroid/kotlin/okhttp3/Request.kt",
                ],
            },
            CodeRepo {
                key: "kotlinx-serialization",
                urls: &[
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.serialization/c49dc47fbc3c9f2d2cbf42cd2a3876a1337c88bf/core/commonMain/src/kotlinx/serialization/Serializers.kt",
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.serialization/c49dc47fbc3c9f2d2cbf42cd2a3876a1337c88bf/core/commonMain/src/kotlinx/serialization/descriptors/SerialDescriptors.kt",
                ],
            },
            CodeRepo {
                key: "kotlinx-coroutines-extra",
                urls: &[
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/flow/operators/Merge.kt",
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/Delay.kt",
                    "https://raw.githubusercontent.com/Kotlin/kotlinx.coroutines/b11abdf01d4d5db85247ab365abc72efc7b95062/kotlinx-coroutines-core/common/src/CoroutineScope.kt",
                ],
            },
            CodeRepo {
                key: "kotlin-stdlib-collections",
                urls: &[
                    "https://raw.githubusercontent.com/JetBrains/kotlin/6ddfa0b9759c13a6ea97ef7f7e3efc0469730218/libraries/stdlib/src/kotlin/collections/Maps.kt",
                    "https://raw.githubusercontent.com/JetBrains/kotlin/6ddfa0b9759c13a6ea97ef7f7e3efc0469730218/libraries/stdlib/src/kotlin/collections/Collections.kt",
                ],
            },
            CodeRepo {
                key: "kotlin-stdlib-sequences",
                urls: &[
                    "https://raw.githubusercontent.com/JetBrains/kotlin/6ddfa0b9759c13a6ea97ef7f7e3efc0469730218/libraries/stdlib/src/kotlin/collections/Sequences.kt",
                    "https://raw.githubusercontent.com/JetBrains/kotlin/6ddfa0b9759c13a6ea97ef7f7e3efc0469730218/libraries/stdlib/src/kotlin/text/Strings.kt",
                ],
            },
            CodeRepo {
                key: "arrow-core",
                urls: &[
                    "https://raw.githubusercontent.com/arrow-kt/arrow/83618a806cd05b5ebf09b20ef9cfd1464e0671e8/arrow-libs/core/arrow-core/src/commonMain/kotlin/arrow/core/Either.kt",
                ],
            },
            CodeRepo {
                key: "koin",
                urls: &[
                    "https://raw.githubusercontent.com/InsertKoinIO/koin/767281ef63d1e742522a4bd4d73cf7276926d70f/projects/core/koin-core/src/commonMain/kotlin/org/koin/core/KoinApplication.kt",
                    "https://raw.githubusercontent.com/InsertKoinIO/koin/767281ef63d1e742522a4bd4d73cf7276926d70f/projects/core/koin-core/src/commonMain/kotlin/org/koin/core/Koin.kt",
                ],
            },
            CodeRepo {
                key: "sqldelight",
                urls: &[
                    "https://raw.githubusercontent.com/cashapp/sqldelight/585cd14faac58d9045d53bf0ee95f278a73f978b/sqldelight-compiler/src/main/kotlin/app/cash/sqldelight/core/compiler/QueryGenerator.kt",
                ],
            },
            CodeRepo {
                key: "accompanist",
                urls: &[
                    "https://raw.githubusercontent.com/google/accompanist/12ec3408fc101e0006e38dc62f7acff7a822e20b/permissions/src/main/java/com/google/accompanist/permissions/PermissionsUtil.kt",
                ],
            },
            CodeRepo {
                key: "kotlin-stdlib-strings-jvm",
                urls: &[
                    "https://raw.githubusercontent.com/JetBrains/kotlin/6ddfa0b9759c13a6ea97ef7f7e3efc0469730218/libraries/stdlib/jvm/src/kotlin/text/StringsJVM.kt",
                ],
            },
        ],
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
        repos: &[
            CodeRepo {
                key: "scala-stdlib",
                urls: &[
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/collection/immutable/List.scala",
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/collection/mutable/HashMap.scala",
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/Option.scala",
                ],
            },
            CodeRepo {
                key: "akka",
                urls: &[
                    "https://raw.githubusercontent.com/akka/akka/1717068c7905dc7641194d86a7b84579896c0020/akka-actor/src/main/scala/akka/actor/Actor.scala",
                    "https://raw.githubusercontent.com/akka/akka/1717068c7905dc7641194d86a7b84579896c0020/akka-stream/src/main/scala/akka/stream/scaladsl/Flow.scala",
                ],
            },
            CodeRepo {
                key: "spark",
                urls: &[
                    "https://raw.githubusercontent.com/apache/spark/d90b6c5bbd7128a75c17443a000a0e3ec32e366b/sql/core/src/main/scala/org/apache/spark/sql/classic/Dataset.scala",
                    "https://raw.githubusercontent.com/apache/spark/d90b6c5bbd7128a75c17443a000a0e3ec32e366b/sql/core/src/main/scala/org/apache/spark/sql/classic/SparkSession.scala",
                ],
            },
            CodeRepo {
                key: "spark-rdd",
                urls: &[
                    "https://raw.githubusercontent.com/apache/spark/d90b6c5bbd7128a75c17443a000a0e3ec32e366b/core/src/main/scala/org/apache/spark/rdd/RDD.scala",
                ],
            },
            CodeRepo {
                key: "scala-stdlib-extra",
                urls: &[
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/collection/immutable/Map.scala",
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/collection/immutable/Vector.scala",
                ],
            },
            CodeRepo {
                key: "cats",
                urls: &[
                    "https://raw.githubusercontent.com/typelevel/cats/ef54b470abec39f901ffb36596adaca29fd84687/core/src/main/scala/cats/Monad.scala",
                    "https://raw.githubusercontent.com/typelevel/cats/ef54b470abec39f901ffb36596adaca29fd84687/core/src/main/scala/cats/Functor.scala",
                ],
            },
            CodeRepo {
                key: "cats-effect",
                urls: &[
                    "https://raw.githubusercontent.com/typelevel/cats-effect/10756cafd3ebbbd2f75c2ab2bc61fd4ab414bfb7/core/shared/src/main/scala/cats/effect/IO.scala",
                ],
            },
            CodeRepo {
                key: "play",
                urls: &[
                    "https://raw.githubusercontent.com/playframework/playframework/6c14473a4a581b24b12121dee4952cf9615065d0/core/play/src/main/scala/play/api/mvc/Results.scala",
                ],
            },
            CodeRepo {
                key: "http4s",
                urls: &[
                    "https://raw.githubusercontent.com/http4s/http4s/c63fbff43dfe7e3bec25b789fd0a0027ec40ed62/core/shared/src/main/scala/org/http4s/Uri.scala",
                    "https://raw.githubusercontent.com/http4s/http4s/c63fbff43dfe7e3bec25b789fd0a0027ec40ed62/core/shared/src/main/scala/org/http4s/Headers.scala",
                ],
            },
            CodeRepo {
                key: "zio",
                urls: &[
                    "https://raw.githubusercontent.com/zio/zio/9f65dd2154192fddf00a04b33989cd933b40ecf9/core/shared/src/main/scala/zio/ZIO.scala",
                ],
            },
            CodeRepo {
                key: "scala-iterator",
                urls: &[
                    "https://raw.githubusercontent.com/scala/scala/30a9dff1e0ccca2a79137dea5bf602a3354e9155/src/library/scala/collection/Iterator.scala",
                ],
            },
        ],
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
                    "https://raw.githubusercontent.com/dotnet/aspnetcore/c1a3e7b0f3a45cf91064f4fa4ebf1801f5efd58f/src/Http/Http.Abstractions/src/HttpContext.cs",
                ],
            },
            CodeRepo {
                key: "roslyn",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/roslyn/68d99b22ba5d08644224a3160266d798ae3dd7b7/src/Compilers/CSharp/Portable/Syntax/SyntaxFactory.cs",
                ],
            },
            CodeRepo {
                key: "runtime",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Text.Json/src/System/Text/Json/Serialization/JsonSerializer.Read.String.cs",
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Text.Json/src/System/Text/Json/Serialization/JsonSerializerOptions.cs",
                ],
            },
            CodeRepo {
                key: "newtonsoft-json",
                urls: &[
                    "https://raw.githubusercontent.com/JamesNK/Newtonsoft.Json/4f73e74372445108d2c1bda37b36e6f5e43402e0/Src/Newtonsoft.Json/JsonConvert.cs",
                ],
            },
            CodeRepo {
                key: "runtime-stringbuilder",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Private.CoreLib/src/System/Text/StringBuilder.cs",
                ],
            },
            CodeRepo {
                key: "runtime-collections",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Collections/src/System/Collections/Generic/SortedList.cs",
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Private.CoreLib/src/System/Collections/Generic/Dictionary.cs",
                ],
            },
            CodeRepo {
                key: "runtime-string",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/runtime/dd96f2c77c57efe67eba429b5e689fde6f5c0248/src/libraries/System.Private.CoreLib/src/System/String.Manipulation.cs",
                ],
            },
            CodeRepo {
                key: "aspnetcore-mvc",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/aspnetcore/c1a3e7b0f3a45cf91064f4fa4ebf1801f5efd58f/src/Http/Http.Abstractions/src/HttpRequest.cs",
                    "https://raw.githubusercontent.com/dotnet/aspnetcore/c1a3e7b0f3a45cf91064f4fa4ebf1801f5efd58f/src/Http/Http.Abstractions/src/HttpResponse.cs",
                    "https://raw.githubusercontent.com/dotnet/aspnetcore/c1a3e7b0f3a45cf91064f4fa4ebf1801f5efd58f/src/Mvc/Mvc.Core/src/ControllerBase.cs",
                ],
            },
            CodeRepo {
                key: "polly",
                urls: &[
                    "https://raw.githubusercontent.com/App-vNext/Polly/7e9960c702379f40e628d099ab2bdd0995c5bd90/src/Polly.Core/Retry/RetryResilienceStrategy.cs",
                    "https://raw.githubusercontent.com/App-vNext/Polly/7e9960c702379f40e628d099ab2bdd0995c5bd90/src/Polly.Core/Registry/ResiliencePipelineRegistry.cs",
                ],
            },
            CodeRepo {
                key: "dapper",
                urls: &[
                    "https://raw.githubusercontent.com/DapperLib/Dapper/288730e69b05c32cac898d9b55ebea219ea8a2d1/Dapper/SqlMapper.cs",
                    "https://raw.githubusercontent.com/DapperLib/Dapper/288730e69b05c32cac898d9b55ebea219ea8a2d1/Dapper/SqlMapper.Async.cs",
                ],
            },
            CodeRepo {
                key: "serilog",
                urls: &[
                    "https://raw.githubusercontent.com/serilog/serilog/6c3fbcf636b0671bbd6f5032b61a2254937d8408/src/Serilog/LoggerConfiguration.cs",
                    "https://raw.githubusercontent.com/serilog/serilog/6c3fbcf636b0671bbd6f5032b61a2254937d8408/src/Serilog/Log.cs",
                ],
            },
            CodeRepo {
                key: "automapper",
                urls: &[
                    "https://raw.githubusercontent.com/AutoMapper/AutoMapper/1af71bfe831e337254a555a611de6c22b85d0426/src/AutoMapper/Execution/ExpressionBuilder.cs",
                    "https://raw.githubusercontent.com/AutoMapper/AutoMapper/1af71bfe831e337254a555a611de6c22b85d0426/src/AutoMapper/ProfileMap.cs",
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
                    "https://raw.githubusercontent.com/WordPress/WordPress/3a476a9f7dbfed77f2f007fd99e967b075a6602e/wp-includes/formatting.php",
                ],
            },
            CodeRepo {
                key: "symfony",
                urls: &[
                    "https://raw.githubusercontent.com/symfony/symfony/66f06e5e066c95109dd3251cf93d00adbbb82309/src/Symfony/Component/HttpFoundation/Request.php",
                ],
            },
            CodeRepo {
                key: "laravel",
                urls: &[
                    "https://raw.githubusercontent.com/laravel/framework/f4d44f467696a822e50521e0c4a45012aa84cf97/src/Illuminate/Database/Eloquent/Builder.php",
                    "https://raw.githubusercontent.com/laravel/framework/f4d44f467696a822e50521e0c4a45012aa84cf97/src/Illuminate/Collections/Collection.php",
                    "https://raw.githubusercontent.com/laravel/framework/f4d44f467696a822e50521e0c4a45012aa84cf97/src/Illuminate/Routing/Router.php",
                ],
            },
            CodeRepo {
                key: "composer",
                urls: &[
                    "https://raw.githubusercontent.com/composer/composer/b83fd2977ffad7ec46e4ef6108e1912698d647f7/src/Composer/DependencyResolver/Solver.php",
                ],
            },
            CodeRepo {
                key: "symfony-response",
                urls: &[
                    "https://raw.githubusercontent.com/symfony/symfony/66f06e5e066c95109dd3251cf93d00adbbb82309/src/Symfony/Component/HttpFoundation/Response.php",
                ],
            },
            CodeRepo {
                key: "wordpress-post",
                urls: &[
                    "https://raw.githubusercontent.com/WordPress/WordPress/3a476a9f7dbfed77f2f007fd99e967b075a6602e/wp-includes/post.php",
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
        repos: &[
            CodeRepo {
                key: "flutter",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/widgets/framework.dart",
                ],
            },
            CodeRepo {
                key: "dart-sdk",
                urls: &[
                    "https://raw.githubusercontent.com/dart-lang/sdk/3177f01f5f492f3780d342dbce91cb21649cfeb8/sdk/lib/collection/list.dart",
                    "https://raw.githubusercontent.com/dart-lang/sdk/3177f01f5f492f3780d342dbce91cb21649cfeb8/sdk/lib/async/future.dart",
                ],
            },
            CodeRepo {
                key: "riverpod",
                urls: &[
                    "https://raw.githubusercontent.com/rrousselGit/riverpod/6b8a0aa1ab299a8266ee880d8390a2b578836c1b/packages/riverpod/lib/src/core/element.dart",
                    "https://raw.githubusercontent.com/rrousselGit/riverpod/6b8a0aa1ab299a8266ee880d8390a2b578836c1b/packages/riverpod/lib/src/core/provider_container.dart",
                    "https://raw.githubusercontent.com/rrousselGit/riverpod/6b8a0aa1ab299a8266ee880d8390a2b578836c1b/packages/riverpod/lib/src/core/ref.dart",
                ],
            },
            CodeRepo {
                key: "flutter-widgets",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/widgets/basic.dart",
                ],
            },
            CodeRepo {
                key: "flutter-material",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/material/app_bar.dart",
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/material/text_field.dart",
                ],
            },
            CodeRepo {
                key: "dart-sdk-core",
                urls: &[
                    "https://raw.githubusercontent.com/dart-lang/sdk/3177f01f5f492f3780d342dbce91cb21649cfeb8/sdk/lib/core/list.dart",
                    "https://raw.githubusercontent.com/dart-lang/sdk/3177f01f5f492f3780d342dbce91cb21649cfeb8/sdk/lib/core/string.dart",
                    "https://raw.githubusercontent.com/dart-lang/sdk/3177f01f5f492f3780d342dbce91cb21649cfeb8/sdk/lib/core/map.dart",
                ],
            },
            CodeRepo {
                key: "flame",
                urls: &[
                    "https://raw.githubusercontent.com/flame-engine/flame/86495694665cc4e85f7d3a94b05766cc6f6b95ba/packages/flame/lib/src/game/game.dart",
                ],
            },
            CodeRepo {
                key: "flutter-box",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/rendering/box.dart",
                ],
            },
            CodeRepo {
                key: "flutter-dropdown",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/material/dropdown.dart",
                ],
            },
            CodeRepo {
                key: "flutter-scroll-view",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/widgets/scroll_view.dart",
                ],
            },
            CodeRepo {
                key: "flutter-navigator",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/widgets/navigator.dart",
                ],
            },
            CodeRepo {
                key: "flutter-editable-text",
                urls: &[
                    "https://raw.githubusercontent.com/flutter/flutter/aaa24c388b43b5504b21747684923eaae4356dbc/packages/flutter/lib/src/widgets/editable_text.dart",
                ],
            },
            CodeRepo {
                key: "bloc",
                urls: &[
                    "https://raw.githubusercontent.com/felangel/bloc/82e12837156918ad15a082be357bd2bd2e12f742/packages/bloc/lib/src/bloc.dart",
                    "https://raw.githubusercontent.com/felangel/bloc/82e12837156918ad15a082be357bd2bd2e12f742/packages/bloc/lib/src/bloc_base.dart",
                ],
            },
            CodeRepo {
                key: "dio",
                urls: &[
                    "https://raw.githubusercontent.com/cfug/dio/85aa6f1216203b1ca707289e82e35bcff5070b54/dio/lib/src/dio_mixin.dart",
                    "https://raw.githubusercontent.com/cfug/dio/85aa6f1216203b1ca707289e82e35bcff5070b54/dio/lib/src/options.dart",
                ],
            },
            CodeRepo {
                key: "getx",
                urls: &[
                    "https://raw.githubusercontent.com/jonataslaw/getx/7bfcd9c3711c8880ee730579724dabe54f4e2598/lib/get_navigation/src/extension_navigation.dart",
                ],
            },
            CodeRepo {
                key: "drift",
                urls: &[
                    "https://raw.githubusercontent.com/simolus3/drift/cb0257aed4ce376d60a1dd405864f69733cf00fe/drift/lib/src/runtime/query_builder/migration.dart",
                    "https://raw.githubusercontent.com/simolus3/drift/cb0257aed4ce376d60a1dd405864f69733cf00fe/drift/lib/src/runtime/query_builder/query_builder.dart",
                ],
            },
        ],
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
                    "https://raw.githubusercontent.com/phoenixframework/phoenix/2db25def4aa93c501eee0b0ea4f7bbc4954e4ed3/lib/phoenix/router.ex",
                ],
            },
            CodeRepo {
                key: "elixir-lang",
                urls: &[
                    "https://raw.githubusercontent.com/elixir-lang/elixir/2916f201899843d3632645a1074a76553639dcd1/lib/elixir/lib/enum.ex",
                ],
            },
            CodeRepo {
                key: "ecto",
                urls: &[
                    "https://raw.githubusercontent.com/elixir-ecto/ecto/d962b66b77e744598b866365ce3be94dfa803273/lib/ecto/query.ex",
                    "https://raw.githubusercontent.com/elixir-ecto/ecto/d962b66b77e744598b866365ce3be94dfa803273/lib/ecto/changeset.ex",
                ],
            },
            CodeRepo {
                key: "livebook",
                urls: &[
                    "https://raw.githubusercontent.com/livebook-dev/livebook/5ded53df10fb90b2ae960550a2af073923e64161/lib/livebook/session.ex",
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
        repos: &[
            CodeRepo {
                key: "mojolicious",
                urls: &["https://raw.githubusercontent.com/mojolicious/mojo/19fc4f19a0d83204a458ae4a19d192b7eaf4ba81/lib/Mojolicious.pm"],
            },
            CodeRepo {
                key: "moose",
                urls: &[
                    "https://raw.githubusercontent.com/moose/Moose/b31896a655ef061e3459c03e5feb2382de009fc4/lib/Moose.pm",
                    "https://raw.githubusercontent.com/moose/Moose/b31896a655ef061e3459c03e5feb2382de009fc4/lib/Moose/Object.pm",
                ],
            },
            CodeRepo {
                key: "dancer2",
                urls: &[
                    "https://raw.githubusercontent.com/PerlDancer/Dancer2/25176c5b860493b4a6dcda5bc12ecbefa67df716/lib/Dancer2/Core/App.pm",
                ],
            },
            CodeRepo {
                key: "http-tiny",
                urls: &[
                    "https://raw.githubusercontent.com/Perl/perl5/998bf44359cb8ecec81cf136272558ce43409066/cpan/HTTP-Tiny/lib/HTTP/Tiny.pm",
                ],
            },
            CodeRepo {
                key: "lwp-useragent",
                urls: &[
                    "https://raw.githubusercontent.com/libwww-perl/libwww-perl/7420d1bfff7cd5369ca24e87c37edf97b2cbb0c1/lib/LWP/UserAgent.pm",
                    "https://raw.githubusercontent.com/libwww-perl/libwww-perl/7420d1bfff7cd5369ca24e87c37edf97b2cbb0c1/lib/LWP/Protocol/http.pm",
                ],
            },
            CodeRepo {
                key: "test-more",
                urls: &[
                    "https://raw.githubusercontent.com/Perl/perl5/998bf44359cb8ecec81cf136272558ce43409066/cpan/Test-Simple/lib/Test/More.pm",
                ],
            },
            CodeRepo {
                key: "mojo-dom",
                urls: &[
                    "https://raw.githubusercontent.com/mojolicious/mojo/19fc4f19a0d83204a458ae4a19d192b7eaf4ba81/lib/Mojo/DOM.pm",
                    "https://raw.githubusercontent.com/mojolicious/mojo/19fc4f19a0d83204a458ae4a19d192b7eaf4ba81/lib/Mojo/Util.pm",
                ],
            },
            CodeRepo {
                key: "plack",
                urls: &[
                    "https://raw.githubusercontent.com/plack/Plack/b3984f1c59de36903bb924c9da1273f3e11d4d2b/lib/Plack/Util.pm",
                ],
            },
            CodeRepo {
                key: "json-pp",
                urls: &[
                    "https://raw.githubusercontent.com/Perl/perl5/998bf44359cb8ecec81cf136272558ce43409066/cpan/JSON-PP/lib/JSON/PP.pm",
                ],
            },
            CodeRepo {
                key: "bioperl",
                urls: &[
                    "https://raw.githubusercontent.com/bioperl/bioperl-live/2b622d6a291b95126f3da6d3799a23d8ac7d1ef1/lib/Bio/SeqIO.pm",
                ],
            },
            CodeRepo {
                key: "perl-file-copy",
                urls: &[
                    "https://raw.githubusercontent.com/Perl/perl5/998bf44359cb8ecec81cf136272558ce43409066/lib/File/Copy.pm",
                ],
            },
            CodeRepo {
                key: "dbix-class",
                urls: &[
                    "https://raw.githubusercontent.com/Perl5/DBIx-Class/d8cf3aa31fb3d6ff7813f021fcc002663725fc41/lib/DBIx/Class/ResultSet.pm",
                    "https://raw.githubusercontent.com/Perl5/DBIx-Class/d8cf3aa31fb3d6ff7813f021fcc002663725fc41/lib/DBIx/Class/Schema.pm",
                ],
            },
            CodeRepo {
                key: "template-toolkit",
                urls: &[
                    "https://raw.githubusercontent.com/abw/Template2/b3dcb01a6df44e822fed28dc801ab04b442db77a/lib/Template/Context.pm",
                    "https://raw.githubusercontent.com/abw/Template2/b3dcb01a6df44e822fed28dc801ab04b442db77a/lib/Template/Provider.pm",
                ],
            },
            CodeRepo {
                key: "catalyst",
                urls: &[
                    "https://raw.githubusercontent.com/perl-catalyst/catalyst-runtime/1d40b8ea5a7f4a4ae99af921b914f04e7c9a21c3/lib/Catalyst.pm",
                    "https://raw.githubusercontent.com/perl-catalyst/catalyst-runtime/1d40b8ea5a7f4a4ae99af921b914f04e7c9a21c3/lib/Catalyst/Request.pm",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Braces(&["sub "]),
    },
    CodeLanguage {
        key: "zig",
        display_name: "Zig",
        extensions: &[".zig"],
        repos: &[
            CodeRepo {
                key: "zig-stdlib",
                urls: &[
                    "https://raw.githubusercontent.com/ziglang/zig/738d2be9d6b6ef3ff3559130c05159ef53336224/lib/std/mem.zig",
                    "https://raw.githubusercontent.com/ziglang/zig/738d2be9d6b6ef3ff3559130c05159ef53336224/lib/std/fmt.zig",
                ],
            },
            CodeRepo {
                key: "mach",
                urls: &[
                    "https://raw.githubusercontent.com/hexops/mach/c1b78f519cf0be283472627f32d53bea2d74d206/src/Core.zig",
                    "https://raw.githubusercontent.com/hexops/mach/c1b78f519cf0be283472627f32d53bea2d74d206/src/graph.zig",
                    "https://raw.githubusercontent.com/hexops/mach/c1b78f519cf0be283472627f32d53bea2d74d206/src/gfx/Text.zig",
                ],
            },
            CodeRepo {
                key: "zig-stdlib-extra",
                urls: &[
                    "https://raw.githubusercontent.com/ziglang/zig/738d2be9d6b6ef3ff3559130c05159ef53336224/lib/std/hash_map.zig",
                    "https://raw.githubusercontent.com/ziglang/zig/738d2be9d6b6ef3ff3559130c05159ef53336224/lib/std/array_list.zig",
                    "https://raw.githubusercontent.com/ziglang/zig/738d2be9d6b6ef3ff3559130c05159ef53336224/lib/std/os.zig",
                ],
            },
            CodeRepo {
                key: "ghostty",
                urls: &[
                    "https://raw.githubusercontent.com/ghostty-org/ghostty/20cfaae2e5ec84cca2c5a55843b399b32fb9c810/src/terminal/Terminal.zig",
                    "https://raw.githubusercontent.com/ghostty-org/ghostty/20cfaae2e5ec84cca2c5a55843b399b32fb9c810/src/terminal/Screen.zig",
                ],
            },
            CodeRepo {
                key: "tigerbeetle",
                urls: &[
                    "https://raw.githubusercontent.com/tigerbeetle/tigerbeetle/49c92bf96496b1a3b71a689d3a9348df599004ad/src/vsr/replica.zig",
                    "https://raw.githubusercontent.com/tigerbeetle/tigerbeetle/49c92bf96496b1a3b71a689d3a9348df599004ad/src/lsm/tree.zig",
                ],
            },
            CodeRepo {
                key: "river",
                urls: &[
                    "https://raw.githubusercontent.com/riverwm/river/7df0854c59c9d980f8f9eb21ae1778d737d484de/river/Server.zig",
                    "https://raw.githubusercontent.com/riverwm/river/7df0854c59c9d980f8f9eb21ae1778d737d484de/river/Window.zig",
                ],
            },
            CodeRepo {
                key: "bun",
                urls: &[
                    "https://raw.githubusercontent.com/oven-sh/bun/5c59842f78880a8b5d9c2eb99c8928fc2ec50a2d/src/bun.js/event_loop.zig",
                    "https://raw.githubusercontent.com/oven-sh/bun/5c59842f78880a8b5d9c2eb99c8928fc2ec50a2d/src/bun.js/ConsoleObject.zig",
                ],
            },
        ],
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
        repos: &[
            CodeRepo {
                key: "julia-stdlib",
                urls: &["https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/array.jl"],
            },
            CodeRepo {
                key: "flux",
                urls: &[
                    "https://raw.githubusercontent.com/FluxML/Flux.jl/cec0db7b7db64b066a472f21e002f9ad7a2460c6/src/layers/basic.jl",
                    "https://raw.githubusercontent.com/FluxML/Flux.jl/cec0db7b7db64b066a472f21e002f9ad7a2460c6/src/layers/conv.jl",
                ],
            },
            CodeRepo {
                key: "dataframes",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaData/DataFrames.jl/feaaf37648bfa3a6a02263f059ff3b3db356a97d/src/dataframe/dataframe.jl",
                ],
            },
            CodeRepo {
                key: "julia-strings",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/strings/basic.jl",
                ],
            },
            CodeRepo {
                key: "julia-dict",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/dict.jl",
                ],
            },
            CodeRepo {
                key: "julia-math",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/math.jl",
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/sort.jl",
                ],
            },
            CodeRepo {
                key: "julia-iterators",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/iterators.jl",
                ],
            },
            CodeRepo {
                key: "distributions",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaStats/Distributions.jl/196d79be2f21744a88c01295f2772f091244d557/src/univariates.jl",
                    "https://raw.githubusercontent.com/JuliaStats/Distributions.jl/196d79be2f21744a88c01295f2772f091244d557/src/univariate/continuous/normal.jl",
                ],
            },
            CodeRepo {
                key: "dataframes-abstract",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaData/DataFrames.jl/feaaf37648bfa3a6a02263f059ff3b3db356a97d/src/abstractdataframe/abstractdataframe.jl",
                ],
            },
            CodeRepo {
                key: "julia-reduce",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaLang/julia/e8208497f7f8b4c2ff1282233a65def720328579/base/reduce.jl",
                ],
            },
            CodeRepo {
                key: "jump",
                urls: &[
                    "https://raw.githubusercontent.com/jump-dev/JuMP.jl/afab75cb4868b69fed16c7b0cd0ef779c17330b7/src/variables.jl",
                    "https://raw.githubusercontent.com/jump-dev/JuMP.jl/afab75cb4868b69fed16c7b0cd0ef779c17330b7/src/constraints.jl",
                ],
            },
            CodeRepo {
                key: "makie",
                urls: &[
                    "https://raw.githubusercontent.com/MakieOrg/Makie.jl/b7e263ad464f8eae7fbe02c0d508a01851c689ee/Makie/src/basic_plots.jl",
                    "https://raw.githubusercontent.com/MakieOrg/Makie.jl/b7e263ad464f8eae7fbe02c0d508a01851c689ee/Makie/src/scenes.jl",
                ],
            },
            CodeRepo {
                key: "plots",
                urls: &[
                    "https://raw.githubusercontent.com/JuliaPlots/Plots.jl/6df02e64ff646f1824528ac9f3482d186fcb3179/PlotsBase/src/recipes.jl",
                    "https://raw.githubusercontent.com/JuliaPlots/Plots.jl/6df02e64ff646f1824528ac9f3482d186fcb3179/PlotsBase/src/Commons/attrs.jl",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::EndDelimited(&["function ", "macro "]),
    },
    CodeLanguage {
        key: "nim",
        display_name: "Nim",
        extensions: &[".nim"],
        repos: &[
            CodeRepo {
                key: "nim-stdlib",
                urls: &["https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/strutils.nim"],
            },
            CodeRepo {
                key: "nim-json",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/json.nim",
                ],
            },
            CodeRepo {
                key: "nim-os",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/os.nim",
                ],
            },
            CodeRepo {
                key: "nim-tables",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/collections/tables.nim",
                ],
            },
            CodeRepo {
                key: "nim-sequtils",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/collections/sequtils.nim",
                ],
            },
            CodeRepo {
                key: "nim-asyncdispatch",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/asyncdispatch.nim",
                ],
            },
            CodeRepo {
                key: "nim-httpclient",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/httpclient.nim",
                ],
            },
            CodeRepo {
                key: "nim-parseutils",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/parseutils.nim",
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/uri.nim",
                ],
            },
            CodeRepo {
                key: "nim-sugar",
                urls: &[
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/sugar.nim",
                    "https://raw.githubusercontent.com/nim-lang/Nim/7a82c5920c46fa7a3393ebdecc54716cb1015366/lib/pure/logging.nim",
                ],
            },
            CodeRepo {
                key: "jester",
                urls: &[
                    "https://raw.githubusercontent.com/dom96/jester/ac9b8541dce64feff9b53b700cab8496c1816651/jester.nim",
                    "https://raw.githubusercontent.com/dom96/jester/ac9b8541dce64feff9b53b700cab8496c1816651/jester/request.nim",
                ],
            },
            CodeRepo {
                key: "prologue",
                urls: &[
                    "https://raw.githubusercontent.com/planety/prologue/555fbf61500118d390216bae3970d30fad2406b0/src/prologue/core/context.nim",
                    "https://raw.githubusercontent.com/planety/prologue/555fbf61500118d390216bae3970d30fad2406b0/src/prologue/core/route.nim",
                ],
            },
            CodeRepo {
                key: "chronos",
                urls: &[
                    "https://raw.githubusercontent.com/status-im/nim-chronos/6d89155294479871de019e35a4787a9f0bfd7f3a/chronos/asyncsync.nim",
                    "https://raw.githubusercontent.com/status-im/nim-chronos/6d89155294479871de019e35a4787a9f0bfd7f3a/chronos/transport.nim",
                ],
            },
            CodeRepo {
                key: "karax",
                urls: &[
                    "https://raw.githubusercontent.com/karaxnim/karax/7e1471aea2ea1001134ae6862743902c1d1e1814/karax/karax.nim",
                    "https://raw.githubusercontent.com/karaxnim/karax/7e1471aea2ea1001134ae6862743902c1d1e1814/karax/vdom.nim",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["proc ", "func ", "method ", "type "]),
    },
    CodeLanguage {
        key: "ocaml",
        display_name: "OCaml",
        extensions: &[".ml", ".mli"],
        repos: &[
            CodeRepo {
                key: "ocaml-stdlib",
                urls: &["https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/list.ml"],
            },
            CodeRepo {
                key: "ocaml-map",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/map.ml",
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/hashtbl.ml",
                ],
            },
            CodeRepo {
                key: "dune",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/dune/c30a6e555e3f79d6196b59ca82eaba9ee08e1e6f/src/dune_rules/simple_rules.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-string",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/string.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-array",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/array.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-seq",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/seq.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-buffer",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/buffer.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-set",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/set.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-format",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/format.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-scanf",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/scanf.ml",
                ],
            },
            CodeRepo {
                key: "cohttp",
                urls: &[
                    "https://raw.githubusercontent.com/mirage/ocaml-cohttp/d2e6993f482b7ec95778d786eef2f2c119a8a6eb/cohttp/src/s.ml",
                ],
            },
            CodeRepo {
                key: "ocaml-filename",
                urls: &[
                    "https://raw.githubusercontent.com/ocaml/ocaml/d7ee697596b9688569c4db06fc32d3f9fffdeeef/stdlib/filename.ml",
                ],
            },
            CodeRepo {
                key: "lwt",
                urls: &[
                    "https://raw.githubusercontent.com/ocsigen/lwt/b935bf78c6b73c04fb304447374972a2604a9120/src/core/lwt.ml",
                    "https://raw.githubusercontent.com/ocsigen/lwt/b935bf78c6b73c04fb304447374972a2604a9120/src/core/lwt_stream.ml",
                ],
            },
            CodeRepo {
                key: "dream",
                urls: &[
                    "https://raw.githubusercontent.com/aantron/dream/1fbb7fd440cff3cf6514d7f9aa0da73d21466611/src/server/router.ml",
                    "https://raw.githubusercontent.com/aantron/dream/1fbb7fd440cff3cf6514d7f9aa0da73d21466611/src/http/http.ml",
                ],
            },
            CodeRepo {
                key: "janestreet-core",
                urls: &[
                    "https://raw.githubusercontent.com/janestreet/core/3998718a19af8b148bc1864c24ca37198e6d3820/core/src/time_ns.ml",
                    "https://raw.githubusercontent.com/janestreet/core/3998718a19af8b148bc1864c24ca37198e6d3820/core/src/map.ml",
                ],
            },
        ],
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
                    "https://raw.githubusercontent.com/haskell/aeson/45d31f1bd9a0edbd6ba55fbcdd5082b159d33106/src/Data/Aeson/Types/Internal.hs",
                ],
            },
            CodeRepo {
                key: "xmonad",
                urls: &[
                    "https://raw.githubusercontent.com/xmonad/xmonad/f87bfb23ce809611a44ff9ea4373830956a6e180/src/XMonad/Operations.hs",
                ],
            },
            CodeRepo {
                key: "pandoc",
                urls: &[
                    "https://raw.githubusercontent.com/jgm/pandoc/7777de6adb166d92b4c9ee4b24054637ab8477b7/src/Text/Pandoc/Readers/Markdown.hs",
                ],
            },
            CodeRepo {
                key: "servant",
                urls: &[
                    "https://raw.githubusercontent.com/haskell-servant/servant/0767cd8b37358d051b8405c28bb1f7ff7de5e313/servant/src/Servant/API.hs",
                ],
            },
            CodeRepo {
                key: "aeson-tojson",
                urls: &[
                    "https://raw.githubusercontent.com/haskell/aeson/45d31f1bd9a0edbd6ba55fbcdd5082b159d33106/src/Data/Aeson/Types/ToJSON.hs",
                    "https://raw.githubusercontent.com/haskell/aeson/45d31f1bd9a0edbd6ba55fbcdd5082b159d33106/src/Data/Aeson/Types/FromJSON.hs",
                ],
            },
            CodeRepo {
                key: "servant-server",
                urls: &[
                    "https://raw.githubusercontent.com/haskell-servant/servant/0767cd8b37358d051b8405c28bb1f7ff7de5e313/servant-server/src/Servant/Server/Internal.hs",
                ],
            },
            CodeRepo {
                key: "pandoc-writers",
                urls: &[
                    "https://raw.githubusercontent.com/jgm/pandoc/7777de6adb166d92b4c9ee4b24054637ab8477b7/src/Text/Pandoc/Writers/HTML.hs",
                    "https://raw.githubusercontent.com/jgm/pandoc/7777de6adb166d92b4c9ee4b24054637ab8477b7/src/Text/Pandoc/Options.hs",
                ],
            },
            CodeRepo {
                key: "yesod",
                urls: &[
                    "https://raw.githubusercontent.com/yesodweb/yesod/1b033c741ce81d01070de993b285a17e71178156/yesod-core/src/Yesod/Core/Handler.hs",
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
        repos: &[
            CodeRepo {
                key: "clojure-core",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/core.clj",
                ],
            },
            CodeRepo {
                key: "ring",
                urls: &[
                    "https://raw.githubusercontent.com/ring-clojure/ring/0a36f21646232cdcaed92e55da8f31ba7f44e856/ring-core/src/ring/middleware/params.clj",
                    "https://raw.githubusercontent.com/ring-clojure/ring/0a36f21646232cdcaed92e55da8f31ba7f44e856/ring-core/src/ring/util/response.clj",
                ],
            },
            CodeRepo {
                key: "clojure-set",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/set.clj",
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/string.clj",
                ],
            },
            CodeRepo {
                key: "clojure-io",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/java/io.clj",
                ],
            },
            CodeRepo {
                key: "clojure-walk",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/walk.clj",
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/pprint/dispatch.clj",
                ],
            },
            CodeRepo {
                key: "data-json",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/data.json/94463ffb54482427fd9b31f264b06bff6dcfd557/src/main/clojure/clojure/data/json.clj",
                ],
            },
            CodeRepo {
                key: "compojure",
                urls: &[
                    "https://raw.githubusercontent.com/weavejester/compojure/8a4758d28e8fcd28fa7610dca6e2908d039e6a4f/src/compojure/core.clj",
                ],
            },
            CodeRepo {
                key: "core-async",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/core.async/6a0f4cfa2cdf27638a8a918181aeab0f7e06f3be/src/main/clojure/clojure/core/async.clj",
                ],
            },
            CodeRepo {
                key: "clojure-zip",
                urls: &[
                    "https://raw.githubusercontent.com/clojure/clojure/a3fa897590f70207eea3573759739810f2b6ab6c/src/clj/clojure/zip.clj",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["(defn ", "(defn- ", "(defmacro "]),
    },
    CodeLanguage {
        key: "r",
        display_name: "R",
        extensions: &[".r", ".R"],
        repos: &[
            CodeRepo {
                key: "shiny",
                urls: &[
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/bootstrap.R",
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/input-text.R",
                ],
            },
            CodeRepo {
                key: "ggplot2",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/ggplot2/c02c05aa6303e9592e37289d780224a06be5a27e/R/plot.R",
                    "https://raw.githubusercontent.com/tidyverse/ggplot2/c02c05aa6303e9592e37289d780224a06be5a27e/R/geom-point.R",
                ],
            },
            CodeRepo {
                key: "dplyr",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/mutate.R",
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/filter.R",
                ],
            },
            CodeRepo {
                key: "dplyr-extra",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/select.R",
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/join.R",
                ],
            },
            CodeRepo {
                key: "ggplot2-extra",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/ggplot2/c02c05aa6303e9592e37289d780224a06be5a27e/R/aes.R",
                    "https://raw.githubusercontent.com/tidyverse/ggplot2/c02c05aa6303e9592e37289d780224a06be5a27e/R/scale-colour.R",
                    "https://raw.githubusercontent.com/tidyverse/ggplot2/c02c05aa6303e9592e37289d780224a06be5a27e/R/theme.R",
                ],
            },
            CodeRepo {
                key: "tidyr",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/tidyr/cdaeeb352c73dce9ebf3a5d785a2ed5fd6c5fdbc/R/pivot-long.R",
                ],
            },
            CodeRepo {
                key: "purrr",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/purrr/90e74842bcd76d310e578351b27501408c5ed1f9/R/map.R",
                ],
            },
            CodeRepo {
                key: "stringr",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/stringr/ae054b1d28f630fee22ddb3cb7525396e62af4fe/R/modifiers.R",
                ],
            },
            CodeRepo {
                key: "shiny-reactives",
                urls: &[
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/reactives.R",
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/update-input.R",
                ],
            },
            CodeRepo {
                key: "shiny-inputs",
                urls: &[
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/input-select.R",
                    "https://raw.githubusercontent.com/rstudio/shiny/75a63716e578976965daeadde81af7166a50faac/R/input-slider.R",
                ],
            },
            CodeRepo {
                key: "readr",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/readr/9620048a5b79b27467285c089efdf019bcc1056e/R/read_delim.R",
                ],
            },
            CodeRepo {
                key: "tibble",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/tibble/8b004f20bda23ba4d1254222ff5adcd5da173d64/R/tibble.R",
                ],
            },
            CodeRepo {
                key: "dplyr-groupby",
                urls: &[
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/group-by.R",
                    "https://raw.githubusercontent.com/tidyverse/dplyr/f7c3788be32a90c83e705849c83bb111cd9d4a13/R/summarise.R",
                ],
            },
            CodeRepo {
                key: "devtools",
                urls: &[
                    "https://raw.githubusercontent.com/r-lib/devtools/07292edc8d475aa43a89f4f21053d36becb68093/R/install.R",
                ],
            },
            CodeRepo {
                key: "testthat",
                urls: &[
                    "https://raw.githubusercontent.com/r-lib/testthat/4b13712bb08bbcf19e3b5265e95218043c3d528e/R/expect-equality.R",
                    "https://raw.githubusercontent.com/r-lib/testthat/4b13712bb08bbcf19e3b5265e95218043c3d528e/R/expect-comparison.R",
                ],
            },
            CodeRepo {
                key: "rlang-env",
                urls: &[
                    "https://raw.githubusercontent.com/r-lib/rlang/0337a22ba3fdacf476c38f0cd8a85ec5b1e9e250/R/env.R",
                ],
            },
            CodeRepo {
                key: "rlang-arg",
                urls: &[
                    "https://raw.githubusercontent.com/r-lib/rlang/0337a22ba3fdacf476c38f0cd8a85ec5b1e9e250/R/arg.R",
                ],
            },
            CodeRepo {
                key: "data-table",
                urls: &[
                    "https://raw.githubusercontent.com/Rdatatable/data.table/8198bf0cd9ee40ea8b5d1b3c9baeff4919b7794d/R/data.table.R",
                    "https://raw.githubusercontent.com/Rdatatable/data.table/8198bf0cd9ee40ea8b5d1b3c9baeff4919b7794d/R/fread.R",
                ],
            },
            CodeRepo {
                key: "mlr3",
                urls: &[
                    "https://raw.githubusercontent.com/mlr-org/mlr3/0defc99bcc7c5338d0affc26cd8689e637da7383/R/Learner.R",
                    "https://raw.githubusercontent.com/mlr-org/mlr3/0defc99bcc7c5338d0affc26cd8689e637da7383/R/Task.R",
                ],
            },
            CodeRepo {
                key: "caret",
                urls: &[
                    "https://raw.githubusercontent.com/topepo/caret/c98cc1a3ba5f0b087d51f5c4362a3b751515e243/pkg/caret/R/train.default.R",
                    "https://raw.githubusercontent.com/topepo/caret/c98cc1a3ba5f0b087d51f5c4362a3b751515e243/pkg/caret/R/preProcess.R",
                ],
            },
        ],
        has_builtin: false,
        // R functions are defined as `name <- function(...)`. Since our extractor only
        // supports `starts_with`, we match roxygen doc blocks that precede functions.
        block_style: BlockStyle::Braces(&["#' "]),
    },
    CodeLanguage {
        key: "erlang",
        display_name: "Erlang",
        extensions: &[".erl"],
        repos: &[
            CodeRepo {
                key: "cowboy",
                urls: &[
                    "https://raw.githubusercontent.com/ninenines/cowboy/9f580ea964c4ea585aeeeb9f29613bf027d44269/src/cowboy_req.erl",
                    "https://raw.githubusercontent.com/ninenines/cowboy/9f580ea964c4ea585aeeeb9f29613bf027d44269/src/cowboy_http.erl",
                ],
            },
            CodeRepo {
                key: "rabbitmq",
                urls: &[
                    "https://raw.githubusercontent.com/rabbitmq/rabbitmq-server/ac254196aa0883a7254ef617768fcbcea86220a0/deps/rabbit/src/rabbit_channel.erl",
                ],
            },
            CodeRepo {
                key: "otp",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/lists.erl",
                ],
            },
            CodeRepo {
                key: "otp-gen-server",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/gen_server.erl",
                ],
            },
            CodeRepo {
                key: "otp-gen-statem",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/gen_statem.erl",
                ],
            },
            CodeRepo {
                key: "otp-ets",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/ets.erl",
                ],
            },
            CodeRepo {
                key: "otp-timer",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/timer.erl",
                ],
            },
            CodeRepo {
                key: "otp-erl-scan",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/erl_scan.erl",
                ],
            },
            CodeRepo {
                key: "otp-erl-lint",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/erl_lint.erl",
                ],
            },
            CodeRepo {
                key: "otp-io",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/io.erl",
                ],
            },
            CodeRepo {
                key: "otp-beam-lib",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/beam_lib.erl",
                ],
            },
            CodeRepo {
                key: "otp-dict",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/dict.erl",
                ],
            },
            CodeRepo {
                key: "otp-gen-event",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/gen_event.erl",
                ],
            },
            CodeRepo {
                key: "otp-supervisor",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/supervisor.erl",
                ],
            },
            CodeRepo {
                key: "otp-logger",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/kernel/src/logger.erl",
                ],
            },
            CodeRepo {
                key: "otp-file",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/kernel/src/file.erl",
                ],
            },
            CodeRepo {
                key: "otp-code",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/kernel/src/code.erl",
                ],
            },
            CodeRepo {
                key: "otp-ssl",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/ssl/src/ssl.erl",
                ],
            },
            CodeRepo {
                key: "otp-httpc",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/inets/src/http_client/httpc.erl",
                ],
            },
            CodeRepo {
                key: "emqx-channel",
                urls: &[
                    "https://raw.githubusercontent.com/emqx/emqx/0173360ca3c5a7087ff8d83a813c9ec65858624a/apps/emqx/src/emqx_channel.erl",
                ],
            },
            CodeRepo {
                key: "emqx-session",
                urls: &[
                    "https://raw.githubusercontent.com/emqx/emqx/0173360ca3c5a7087ff8d83a813c9ec65858624a/apps/emqx/src/emqx_session.erl",
                ],
            },
            CodeRepo {
                key: "otp-filename",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/filename.erl",
                ],
            },
            CodeRepo {
                key: "otp-binary",
                urls: &[
                    "https://raw.githubusercontent.com/erlang/otp/f67aa9da945f01315fcfd2d6ac2a90909a34afa4/lib/stdlib/src/binary.erl",
                ],
            },
            CodeRepo {
                key: "ejabberd",
                urls: &[
                    "https://raw.githubusercontent.com/processone/ejabberd/27b87d4a8af84cab619fa33095b42cb03f9e6020/src/ejabberd_c2s.erl",
                    "https://raw.githubusercontent.com/processone/ejabberd/27b87d4a8af84cab619fa33095b42cb03f9e6020/src/ejabberd_router.erl",
                ],
            },
            CodeRepo {
                key: "vernemq",
                urls: &[
                    "https://raw.githubusercontent.com/vernemq/vernemq/17550aa92c2dd9f5a11862dc843d1981f12a7e0c/apps/vmq_server/src/vmq_mqtt_fsm.erl",
                    "https://raw.githubusercontent.com/vernemq/vernemq/17550aa92c2dd9f5a11862dc843d1981f12a7e0c/apps/vmq_server/src/vmq_queue.erl",
                ],
            },
            CodeRepo {
                key: "couchdb",
                urls: &[
                    "https://raw.githubusercontent.com/apache/couchdb/72936f80eca5e99ce00426085f7608391862db3a/src/couch/src/couch_db.erl",
                    "https://raw.githubusercontent.com/apache/couchdb/72936f80eca5e99ce00426085f7608391862db3a/src/couch/src/couch_btree.erl",
                ],
            },
        ],
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
        repos: &[
            CodeRepo {
                key: "nextflow",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/processor/TaskProcessor.groovy",
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/Session.groovy",
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/Channel.groovy",
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/processor/TaskConfig.groovy",
                ],
            },
            CodeRepo {
                key: "codenarc",
                urls: &[
                    "https://raw.githubusercontent.com/CodeNarc/CodeNarc/0eb23cff04cbf71794c8852e2ea25ac1fe81b36c/src/main/groovy/org/codenarc/CodeNarcRunner.groovy",
                ],
            },
            CodeRepo {
                key: "spock",
                urls: &[
                    "https://raw.githubusercontent.com/spockframework/spock/b71e3d7590dae28d608aa92f90b45bef33aaeda8/spock-core/src/main/groovy/spock/util/EmbeddedSpecRunner.groovy",
                    "https://raw.githubusercontent.com/spockframework/spock/b71e3d7590dae28d608aa92f90b45bef33aaeda8/spock-core/src/main/groovy/spock/util/EmbeddedSpecCompiler.groovy",
                ],
            },
            CodeRepo {
                key: "nextflow-runner",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/script/ScriptRunner.groovy",
                ],
            },
            CodeRepo {
                key: "nextflow-executor",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/executor/Executor.groovy",
                ],
            },
            CodeRepo {
                key: "nextflow-core",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/Nextflow.groovy",
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/script/WorkflowMetadata.groovy",
                ],
            },
            CodeRepo {
                key: "nextflow-trace",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nextflow/src/main/groovy/nextflow/trace/TraceRecord.groovy",
                ],
            },
            CodeRepo {
                key: "grails",
                urls: &[
                    "https://raw.githubusercontent.com/grails/grails-core/13617a4a4a78894146f6b3b85b15e1a52e29c5b8/grails-core/src/main/groovy/grails/boot/GrailsApp.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-execution",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/services/rundeck/services/ExecutionService.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-scheduled",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/services/rundeck/services/ScheduledExecutionService.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-exec-controller",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/controllers/rundeck/controllers/ExecutionController.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-sched-controller",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/controllers/rundeck/controllers/ScheduledExecutionController.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-framework",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/services/rundeck/services/FrameworkService.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-project",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/services/rundeck/services/ProjectService.groovy",
                ],
            },
            CodeRepo {
                key: "rundeck-logfile",
                urls: &[
                    "https://raw.githubusercontent.com/rundeck/rundeck/4c70fb21fb1dbf58beb91fb64ca8265e5849f857/rundeckapp/grails-app/services/rundeck/services/LogFileStorageService.groovy",
                ],
            },
            CodeRepo {
                key: "nextflow-file-helper",
                urls: &[
                    "https://raw.githubusercontent.com/nextflow-io/nextflow/2e1d42ef84a74998af34ef0c41b4afbae6c41d17/modules/nf-commons/src/main/nextflow/file/FileHelper.groovy",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Braces(&["def ", "void ", "static ", "public ", "private "]),
    },
    CodeLanguage {
        key: "fsharp",
        display_name: "F#",
        extensions: &[".fs", ".fsx"],
        repos: &[
            CodeRepo {
                key: "fsharp-compiler",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/Compiler/Utilities/lib.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-core",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/list.fs",
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/map.fs",
                ],
            },
            CodeRepo {
                key: "Saturn",
                urls: &[
                    "https://raw.githubusercontent.com/SaturnFramework/Saturn/4ae4d6126a6c7c0e91bded2ddbe1103030010c78/src/Saturn/Router.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-array",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/array.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-seq",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/seq.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-option",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/option.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-result",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/result.fs",
                ],
            },
            CodeRepo {
                key: "fsharp-async",
                urls: &[
                    "https://raw.githubusercontent.com/dotnet/fsharp/68fb63daa05d38077a4ba32740ef7deda7ebded2/src/FSharp.Core/async.fs",
                ],
            },
            CodeRepo {
                key: "giraffe",
                urls: &[
                    "https://raw.githubusercontent.com/giraffe-fsharp/Giraffe/ebe4a39fc6b6dd8d532eefaca299742c38d98711/src/Giraffe/Routing.fs",
                    "https://raw.githubusercontent.com/giraffe-fsharp/Giraffe/ebe4a39fc6b6dd8d532eefaca299742c38d98711/src/Giraffe/Core.fs",
                    "https://raw.githubusercontent.com/giraffe-fsharp/Giraffe/ebe4a39fc6b6dd8d532eefaca299742c38d98711/src/Giraffe/FormatExpressions.fs",
                ],
            },
            CodeRepo {
                key: "fable",
                urls: &[
                    "https://raw.githubusercontent.com/fable-compiler/Fable/098f8700a019854fec2d21d2857afa2dc0d48c26/src/Fable.Transforms/FableTransforms.fs",
                    "https://raw.githubusercontent.com/fable-compiler/Fable/098f8700a019854fec2d21d2857afa2dc0d48c26/src/Fable.Transforms/FSharp2Fable.fs",
                ],
            },
            CodeRepo {
                key: "farmer",
                urls: &[
                    "https://raw.githubusercontent.com/CompositionalIT/farmer/1df037941bf30e1a20bc58ada939d32f2a34c982/src/Farmer/Common.fs",
                    "https://raw.githubusercontent.com/CompositionalIT/farmer/1df037941bf30e1a20bc58ada939d32f2a34c982/src/Farmer/Builders/Builders.WebApp.fs",
                ],
            },
        ],
        has_builtin: false,
        block_style: BlockStyle::Indentation(&["let ", "member ", "type ", "module "]),
    },
    CodeLanguage {
        key: "objective-c",
        display_name: "Objective-C",
        extensions: &[".m", ".h"],
        repos: &[
            CodeRepo {
                key: "afnetworking",
                urls: &[
                    "https://raw.githubusercontent.com/AFNetworking/AFNetworking/d9f589cc2c1fe9d55eb5eea00558010afea7a41e/AFNetworking/AFURLSessionManager.m",
                ],
            },
            CodeRepo {
                key: "sdwebimage",
                urls: &[
                    "https://raw.githubusercontent.com/SDWebImage/SDWebImage/2de3a496eaf6df9a1312862adcfd54acd73c39c0/SDWebImage/Core/SDWebImageManager.m",
                ],
            },
            CodeRepo {
                key: "realm-cocoa",
                urls: &[
                    "https://raw.githubusercontent.com/realm/realm-cocoa/c22f9303d446fc3c044e4c151ed19597464b6bd7/Realm/RLMRealm.mm",
                ],
            },
            CodeRepo {
                key: "afnetworking-http",
                urls: &[
                    "https://raw.githubusercontent.com/AFNetworking/AFNetworking/d9f589cc2c1fe9d55eb5eea00558010afea7a41e/AFNetworking/AFHTTPSessionManager.m",
                ],
            },
            CodeRepo {
                key: "sdwebimage-cache",
                urls: &[
                    "https://raw.githubusercontent.com/SDWebImage/SDWebImage/2de3a496eaf6df9a1312862adcfd54acd73c39c0/SDWebImage/Core/SDImageCache.m",
                ],
            },
            CodeRepo {
                key: "iterm2",
                urls: &[
                    "https://raw.githubusercontent.com/gnachman/iTerm2/1d997ee0082b3ad413366253d2868f7665b8c25f/sources/iTermController.m",
                    "https://raw.githubusercontent.com/gnachman/iTerm2/1d997ee0082b3ad413366253d2868f7665b8c25f/sources/PTYSession.m",
                    "https://raw.githubusercontent.com/gnachman/iTerm2/1d997ee0082b3ad413366253d2868f7665b8c25f/sources/VT100Terminal.m",
                ],
            },
            CodeRepo {
                key: "mbprogresshud",
                urls: &[
                    "https://raw.githubusercontent.com/jdg/MBProgressHUD/4a7c5f3e53cdea77c5dcb8578c2ee5acacdf6781/MBProgressHUD.m",
                ],
            },
            CodeRepo {
                key: "cocoalumberjack",
                urls: &[
                    "https://raw.githubusercontent.com/CocoaLumberjack/CocoaLumberjack/f817a936d8ff9ddc7e3cbb2dcf0eb7a6f75c6e44/Sources/CocoaLumberjack/DDLog.m",
                ],
            },
            CodeRepo {
                key: "fmdb",
                urls: &[
                    "https://raw.githubusercontent.com/ccgus/fmdb/d3abf748a2788471535993286603322ccdd02c3d/src/fmdb/FMDatabase.m",
                ],
            },
            CodeRepo {
                key: "mantle",
                urls: &[
                    "https://raw.githubusercontent.com/Mantle/Mantle/2a8e2123a3931038179ee06105c9e6ec336b12ea/Mantle/MTLJSONAdapter.m",
                ],
            },
            CodeRepo {
                key: "reactive-objc",
                urls: &[
                    "https://raw.githubusercontent.com/ReactiveCocoa/ReactiveObjC/1af6617f007cae727dce48d2031cc1e66d06f04a/ReactiveObjC/RACSignal.m",
                ],
            },
            CodeRepo {
                key: "masonry",
                urls: &[
                    "https://raw.githubusercontent.com/Masonry/Masonry/8bd77ea92bbe995e14c454f821200b222e5a8804/Masonry/MASConstraintMaker.m",
                    "https://raw.githubusercontent.com/Masonry/Masonry/8bd77ea92bbe995e14c454f821200b222e5a8804/Masonry/MASViewConstraint.m",
                ],
            },
        ],
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
    let sources_path =
        std::path::Path::new(cache_dir).join(format!("{}_{}.sources.txt", language_key, repo.key));
    let combined = all_snippets.join("\n---SNIPPET---\n");
    if fs::write(cache_path, combined).is_err() {
        return false;
    }

    let mut sources = String::from(
        "Downloaded snippet sources for keydr code drill cache.\n\
         Upstream licenses remain with original repositories.\n\
         Source URLs:\n",
    );
    for url in repo.urls {
        sources.push_str("- ");
        sources.push_str(url);
        sources.push('\n');
    }
    let _ = fs::write(sources_path, sources);
    true
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
