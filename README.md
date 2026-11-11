# tesslang

A tiny, zero-dependency-on-runtime configuration language for Tessera

`tesslang` is to Tessera what `serde_json` is to JSON: a small crate that turns a text config file into any `serde`-deserializable Rust structure. It has an HCL-inspired syntax — blocks, arrays, `$variables`, comments — and compiles to a plain `Value` tree, which is then handed to `serde` through a hand-written `Deserializer`.

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct Config {
    general: General,
    bind: Vec<Vec<String>>,
}

#[derive(Deserialize)]
struct General {
    gaps: f64,
    border_color_active: String,
}

fn main() -> Result<(), tesslang::Error> {
    let cfg: Config = tesslang::from_file("tessera.conf")?;
    println!("gaps = {}", cfg.general.gaps);
    Ok(())
}
```

## Why another config language?

- **Reads like HCL.** Blocks, arrays, `$variables`, `#` comments — familiar if you've touched Terraform, Nomad, or Cue.
- **Plain `serde` under the hood.** No custom derive macros, no runtime, no reflection. If it works with `serde_json`, it works here.
- **Small.** ~600 lines of Rust. One dependency: `serde`.
- **Errors with positions.** The lexer tracks `line:column`, so a typo points to the exact spot.

## Installation

```toml
[dependencies]
tesslang = { path = "../tesslang" }
serde = { version = "1", features = ["derive"] }
```

or via git:

```toml
[dependencies]
tesslang = { git = "https://github.com/TimeStop34/tesslang" }
serde = { version = "1", features = ["derive"] }
```


## Language

### Values

| Type | Example | Notes |
|---|---|---|
| String | `"SUPER"` | Double quotes. Escapes: `\"`, `\\`, `\n`, `\t` |
| Number | `10`, `-30`, `0.5`, `3.14` | Integer or float, signed |
| Boolean | `true`, `false` | |
| Array | `[ "a", "b", 10 ]` | Comma-separated; newline also acts as separator |
| Variable | `$mod` | Declared with `$mod = "SUPER"` |

### Statements

Every statement is on its own line.

```hcl
# 1. Variable declaration (top-level only)
$mod = "SUPER"

# 2. Field assignment
gaps = 10

# 3. Block (nested field set)
general {
    gaps = 10
    default_window_width_ratio = 0.5
}
```

### Full example

```hcl
# ~/.config/tessera/tessera.conf

$mod  = "SUPER"
$term = "alacritty"

general {
    gaps                        = 10
    default_window_width_ratio  = 0.5
    border_width                = 2
    border_color_active         = "#5e81ac"
    border_color_inactive       = "#4c566a"
}

layout {
    mode                 = "line"
    scroll_speed         = 30.0
    auto_scroll_to_new   = true
}

bind = [
    [ $mod, "Q",      "killactive" ],
    [ $mod, "Return", "exec", $term ],
    [ $mod, "1",      "scroll", -30 ],
    [ $mod, "2",      "scroll",  30 ],
    [ $mod, "CTRL",   "G", "toggle-layout" ],
]
```

## Semantics

- **Variables** are substituted at parse time. Every `$name` is replaced by the value bound to it. Using an undefined variable is an error.
- **Variables are declared once, at top level.** They can be used anywhere (including inside blocks and arrays) as long as they're declared *before* the point of use.
- **Blocks** are just nested objects. `general { gaps = 10 }` is exactly `general = { gaps: 10 }`.
- **Blocks can nest.** `a { b { c = 1 } }` deserializes into `struct A { b: B }`, `struct B { c: i64 }`.
- **Arrays are ordered.** They can contain any values, including nested arrays (`bind` is an array of arrays).
- **Comments** start with `#` and run to end of line.
- **No expressions.** No arithmetic, no conditionals, no function calls. This is a configuration language, not a scripting one.

## API

```rust
pub fn from_str<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, Error>;
pub fn from_file<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error>;

pub struct Error { /* ... */ }
pub enum Value { String(String), Number(f64), Bool(bool), Array(Vec<Value>), Object(HashMap<String, Value>) }
```

Both functions return `Result<T, tesslang::Error>`. `Error` implements `Display` and `std::error::Error`, and — when the failure comes from the lexer or parser — carries `line`/`column` position information.

`tesslang::Value` is exposed for two reasons:

1. As a target for `#[serde(flatten)]` fields (see below).
2. As a way to inspect a config without committing to a Rust struct.

## Mapping to Rust

The mapping is straightforward and matches the shape of the file:

```hcl
general { gaps = 10 }
```

```rust
#[derive(Deserialize)]
struct Config { general: General }

#[derive(Deserialize)]
struct General { gaps: f64 }
```

### Numbers are flexible

An integer in the file can land in `i8`, `i16`, `i32`, `i64`, `u8`, `u16`, `u32`, `u64`, `f32`, or `f64`. Integers are checked for range; `3.14` into `u32` is an error.

### Strings accept numbers and booleans

To make `bind = [ [ $mod, "1", "scroll", -30 ] ]` work with `Vec<Vec<String>>`, `deserialize_string` converts numbers and booleans to their textual form (`-30` → `"-30"`, `true` → `"true"`). If you want strictness, use a custom enum (see below).

### Custom enum for heterogeneous arrays

```rust
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum BindValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

pub struct Bind(pub Vec<BindValue>);
```

Now `[ $mod, "1", "scroll", -30 ]` deserializes into `[Str("SUPER"), Str("1"), Str("scroll"), Num(-30.0)]`.

### `#[serde(default)]`, aliases, `flatten`

Everything `serde` normally provides works here:

```rust
#[derive(Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: General,
    #[serde(alias = "layout_mode")]
    pub layout: Option<Layout>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, tesslang::Value>,
}
```

### Enums

Standard serde conventions apply:

```rust
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Action {
    Killactive,
    ToggleLayout,
    Exec(String),
}
```

```hcl
action = "killactive"             # Action::Killactive
action = { exec = "alacritty" }   # Action::Exec("alacritty".into())
```

## Error handling

```rust
match tesslang::from_file::<Config>("tessera.conf") {
    Ok(cfg)  => { /* use cfg */ }
    Err(e)   => eprintln!("config error: {e}"),
}
```

Example error messages:

```
line 3:12: unexpected character: '?'
line 8:5:  undefined variable: $nope
line 5:10: expected '=' or '{' after 'gaps', found String("x")
expected number (u32), got Number(3.14)
```

## Limitations (by design)

- **No serialization.** This crate only reads config. No `to_string`, no `Serializer`. Config is not meant to be written back by the program.
- **No string interpolation.** `$x` is a whole value; it isn't spliced inside `"..."`.
- **No expressions.** No `1 + 2`, no conditionals, no function calls.
- **Variables are top-level only.** You cannot declare `$x` inside a block.
- **Variables must be declared before use.** No forward references.

## Architecture

```
input  ──►  Lexer  ──►  tokens  ──►  Parser  ──►  AST  ──►  Value  ──►  Deserializer  ──►  T: Deserialize
```

- **Lexer** — hand-written, tracks `line`/`column` for every token.
- **Parser** — recursive descent. Substitutes `$variables` at parse time into `Value` nodes.
- **AST** — a thin layer (`Statement::Assignment`, `Statement::Block`) that `into_value()` collapses into a `HashMap<String, Value>`.
- **Deserializer** — a manual `serde::Deserializer` impl for `Value`, roughly following what `serde_json::Value` does but shorter.

There is no `serde-value` dependency; the `Value` → `Deserializer` bridge is implemented directly.

## License
[AGPL v3.0 only](LICENSE)
