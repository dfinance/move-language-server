use rowan::{GreenNodeBuilder, TextRange, TextSize};

use crate::parser::lexer;
use crate::parser::lexer::Token;
use crate::parser::syntax_kind::SyntaxKind;

pub fn syntax_kind_at(pos: usize, tokens: &[Token]) -> SyntaxKind {
    tokens.get(pos).map(|t| t.kind).unwrap_or(SyntaxKind::EOF)
}

pub struct Tokens<'i> {
    text: &'i str,
    start_offsets: Vec<TextSize>,
    tokens: Vec<Token>,
    // current token kind and current position
    curr: (SyntaxKind, usize),
}

impl<'t> Tokens<'t> {
    pub fn new(text: &'t str, raw_tokens: &'t [Token]) -> Tokens<'t> {
        let mut tokens = vec![];
        let mut start_offsets = vec![];
        let mut last_token_offset = TextSize::zero();
        for &token in raw_tokens.iter() {
            if !token.kind.is_trivia() {
                tokens.push(token);
                start_offsets.push(last_token_offset);
            }
            last_token_offset += token.len;
        }
        let first_kind = syntax_kind_at(0, &tokens);
        Tokens {
            text,
            start_offsets,
            tokens,
            curr: (first_kind, 0),
        }
    }

    pub fn current(&self) -> SyntaxKind {
        self.curr.0
    }

    pub fn current_text(&self) -> &str {
        let pos = self.curr.1;
        let start = self.start_offsets.get(pos).unwrap();
        let end = self.start_offsets.get(pos).unwrap_or(start);
        &self.text[TextRange::new(*start, *end)]
    }

    pub fn bump(&mut self) {
        if self.curr.0 == SyntaxKind::EOF {
            return;
        }
        let pos = self.curr.1 + 1;
        self.curr = (syntax_kind_at(pos, &self.tokens), pos);
    }
}
