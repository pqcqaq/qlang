use ql_analysis::Analysis;
use ql_lexer::{Token, TokenKind, lex};
use ql_span::Span;
use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemKind, CompletionResponse,
    CompletionTextEdit, Documentation, FoldingRange, Hover, HoverContents, InlayHint,
    InlayHintKind, InlayHintLabel, InlayHintTooltip, InsertTextFormat, MarkupContent, MarkupKind,
    ParameterInformation, ParameterLabel, Position, Range, SelectionRange, SignatureHelp,
    SignatureInformation, TextEdit,
};

use crate::bridge::{position_to_offset, span_to_range};

struct KeywordInfo {
    category: &'static str,
    summary: &'static str,
    example: &'static str,
}

pub fn hover_for_keyword(source: &str, position: Position) -> Option<Hover> {
    let offset = position_to_offset(source, position)?;
    let keyword = keyword_target_at_offset(source, offset)?;
    let info = keyword_info(keyword.kind)?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "**{} keyword `{}`**\n\n{}\n\n```ql\n{}\n```",
                info.category, keyword.text, info.summary, info.example,
            ),
        }),
        range: Some(span_to_range(source, keyword.span)),
    })
}

pub fn completion_for_keywords(source: &str, position: Position) -> Option<CompletionResponse> {
    let offset = position_to_offset(source, position)?;
    let replace_span = identifier_prefix_span(source, offset);
    let prefix = &source[replace_span.start..offset];
    let items = keyword_completion_items(source, replace_span)
        .into_iter()
        .filter(|item| item.label.starts_with(prefix))
        .collect::<Vec<_>>();
    (!items.is_empty()).then_some(CompletionResponse::Array(items))
}

pub fn resolve_completion_item(mut item: LspCompletionItem) -> LspCompletionItem {
    if item.detail.is_none()
        && let Some(info) = keyword_kind_for_label(&item.label).and_then(keyword_info)
    {
        item.detail = Some(format!("{} keyword", info.category));
    }

    if item.documentation.is_none() {
        if let Some(info) = keyword_kind_for_label(&item.label).and_then(keyword_info) {
            item.documentation = Some(keyword_documentation(info));
        } else if let Some(detail) = item
            .detail
            .as_ref()
            .filter(|detail| !detail.trim().is_empty())
        {
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```ql\n{detail}\n```"),
            }));
        }
    }

    item
}

pub fn signature_help_for_analysis(
    source: &str,
    analysis: &Analysis,
    position: Position,
) -> Option<SignatureHelp> {
    let offset = position_to_offset(source, position)?;
    let (tokens, _) = lex(source);
    let (open_index, callee_index) = active_call_indexes(&tokens, offset)?;
    let callee = tokens.get(callee_index)?;
    let hover = analysis.hover_at(callee.span.start)?;
    let mut parameters = signature_parameters(&hover.detail);
    if is_method_call(&tokens, callee_index) && parameters.first().is_some_and(|p| p == "self") {
        parameters.remove(0);
    }
    let active_parameter = active_parameter_index(&tokens, open_index, offset)
        .min(parameters.len().saturating_sub(1)) as u32;
    let parameter_info = parameters
        .iter()
        .map(|parameter| ParameterInformation {
            label: ParameterLabel::Simple(parameter.clone()),
            documentation: None,
        })
        .collect::<Vec<_>>();
    Some(SignatureHelp {
        signatures: vec![SignatureInformation {
            label: hover.detail.clone(),
            documentation: Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: format!("```ql\n{}\n```", hover.detail),
            })),
            parameters: Some(parameter_info),
            active_parameter: Some(active_parameter),
        }],
        active_signature: Some(0),
        active_parameter: Some(active_parameter),
    })
}

