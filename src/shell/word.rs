use env;
use expr;
use nom;
use nom::types::CompleteStr;

#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    Tilde,
    WildcardString,
    WildcardChar,
    Unquoted(Word<'a>),
    Quoted(Word<'a>),
    Multi(Vec<Word<'a>>),
    Regex,
    Escape(char),
    Parameter(&'a str, char, Word<'a>),
    Variable(&'a str),
    Command(Word<'a>),
    Expr(Word<'a>),
    QuotedCommand(&'a str),
    Slice(&'a str),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Word<'a> {
    parts: Vec<Token<'a>>,
}

pub fn sigiled_expression<'a>(i: CompleteStr<'a>) -> nom::IResult<CompleteStr<'a>, Token<'a>, u32> {
    alt!(i,
        delimited!(tag!("(("), expression_word, tag!("))")) => {|x| Token::Expr(x)}
        | take_while!(|x| (x >= 'a' && x <= 'z') || (x >= 'A' && x <= 'Z') || x == '_') => {|x : CompleteStr<'a>| Token::Variable(x.0)}
    )
}

pub fn expression_word<'a>(i: CompleteStr<'a>) -> nom::IResult<CompleteStr<'a>, Word<'a>, u32> {
    map!(
        i,
        many0!(preceded!(
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
                | take_until_either1!(")\"") => {|x : CompleteStr<'a>| Token::Slice(x.0)}
            )
        )),
        |x| Word::from(x)
    )
}

pub fn double_quoted_token<'a>(
    i: CompleteStr<'a>,
) -> nom::IResult<CompleteStr<'a>, Token<'a>, u32> {
    alt!(i,
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
        | take_until_either1!("\\$\"") => {|x : CompleteStr<'a>| Token::Slice(x.0)}
    )
}

pub fn single_quoted_token<'a>(
    i: CompleteStr<'a>,
) -> nom::IResult<CompleteStr<'a>, Token<'a>, u32> {
    alt!(i,
        preceded!(char!('\\'),
            alt!(
                char!('\'')
                | char!('\\')
            )) => {|c| Token::Escape(c)}
        | take_until_either1!("'") => {|x : CompleteStr<'a>| Token::Slice(x.0)}
    )
}

pub fn unquoted_token<'a>(i: CompleteStr<'a>) -> nom::IResult<CompleteStr<'a>, Token<'a>, u32> {
    alt!(i,
        preceded!(char!('\\'),
            alt!(
                char!('"')
                | char!('\\')
                | char!('n')
                | char!('\n')
                | char!('\'')
                | char!('t')
                | char!('$')
                | char!('`')
                | char!(' ')
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
        | take_while1!(|c : char| c != '"' && c != '\'' && c != '\\' && c != '$' && !nom::is_space(c as u8)) => {|x : CompleteStr<'a>| Token::Slice(x.0)}
    )
}

named!(pub word<CompleteStr, Word>,
    map!(
        many0!(alt!(
            unquoted_token
            | delimited!(char!('\''), many0!(single_quoted_token), char!('\'')) => {|x| Token::Quoted(Word::from(x))}
            | delimited!(char!('"'), many0!(double_quoted_token), char!('"')) => {|x| Token::Quoted(Word::from(x))}
        )),
        {|x| Word{parts : x}}
    )
);

impl<'a> Word<'a> {
    pub fn new() -> Word<'a> {
        Word { parts: Vec::new() }
    }

    pub fn from(toks: Vec<Token<'a>>) -> Word<'a> {
        Word { parts: toks }
    }

    pub fn parse(s: &'a str) -> Word<'a> {
        word(CompleteStr(s)).unwrap().1
    }

    pub fn compile(&self, vars: &mut env::Variables) -> String {
        use std::ffi::OsString;

        let mut s = String::new(); // TODO set capacity to avoid reallocations
        for x in &self.parts {
            match x {
                Token::Tilde => {
                    s.push_str(vars.value(&OsString::from("HOME")).to_str().unwrap_or(""))
                }
                Token::Slice(v) => s.push_str(v),
                Token::Expr(v) => s.push_str(&expr::eval(&v.compile(vars), vars).unwrap()), // TODO error handling
                Token::Variable(v) => {
                    s.push_str(vars.value(&OsString::from(v)).to_str().unwrap_or(""))
                }
                Token::Escape(v) => s.push(*v),
                Token::Quoted(v) => s.extend(v.compile(vars).chars()),
                _ => unimplemented!(),
            };
        }
        s
    }
}
