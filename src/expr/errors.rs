use expr::parser::Context;
use expr::types::*;
use std::fmt;
use std::result;

pub type ContextResult<'a, T> = result::Result<T, (Context<'a>, Error)>;

error_chain!{
    errors {
        InvalidCharacter(column : usize, unread : String) {
            description("unable to parse string, invalid character"),
            display("1:{} unable to parse string, invalid character \"{}\"", column, unread.chars().nth(0).unwrap_or(' ')),
        }

        InvalidToken(column : usize, tok : String) {
            description("invalid token"),
            display("1:{} invalid token \"{}\"", column, tok),
        }

        UnexpectedPrefixOperator(column : usize, tok : String) {
            description("unexpected prefix operator"),
            display("1:{} Unexpected operator \"{}\". Expecting one of ~, !, +, -, ++, --, a number, or a variable.", column, tok),
        }

        UnexpectedInfixOperator(column : usize, tok : String) {
            description("unexpected infix operator"),
            display("1:{} Unexpected infix operator \"{}\". Expecting an operator like +, -, *, %, etc.", column, tok),
        }

        ExpectingTernaryElse(column : usize, tok : String) {
            description("expecting a ternary condition 'else' block starting with ':'")
            display("1:{} expecting the conditional 'else' block (starting with ':'), found \"{}\"", column, tok)
        }

        ExpectingRightParentheses(column : usize, tok : String) {
            description("expecting right parentheses (')')")
            display("1:{} expecting right parentheses (')'), found \"{}\"", column, tok)
        }

        UnexpectedEof(column: usize) {
            description("unexpected end of expression"),
            display("1:{} unexpected end of expression", column),
        }
    }
}