pub fn inlay_hints_for_analysis(
    source: &str,
    analysis: &Analysis,
    range: Range,
) -> Option<Vec<InlayHint>> {
    let (tokens, _) = lex(source);
    let mut hints = Vec::new();
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        let token = &tokens[index];
        if !matches!(token.kind, TokenKind::Let | TokenKind::Var) {
            index += 1;
            continue;
        }
        let Some(name) = tokens
            .get(index + 1)
            .filter(|token| token.kind == TokenKind::Ident)
        else {
            index += 1;
            continue;
        };
        if has_explicit_type_before_eq(&tokens[index + 2..]) {
            index += 1;
            continue;
        }
        let Some(hover) = analysis.hover_at(name.span.start) else {
            index += 1;
            continue;
        };
        let Some(ty) = hover.ty.filter(|ty| !ty.trim().is_empty()) else {
            index += 1;
            continue;
        };
        let position = span_to_range(source, Span::new(name.span.end, name.span.end)).start;
        if range_contains_position(range, position) {
            hints.push(InlayHint {
                position,
                label: InlayHintLabel::String(format!(": {ty}")),
                kind: Some(InlayHintKind::TYPE),
                text_edits: None,
                tooltip: Some(InlayHintTooltip::String("Inferred local type".to_owned())),
                padding_left: Some(false),
                padding_right: Some(true),
                data: None,
            });
        }
        index += 1;
    }
    (!hints.is_empty()).then_some(hints)
}

pub fn folding_ranges_for_source(source: &str) -> Option<Vec<FoldingRange>> {
    let (tokens, _) = lex(source);
    let mut stack = Vec::<&Token>::new();
    let mut ranges = Vec::new();
    for token in &tokens {
        match token.kind {
            TokenKind::LBrace | TokenKind::LBracket => stack.push(token),
            TokenKind::RBrace | TokenKind::RBracket => {
                let Some(start) = stack.pop() else {
                    continue;
                };
                if !matching_delimiters(start.kind, token.kind) {
                    continue;
                }
                let range = span_to_range(source, Span::new(start.span.start, token.span.end));
                if range.start.line < range.end.line {
                    ranges.push(FoldingRange {
                        start_line: range.start.line,
                        start_character: Some(range.start.character),
                        end_line: range.end.line,
                        end_character: Some(range.end.character),
                        kind: None,
                        collapsed_text: None,
                    });
                }
            }
            _ => {}
        }
    }
    (!ranges.is_empty()).then_some(ranges)
}

pub fn selection_ranges_for_source(
    source: &str,
    positions: Vec<Position>,
) -> Option<Vec<SelectionRange>> {
    let selections = positions
        .into_iter()
        .filter_map(|position| selection_range_for_position(source, position))
        .collect::<Vec<_>>();
    (!selections.is_empty()).then_some(selections)
}

