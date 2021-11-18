use super::{
    lexer::TokenStream,
    types::{Condition, Infix, Precedence, Prefix, Suffix, Token},
    Context, Error, ErrorKind, Expr, Result,
};

pub struct Parser<'a> {
    tokens: TokenStream<'a>,
    peek: Option<Token<'a>>,
    column: usize,
}

macro_rules! expect_infix {
    ($_self:ident, $working_tree:expr) => {{
        match Precedence::from_token(match $_self.peek() {
            Some(v) => v,
            None => return Ok(Some($working_tree)),
        }) {
            Some(v) => v,
            None => {
                return Err(Error::from(ErrorKind::InvalidInfixOperator)
                    .with($_self.context($_self.peek().clone().unwrap())))
            }
        }
    }};
}

pub fn parse<T: AsRef<str>>(s: T) -> Result<Expr> {
    Parser::from(s.as_ref()).parse()
}

impl<'a> Parser<'a> {
    pub fn new(t: TokenStream<'a>) -> Parser<'a> {
        Parser {
            peek: None,
            tokens: t,
            column: 1,
        }
    }

    pub fn from(s: &'a str) -> Parser<'a> {
        Parser::new(TokenStream::new(s))
    }

    pub fn parse(&mut self) -> Result<Expr> {
        self.next_token()?;
        self.must_parse_precedence(Precedence::Separator)
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn peek<'b>(&'b self) -> &'b Option<Token<'a>> {
        &self.peek
    }

    pub fn next_token(&mut self) -> Result<Option<Token<'a>>> {
        let tok = self.peek.clone();
        self.column = self.tokens.column();
        self.peek = match self.tokens.next() {
            Some(v) => Some(match v {
                Ok(v) => v,
                Err(e) => {
                    return Err(e.with(Context {
                        token: String::from(" "),
                        input: self.tokens.full().to_string(),
                        column: self.column(),
                        line: 1,
                    }));
                }
            }),
            None => None,
        };
        Ok(tok)
    }

    fn context(&self, tok: Token<'a>) -> Context {
        Context {
            token: tok.to_string(),
            input: self.tokens.full().to_string(),
            column: self.column(),
            line: 1,
        }
    }

    fn must_parse_precedence(&mut self, p: Precedence) -> Result<Expr> {
        match self.parse_precedence(p)? {
            Some(v) => Ok(v),
            None => Err(Error::from(ErrorKind::UnexpectedEof).with(Context {
                token: String::from(" "),
                input: self.tokens.full().to_string(),
                column: self.tokens.full().len(),
                line: 1,
            })),
        }
    }

    fn parse_precedence(&mut self, precedence: Precedence) -> Result<Option<Expr>> {
        let mut left = match self.next_token()? {
            Some(v) => match v {
                Token::Number(n) => Expr::Number(n as f64),
                Token::FloatingNumber(n) => Expr::Number(n),
                Token::Variable(n) => Expr::Variable(n.to_string()),
                Token::Operator(operator) => {
                    if !operator.is_prefix() {
                        return Err(
                            Error::from(ErrorKind::InvalidPrefixOperator).with(self.context(v))
                        );
                    }

                    Expr::Prefix(Box::new(Prefix {
                        operator,
                        right: self.must_parse_precedence(Precedence::Prefix)?,
                    }))
                }
                Token::LeftParen => {
                    let new_left = self.must_parse_precedence(Precedence::Parentheses)?;

                    match self.next_token()? {
                        Some(v) => match v {
                            Token::RightParen => new_left,
                            _ => {
                                return Err(Error::from(ErrorKind::ExpectingRightParentheses)
                                    .with(self.context(v)))
                            }
                        },
                        None => {
                            return Err(Error::from(ErrorKind::ExpectingRightParentheses)
                                .with(self.context(v)))
                        }
                    }
                }
                _ => return Err(Error::from(ErrorKind::InvalidToken).with(self.context(v))),
            },
            None => return Ok(None),
        };

        match self.peek().clone() {
            Some(v) => match v {
                Token::Operator(operator) => {
                    if operator.is_suffix() {
                        left = Expr::Suffix(Box::new(Suffix { left, operator }));
                        self.next_token()?;
                    }
                }
                Token::RightParen => return Ok(Some(left)),
                _ => (),
            },
            _ => return Ok(Some(left)),
        };
        let mut token_precedence = expect_infix!(self, left);

        while token_precedence < precedence {
            left = match self.next_token()? {
                Some(v) => match v {
                    Token::Operator(operator) => Expr::Infix(Box::new(Infix {
                        left,
                        operator,
                        right: self.must_parse_precedence(token_precedence)?,
                    })),
                    Token::QuestionMark => {
                        let on_true = self.must_parse_precedence(token_precedence)?;
                        match self.next_token()? {
                            Some(Token::Colon) => (),
                            _ => {
                                return Err(Error::from(ErrorKind::ExpectingTernaryElse)
                                    .with(self.context(v)))
                            }
                        };

                        let on_false = self.must_parse_precedence(token_precedence)?;

                        Expr::Condition(Box::new(Condition {
                            condition: left,
                            on_true,
                            on_false,
                        }))
                    }
                    Token::Comma | Token::Colon | Token::RightParen => break,
                    Token::LeftParen => {
                        return Err(
                            Error::from(ErrorKind::InvalidInfixOperator).with(self.context(v))
                        )
                    }
                    _ => unreachable!(),
                },
                None => return Ok(None),
            };

            token_precedence = expect_infix!(self, left);
        }

        Ok(Some(left))
    }
}

#[cfg(test)]
mod test {
    use crate::expr::{
        errors::ErrorKind,
        lexer::TokenStream,
        parser::Parser,
        types::{Condition, Expr, Infix, Operator, Prefix, Suffix},
        Error,
    };

