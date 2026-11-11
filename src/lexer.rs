use crate::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Identifier(String),
    String(String),
    Number(f64),
    Bool(bool),
    Variable(String),
    OpenBrace,
    CloseBrace,
    OpenBracket,
    CloseBracket,
    Comma,
    Equals,
    Newline,
    Eof,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self { chars: input.chars().collect(), pos: 0, line: 1, column: 1 }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if let Some(c) = c {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
        c
    }

    fn error(&self, msg: impl Into<String>) -> Error {
        Error::at(msg, self.line, self.column)
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Error> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\r' => {
                    self.bump();
                }
                '\n' => {
                    tokens.push(Token::Newline);
                    self.bump();
                }
                '#' => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                '{' => { tokens.push(Token::OpenBrace); self.bump(); }
                '}' => { tokens.push(Token::CloseBrace); self.bump(); }
                '[' => { tokens.push(Token::OpenBracket); self.bump(); }
                ']' => { tokens.push(Token::CloseBracket); self.bump(); }
                ',' => { tokens.push(Token::Comma); self.bump(); }
                '=' => { tokens.push(Token::Equals); self.bump(); }
                '$' => {
                    self.bump();
                    let mut name = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            name.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    if name.is_empty() {
                        return Err(self.error("expected variable name after '$'"));
                    }
                    tokens.push(Token::Variable(name));
                }
                '"' => {
                    self.bump();
                    let mut s = String::new();
                    loop {
                        match self.bump() {
                            Some('"') => break,
                            Some('\\') => match self.bump() {
                                Some('"') => s.push('"'),
                                Some('\\') => s.push('\\'),
                                Some('n') => s.push('\n'),
                                Some('t') => s.push('\t'),
                                Some(other) => {
                                    return Err(self.error(format!("unknown escape: \\{other}")))
                                }
                                None => return Err(self.error("unterminated string")),
                            },
                            Some(ch) => s.push(ch),
                            None => return Err(self.error("unterminated string")),
                        }
                    }
                    tokens.push(Token::String(s));
                }
                c if c.is_ascii_digit() || c == '-' => {
                    let mut s = String::new();
                    s.push(c);
                    self.bump();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() || c == '.' {
                            s.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    let n: f64 = s
                        .parse()
                        .map_err(|_| self.error(format!("invalid number: {s}")))?;
                    tokens.push(Token::Number(n));
                }
                c if c.is_alphabetic() || c == '_' => {
                    let mut s = String::new();
                    while let Some(c) = self.peek() {
                        if c.is_alphanumeric() || c == '_' {
                            s.push(c);
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    match s.as_str() {
                        "true" => tokens.push(Token::Bool(true)),
                        "false" => tokens.push(Token::Bool(false)),
                        _ => tokens.push(Token::Identifier(s)),
                    }
                }
                other => {
                    return Err(self.error(format!("unexpected character: {other:?}")));
                }
            }
        }

        tokens.push(Token::Eof);
        Ok(tokens)
    }
}