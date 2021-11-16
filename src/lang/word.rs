use crate::env;
use crate::expr;
use crate::lang::{ErrorKind, Result};
use failure::ResultExt;
use nom;
use nom::types::CompleteStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Token {
    Tilde,
    WildcardString,
    WildcardChar,
    Unquoted(Word),
    Quoted(Word),
    Multi(Vec<Word>),
    Regex,
    Escape(char),
    Parameter(String, char, Word),
    Variable(String),
    Command(Word),
    Expr(Word),
    QuotedCommand(String),
    Slice(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Word {
    parts: Vec<Token>,
}

named!(
    pub sigiled_expression<CompleteStr, Token>,
    alt!(
        delimited!(tag!("(("), expression_word, tag!("))")) => {|x| Token::Expr(x)}
        | take_while!(|x| (x >= 'a' && x <= 'z') || (x >= 'A' && x <= 'Z') || x == '_') => {|x : CompleteStr| Token::Variable(x.0.to_string())}
    )
);

named!(
    pub expression_word<CompleteStr, Word>,
    map!(many0!(preceded!(
            not!(tag!("))")),
            alt!(
                preceded!(char!('\\'),
                    alt!(
                        char!('"')
                        | char!('\\')
                        | char!('n')
                        | char!('t')
                        | char!('$')
                        | char!('`')
                    )
                ) => {|c| Token::Escape(c)}
                | delimited!(
                    char!('"'),
                        many0!(double_quoted_token),
                    char!('"')
                ) => { |c| Token::Quoted(Word::from(c)) }
                | preceded!(char!('$'), sigiled_expression) => {|w| w}
                | take_until_either1!(")\"") => {|x : CompleteStr| Token::Slice(x.0.to_string())}
            )
        )),
        |x| Word::from(x)
    )
);

named!{
    pub double_quoted_token<CompleteStr, Token>,
    alt!(
        preceded!(char!('\\'),
            alt!(
                char!('"')
                | char!('\\')
                | char!('n')
                | char!('t')
                | char!('$')
                | char!('`')
            )
        ) => {|c| Token::Escape(c)}
        | preceded!(char!('$'), sigiled_expression) => {|w| w}
        | take_until_either1!("\\$\"") => {|x : CompleteStr| Token::Slice(x.0.to_string())}
    )
}

named!(
    pub single_quoted_token<CompleteStr, Token>,
    alt!(preceded!(char!('\\'),
            alt!(
                char!('\'')
                | char!('\\')
            )) => {|c| Token::Escape(c)}
        | take_until_either1!("'") => {|x : CompleteStr| Token::Slice(x.0.to_string())}
    )
);

named!(
    pub unquoted_token<CompleteStr, Token>,
    alt!(preceded!(char!('\\'),
            alt!(
                char!('"')
                | char!('\\')
                | char!('|')
                | char!('n')
                | char!('\n')
                | char!('\'')
                | char!('t')
                | char!('$')
                | char!('`')
                | char!('&')
                | char!('{')
                | char!('}')
            )
        ) => {|c| Token::Escape(c)}
        | preceded!(char!('$'), sigiled_expression) => {|w| w}
        | delimited!(
            char!('"'),
                many0!(double_quoted_token),
            char!('"')
        )  => { |c| Token::Quoted(Word::from(c)) }
        | delimited!(
            char!('\''),
                many0!(single_quoted_token),
            char!('\'')
        ) => { |c| Token::Quoted(Word::from(c)) }
        | take_while1!(|c : char| c != '&'  && c != '"' && c != '{' && c != '}' && c != '\'' &&  c != '|' && c != ';' && c != '\n' && c != '\\' && c != '$' && !nom::is_space(c as u8)) => {|x : CompleteStr| Token::Slice(x.0.to_string())}
    )
);

named!(pub word<CompleteStr, Word>,
    map!(many0!(alt!(
            unquoted_token
            | delimited!(char!('\''), many0!(single_quoted_token), char!('\'')) => {|x| Token::Quoted(Word::from(x))}
            | delimited!(char!('"'), many0!(double_quoted_token), char!('"')) => {|x| Token::Quoted(Word::from(x))}
        )),
        {|x| Word{parts : x}}
    )
);

impl<T> From<T> for Word
where
    T: IntoIterator<Item = Token>,
{
    fn from(v: T) -> Word {
        Word {
            parts: v.into_iter().collect(),
        }
    }
}

impl Word {
    pub fn new() -> Word {
        Word { parts: Vec::new() }
    }
    pub fn parse<T: AsRef<str>>(s: T) -> Word {
        word(CompleteStr(s.as_ref())).unwrap().1
    }

    pub fn compile(&self, vars: &mut env::Variables) -> Result<String> {
        use std::ffi::OsString;

        let mut s = String::new(); // TODO set capacity to avoid reallocations
        for x in &self.parts {
            match x {
                Token::Tilde => {
                    s.push_str(vars.value(&OsString::from("HOME")).to_str().unwrap_or(""))
                }
                Token::Slice(v) => s.push_str(v),
                Token::Expr(v) => {
                    let evaluated: String = expr::eval(v.compile(vars)?.as_str(), vars)
                        .context(ErrorKind::ExpressionError)?;
                    s.push_str(&evaluated)
                }
                Token::Variable(v) => {
                    s.push_str(vars.value(&OsString::from(v)).to_str().unwrap_or(""))
                }
                Token::Escape(v) => s.push(match *v {
                    'n' => '\n',
                    't' => '\t',
                    '"' => '"',
                    '\'' => '\'',
                    ' ' => ' ',
                    '$' => '$',
                    '|' => '|',
                    '\n' => '\n',
                    '`' => '`',
                    _ => '\u{FFFD}',
                }),
                Token::Quoted(v) => s.extend(v.compile(vars)?.chars()),
                _ => unimplemented!(),
            };
        }
        Ok(s)
    }
}
