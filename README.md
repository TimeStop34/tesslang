# tesslang

A tiny, zero-dependency-on-runtime configuration language for Tessera

`tesslang` is to Tessera what `serde_json` is to JSON: a small crate that turns a text config file into any `serde`-deserializable Rust structure. It has an HCL-inspired syntax — blocks, arrays, inline objects, `$variables`, percent values, comments — and compiles to a plain `Value` tree, which is then handed to `serde` through a hand-written `Deserializer`.

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

- **Reads like HCL.** Blocks, arrays, inline objects, `$variables`, `#` comments — familiar if you've touched Terraform, Nomad, or Cue.
- **Plain `serde` under the hood.** No custom derive macros, no runtime, no reflection. If it works with `serde_json`, it works here.
- **Percent literals.** `60%` is `0.6` for `f64` / `i64`, or a distinct variant in your own enum.
- **Small.** ~700 lines of Rust. One dependency: `serde`.
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
| String | `"SUPER"` | Double quotes. Escapes: `\"`, `\\`, `\n`, `\t`, `\$` |
| Number | `10`, `-30`, `0.5`, `3.14` | Integer or float, signed |
| Percent | `60%`, `-12.5%` | `%` directly after the number. Deserialized as `0.6` / `-0.125` into `f64`/`i64`; see `IntegerOrPercent` to keep the distinction |
| Boolean | `true`, `false` | |
| Array | `[ "a", "b", 10 ]` | Comma-separated; newline also acts as separator |
| Object (inline) | `{ id = 1, layout = "bsp" }` | Same grammar as blocks — see below |
| Variable | `$mod` | Declared with `$mod = "SUPER"` |

### Statements

Every statement is either on its own line or separated by a comma. Both forms are equivalent — there is no separate "inline mode".

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

# 4. Inline object — same grammar as a block, just used as a value
line = { id = 1, next_window_ratio = 0.5 }
```

Inside a block or an inline object, fields may be separated by newlines **or** by commas. Mixing is fine:

```hcl
general {
    gaps = 10,
    border_width = 2
    border_color_active = "#5e81ac",
}
```

Arrays accept the same optional-comma rule:

```hcl
bind = [
    [ $mod, "Return", "exec", "alacritty" ],
    [ $mod, "W",      "killactive" ],
    [ $mod, "1",      "switch-workspace", 1 ],
]
```

### Full example

```hcl
# ~/.config/tessera/tessera.conf

$mod  = "SUPER"
$term = "alacritty"

general {
    gaps                  = 10
    default_window_width_ratio = 0.5
    border_width          = 2
    border_color_active   = "#5e81ac"
    border_color_inactive = "#4c566a"
}

layout {
    mode        = "workspaces"   # "line", "workspaces" или "floating"
    gaps        = 10
    margin_gaps = 15

    # Используется, если mode = "line"
    lines = [
        { id = 1, next_window_ratio = 0.5 },
        { id = 2, next_window_ratio = 0.3 },
    ]

    # Используется, если mode = "workspaces"
    workspaces = [
        { id = 1, layout = "bsp" },
        { id = 2, layout = "master-stack", master_ratio = 0.6, master_count = 1 },
        { id = 3, layout = "monocle" },
    ]
}

bind = [
    [ $mod, "Return", "exec", "alacritty" ],
    [ $mod, "W",      "killactive" ],
    [ $mod, "V",      "hide" ],
    [ $mod, "1",      "switch-workspace", 1 ],
    [ $mod, "2",      "switch-workspace", 2 ],
]
```

## Semantics

- **Variables** are substituted at parse time. Every `$name` is replaced by the value bound to it. Using an undefined variable is an error.
- **Variables are declared once, at top level.** They can be used anywhere (including inside blocks, inline objects and arrays) as long as they're declared *before* the point of use.
- **Blocks** are just nested objects. `general { gaps = 10 }` is exactly `general = { gaps: 10 }`.
- **Inline objects** use the same grammar as blocks. `{ id = 1, layout = "bsp" }` is a regular object value.
- **Blocks can nest.** `a { b { c = 1 } }` deserializes into `struct A { b: B }`, `struct B { c: i64 }`.
- **Arrays are ordered.** They can contain any values, including nested arrays (`bind` is an array of arrays).
- **Commas are optional separators.** Inside blocks, inline objects and arrays a `,` may appear after each element. Newlines work the same way. Trailing commas are allowed.
- **Percents** (`60%`) are a distinct `Value` variant. See the *Percents* section below for how they deserialize.
- **Comments** start with `#` and run to end of line.
- **No expressions.** No arithmetic, no conditionals, no function calls. This is a configuration language, not a scripting one.

