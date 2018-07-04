use nom;
/// Types & parser for shell expressions (everything inside "$(())" )
use nom::types::CompleteStr;
use nom::Needed;
use std::os::unix::io::RawFd;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub enum InfixOperator {
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
}

#[derive(Debug, Clone)]
pub enum PrefixOperator {
    Positive,
    Negative,
    Negate,
    Not,
    Increment,
    Decrement,
}

#[derive(Debug, Clone)]
pub enum SuffixOperator {
    CopyIncrement,
    CopyDecrement,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64),
    Variable(String),
    Infix(Box<Infix>),
    Prefix(Box<Prefix>),
    Suffix(Box<Suffix>),
    Condition(Box<Condition>),
    Assignment(Box<Assignment>),
}

#[derive(Debug, Clone)]
pub struct Infix {
    pub left: Expr,
    pub operator: InfixOperator,
    pub right: Expr,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub left: Expr,
    pub operator: Option<InfixOperator>,
    pub right: Expr,
}

#[derive(Debug, Clone)]
pub struct Prefix {
    pub operator: PrefixOperator,
    pub right: Expr,
}

#[derive(Debug, Clone)]
pub struct Suffix {
    pub operator: SuffixOperator,
    pub left: Expr,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub condition: Expr,
    pub on_true: Expr,
    pub on_false: Expr,
}

named!(
    pub digit<CompleteStr, CompleteStr>,
    take_while1!(|c| c >= '0' && c <= '9')
);

