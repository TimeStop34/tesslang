mod ast;
mod de;
mod lexer;
mod parser;

pub use ast::Value;

use std::fmt;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), line: None, column: None }
    }

    pub fn at(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self { message: message.into(), line: Some(line), column: Some(column) }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.column) {
            (Some(l), Some(c)) => write!(f, "line {l}:{c}: {}", self.message),
            _ => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for Error {}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(msg.to_string())
    }
}

pub fn from_str<T: serde::de::DeserializeOwned>(input: &str) -> Result<T, Error> {
    let mut parser = parser::Parser::new(input)?;
    let ast = parser.parse()?;
    let value = ast.into_value()?;
    T::deserialize(value)
}

pub fn from_file<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, Error> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .map_err(|e| Error::new(format!("failed to read {}: {}", path.display(), e)))?;
    from_str(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct Config {
        general: General,
        layout: Layout,
        bind: Vec<Vec<String>>,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct General {
        gaps: f64,
        border_width: u32,
        border_color_active: String,
    }

    #[derive(Deserialize, Debug, PartialEq)]
    struct Layout {
        mode: String,
        scroll_speed: f64,
        auto_scroll_to_new: bool,
    }

    #[test]
    fn full_config() {
        let input = r##"
            # комментарий
            $mod = "SUPER"
            $term = "alacritty"

            general {
                gaps = 10
                border_width = 2
                border_color_active = "#5e81ac"
            }

            layout {
                mode = "line"
                scroll_speed = 30.0
                auto_scroll_to_new = true
            }

            bind = [
                [ $mod, "Q", "killactive" ],
                [ $mod, "Return", "exec", $term ],
                [ $mod, "1", "scroll", -30 ],
                [ $mod, "2", "scroll", 30 ],
            ]
        "##;

        let cfg: Config = from_str(input).unwrap();
        assert_eq!(cfg.general.gaps, 10.0);
        assert_eq!(cfg.general.border_width, 2);
        assert_eq!(cfg.layout.scroll_speed, 30.0);
        assert!(cfg.layout.auto_scroll_to_new);
        assert_eq!(cfg.bind[0], vec!["SUPER", "Q", "killactive"]);
        assert_eq!(cfg.bind[1], vec!["SUPER", "Return", "exec", "alacritty"]);
        assert_eq!(cfg.bind[2], vec!["SUPER", "1", "scroll", "-30"]);
    }

    #[test]
    fn undefined_variable_errors() {
        let err = from_str::<serde::de::IgnoredAny>("a = $nope").unwrap_err();
        assert!(err.to_string().contains("undefined variable"));
        // Позиция должна указывать на сам `$nope`, а не на соседние токены.
        assert_eq!(err.line, Some(1));
        assert_eq!(err.column, Some(5));
    }

    #[test]
    fn nested_blocks() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct C { a: A }
        #[derive(Deserialize, Debug, PartialEq)]
        struct A { b: B }
        #[derive(Deserialize, Debug, PartialEq)]
        struct B { x: i64 }

        let c: C = from_str("a { b { x = 7 } }").unwrap();
        assert_eq!(c.a.b.x, 7);
    }

    #[test]
    fn empty_object() {
        #[derive(Deserialize, Debug)]
        struct C { #[allow(dead_code)] a: std::collections::HashMap<String, Value> }
        let _: C = from_str("a { }").unwrap();
    }

    /// Ошибки парсера теперь содержат позицию.
    /// Раньше `expected '=' or '{' after 'b', found ...` приходило без line/column.
    #[test]
    fn parser_error_includes_position() {
        let err = from_str::<serde::de::IgnoredAny>("a = 1\nb c\n").unwrap_err();
        assert!(err.message.contains("after 'b'"), "message = {}", err.message);
        // `c` находится на второй строке, третьем столбце.
        assert_eq!(err.line, Some(2));
        assert_eq!(err.column, Some(3));
    }

    /// `Option<T>` для отсутствующего поля должен быть `None`.
    /// `deserialize_option` вызывается только когда значение *есть*,
    /// поэтому `visit_some(self)` корректен.
    #[test]
    fn option_missing_field_is_none() {
        #[derive(Deserialize, Debug)]
        struct C {
            #[allow(dead_code)]
            a: Option<i64>,
            #[allow(dead_code)]
            b: i64,
        }
        let c: C = from_str("b = 1").unwrap();
        assert_eq!(c.a, None);
    }

    /// Все escape-последовательности, которые поддерживает лексер.
    #[test]
    fn string_escapes() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct C { s: String }

        let c: C = from_str(r#"s = "line1\nline2""#).unwrap();
        assert_eq!(c.s, "line1\nline2");

        let c: C = from_str(r#"s = "a\tb""#).unwrap();
        assert_eq!(c.s, "a\tb");

        let c: C = from_str(r#"s = "say \"hi\"""#).unwrap();
        assert_eq!(c.s, "say \"hi\"");

        let c: C = from_str(r#"s = "back\\slash""#).unwrap();
        assert_eq!(c.s, "back\\slash");
    }

    /// Отрицательные числа и числа с плавающей точкой.
    #[test]
    fn number_parsing() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct C {
            neg_int: i64,
            neg_float: f64,
            pos_float: f64,
            zero: f64,
        }

        let c: C = from_str(
            "neg_int = -30\nneg_float = -3.14\npos_float = 3.14\nzero = 0",
        )
        .unwrap();
        assert_eq!(c.neg_int, -30);
        assert_eq!(c.neg_float, -3.14);
        assert_eq!(c.pos_float, 3.14);
        assert_eq!(c.zero, 0.0);

        // `-3.14` в `u32` должно быть ошибкой диапазона, а не «тихо пройти».
        #[derive(Deserialize, Debug)]
        struct U { #[allow(dead_code)] x: u32 }
        assert!(from_str::<U>("x = -3.14").is_err());
    }
}