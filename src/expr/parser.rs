use expr::errors::*;
use expr::lexer::TokenStream;
use expr::types::*;
use std::fmt;

pub struct Parser<'a> {
    tokens: TokenStream<'a>,
    peek: Option<Token<'a>>,
    error_ctx: Context<'a>,
    column: usize,
}

pub struct Context<'a> {
    full: &'a str,
    unread: &'a str,
    token: Token<'a>,
    column: usize,
}

pub struct TokenHighlight<'a> {
    ctx: Context<'a>,
}

pub struct UnreadHighlight<'a> {
    ctx: Context<'a>,
}

macro_rules! fail_parse {
    ($_self:ident, $err:ident, $fail_tok:expr) => {{
        $_self.error_ctx.token = $fail_tok;
        $_self.error_ctx.column = $_self.column();
        $_self.error_ctx.unread = $_self.tokens.unread();
        return Err(ErrorKind::$err($_self.error_ctx.column, $fail_tok.to_string()).into());
    }};
}

macro_rules! expect_infix {
    ($_self:ident, $working_tree:expr) => {{
        match Precedence::from_token(match $_self.peek() {
            Some(v) => v,
            None => return Ok(Some($working_tree)),
        }) {
            Some(v) => v,
            None => fail_parse!(
                $_self,
                UnexpectedInfixOperator,
                $_self.peek().clone().unwrap()
            ),
        }
    }};
}

pub fn parse<T: AsRef<str>>(s: T) -> Result<Expr> {
    Parser::from(s.as_ref()).parse()
}

pub fn parse_ctx<'a>(s: &'a str) -> ContextResult<'a, Expr> {
    let mut p = Parser::from(s);
    p.parse().map_err(|e| (p.error_ctx, e))
}

impl<'a> Parser<'a> {
    pub fn new(t: TokenStream<'a>) -> Parser<'a> {
        Parser {
            peek: None,
            error_ctx: Context {
                full: t.full(),
                unread: t.unread(),
                token: Token::Comma,
                column: 0,
            },
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
                    self.error_ctx.column = self.tokens.column();
                    self.error_ctx.unread = self.tokens.unread();
                    return Err(e);
                }
            }),
            None => None,
        };
        Ok(tok)
    }

    fn must_parse_precedence(&mut self, p: Precedence) -> Result<Expr> {
        match try!(self.parse_precedence(p)) {
            Some(v) => Ok(v),
            None => {
                self.error_ctx.column = self.tokens.full().len();
                self.error_ctx.unread = self.tokens.unread();
                Err(ErrorKind::UnexpectedEof(self.error_ctx.column).into())
            }
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
                        return Err(ErrorKind::UnexpectedPrefixOperator(
                            self.column(),
                            op.to_string(),
                        ).into());
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
                            _ => fail_parse!(self, ExpectingRightParentheses, v.clone()),
                        },
                        None => fail_parse!(self, ExpectingRightParentheses, v.clone()),
                    }
                }
                _ => fail_parse!(self, InvalidToken, v.clone()),
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
                                _ => fail_parse!(self, ExpectingTernaryElse, v.clone()),
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
                                    _ => fail_parse!(self, UnexpectedInfixOperator, v.clone()),
                                },
                                None => return Ok(None),
                            },
                            right: self.must_parse_precedence(Precedence::Separator)?,
                        }));

                        match self.next()? {
                            Some(v) => match v {
                                Token::RightParen => new_left,
                                _ => fail_parse!(self, ExpectingRightParentheses, v.clone()),
                            },
                            None => fail_parse!(self, ExpectingRightParentheses, v.clone()),
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

impl<'a> Context<'a> {
    pub fn token_highlighter(self) -> TokenHighlight<'a> {
        TokenHighlight { ctx: self }
    }

    pub fn unread_highlighter(self) -> UnreadHighlight<'a> {
        UnreadHighlight { ctx: self }
    }

    pub fn full_string(&self) -> String {
        self.full.to_string()
    }

    pub fn unread_string(&self) -> String {
        self.unread.to_string()
    }

    pub fn line(&self) -> usize {
        1
    }

    pub fn column(&self) -> usize {
        self.column
    }

    pub fn token(&self) -> Token<'a> {
        self.token.clone()
    }
}

impl<'a> fmt::Display for TokenHighlight<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let token = self.ctx.token().to_string();
        let lineno = self.ctx.line().to_string();
        let prefix = format!("{} |", " ".repeat(lineno.len()));
        write!(fmt, "{}\n", prefix)?;
        write!(fmt, "{} |  {}\n", lineno, self.ctx.full)?;
        write!(
            fmt,
            "{}  {}{}\n",
            prefix,
            " ".repeat(self.ctx.column()),
            "^".repeat(token.len())
        )
    }
}

impl<'a> fmt::Display for UnreadHighlight<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let lineno = self.ctx.line().to_string();
        let prefix = format!("{} |", " ".repeat(lineno.len()));

        write!(fmt, "{}\n", prefix)?;
        write!(fmt, "{} |  {}\n", lineno, self.ctx.full)?;
        write!(
            fmt,
            "{}{}{}\n",
            prefix,
            " ".repeat(self.ctx.column() - 1),
            "^".repeat(self.ctx.unread.len())
        )
    }
}