fn keyword_info(kind: TokenKind) -> Option<KeywordInfo> {
    Some(match kind {
        TokenKind::Package => KeywordInfo {
            category: "declaration",
            summary: "Declares the current package identity.",
            example: "package app.core",
        },
        TokenKind::Use => KeywordInfo {
            category: "declaration",
            summary: "Imports a visible symbol or module path into the current scope.",
            example: "use core.math.add as add",
        },
        TokenKind::Pub => KeywordInfo {
            category: "modifier",
            summary: "Exports an item from the package interface.",
            example: "pub fn run() -> Int { return 0 }",
        },
        TokenKind::Const => KeywordInfo {
            category: "declaration",
            summary: "Declares an immutable compile-time value.",
            example: "const LIMIT = 8",
        },
        TokenKind::Static => KeywordInfo {
            category: "declaration",
            summary: "Declares a package-level immutable runtime value.",
            example: "static DEFAULT = 1",
        },
        TokenKind::Let => KeywordInfo {
            category: "declaration",
            summary: "Creates an immutable local binding.",
            example: "let value = compute()",
        },
        TokenKind::Var => KeywordInfo {
            category: "declaration",
            summary: "Creates a mutable local binding.",
            example: "var total = 0",
        },
        TokenKind::Fn => KeywordInfo {
            category: "declaration",
            summary: "Declares a function or method.",
            example: "fn add(left: Int, right: Int) -> Int { return left + right }",
        },
        TokenKind::Async => KeywordInfo {
            category: "modifier",
            summary: "Marks a function body as asynchronous.",
            example: "async fn load() -> Int { return await fetch() }",
        },
        TokenKind::Await => KeywordInfo {
            category: "control",
            summary: "Waits for an async operation inside an async context.",
            example: "let value = await load()",
        },
        TokenKind::Spawn => KeywordInfo {
            category: "control",
            summary: "Starts an async task.",
            example: "let task = spawn load()",
        },
        TokenKind::Defer => KeywordInfo {
            category: "control",
            summary: "Runs cleanup at scope exit.",
            example: "defer close(handle)",
        },
        TokenKind::Return => KeywordInfo {
            category: "control",
            summary: "Returns a value from the current function.",
            example: "return value",
        },
        TokenKind::Break => KeywordInfo {
            category: "control",
            summary: "Exits the nearest loop.",
            example: "break",
        },
        TokenKind::Continue => KeywordInfo {
            category: "control",
            summary: "Skips to the next loop iteration.",
            example: "continue",
        },
        TokenKind::If => KeywordInfo {
            category: "control",
            summary: "Starts a conditional expression.",
            example: "if ready { run() } else { wait() }",
        },
        TokenKind::Else => KeywordInfo {
            category: "control",
            summary: "Starts the fallback branch of an `if` expression.",
            example: "if ok { value } else { fallback }",
        },
        TokenKind::Match => KeywordInfo {
            category: "control",
            summary: "Branches by pattern matching.",
            example: "match state { _ => 0 }",
        },
        TokenKind::For => KeywordInfo {
            category: "control",
            summary: "Iterates over an iterable value.",
            example: "for item in items { consume(item) }",
        },
        TokenKind::While => KeywordInfo {
            category: "control",
            summary: "Repeats while a condition stays true.",
            example: "while running { tick() }",
        },
        TokenKind::Loop => KeywordInfo {
            category: "control",
            summary: "Starts an unconditional loop.",
            example: "loop { break }",
        },
        TokenKind::In => KeywordInfo {
            category: "operator",
            summary: "Separates a loop binding from its iterable expression.",
            example: "for item in items { item }",
        },
        TokenKind::Where => KeywordInfo {
            category: "constraint",
            summary: "Introduces generic or trait constraints.",
            example: "fn id[T](value: T) -> T where T satisfies Copy { return value }",
        },
        TokenKind::Struct => KeywordInfo {
            category: "type",
            summary: "Declares a named product type.",
            example: "struct Point { x: Int, y: Int }",
        },
        TokenKind::Data => KeywordInfo {
            category: "type",
            summary: "Marks a struct declaration as data-oriented.",
            example: "data struct Point { x: Int }",
        },
        TokenKind::Enum => KeywordInfo {
            category: "type",
            summary: "Declares a tagged union type.",
            example: "enum State { Ready, Failed(String) }",
        },
        TokenKind::Trait => KeywordInfo {
            category: "type",
            summary: "Declares a behavior interface.",
            example: "trait Show { fn show(self) -> String }",
        },
        TokenKind::Impl => KeywordInfo {
            category: "type",
            summary: "Implements methods or traits for a type.",
            example: "impl Counter { fn get(self) -> Int { return self.value } }",
        },
        TokenKind::Extend => KeywordInfo {
            category: "type",
            summary: "Adds extension methods to a type.",
            example: "extend Counter { fn reset(self) -> Counter { return self } }",
        },
        TokenKind::Type => KeywordInfo {
            category: "type",
            summary: "Declares a type alias.",
            example: "type Count = Int",
        },
        TokenKind::Opaque => KeywordInfo {
            category: "type",
            summary: "Declares an alias that hides its representation outside the defining package.",
            example: "opaque type UserId = Int",
        },
        TokenKind::Extern => KeywordInfo {
            category: "interop",
            summary: "Declares foreign ABI functions or exported ABI items.",
            example: "extern \"c\" fn puts(value: String) -> Int",
        },
        TokenKind::Unsafe => KeywordInfo {
            category: "modifier",
            summary: "Marks an operation or block as requiring explicit unsafe intent.",
            example: "unsafe { call_raw() }",
        },
        TokenKind::Is => KeywordInfo {
            category: "operator",
            summary: "Tests a value against a type or pattern.",
            example: "value is Some",
        },
        TokenKind::As => KeywordInfo {
            category: "operator",
            summary: "Introduces an alias or cast-like projection depending on context.",
            example: "use core.math.add as add",
        },
        TokenKind::Satisfies => KeywordInfo {
            category: "constraint",
            summary: "Checks that a type satisfies a trait constraint.",
            example: "where T satisfies Show",
        },
        TokenKind::NoneKw => KeywordInfo {
            category: "literal",
            summary: "Represents the empty option value.",
            example: "return none",
        },
        TokenKind::TrueKw => KeywordInfo {
            category: "literal",
            summary: "Boolean true literal.",
            example: "let ok = true",
        },
        TokenKind::FalseKw => KeywordInfo {
            category: "literal",
            summary: "Boolean false literal.",
            example: "let ok = false",
        },
        TokenKind::SelfKw => KeywordInfo {
            category: "receiver",
            summary: "References the current method receiver.",
            example: "return self.value",
        },
        TokenKind::MoveKw => KeywordInfo {
            category: "modifier",
            summary: "Forces a closure or value use to move captured state.",
            example: "let f = move || value",
        },
        _ => return None,
    })
}

