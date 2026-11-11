use std::collections::HashMap;

use crate::ast::{Ast, Statement, Value};
use crate::lexer::{Lexer, Token};
use crate::Error;

pub struct Parser {
    tokens: Vec<Token>,
    positions: Vec<(usize, usize)>,
    pos: usize,
    variables: HashMap<String, Value>,
}

impl Parser {
    pub fn new(input: &str) -> Result<Self, Error> {
        let (tokens, positions) = Lexer::new(input).tokenize()?;
        Ok(Self { tokens, positions, pos: 0, variables: HashMap::new() })
    }

    pub fn parse(&mut self) -> Result<Ast, Error> {
        let mut statements = Vec::new();
        self.skip_newlines();
        while self.current() != &Token::Eof {
            statements.push(self.parse_statement()?);
            self.skip_newlines();
        }
        Ok(Ast { statements })
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn position(&self) -> (usize, usize) {
        self.positions.get(self.pos).copied().unwrap_or((0, 0))
    }

    fn error(&self, message: impl Into<String>) -> Error {
        let (line, column) = self.position();
        Error::at(message, line, column)
    }

    fn advance(&mut self) {
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.current(), Token::Newline) {
            self.advance();
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), Error> {
        if self.current() == &expected {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!(
                "expected {:?}, found {:?}",
                expected,
                self.current()
            )))
        }
    }

    fn parse_statement(&mut self) -> Result<Statement, Error> {
        match self.current().clone() {
            Token::Variable(name) => {
                self.advance();
                self.expect(Token::Equals)?;
                let value = self.parse_value()?;
                self.variables.insert(name.clone(), value.clone());
                Ok(Statement::VariableAssignment(name, value))
            }
            Token::Identifier(key) => {
                self.advance();
                if self.current() == &Token::Equals {
                    self.advance();
                    let value = self.parse_value()?;
                    Ok(Statement::Assignment(key, value))
                } else if self.current() == &Token::OpenBrace {
                    self.advance();
                    let mut inner = Vec::new();
                    self.skip_newlines();
                    while self.current() != &Token::CloseBrace && self.current() != &Token::Eof {
                        inner.push(self.parse_statement()?);
                        self.skip_newlines();
                    }
                    self.expect(Token::CloseBrace)?;
                    Ok(Statement::Block(key, inner))
                } else {
                    Err(self.error(format!(
                        "expected '=' or '{{' after '{key}', found {:?}",
                        self.current()
                    )))
                }
            }
            other => Err(self.error(format!("unexpected token at start of statement: {other:?}"))),
        }
    }

    fn parse_value(&mut self) -> Result<Value, Error> {
        match self.current().clone() {
            Token::String(s) => {
                self.advance();
                Ok(Value::String(s))
            }
            Token::Number(n) => {
                self.advance();
                Ok(Value::Number(n))
            }
            Token::Bool(b) => {
                self.advance();
                Ok(Value::Bool(b))
            }
            Token::Variable(name) => {
                let (line, column) = self.position();
                self.advance();
                self.variables
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| Error::at(format!("undefined variable: ${name}"), line, column))
            }
            Token::OpenBracket => {
                self.advance();
                let mut items = Vec::new();
                self.skip_newlines();
                while self.current() != &Token::CloseBracket && self.current() != &Token::Eof {
                    items.push(self.parse_value()?);
                    self.skip_newlines();
                    if self.current() == &Token::Comma {
                        self.advance();
                        self.skip_newlines();
                    }
                }
                self.expect(Token::CloseBracket)?;
                Ok(Value::Array(items))
            }
            other => Err(self.error(format!("unexpected token in value: {other:?}"))),
        }
    }
}