    fn parse(source: &str) -> Expr {
        Parser::new(TokenStream::new(source))
            .parse()
            .unwrap_or_else(|err| panic!("failed to parse expression '{}': {}", source, err))
    }

    fn parse_error(source: &str) -> Error {
        match Parser::new(TokenStream::new(source)).parse() {
            Ok(v) => panic!(
                "expected to fail parsing '{}', instead succeeded with '{}':\nast = {:#?}",
                source, v, v
            ),
            Err(e) => e,
        }
    }

    macro_rules! expr {
        // erase extra parentheses
        ( ( $($x:tt)+ ) ) => { expr!($($x)*) };

        ($cond:tt ? $succ:tt : $fail:tt) => {
            Expr::Condition(Box::new(Condition {
                condition: expr!($cond),
                on_true: expr!($succ),
                on_false: expr!($fail),
            }))
        };

        (pre $op:ident $rhs:tt) => {
            Expr::Prefix(Box::new(Prefix {
                operator: Operator::$op,
                right: expr!($rhs),
            }))
        };
        (suf $op:ident $lhs:tt) => {
            Expr::Suffix(Box::new(Suffix {
                operator: Operator::$op,
                left: expr!($lhs),
            }))
        };
        ($op:ident $lhs:tt $rhs:tt) => {
            Expr::Infix(Box::new(Infix {
                left: expr!($lhs),
                operator: Operator::$op,
                right: expr!($rhs),
            }))
        };
        ($ident:ident) => {
            Expr::Variable(std::stringify!($ident).to_string())
        };
        ($num:tt) => {
            Expr::Number($num)
        };
    }

    #[test]
    fn simple_infix() {
        assert_eq!(parse("1 + 1"), expr!(Add 1.0 1.0));
        assert_eq!(parse("1 / hello"), expr!(Divide 1.0 hello));
    }

    #[test]
    fn simple_prefix() {
        assert_eq!(parse("~11e5"), expr!(pre Negate 11.0e5));
        assert_eq!(parse("!yes"), expr!(pre Not yes));
        assert_eq!(parse("++zero"), expr!(pre Increment zero));
        assert_eq!(parse("--1"), expr!(pre Decrement 1.0));
    }

    #[test]
    fn simple_suffix() {
        assert_eq!(parse("0++"), expr!(suf Increment 0.0));
        assert_eq!(parse("five--"), expr!(suf Decrement five));
    }

    #[test]
    fn simple_cond() {
        assert_eq!(parse("0 ? ok : not_ok"), expr!(0.0 ? ok : not_ok));
        assert_eq!(parse("test ? 5 : zero"), expr!(test ? 5.0 : zero));
    }