## API

```rust
pub fn from_str<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, Error>;
pub fn from_file<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error>;

pub struct Error { /* ... */ }

pub enum Value {
    String(String),
    Number(f64),
    Percent(f64),
    Bool(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

pub struct UnstrictString(pub String);

pub enum IntegerOrPercent {
    Integer(f64),
    Percent(f64),
}
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

### Percents

`60%` is a value of its own kind. How it deserializes depends on the target type:

- Into `f64` / `f32` / `i*` / `u*` — the percentage is **evaluated**:
  `60%` → `0.6`, `-12.5%` → `-0.125`, `100%` → `1.0`, `50%` into `u8` → `0`.
- Into `tesslang::IntegerOrPercent` — the distinction is **preserved**:

```rust
use tesslang::IntegerOrPercent;

#[derive(Deserialize)]
struct Sizing {
    width: f64,                       // 60%  ->  0.6
    height: IntegerOrPercent,         // 60%  ->  Percent(0.6)
    margin: IntegerOrPercent,         // 60   ->  Integer(60.0)
}

let s: Sizing = tesslang::from_str(
    "width = 60%\nheight = 60%\nmargin = 60"
).unwrap();

assert_eq!(s.width, 0.6);
assert_eq!(s.height, IntegerOrPercent::Percent(0.6));
assert_eq!(s.margin, IntegerOrPercent::Integer(60.0));
```

This is the pattern Tessera uses to distinguish *pixels* (plain number) from *screen fraction* (percent).

`IntegerOrPercent` also has `as_f64()` which returns the underlying value (`60` or `0.6`).

### Strings accept numbers and booleans

To make `bind = [ [ $mod, "1", "scroll", -30 ] ]` work with `Vec<Vec<String>>`, `deserialize_string` converts numbers and booleans (and percents) to their textual form (`-30` → `"-30"`, `true` → `"true"`, `60%` → `"60%"`). If you want strictness, use a custom enum (see below) or `UnstrictString` for explicit opt-in.

### `UnstrictString`

`UnstrictString` is an explicit opt-in for the same lenient behaviour: it accepts a string, number, boolean, or percent, and always yields a `String`.

```rust
use tesslang::UnstrictString;

#[derive(Deserialize)]
struct Config {
    bind: Vec<Vec<UnstrictString>>,
    name: UnstrictString,   // "x" or 42 or true all accepted
}
```

Internally `UnstrictString` uses `deserialize_any`, so it works equally well for `bind = [ [ $mod, 1, "switch-workspace", 2 ] ]` as for `bind = [ [ $mod, "1", "switch-workspace", "2" ] ]`.

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
percent sign '%' must directly follow a number (e.g. 60%)
expected number (u32), got Number(3.14)
```

## Limitations (by design)

- **No serialization.** This crate only reads config. No `to_string`, no `Serializer`. Config is not meant to be written back by the program.
- **No expressions.** No `1 + 2`, no conditionals, no function calls.
- **Variables are top-level only.** You cannot declare `$x` inside a block.
- **Variables must be declared before use.** No forward references.
- **Percent is a literal, not an operator.** `60 %` (with a space) is a syntax error; the `%` must follow the number directly.

## Architecture

```
input  ──▶  Lexer  ──▶  tokens  ──▶  Parser  ──▶  AST  ──▶  Value  ──▶  Deserializer  ──▶  T: Deserialize
```

- **Lexer** — hand-written, tracks `line`/`column` for every token. Handles escapes and `$interpolation` inside strings, and emits `Token::Percent` for `60%`.
- **Parser** — recursive descent. Substitutes `$variables` at parse time into `Value` nodes. A single `parse_statements_until(close)` routine handles the file root, `key { ... }` blocks, and `{ ... }` inline objects; a `,` between statements is an optional separator.
- **AST** — a thin layer (`Statement::Assignment`, `Statement::Block`) that `into_value()` collapses into a `HashMap<String, Value>`.
- **Deserializer** — a manual `serde::Deserializer` impl for `Value`, roughly following what `serde_json::Value` does but shorter. `Percent` is deserialized as `0.6` for floats/ints, and as the marker string `"60%"` for `deserialize_any` (so that `IntegerOrPercent` and `UnstrictString` can tell it apart from a plain number).

There is no `serde-value` dependency; the `Value` → `Deserializer` bridge is implemented directly.

## License
[AGPL v3.0 only](LICENSE)
