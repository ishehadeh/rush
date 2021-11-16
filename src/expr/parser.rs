use crate::expr::errors::*;
use crate::expr::lexer::TokenStream;
use crate::expr::types::*;

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
                return Err(Error::from(ErrorKind::InvalidPrefixOperator)
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
        self.next()?;
        self.must_parse_precedence(Precedence::Separator)
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn peek<'b>(&'b self) -> &'b Option<Token<'a>> {
        &self.peek
    }

    pub fn next(&mut self) -> Result<Option<Token<'a>>> {
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
        let mut left = match self.next()? {
            Some(v) => match v {
                Token::Number(n) => Expr::Number(n as f64),
                Token::FloatingNumber(n) => Expr::Number(n),
                Token::Variable(n) => Expr::Variable(n.to_string()),
                Token::Operator(op) => {
                    if !op.is_prefix() {
                        return Err(Error::from(ErrorKind::InvalidPrefixOperator).with(self.context(v)));
                    }

                    Expr::Prefix(Box::new(Prefix {
                        operator: op,
                        right: self.must_parse_precedence(Precedence::Prefix)?,
                    }))
                }
                Token::LeftParen => {
                    let new_left = self.must_parse_precedence(Precedence::Parentheses)?;

                    match self.next()? {
                        Some(v) => match v {
                            Token::RightParen => new_left,
                            _ => {
                                return Err(Error::from(ErrorKind::ExpectingRightParentheses)
                                    .with(self.context(v)))
                            }
                        },
                        None => return Err(Error::from(ErrorKind::ExpectingRightParentheses).with(self.context(v))),
                    }
                }
                _ => return Err(Error::from(ErrorKind::InvalidToken).with(self.context(v))),
            },
            None => return Ok(None),
        };

        match self.peek().clone() {
            Some(v) => match v {
                Token::Operator(o) => if o.is_suffix() {
                    left = Expr::Suffix(Box::new(Suffix {
                        left: left,
                        operator: o,
                    }));
                    self.next()?;
                },
                Token::RightParen => return Ok(Some(left)),
                _ => (),
            },
            _ => return Ok(Some(left)),
        };
        let mut token_precedence = expect_infix!(self, left);

        while token_precedence < precedence {
            left = match self.next()? {
                Some(v) => match v {
                    Token::Operator(o) => Expr::Infix(Box::new(Infix {
                        left: left,
                        operator: o,
                        right: self.must_parse_precedence(token_precedence)?,
                    })),
                    Token::QuestionMark => {
                        let on_true = self.must_parse_precedence(token_precedence.clone())?;
                        match self.next()? {
                            Some(v) => match v {
                                Token::Colon => (),
                                _ => {
                                    return Err(Error::from(ErrorKind::ExpectingTernaryElse)
                                        .with(self.context(v)))
                                }
                            },
                            None => return Ok(None),
                        };

                        let on_false = self.must_parse_precedence(token_precedence)?;

                        Expr::Condition(Box::new(Condition {
                            condition: left,
                            on_true: on_true,
                            on_false: on_false,
                        }))
                    }
                    Token::Comma | Token::Colon | Token::RightParen => break,
                    Token::LeftParen => {
                        let next = self.next()?;
                        let new_left = Expr::Infix(Box::new(Infix {
                            left: left,
                            operator: match next {
                                Some(v) => match v {
                                    Token::Operator(o) => o,
                                    _ => {
                                        return Err(Error::from(ErrorKind::InvalidInfixOperator)
                                            .with(self.context(v)))
                                    }
                                },
                                None => return Ok(None),
                            },
                            right: self.must_parse_precedence(Precedence::Separator)?,
                        }));

                        match self.next()? {
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
                    _ => unreachable!(),
                },
                None => return Ok(None),
            };

            token_precedence = expect_infix!(self, left);
        }

        Ok(Some(left))
    }
}