    #[test]
    fn operator_precedence() {
        assert_eq!(parse("1 + 2 * 3"), expr!(Add 1.0 (Multiply 2.0 3.0)));
        assert_eq!(
            parse("((3 + 1) * 2) / 3"),
            expr!(Divide (Multiply (Add 3.0 1.0) 2.0) 3.0)
        );
        assert_eq!(
            parse("hello += 2 & 0b0010"),
            expr!(AssignAdd hello (BitAnd 2.0 2.0))
        );
        assert_eq!(
            parse("(hello %= 2) & 0b0010"),
            expr!(BitAnd (AssignModulo hello 2.0) 2.0)
        );
        assert_eq!(
            parse("test ? ++2 : test % one++"),
            expr!(test ? (pre Increment 2.0) : (Modulo test (suf Increment one)))
        );
        assert_eq!(
            parse("(test ? 5 : 5 * 9 + 2) % 2 == 0"),
            expr!((Equal (Modulo (test ? 5.0 : (Add (Multiply 5.0 9.0) 2.0)) 2.0) 0.0))
        );
        assert_eq!(
            parse("(test ? 5 : 5 * 9 + 2) % 2 == 0"),
            expr!((Equal (Modulo (test ? 5.0 : (Add (Multiply 5.0 9.0) 2.0)) 2.0) 0.0))
        );
        assert_eq!(
            parse("(!2 * (3 + 2) ? (5 ^ hi) : 2 >> (world = 3))"),
            expr!((Multiply (pre Not 2.0) (Add 3.0 2.0)) ? (BitExclusiveOr 5.0 hi) : (RightShift 2.0 (Assign world 3.0)))
        );

        assert_eq!(
            parse("(!2 * (3 + 2) ? (5 ^ hi) : 2 >> (world = 3))"),
            expr!((Multiply (pre Not 2.0) (Add 3.0 2.0)) ? (BitExclusiveOr 5.0 hi) : (RightShift 2.0 (Assign world 3.0)))
        );
        assert_eq!(
            parse("(2 + 3) * (5 - 8)"),
            expr!((Multiply (Add 2.0 3.0) (Subtract 5.0 8.0)))
        );
        assert_eq!(
            parse("(x = (+(3 - 5) - -(5 % 2)))"),
            expr!((Assign x (Subtract (pre Add (Subtract 3.0 5.0)) (pre Subtract (Modulo 5.0 2.0)))))
        );
        assert_eq!(parse("1 * (3 + 2)"), expr!(Multiply 1.0 (Add 3.0 2.0)));
        assert_eq!(parse("++(1)"), expr!(pre Increment 1.0));
        assert_eq!(
            parse("++(1-- * (3))"),
            expr!(pre Increment (Multiply (suf Decrement 1.0) 3.0))
        );
    }

    #[test]
    fn errors() {
        assert_eq!(
            parse_error("2 * (1 + 2").kind(),
            &ErrorKind::ExpectingRightParentheses
        );
        assert_eq!(parse_error("2 * 1 +").kind(), &ErrorKind::UnexpectedEof);
        assert_eq!(
            parse_error("2 ? 1").kind(),
            &ErrorKind::ExpectingTernaryElse
        );
        assert_eq!(parse_error("*hi").kind(), &ErrorKind::InvalidPrefixOperator);
        assert_eq!(
            parse_error("hello ++ world").kind(),
            &ErrorKind::InvalidInfixOperator
        );
        assert_eq!(parse_error("1/").kind(), &ErrorKind::UnexpectedEof);
        assert_eq!(
            parse_error("1 three").kind(),
            &ErrorKind::InvalidInfixOperator
        );
        assert_eq!(parse_error(",").kind(), &ErrorKind::InvalidToken);
        assert_eq!(parse_error("`").kind(), &ErrorKind::InvalidCharacter('`'));
        assert_eq!(
            parse_error("(2 + (1)(").kind(),
            &ErrorKind::ExpectingRightParentheses
        );
        assert_eq!(parse_error("a(5)").kind(), &ErrorKind::InvalidInfixOperator);
    }
}