named!(
    pub hexadecimal_digit<CompleteStr, CompleteStr>,
    take_while1!(|c| (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'))
);

named!(
    pub octal_digit<CompleteStr, CompleteStr>,
    take_while1!(|c| c >= '0' && c <= '7')
);

named!(
    pub binary_digit<CompleteStr, CompleteStr>,
    take_while!(|c| c >= '0' && c <= '1')
);

named!(
    pub decimal<CompleteStr, Expr>,
    map!(
        flat_map!(
            recognize!(
                tuple!(
                    alt!(
                          delimited!(digit, tag!("."), opt!(digit))
                        | delimited!(opt!(digit), tag!("."), digit)
                        | digit
                    ),
                    opt!(tuple!(
                        alt!(tag!("e") | tag!("E")),
                        opt!(alt!(tag!("+") | tag!("-"))),
                        digit
                    ))
                )
            ),
            parse_to!(f64)
        ),
        |v| Expr::Number(v)
    )
);

named!(
    pub hexadecimal<CompleteStr, Expr>,
    map!(
        preceded!(
            alt!(tag!("0X") | tag!("0x")),
            call!(hexadecimal_digit)
        ),
        |v| Expr::Number(isize::from_str_radix(v.0, 16).unwrap() as f64)
    )
);

named!(
    pub octal<CompleteStr, Expr>,
    map!(
        preceded!(
            alt!(tag!("0O") | tag!("0o")),
            call!(octal_digit)
        ),
        |v| Expr::Number(isize::from_str_radix(v.0, 8).unwrap() as f64)
    )
);

named!(
    pub binary<CompleteStr, Expr>,
    map!(
        preceded!(
            alt!(tag!("0B") | tag!("0b")),
            call!(octal_digit)
        ),
        |v| Expr::Number(isize::from_str_radix(v.0, 8).unwrap() as f64)
    )
);

named!(
    pub number<CompleteStr, Expr>,
    ws!(alt!(
          hexadecimal
        | octal
        | binary
        | decimal
    ))
);

named!(
    pub variable<CompleteStr, Expr>,
    map!(take_while1!(|c| nom::is_alphanumeric(c as u8) || c == '_'), |v : CompleteStr| Expr::Variable(String::from(v.0)))
);

named!(
    pub infix_operator<CompleteStr, InfixOperator>,
    alt!(
          tag!("<<") => { |_| InfixOperator::LeftShift }
        | tag!(">>") => { |_| InfixOperator::RightShift }
        | tag!("==") => { |_| InfixOperator::Equal }
        | tag!("!=") => { |_| InfixOperator::NotEqual }
        | tag!("&&") => { |_| InfixOperator::And }
        | tag!("||") => { |_| InfixOperator::Or }
        | tag!("<") => { |_| InfixOperator::LessThan }
        | tag!(">") => { |_| InfixOperator::GreaterThan }
        | tag!("^") => { |_| InfixOperator::BitExclusiveOr }
        | tag!("|") => { |_| InfixOperator::BitOr }
        | tag!("&") => { |_| InfixOperator::BitAnd }
        | tag!("+") => { |_| InfixOperator::Add }
        | tag!("-") => { |_| InfixOperator::Subtract }
        | tag!("*") => { |_| InfixOperator::Multiply }
        | tag!("/") => { |_| InfixOperator::Divide }
        | tag!("%") => { |_| InfixOperator::Modulo }
    )
);

named!(
    pub prefix_operator<CompleteStr, PrefixOperator>,
    alt!(
          tag!("++") => { |_| PrefixOperator::Increment }
        | tag!("--") => { |_| PrefixOperator::Decrement }
        | tag!("-") => { |_| PrefixOperator::Negative }
        | tag!("+") => { |_| PrefixOperator::Positive }
        | tag!("~") => { |_| PrefixOperator::Negate }
        | tag!("!") => { |_| PrefixOperator::Not }
    )
);

named!(
    pub suffix_operator<CompleteStr, SuffixOperator>,
    alt!(
          tag!("++") => { |_| SuffixOperator::CopyIncrement }
        | tag!("--") => { |_| SuffixOperator::CopyDecrement }
    )
);

named!(
    pub prefix<CompleteStr, Expr>,
    ws!(do_parse!(
        op: ws!(opt!(prefix_operator)) >>
        expr: ws!(suffix) >>
        (match op {
            Some(v) => Expr::Prefix(Box::new(Prefix{operator: v, right: expr})),
            None => expr,
        })
    ))
);

named!(
    pub suffix<CompleteStr, Expr>,
    ws!(do_parse!(
        expr: ws!(alt!(number | variable)) >>
        op: ws!(opt!(suffix_operator)) >>
        (match op {
            Some(v) => Expr::Suffix(Box::new(Suffix{operator: v, left: expr})),
            None => expr,
        })
    ))
);

macro_rules! left_recursive {
    ($i:ident, $next:tt, $($tags:tt)*) => {
        do_parse!($i,
            initial: ws!($next) >>
            sub: fold_many0!(
                    do_parse!(
                        op: ws!(alt!($($tags)*)) >>
                        expr: ws!($next) >>
                        (op, expr)
                    ),
                    initial,
                    |l, (op, r)| {
                        Expr::Infix(Box::new(Infix {
                            left: l,
                            operator: op,
                            right: r,
                        }))
                    }
            ) >> (sub)
        )
    };

    ($i:ident, $sub:ident ! ( $($args:tt)* ) , $($tags:tt)*) => {
        do_parse!($i,
            initial: ws!($sub!($($args)*)) >>
            sub: fold_many0!(
                    do_parse!(
                        op: ws!(alt!($($tags)*)) >>
                        expr: ws!($sub!($($args)*)) >>
                        (op, expr)
                    ),
                    initial,
                    |l, (op, r)| {
                        Expr::Infix(Box::new(Infix {
                            left: l,
                            operator: op,
                            right: r,
                        }))
                    }
            ) >> (sub)
        )
    };
}

named!(infix_precedence3<CompleteStr, Expr>,
    left_recursive!(prefix,
          tag!("*") => { |_| InfixOperator::Multiply }
        | tag!("/") => { |_| InfixOperator::Divide }
        | tag!("%") => { |_| InfixOperator::Modulo }
    )
);

named!(infix_precedence4<CompleteStr, Expr>,
    left_recursive!(infix_precedence3,
          tag!("+") => { |_| InfixOperator::Add }
        | tag!("-") => { |_| InfixOperator::Subtract }
    )
);

named!(infix_precedence5<CompleteStr, Expr>,
    left_recursive!(infix_precedence4,
          tag!("<<") => { |_| InfixOperator::LeftShift  }
        | tag!(">>") => { |_| InfixOperator::RightShift }
    )
);

named!(infix_precedence6<CompleteStr, Expr>,
    left_recursive!(infix_precedence5,
          tag!("<=") => { |_| InfixOperator::LessThanOrEqual }
        | tag!(">=") => { |_| InfixOperator::GreaterThanOrEqual }
        | tag!("<") => { |_| InfixOperator::LessThan }
        | tag!(">") => { |_| InfixOperator::GreaterThan }
    )
);

named!(infix_precedence7<CompleteStr, Expr>,
    left_recursive!(infix_precedence6,
          tag!("==") => { |_| InfixOperator::Equal }
        | tag!("!=") => { |_| InfixOperator::NotEqual }
    )
);

named!(infix_precedence8_10<CompleteStr, Expr>,
    left_recursive!(left_recursive!(left_recursive!(infix_precedence7,
        tag!("&") => { |_| InfixOperator::BitAnd }),
        tag!("^") => { |_| InfixOperator::BitExclusiveOr }),
        tag!("|") => { |_| InfixOperator::BitOr })
);

named!(infix_precedence11_12<CompleteStr, Expr>,
    left_recursive!(left_recursive!(infix_precedence8_10,
        tag!("&&") => { |_| InfixOperator::And }),
        tag!("||") => { |_| InfixOperator::Or })
);

named!(infix_precedence14<CompleteStr, Expr>,
    do_parse!(
        initial: ws!(infix_precedence11_12) >>
        sub: fold_many0!(
            do_parse!(
                op: ws!(alt!(
                      tag!("+=") => { |_| Some(InfixOperator::Add) }
                    | tag!("-=") => { |_| Some(InfixOperator::Subtract) }
                    | tag!("*=") => { |_| Some(InfixOperator::Multiply) }
                    | tag!("/=") => { |_| Some(InfixOperator::Divide) }
                    | tag!("&=") => { |_| Some(InfixOperator::BitAnd) }
                    | tag!("|=") => { |_| Some(InfixOperator::BitOr) }
                    | tag!("^=") => { |_| Some(InfixOperator::BitExclusiveOr) }
                    | tag!("=")  => { |_| None }
                )) >>
                expr: ws!(infix_precedence11_12) >>
                (op, expr)
            ),
            initial,
            |l, (op, r)| {
                Expr::Assignment(Box::new(Assignment {
                    left: l,
                    operator: op,
                    right: r,
                }))
            }) >> (sub)
    )
);

named!(pub expression<CompleteStr, Expr>, call!(infix_precedence14));

pub fn parse<T: AsRef<str>>(s: T) -> Expr {
    expression(CompleteStr(s.as_ref())).unwrap().1
}