fn token_at_offset(source: &str, offset: usize) -> Option<Token> {
    let (tokens, _) = lex(source);
    tokens.into_iter().find(|token| {
        token.span.contains(offset) || token.span.end == offset && token.span.start < token.span.end
    })
}

struct KeywordTarget {
    kind: TokenKind,
    text: String,
    span: Span,
}

fn keyword_target_at_offset(source: &str, offset: usize) -> Option<KeywordTarget> {
    if let Some(token) = token_at_offset(source, offset) {
        if keyword_info(token.kind).is_some() {
            return Some(KeywordTarget {
                kind: token.kind,
                text: token.text,
                span: token.span,
            });
        }
        if token.kind == TokenKind::Ident
            && source.get(token.span.start..token.span.end) == Some(token.text.as_str())
            && let Some(kind) = keyword_kind_for_label(&token.text)
        {
            return Some(KeywordTarget {
                kind,
                text: token.text,
                span: token.span,
            });
        }
    }

    let span = identifier_word_span_at_offset(source, offset)?;
    if is_escaped_identifier_word(source, span) {
        return None;
    }
    let text = source.get(span.start..span.end)?;
    let kind = keyword_kind_for_label(text)?;
    Some(KeywordTarget {
        kind,
        text: text.to_owned(),
        span,
    })
}

fn identifier_word_span_at_offset(source: &str, offset: usize) -> Option<Span> {
    let offset = offset.min(source.len());
    let mut start = offset;
    while start > 0 {
        let Some(ch) = source[..start].chars().next_back() else {
            break;
        };
        if !(ch == '_' || ch.is_ascii_alphanumeric()) {
            break;
        }
        start -= ch.len_utf8();
    }

    let mut end = offset;
    while end < source.len() {
        let Some(ch) = source[end..].chars().next() else {
            break;
        };
        if !(ch == '_' || ch.is_ascii_alphanumeric()) {
            break;
        }
        end += ch.len_utf8();
    }

    (start < end).then_some(Span::new(start, end))
}

fn is_escaped_identifier_word(source: &str, span: Span) -> bool {
    source[..span.start].chars().next_back() == Some('`')
        || source[span.end..].chars().next() == Some('`')
}

fn identifier_prefix_span(source: &str, offset: usize) -> Span {
    let mut start = offset;
    while start > 0 {
        let Some(ch) = source[..start].chars().next_back() else {
            break;
        };
        if !(ch == '_' || ch.is_ascii_alphanumeric()) {
            break;
        }
        start -= ch.len_utf8();
    }
    Span::new(start, offset)
}

