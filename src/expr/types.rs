use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Variable(String),
    Infix(Box<Infix>),
    Prefix(Box<Prefix>),
    Suffix(Box<Suffix>),
    Condition(Box<Condition>),
}

#[derive(Debug, Clone)]
pub struct Infix {
    pub left: Expr,
    pub operator: Operator,
    pub right: Expr,
}

#[derive(Debug, Clone)]
pub struct Prefix {
    pub operator: Operator,
    pub right: Expr,
}

#[derive(Debug, Clone)]
pub struct Suffix {
    pub operator: Operator,
    pub left: Expr,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub condition: Expr,
    pub on_true: Expr,
    pub on_false: Expr,
}

#[derive(Debug, Clone)]
pub enum Token<'a> {
    Operator(Operator),
    Variable(&'a str),
    Number(isize),
    FloatingNumber(f64),
    Comma,
    QuestionMark,
    Colon,
    LeftParen,
    RightParen,
}

#[derive(Debug, Clone)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    LeftShift,
    RightShift,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    Equal,
    NotEqual,
    BitAnd,
    BitExclusiveOr,
    BitOr,
    And,
    Or,
    AssignAdd,
    AssignSubtract,
    AssignMultiply,
    AssignDivide,
    AssignModulo,
    AssignLeftShift,
    AssignRightShift,
    AssignBitAnd,
    AssignBitExclusiveOr,
    AssignBitOr,
    Assign,
    Increment,
    Decrement,
    Negate,
    Not,
}

#[derive(Debug, Clone)]
pub enum Precedence {
    Minimum,
    Prefix,
    Suffix,
    Product,
    Sum,
    BitShift,
    Relational,
    Equality,
    BitAnd,
    BitOr,
    BitExclusiveOr,
    LogicalAnd,
    LogicalOr,
    TernaryConditional,
    Assignment,
    Parentheses,
    Separator,
}

impl Precedence {
    pub fn from_operator(op: &Operator) -> Precedence {
        match op {
            Operator::Multiply | Operator::Divide | Operator::Modulo => Precedence::Product,
            Operator::Add | Operator::Subtract => Precedence::Sum,
            Operator::LeftShift | Operator::RightShift => Precedence::BitShift,
            Operator::Equal | Operator::NotEqual => Precedence::Equality,
            Operator::BitAnd => Precedence::BitAnd,
            Operator::BitOr => Precedence::BitOr,
            Operator::BitExclusiveOr => Precedence::BitExclusiveOr,
            Operator::And => Precedence::LogicalAnd,
            Operator::Or => Precedence::LogicalOr,
            Operator::LessThan
            | Operator::LessThanOrEqual
            | Operator::GreaterThan
            | Operator::GreaterThanOrEqual => Precedence::Relational,
            Operator::Assign
            | Operator::AssignAdd
            | Operator::AssignBitAnd
            | Operator::AssignBitExclusiveOr
            | Operator::AssignBitOr
            | Operator::AssignDivide
            | Operator::AssignLeftShift
            | Operator::AssignModulo
            | Operator::AssignMultiply
            | Operator::AssignRightShift
            | Operator::AssignSubtract => Precedence::Assignment,
            Operator::Increment => Precedence::Prefix,
            Operator::Decrement => Precedence::Prefix,
            Operator::Negate => Precedence::Prefix,
            Operator::Not => Precedence::Prefix,
        }
    }

    pub fn from_token(t: &Token) -> Option<Precedence> {
        match t {
            Token::Number(_) => None,
            Token::FloatingNumber(_) => None,
            Token::Variable(_) => None,
            Token::Operator(o) => Some(Precedence::from_operator(o)),
            Token::Comma => Some(Precedence::Separator),
            Token::Colon | Token::QuestionMark => Some(Precedence::TernaryConditional),
            Token::LeftParen | Token::RightParen => Some(Precedence::Parentheses),
        }
    }
}

impl Operator {
    pub fn is_prefix(&self) -> bool {
        match self {
            Operator::Not
            | Operator::Negate
            | Operator::Increment
            | Operator::Decrement
            | Operator::Add
            | Operator::Subtract => true,
            _ => false,
        }
    }

    pub fn is_suffix(&self) -> bool {
        match self {
            Operator::Increment | Operator::Decrement => true,
            _ => false,
        }
    }

    pub fn precedence(&self) -> Precedence {
        Precedence::from_operator(self)
    }
}

impl PartialOrd for Precedence {
    fn partial_cmp(&self, other: &Precedence) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Precedence {
    fn cmp(&self, other: &Precedence) -> Ordering {
        (self.clone() as isize).cmp(&(other.clone() as isize))
    }
}

impl PartialEq for Precedence {
    fn eq(&self, other: &Precedence) -> bool {
        self == other
    }
}

impl Eq for Precedence {}

impl<'a> fmt::Display for Token<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Token::Variable(v) => v.to_string(),
                Token::Number(n) => n.to_string(),
                Token::FloatingNumber(n) => n.to_string(),
                Token::Operator(o) => o.to_string(),
                Token::LeftParen => "(".to_string(),
                Token::RightParen => ")".to_string(),
                Token::Comma => ",".to_string(),
                Token::QuestionMark => "?".to_string(),
                Token::Colon => ":".to_string(),
            }
        )
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Operator::*;
        write!(
            f,
            "{}",
            match self {
                Add => "+",
                Subtract => "-",
                Multiply => "*",
                Divide => "/",
                Modulo => "%",
                LeftShift => "<<",
                RightShift => ">>",
                LessThan => "<",
                LessThanOrEqual => "<=",
                GreaterThan => ">",
                GreaterThanOrEqual => ">=",
                Equal => "==",
                NotEqual => "!=",
                BitAnd => "&",
                BitExclusiveOr => "^",
                BitOr => "|",
                And => "&&",
                Or => "||",
                Assign => "=",
                AssignAdd => "+=",
                AssignSubtract => "-=",
                AssignMultiply => "*=",
                AssignDivide => "/=",
                AssignModulo => "%=",
                AssignBitAnd => "&=",
                AssignBitExclusiveOr => "^=",
                AssignBitOr => "|=",
                AssignLeftShift => "<<=",
                AssignRightShift => ">>=",
                Not => "!",
                Negate => "~",
                Increment => "++",
                Decrement => "--",
            }
        )
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expr::Condition(cond) => write!(
                f,
                "{} ? {} : {}",
                cond.condition, cond.on_true, cond.on_false
            ),
            Expr::Number(num) => write!(f, "{}", num),
            Expr::Variable(var) => write!(f, "{}", var),
            Expr::Prefix(pre) => write!(f, "{}{}", pre.operator, pre.right),
            Expr::Suffix(suf) => write!(f, "{}{}", suf.left, suf.operator),
            Expr::Infix(inf) => write!(f, "{} {} {}", inf.left, inf.operator, inf.right),
        }
    }
}
