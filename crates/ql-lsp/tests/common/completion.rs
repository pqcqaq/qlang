use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind};

#[derive(Clone, Copy)]
pub enum MemberKind {
    Field,
    Method,
}

impl MemberKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Method => "method",
        }
    }

    pub fn completion_suffix(self) -> &'static str {
        match self {
            Self::Field => ".va",
            Self::Method => ".ge",
        }
    }

    pub fn expected_label(self) -> &'static str {
        match self {
            Self::Field => "value",
            Self::Method => "get",
        }
    }

    pub fn expected_kind(self) -> CompletionItemKind {
        match self {
            Self::Field => CompletionItemKind::FIELD,
            Self::Method => CompletionItemKind::METHOD,
        }
    }

    pub fn expected_detail(self) -> &'static str {
        match self {
            Self::Field => "field value: Int",
            Self::Method => "fn get(self) -> Int",
        }
    }
}

pub fn assert_member_completion_item(member: MemberKind, item: &CompletionItem) {
    assert_eq!(item.label, member.expected_label());
    assert_eq!(item.kind, Some(member.expected_kind()));
    assert_eq!(item.detail.as_deref(), Some(member.expected_detail()));
}
