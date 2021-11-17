//! Types & parser for shell expressions (everything inside "$(())" )

mod errors;
pub mod lexer;
pub mod parser;
pub mod types;

pub use errors::*;
pub use parser::parse;
pub use types::Expr;

use crate::env::Variables;
use types::Operator;

use nom::types::CompleteStr;
use std::ffi::OsString;
use std::str::FromStr;

pub fn eval<T: AsRef<str>>(s: T, vars: &mut Variables) -> Result<String> {
    Ok(parse(s.as_ref())?.evaluate(vars).to_string())
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
                let new_value = f(lexer::float(CompleteStr(
                    vars.value(&name).to_str().unwrap_or("0"),
                ))
                .map(|(_, y)| y as f64)
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
                let new_value = f(lexer::float(CompleteStr(
                    vars.value(&name).to_str().unwrap_or("0"),
                ))
                .map(|(_, y)| y as f64)
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
                    let new_value = f(lexer::float(CompleteStr(
                        vars.value(&name).to_str().unwrap_or("0"),
                    ))
                    .map(|(_, y)| y as f64)
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
                lexer::float(CompleteStr(&vars.value(&n.into()).into_string().unwrap()))
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
                Operator::Not => {
                    if pre.right.as_boolean() {
                        Expr::Number(0.0_f64)
                    } else {
                        Expr::Number(1.0_f64)
                    }
                }
                Operator::Negate => pre.right.modify_number(vars, |x| !(x as isize) as f64),
                Operator::Add => pre.right.evaluate(vars),
                Operator::Subtract => pre.right.modify_number(vars, |x| -x),
                _ => unreachable!(),
            },
            Expr::Suffix(suf) => {
                let copy = suf.left.clone().evaluate(vars);
                match suf.operator {
                    Operator::Increment => suf.left.modify_variable(vars, |v| v + 1.0),
                    Operator::Decrement => suf.left.modify_variable(vars, |v| v - 1.0),
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
                    Operator::And => {
                        if inf.left.evaluate(vars).as_boolean()
                            && inf.right.evaluate(vars).as_boolean()
                        {
                            Expr::Number(1.0_f64)
                        } else {
                            Expr::Number(0.0_f64)
                        }
                    }
                    Operator::Or => {
                        if inf.left.evaluate(vars).as_boolean()
                            || inf.right.evaluate(vars).as_boolean()
                        {
                            Expr::Number(1.0_f64)
                        } else {
                            Expr::Number(0.0_f64)
                        }
                    }
                    Operator::Assign => inf.left.assign_variable(vars, |_| right),
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

#[cfg(test)]
mod test {
    use std::ffi::OsString;

    use crate::{
        env::Variables,
        expr::{parse, Expr},
    };

    fn eval(source: &str, vars: &mut Variables) -> Expr {
        parse(&source)
            .unwrap_or_else(|err| panic!("failed to evaluate '{}': {}", source, err))
            .evaluate(vars)
    }

    #[test]
    fn ops_assignemnt() {
        let mut vars = Variables::new();
        eval("a = 1", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "1");

        vars.define("a", "1");
        eval("a += 5/2", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "3.5");

        vars.define("a", "1");
        eval("a -= 2.5", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "-1.5");

        vars.define("a", "5");
        eval("a *= 2", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "10");

        vars.define("a", "9");
        eval("a /= 3", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "3");

        vars.define("a", "9");
        eval("a %= 2", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "1");

        vars.define("a", "2");
        eval("a ^= 3", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "1");

        vars.define("a", "3");
        eval("a |= 8", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "11");

        vars.define("a", "3");
        eval("a >>= 1", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "1");

        vars.define("a", "3");
        eval("a <<= 3", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "24");

        vars.define("a", "3");
        eval("a &= 0b101", &mut vars);
        assert_eq!(vars.value(&OsString::from("a")), "1");
    }

    #[test]
    fn ops_suffix() {
        let mut vars = Variables::new();

        vars.define("n", "0");
        assert_eq!(eval("n++", &mut vars), Expr::Number(0.0));
        assert_eq!(vars.value(&OsString::from("n")), "1");
        assert_eq!(eval("n--", &mut vars), Expr::Number(1.0));
        assert_eq!(vars.value(&OsString::from("n")), "0");

        assert_eq!(eval("2++", &mut vars), Expr::Number(2.0));
        assert_eq!(eval("5.1--", &mut vars), Expr::Number(5.1));
    }

    #[test]
    fn ops_prefix() {
        let mut vars = Variables::new();

        vars.define("n", "0");
        assert_eq!(eval("--n", &mut vars), Expr::Number(-1.0));
        assert_eq!(vars.value(&OsString::from("n")), "-1");
        assert_eq!(eval("++n", &mut vars), Expr::Number(0.0));
        assert_eq!(vars.value(&OsString::from("n")), "0");

        assert_eq!(eval("!0", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("!5", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("~0b10111001", &mut vars), Expr::Number(-186.0));

        vars.define("x", "-5");
        vars.define("y", "9");
        assert_eq!(eval("-x", &mut vars), Expr::Number(5.0));
        assert_eq!(vars.value(&OsString::from("x")), "-5");
        assert_eq!(eval("+x", &mut vars), Expr::Number(-5.0));
        assert_eq!(vars.value(&OsString::from("x")), "-5");

        assert_eq!(eval("+y", &mut vars), Expr::Number(9.0));
        assert_eq!(eval("-y", &mut vars), Expr::Number(-9.0));

        assert_eq!(eval("+(1 + 2)", &mut vars), Expr::Number(3.0));
    }

    #[test]
    fn ops_bitwise() {
        let mut vars = Variables::new();

        assert_eq!(eval("8 >> 1", &mut vars), Expr::Number(4.0));
        assert_eq!(eval("4 << 3", &mut vars), Expr::Number(32.0));
        assert_eq!(eval("32 & 2", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("5 | 8", &mut vars), Expr::Number(13.0));
        assert_eq!(eval("5 ^ 9", &mut vars), Expr::Number(12.0));
    }

    #[test]
    fn ops_arithmetic() {
        let mut vars = Variables::new();

        assert_eq!(eval("2.53 + 1", &mut vars), Expr::Number(3.53));
        assert_eq!(eval("11 - 5", &mut vars), Expr::Number(6.0));
        assert_eq!(eval("3 * 9", &mut vars), Expr::Number(27.0));
        assert_eq!(eval("1 / 10", &mut vars), Expr::Number(0.1));
        assert_eq!(eval("5 % 3", &mut vars), Expr::Number(2.0));
    }

    #[test]
    fn ops_comparison() {
        let mut vars = Variables::new();

        assert_eq!(eval("2.53 < 2.54", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("3 > 5", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("2.99 <= 3", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("3.00 >= 3", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("99.0002 == 99", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("5 != -5", &mut vars), Expr::Number(1.0));
    }

    #[test]
    fn ops_boolean() {
        let mut vars = Variables::new();

        vars.define("a", "0.5");

        assert_eq!(eval("0 || 11", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("5 || 0", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("0 || -1 + 1", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("2 && 0", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("0 && 5", &mut vars), Expr::Number(0.0));
        assert_eq!(eval("1 && ~0", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("5 && a && -1", &mut vars), Expr::Number(1.0));
        assert_eq!(eval("0 || !1 || a", &mut vars), Expr::Number(1.0));
    }

    #[test]
    fn ops_complex() {
        let mut vars = Variables::new();
        assert_eq!(
            eval(
                "(((a = 0 ? 3 : 1) + 5 | (3 + 5) / 2 == 7 & ~a) ? (7 % 2 > 0) ^ 2 : -1) / 3 + !a * 1.5",
                &mut vars,
            ),
            Expr::Number(2.5)
        );
    }
}
