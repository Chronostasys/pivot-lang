use nom::{
    combinator::{map_res, opt},
    multi::many0,
    sequence::{preceded, tuple},
    IResult,
};
use nom_locate::LocatedSpan;
type Span<'a> = LocatedSpan<&'a str>;
use crate::{ast::node::pkg::UseNode, ast::tokens::TokenType};
use internal_macro::test_parser;

use super::*;

/// ```enbf
/// use_statement = "use" identifier ("::" identifier)* ;
/// ```
#[test_parser("use a::b")]
#[test_parser("use a::")]
#[test_parser("use a")]
#[test_parser("use a:")]
pub fn use_statement(input: Span) -> IResult<Span, Box<NodeEnum>> {
    map_res(
        preceded(
            tag_token(TokenType::USE),
            delspace(tuple((
                identifier,
                many0(preceded(tag_token(TokenType::DOUBLE_COLON), identifier)),
                opt(tag_token(TokenType::DOUBLE_COLON)),
                opt(tag_token(TokenType::COLON)),
            ))),
        ),
        |(first, rest, opt, opt2)| {
            let mut path = vec![first];
            path.extend(rest);
            let mut range = path.first().unwrap().range().start.to(path
                .last()
                .unwrap()
                .range()
                .end);
            if opt.is_some() {
                range = range.start.to(opt.unwrap().1.end);
            }
            if opt2.is_some() {
                range = range.start.to(opt2.unwrap().1.end);
            }
            res_enum(NodeEnum::UseNode(UseNode {
                ids: path,
                range,
                complete: opt.is_none() && opt2.is_none(),
                singlecolon: opt2.is_some(),
            }))
        },
    )(input)
}