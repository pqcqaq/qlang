use std::collections::BTreeSet;

use ql_ast::ItemKind;
use ql_parser::parse_source;

use crate::dependency_generic_bridge;

#[derive(Clone, Debug, Default)]
pub(crate) struct RenderedDependencyBridgeItems {
    pub(crate) declarations: String,
    pub(crate) source_rewrites: Vec<dependency_generic_bridge::SourceRewrite>,
}

impl RenderedDependencyBridgeItems {
    pub(crate) fn append_declarations(&mut self, declarations: &str) {
        self.declarations = join_dependency_bridge_sections(&self.declarations, declarations);
    }

    pub(crate) fn append_items(&mut self, items: RenderedDependencyBridgeItems) {
        self.append_declarations(&items.declarations);
        self.source_rewrites.extend(items.source_rewrites);
    }

    pub(crate) fn into_source_override(self, source: &str) -> Option<String> {
        dependency_bridge_source_override(source, &self.declarations, &self.source_rewrites)
    }
}

pub(crate) fn render_local_generic_function_specializations(
    source: &str,
) -> RenderedDependencyBridgeItems {
    let module = match parse_source(source) {
        Ok(module) => module,
        Err(_) => return RenderedDependencyBridgeItems::default(),
    };

    let mut declarations = Vec::new();
    let mut source_rewrites = Vec::new();
    let mut rendered_specializations = BTreeSet::new();
    for item in &module.items {
        let ItemKind::Function(function) = &item.kind else {
            continue;
        };
        let Some(rendered) = dependency_generic_bridge::render_local_function_specializations(
            function,
            source,
            &module,
            &mut rendered_specializations,
        ) else {
            continue;
        };
        declarations.push(rendered.declarations);
        source_rewrites.extend(rendered.call_rewrites);
    }

    RenderedDependencyBridgeItems {
        declarations: declarations.join("\n\n"),
        source_rewrites,
    }
}

pub(crate) fn local_generic_source_override(source: &str) -> Option<String> {
    let local_generic_items = render_local_generic_function_specializations(source);
    dependency_bridge_source_override(
        source,
        &local_generic_items.declarations,
        &local_generic_items.source_rewrites,
    )
}

fn append_dependency_declarations(source: &str, dependency_declarations: &str) -> String {
    let mut combined = source.trim_end_matches(['\r', '\n']).to_owned();
    combined.push_str("\n\n");
    combined.push_str(dependency_declarations);
    if !combined.ends_with('\n') {
        combined.push('\n');
    }
    combined
}

fn dependency_bridge_source_override(
    source: &str,
    dependency_declarations: &str,
    source_rewrites: &[dependency_generic_bridge::SourceRewrite],
) -> Option<String> {
    if dependency_declarations.is_empty() && source_rewrites.is_empty() {
        return None;
    }

    let rewritten_source = apply_dependency_source_rewrites(source, source_rewrites);
    if dependency_declarations.is_empty() {
        Some(rewritten_source)
    } else {
        Some(append_dependency_declarations(
            &rewritten_source,
            dependency_declarations,
        ))
    }
}

fn apply_dependency_source_rewrites(
    source: &str,
    source_rewrites: &[dependency_generic_bridge::SourceRewrite],
) -> String {
    let mut rewrites = source_rewrites.to_vec();
    rewrites.sort_by(|left, right| {
        right
            .span
            .start
            .cmp(&left.span.start)
            .then_with(|| right.span.end.cmp(&left.span.end))
    });

    let mut rewritten = source.to_owned();
    let mut next_start = source.len();
    for rewrite in rewrites {
        if rewrite.span.start > rewrite.span.end
            || rewrite.span.end > source.len()
            || !source.is_char_boundary(rewrite.span.start)
            || !source.is_char_boundary(rewrite.span.end)
            || rewrite.span.end > next_start
        {
            continue;
        }
        rewritten.replace_range(rewrite.span.start..rewrite.span.end, &rewrite.replacement);
        next_start = rewrite.span.start;
    }
    rewritten
}

pub(crate) fn join_dependency_bridge_sections(primary: &str, secondary: &str) -> String {
    let mut sections = Vec::new();
    if !primary.trim().is_empty() {
        sections.push(primary.trim().to_owned());
    }
    if !secondary.trim().is_empty() {
        sections.push(secondary.trim().to_owned());
    }
    sections.join("\n\n")
}
