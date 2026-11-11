use std::collections::HashMap;

use serde::Deserialize;

use crate::Error;

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Array(Vec<Value>),
    Object(HashMap<String, Value>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Statement {
    VariableAssignment(String, Value),
    Assignment(String, Value),
    Block(String, Vec<Statement>),
}

#[derive(Debug, Clone)]
pub struct Ast {
    pub statements: Vec<Statement>,
}

impl Ast {
    pub fn into_value(self) -> Result<Value, Error> {
        Ok(Value::Object(statements_to_map(self.statements)?))
    }
}

fn statements_to_map(statements: Vec<Statement>) -> Result<HashMap<String, Value>, Error> {
    let mut map = HashMap::new();
    for stmt in statements {
        match stmt {
            Statement::Assignment(k, v) => {
                map.insert(k, v);
            }
            Statement::Block(k, inner) => {
                map.insert(k, Value::Object(statements_to_map(inner)?));
            }
            Statement::VariableAssignment(_, _) => {}
        }
    }
    Ok(map)
}