fn keyword_completion_items(source: &str, replace_span: Span) -> Vec<LspCompletionItem> {
    let mut items = keyword_completion_labels()
        .into_iter()
        .filter_map(|label| {
            let info = keyword_info(keyword_kind_for_label(label)?)?;
            Some(keyword_completion_item(
                source,
                replace_span,
                label,
                label,
                info,
            ))
        })
        .collect::<Vec<_>>();
    for (label, snippet) in keyword_snippets() {
        if let Some(info) = keyword_info(keyword_kind_for_label(label).unwrap_or(TokenKind::Fn)) {
            let mut item = keyword_completion_item(source, replace_span, label, snippet, info);
            item.kind = Some(CompletionItemKind::SNIPPET);
            item.insert_text_format = Some(InsertTextFormat::SNIPPET);
            items.push(item);
        }
    }
    items
}

fn keyword_completion_item(
    source: &str,
    replace_span: Span,
    label: &str,
    insert_text: &str,
    info: KeywordInfo,
) -> LspCompletionItem {
    LspCompletionItem {
        label: label.to_owned(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(format!("{} keyword", info.category)),
        documentation: Some(keyword_documentation(info)),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(
            span_to_range(source, replace_span),
            insert_text.to_owned(),
        ))),
        ..LspCompletionItem::default()
    }
}

fn keyword_documentation(info: KeywordInfo) -> Documentation {
    Documentation::MarkupContent(MarkupContent {
        kind: MarkupKind::Markdown,
        value: format!("{}\n\n```ql\n{}\n```", info.summary, info.example),
    })
}

fn keyword_completion_labels() -> Vec<&'static str> {
    vec![
        "package",
        "use",
        "pub",
        "const",
        "static",
        "let",
        "var",
        "fn",
        "async",
        "await",
        "spawn",
        "defer",
        "return",
        "break",
        "continue",
        "if",
        "else",
        "match",
        "for",
        "while",
        "loop",
        "in",
        "where",
        "struct",
        "data",
        "enum",
        "trait",
        "impl",
        "extend",
        "type",
        "opaque",
        "extern",
        "unsafe",
        "is",
        "as",
        "satisfies",
        "none",
        "true",
        "false",
        "self",
        "move",
    ]
}

fn keyword_snippets() -> Vec<(&'static str, &'static str)> {
    vec![
        ("fn", "fn ${1:name}(${2}) -> ${3:Void} {\n    $0\n}"),
        ("if", "if ${1:condition} {\n    $0\n}"),
        ("match", "match ${1:value} {\n    ${2:_} => $0\n}"),
        ("for", "for ${1:item} in ${2:items} {\n    $0\n}"),
        ("struct", "struct ${1:Name} {\n    ${2:field}: ${3:Int},\n}"),
        ("enum", "enum ${1:Name} {\n    ${2:Variant},\n}"),
        ("impl", "impl ${1:Type} {\n    $0\n}"),
    ]
}

fn keyword_kind_for_label(label: &str) -> Option<TokenKind> {
    Some(match label {
        "package" => TokenKind::Package,
        "use" => TokenKind::Use,
        "pub" => TokenKind::Pub,
        "const" => TokenKind::Const,
        "static" => TokenKind::Static,
        "let" => TokenKind::Let,
        "var" => TokenKind::Var,
        "fn" => TokenKind::Fn,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "spawn" => TokenKind::Spawn,
        "defer" => TokenKind::Defer,
        "return" => TokenKind::Return,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "for" => TokenKind::For,
        "while" => TokenKind::While,
        "loop" => TokenKind::Loop,
        "in" => TokenKind::In,
        "where" => TokenKind::Where,
        "struct" => TokenKind::Struct,
        "data" => TokenKind::Data,
        "enum" => TokenKind::Enum,
        "trait" => TokenKind::Trait,
        "impl" => TokenKind::Impl,
        "extend" => TokenKind::Extend,
        "type" => TokenKind::Type,
        "opaque" => TokenKind::Opaque,
        "extern" => TokenKind::Extern,
        "unsafe" => TokenKind::Unsafe,
        "is" => TokenKind::Is,
        "as" => TokenKind::As,
        "satisfies" => TokenKind::Satisfies,
        "none" => TokenKind::NoneKw,
        "true" => TokenKind::TrueKw,
        "false" => TokenKind::FalseKw,
        "self" => TokenKind::SelfKw,
        "move" => TokenKind::MoveKw,
        _ => return None,
    })
}

