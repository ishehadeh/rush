//! Types & parser for shell expressions (everything inside "$(())" )

mod errors;
pub mod lexer;
pub mod parser;
pub mod types;

pub use self::errors::*;
pub use self::parser::{parse, parse_ctx};
pub use self::types::Expr;
use self::types::Operator;
use env::Variables;
use nom::types::CompleteStr;
use std::ffi::OsString;
use std::str::FromStr;

pub fn eval<T: AsRef<str>>(s: T, vars: &mut Variables) -> Result<String> {
    Ok(parse(s.as_ref())?.evaluate(vars).to_string())
}

pub fn eval_ctx<'a>(s: &'a str, vars: &mut Variables) -> ContextResult<'a, String> {
    Ok(parse_ctx(s)?.evaluate(vars).to_string())
}

impl Expr {
    pub fn as_boolean(&self) -> bool {
        match self {
            Expr::Number(n) => *n != 0.0_f64,
            _ => false,
        }
    }

    pub fn modify_variable<F: Fn(f64) -> f64>(self, vars: &mut Variables, f: F) -> Self {
        match self {
            Expr::Variable(n) => {
                let name: OsString = n.to_string().into();
                let new_value = f(lexer::number(CompleteStr(
                    vars.value(&name).to_str().unwrap_or("0"),
                )).map(|(x, y)| y as f64)
                    .unwrap_or(0.0_f64));

                vars.define(&name, new_value.clone().to_string());
                return Expr::Number(new_value);
            }
            _ => (),
        };

        let me = self.evaluate(vars);

        match me {
            Expr::Variable(n) => {
                let name = n.to_string().into();
                let new_value = f(lexer::number(CompleteStr(
                    vars.value(&name).to_str().unwrap_or("0"),
                )).map(|(x, y)| y as f64)
                    .unwrap_or(0.0_f64));

                vars.define(n.clone().to_string(), new_value.clone().to_string());
                Expr::Number(new_value)
            }
            Expr::Number(n) => Expr::Number(n),
            Expr::Condition(n) => Expr::Condition(n),
            Expr::Infix(n) => Expr::Infix(n),
            Expr::Prefix(n) => Expr::Prefix(n),
            Expr::Suffix(n) => Expr::Suffix(n),
        }
    }

    pub fn assign_variable<F: Fn(f64) -> f64>(mut self, vars: &mut Variables, f: F) -> Self {
        for _ in 0..2 {
            match self {
                Expr::Variable(n) => {
                    let name = n.to_string().into();
                    let new_value = f(lexer::number(CompleteStr(
                        vars.value(&name).to_str().unwrap_or("0"),
                    )).map(|(_, y)| y as f64)
                        .unwrap_or(0.0_f64));
                    vars.define(name, new_value.to_string());
                    return Expr::Number(new_value);
                }
                _ => self = self.evaluate(vars),
            }
        }
        self
    }

    pub fn modify_number<F: Fn(f64) -> f64>(self, vars: &mut Variables, f: F) -> Self {
        let me = self.evaluate(vars);
        match me {
            Expr::Number(n) => Expr::Number(f(n)),
            _ => me,
        }
    }

    pub fn modify_number_i<F: Fn(isize) -> isize>(self, vars: &mut Variables, f: F) -> Self {
        let me = self.evaluate(vars);
        match me {
            Expr::Number(n) => Expr::Number(f(n as isize) as f64),
            _ => me,
        }
    }

