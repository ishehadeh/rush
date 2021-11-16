use crate::expr::errors::*;
use crate::expr::types::*;

use nom;
use nom::types::CompleteStr;

#[derive(Debug, Clone)]
pub struct TokenStream<'a> {
    input: &'a str,
    sliced: &'a str,
    column: usize,
}

named!(digit<CompleteStr, CompleteStr>,
    take_while1!(|c| (c >= '0' && c <= '9'))
);

named!(hexadecimal_digit<CompleteStr, CompleteStr>,
    take_while1!(|c| (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F'))
);

named!(octal_digit<CompleteStr, CompleteStr>,
    take_while1!(|c| (c >= '0' && c <= '7'))
);

named!(binary_digit<CompleteStr, CompleteStr>,
    take_while1!(|c| (c >= '0' && c <= '1'))
);

named!(
    pub decimal<CompleteStr, f64>,
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
    )
);

named!(pub decimal_integer<CompleteStr, isize>,
    flat_map!(digit, parse_to!(isize))
);

named!(
    pub hexadecimal<CompleteStr, isize>,
    map!(
        preceded!(
            alt!(tag!("0X") | tag!("0x")),
            call!(hexadecimal_digit)
        ),
        |v| isize::from_str_radix(v.0, 16).unwrap()
    )
);

named!(
    pub octal<CompleteStr, isize>,
    map!(
        preceded!(
            alt!(tag!("0O") | tag!("0o")),
            call!(octal_digit)
        ),
        |v| isize::from_str_radix(v.0, 8).unwrap()
    )
);

named!(
    pub binary<CompleteStr, isize>,
    map!(
        preceded!(
            alt!(tag!("0B") | tag!("0b")),
            call!(binary_digit)
        ),
        |v| isize::from_str_radix(v.0, 2).unwrap()
    )
);

named!(
    pub integer<CompleteStr, isize>,
    ws!(alt!(
          hexadecimal
        | octal
        | binary
        | decimal_integer
    ))
);

named!(
    pub float<CompleteStr, f64>,
    do_parse!(
        prefix: opt!(ws!(alt!(char!('+') | char!('-')))) >>
        number: alt!(
            decimal
            | map!(integer, |x| x as f64)
        ) >>
        (
            match prefix {
                Some(v) => match v {
                    '+' => number,
                    '-' => -number,
                    _=> unreachable!(),
                }
                None => number
            }
        )
    )
);

named!(
    pub variable<CompleteStr, &str>,
    map!(take_while1!(|c| nom::is_alphanumeric(c as u8) || c == '_'), |v : CompleteStr| v.0)
);

named!(
    pub operator<CompleteStr, Operator>,
    alt!(
        tag!(">>=") => { |_| Operator::AssignRightShift }
        | tag!("<<=") => { |_| Operator::AssignLeftShift }
        | tag!("<<")  => { |_| Operator::LeftShift }
        | tag!(">>")  => { |_| Operator::RightShift }
        | tag!("==")  => { |_| Operator::Equal }
        | tag!("!=")  => { |_| Operator::NotEqual }
        | tag!("&&")  => { |_| Operator::And }
        | tag!("||")  => { |_| Operator::Or }
        | tag!("++")  => { |_| Operator::Increment }
        | tag!("--")  => { |_| Operator::Decrement }
        | tag!("+=")  => { |_| Operator::AssignAdd }
        | tag!("-=")  => { |_| Operator::AssignSubtract }
        | tag!("*=")  => { |_| Operator::AssignMultiply }
        | tag!("/=")  => { |_| Operator::AssignDivide }
        | tag!("&=")  => { |_| Operator::AssignBitAnd }
        | tag!("|=")  => { |_| Operator::AssignBitOr }
        | tag!("^=")  => { |_| Operator::AssignBitExclusiveOr }
        | tag!("=")   => { |_| Operator::Assign }
        | tag!("<")   => { |_| Operator::LessThan }
        | tag!(">")   => { |_| Operator::GreaterThan }
        | tag!("^")   => { |_| Operator::BitExclusiveOr }
        | tag!("|")   => { |_| Operator::BitOr }
        | tag!("&")   => { |_| Operator::BitAnd }
        | tag!("+")   => { |_| Operator::Add }
        | tag!("-")   => { |_| Operator::Subtract }
        | tag!("*")   => { |_| Operator::Multiply }
        | tag!("/")   => { |_| Operator::Divide }
        | tag!("%")   => { |_| Operator::Modulo }
        | tag!("~")   => { |_| Operator::Negate }
        | tag!("!")   => { |_| Operator::Not }
    )
);

impl<'a> TokenStream<'a> {
    pub fn new(i: &'a str) -> TokenStream<'a> {
        TokenStream {
            sliced: i,
            input: i,
            column: 1,
        }
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn full(&self) -> &'a str {
        self.input
    }

    pub fn unread(&self) -> &'a str {
        self.sliced
    }
}

impl<'a> Iterator for TokenStream<'a> {
    type Item = Result<Token<'a>>;

    fn next(&mut self) -> Option<Self::Item> {
        let tok = opt!(
            CompleteStr(self.sliced),
            ws!(alt!(
                decimal    => { |v| Token::FloatingNumber(v) }
                | integer    => { |v| Token::Number(v)         }
                | variable   => { |v| Token::Variable(v)       }
                | operator   => { |v| Token::Operator(v)       }
                | char!(',') => { |_| Token::Comma             }
                | char!('?') => { |_| Token::QuestionMark      }
                | char!(':') => { |_| Token::Colon             }
                | char!('(') => { |_| Token::LeftParen         }
                | char!(')') => { |_| Token::RightParen        }
            ))
        );

        let (slice, maybe_token) = tok.unwrap();
        self.column = self.input.len() - self.sliced.len();

        match maybe_token {
            Some(t) => {
                self.sliced = slice.0;
                Some(Ok(t))
            }
            None => {
                if self.sliced.len() == 0 {
                    None
                } else {
                    Some(Err(ErrorKind::InvalidCharacter(
                        self.sliced.chars().next().unwrap(),
                    )
                    .into()))
                }
            }
        }
    }
}