fn active_call_indexes(tokens: &[Token], offset: usize) -> Option<(usize, usize)> {
    let mut stack = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.span.start >= offset {
            break;
        }
        match token.kind {
            TokenKind::LParen => stack.push(index),
            TokenKind::RParen => {
                stack.pop();
            }
            _ => {}
        }
    }
    let open_index = *stack.last()?;
    let callee_index = previous_identifier_index(tokens, open_index)?;
    Some((open_index, callee_index))
}

fn previous_identifier_index(tokens: &[Token], before: usize) -> Option<usize> {
    (0..before)
        .rev()
        .find(|index| matches!(tokens[*index].kind, TokenKind::Ident | TokenKind::SelfKw))
}

fn is_method_call(tokens: &[Token], callee_index: usize) -> bool {
    callee_index > 0 && tokens[callee_index - 1].kind == TokenKind::Dot
}

fn active_parameter_index(tokens: &[Token], open_index: usize, offset: usize) -> usize {
    let mut active = 0usize;
    let mut depth = 0i32;
    for token in &tokens[open_index + 1..] {
        if token.span.start >= offset {
            break;
        }
        match token.kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => depth += 1,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => depth -= 1,
            TokenKind::Comma if depth == 0 => active += 1,
            _ => {}
        }
    }
    active
}

fn signature_parameters(detail: &str) -> Vec<String> {
    let Some(start) = detail.find('(') else {
        return Vec::new();
    };
    let Some(end) = matching_close_paren(detail, start) else {
        return Vec::new();
    };
    split_top_level_commas(&detail[start + 1..end])
        .into_iter()
        .filter(|part| !part.trim().is_empty())
        .map(|part| part.trim().to_owned())
        .collect()
}

fn matching_close_paren(text: &str, open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, ch) in text.char_indices().skip_while(|(offset, _)| *offset < open) {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (offset, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&text[start..offset]);
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn has_explicit_type_before_eq(tokens: &[Token]) -> bool {
    for token in tokens {
        match token.kind {
            TokenKind::Colon => return true,
            TokenKind::Eq => return false,
            TokenKind::Semi | TokenKind::Eof => return false,
            _ => {}
        }
    }
    false
}

fn range_contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

fn matching_delimiters(open: TokenKind, close: TokenKind) -> bool {
    matches!(
        (open, close),
        (TokenKind::LBrace, TokenKind::RBrace) | (TokenKind::LBracket, TokenKind::RBracket)
    )
}

fn selection_range_for_position(source: &str, position: Position) -> Option<SelectionRange> {
    let offset = position_to_offset(source, position)?;
    let full_range = span_to_range(source, Span::new(0, source.len()));
    let full = SelectionRange {
        range: full_range,
        parent: None,
    };
    let block = containing_delimiter_range(source, offset).map(|range| SelectionRange {
        range,
        parent: Some(Box::new(full.clone())),
    });
    let token_range = token_at_offset(source, offset)
        .filter(|token| token.kind != TokenKind::Eof)
        .map(|token| span_to_range(source, token.span));
    if let Some(range) = token_range {
        return Some(SelectionRange {
            range,
            parent: Some(Box::new(block.unwrap_or(full))),
        });
    }
    Some(block.unwrap_or(full))
}

fn containing_delimiter_range(source: &str, offset: usize) -> Option<Range> {
    let (tokens, _) = lex(source);
    let mut stack = Vec::<&Token>::new();
    let mut ranges = Vec::<Range>::new();
    for token in &tokens {
        if token.span.start > offset {
            break;
        }
        match token.kind {
            TokenKind::LBrace | TokenKind::LBracket | TokenKind::LParen => stack.push(token),
            TokenKind::RBrace | TokenKind::RBracket | TokenKind::RParen => {
                let Some(start) = stack.pop() else {
                    continue;
                };
                if start.span.start <= offset && offset <= token.span.end {
                    ranges.push(span_to_range(
                        source,
                        Span::new(start.span.start, token.span.end),
                    ));
                }
            }
            _ => {}
        }
    }
    ranges.into_iter().min_by_key(|range| {
        (
            range.end.line - range.start.line,
            range.end.character - range.start.character,
        )
    })
}