    pub fn evaluate(self, vars: &mut Variables) -> Self {
        match self {
            Expr::Number(n) => Expr::Number(n),
            Expr::Variable(n) => Expr::Number(
                lexer::number(CompleteStr(&vars.value(&n.into()).into_string().unwrap()))
                    .map(|(_, y)| y as f64)
                    .unwrap_or(0.0_f64),
            ),
            Expr::Condition(cond) => {
                if cond.condition.clone().evaluate(vars).as_boolean() {
                    cond.on_true.evaluate(vars)
                } else {
                    cond.on_false.evaluate(vars)
                }
            }
            Expr::Prefix(pre) => match pre.operator {
                Operator::Increment => pre.right.modify_variable(vars, |v| v + 1.0),
                Operator::Decrement => pre.right.modify_variable(vars, |v| v - 1.0),
                Operator::Not => if pre.right.as_boolean() {
                    Expr::Number(0.0_f64)
                } else {
                    Expr::Number(1.0_f64)
                },
                Operator::Negate => pre.right.modify_number(vars, |x| !(x as isize) as f64),
                Operator::Add => pre.right,
                Operator::Subtract => pre.right.modify_number(vars, |x| -x),
                _ => unreachable!(),
            },
            Expr::Suffix(suf) => {
                let copy = suf.left.clone().evaluate(vars);
                match suf.operator {
                    Operator::Increment => suf.left.modify_variable(vars, |v| v + 1.0),
                    Operator::Decrement => suf.left.modify_variable(vars, |v| v + 1.0),
                    _ => unreachable!(),
                };
                copy
            }
            Expr::Infix(inf) => {
                let right = match inf.right.clone().evaluate(vars) {
                    Expr::Number(v) => v,
                    _ => unreachable!(),
                };

                match inf.operator {
                    Operator::Add => inf.left.modify_number(vars, |v| v + right),
                    Operator::Subtract => inf.left.modify_number(vars, |v| v - right),
                    Operator::Multiply => inf.left.modify_number(vars, |v| v * right),
                    Operator::Divide => inf.left.modify_number(vars, |v| v / right),
                    Operator::Modulo => inf.left.modify_number(vars, |v| v % right),
                    Operator::LeftShift => inf.left.modify_number_i(vars, |v| v << right as isize),
                    Operator::RightShift => inf.left.modify_number_i(vars, |v| v >> right as isize),
                    Operator::LessThan => inf
                        .left
                        .modify_number(vars, |v| (v < right) as isize as f64),
                    Operator::LessThanOrEqual => inf
                        .left
                        .modify_number(vars, |v| (v <= right) as isize as f64),
                    Operator::GreaterThan => inf
                        .left
                        .modify_number(vars, |v| (v > right) as isize as f64),
                    Operator::GreaterThanOrEqual => inf
                        .left
                        .modify_number(vars, |v| (v >= right) as isize as f64),
                    Operator::Equal => inf
                        .left
                        .modify_number(vars, |v| (v == right) as isize as f64),
                    Operator::NotEqual => inf
                        .left
                        .modify_number(vars, |v| (v != right) as isize as f64),
                    Operator::BitAnd => inf.left.modify_number_i(vars, |v| v & right as isize),
                    Operator::BitExclusiveOr => {
                        inf.left.modify_number_i(vars, |v| v ^ right as isize)
                    }
                    Operator::BitOr => inf.left.modify_number_i(vars, |v| v | right as isize),
                    Operator::And => if inf.left.as_boolean() && inf.right.as_boolean() {
                        Expr::Number(1.0_f64)
                    } else {
                        Expr::Number(0.0_f64)
                    },
                    Operator::Or => if inf.left.as_boolean() && inf.right.as_boolean() {
                        Expr::Number(1.0_f64)
                    } else {
                        Expr::Number(0.0_f64)
                    },
                    Operator::Assign => inf.left.assign_variable(vars, |v| right),
                    Operator::AssignAdd => inf.left.assign_variable(vars, |v| v + right),
                    Operator::AssignSubtract => inf.left.assign_variable(vars, |v| v - right),
                    Operator::AssignMultiply => inf.left.assign_variable(vars, |v| v * right),
                    Operator::AssignDivide => inf.left.assign_variable(vars, |v| v / right),
                    Operator::AssignModulo => inf.left.assign_variable(vars, |v| v % right),
                    Operator::AssignBitAnd => inf
                        .left
                        .assign_variable(vars, |v| (v as isize & right as isize) as f64),
                    Operator::AssignBitExclusiveOr => inf
                        .left
                        .assign_variable(vars, |v| (v as isize ^ right as isize) as f64),
                    Operator::AssignBitOr => inf
                        .left
                        .assign_variable(vars, |v| (v as isize | right as isize) as f64),
                    Operator::AssignLeftShift => inf
                        .left
                        .assign_variable(vars, |v| ((v as isize) << right as isize) as f64),
                    Operator::AssignRightShift => inf
                        .left
                        .assign_variable(vars, |v| (v as isize >> right as isize) as f64),
                    _ => unreachable!(),
                }
            }
        }
    }
}

impl FromStr for Expr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        parse(s)
    }
}
