mod error;

use std::collections::{HashMap, HashSet, VecDeque};
use std::env::consts::{ARCH, OS};
use std::fmt::Write;

use ql_ast::{BinaryOp, UnaryOp};
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{self as hir, FunctionRef, ItemId, ItemKind, Param, PatternKind};
use ql_mir::{
    self as mir, BodyOwner, Constant, LocalOrigin, Operand, Place, Rvalue, StatementKind,
    TerminatorKind,
};
use ql_resolve::{BuiltinType, ResolutionMap};
use ql_span::Span;
use ql_typeck::{Ty, TypeckResult, lower_type};

pub use error::CodegenError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodegenMode {
    Program,
    Library,
}

pub struct CodegenInput<'a> {
    pub module_name: &'a str,
    pub mode: CodegenMode,
    pub hir: &'a hir::Module,
    pub mir: &'a mir::MirModule,
    pub resolution: &'a ResolutionMap,
    pub typeck: &'a TypeckResult,
}

pub fn emit_module(input: CodegenInput<'_>) -> Result<String, CodegenError> {
    ModuleEmitter::new(input).emit()
}

#[derive(Clone, Debug)]
struct FunctionSignature {
    function_ref: FunctionRef,
    name: String,
    llvm_name: String,
    span: Span,
    return_ty: Ty,
    return_llvm_ty: String,
    params: Vec<ParamSignature>,
    body_style: FunctionBodyStyle,
}

#[derive(Clone, Debug)]
struct ParamSignature {
    name: String,
    ty: Ty,
    llvm_ty: String,
}

#[derive(Clone, Debug)]
struct PreparedFunction {
    signature: FunctionSignature,
    local_types: HashMap<mir::LocalId, Ty>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FunctionBodyStyle {
    Definition,
    Declaration,
}

#[derive(Clone, Debug)]
struct LoweredValue {
    ty: Ty,
    llvm_ty: String,
    repr: String,
}

struct ModuleEmitter<'a> {
    input: CodegenInput<'a>,
    signatures: HashMap<FunctionRef, FunctionSignature>,
}

impl<'a> ModuleEmitter<'a> {
    fn new(input: CodegenInput<'a>) -> Self {
        Self {
            input,
            signatures: HashMap::new(),
        }
    }

    fn emit(mut self) -> Result<String, CodegenError> {
        let (entry, reachable) = match self.input.mode {
            CodegenMode::Program => {
                let entry = self.find_entry_function()?;
                (Some(entry), self.collect_reachable_functions(vec![entry]))
            }
            CodegenMode::Library => (None, self.collect_library_functions()),
        };
        let mut diagnostics = Vec::new();

        for function_ref in &reachable {
            match self.build_signature(*function_ref, entry == Some(*function_ref)) {
                Ok(signature) => {
                    self.signatures.insert(*function_ref, signature);
                }
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }

        if !diagnostics.is_empty() {
            return Err(CodegenError::new(diagnostics));
        }

        let mut prepared = Vec::new();
        for function_ref in &reachable {
            let Some(signature) = self.signatures.get(function_ref) else {
                continue;
            };
            if signature.body_style != FunctionBodyStyle::Definition {
                continue;
            }

            match self.prepare_function(*function_ref) {
                Ok(function) => prepared.push(function),
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }

        if !diagnostics.is_empty() {
            return Err(CodegenError::new(diagnostics));
        }

        Ok(self.render_module(&reachable, &prepared, entry))
    }

    fn find_entry_function(&self) -> Result<FunctionRef, CodegenError> {
        self.input
            .hir
            .items
            .iter()
            .copied()
            .find(|&item_id| {
                matches!(
                    &self.input.hir.item(item_id).kind,
                    ItemKind::Function(function) if function.name == "main"
                )
            })
            .map(FunctionRef::Item)
            .ok_or_else(|| {
                CodegenError::new(vec![
                    Diagnostic::error("missing entry function `main`").with_note(
                        "program-mode codegen currently builds single-file native-style modules from a top-level `main` function",
                    ),
                ])
            })
    }

    fn collect_library_functions(&self) -> Vec<FunctionRef> {
        let mut roots = self
            .input
            .hir
            .items
            .iter()
            .copied()
            .filter_map(|item_id| match &self.input.hir.item(item_id).kind {
                ItemKind::Function(function) if function.body.is_some() => {
                    Some(FunctionRef::Item(item_id))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        roots.sort_by_key(|function_ref| function_sort_key(*function_ref));
        self.collect_reachable_functions(roots)
    }

    fn collect_reachable_functions(&self, roots: Vec<FunctionRef>) -> Vec<FunctionRef> {
        let mut queue = VecDeque::from(roots);
        let mut visited = HashSet::new();
        let mut ordered = Vec::new();

        while let Some(function_ref) = queue.pop_front() {
            if !visited.insert(function_ref) {
                continue;
            }

            ordered.push(function_ref);

            let FunctionBodyStyle::Definition = self
                .signatures
                .get(&function_ref)
                .map(|signature| signature.body_style)
                .unwrap_or_else(|| {
                    if self.input.hir.function(function_ref).body.is_some() {
                        FunctionBodyStyle::Definition
                    } else {
                        FunctionBodyStyle::Declaration
                    }
                })
            else {
                continue;
            };
            let owner = self.input.hir.function_owner_item(function_ref);
            let Some(body) = self.input.mir.body_for_owner(BodyOwner::Item(owner)) else {
                continue;
            };

            for block in body.blocks() {
                for statement_id in &block.statements {
                    if let StatementKind::Assign { value, .. } | StatementKind::Eval { value } =
                        &body.statement(*statement_id).kind
                    {
                        self.collect_rvalue_callees(value, &mut queue);
                    }
                }
            }
        }

        ordered.sort_by_key(|function_ref| function_sort_key(*function_ref));
        ordered
    }

    fn collect_rvalue_callees(&self, value: &Rvalue, queue: &mut VecDeque<FunctionRef>) {
        if let Rvalue::Call {
            callee: Operand::Constant(Constant::Function { function, .. }),
            ..
        } = value
        {
            queue.push_back(*function);
        }
    }

    fn build_signature(
        &self,
        function_ref: FunctionRef,
        is_entry: bool,
    ) -> Result<FunctionSignature, Vec<Diagnostic>> {
        let function = self.lookup_function(function_ref)?;
        let mut diagnostics = Vec::new();
        let body_style = if function.body.is_some() {
            FunctionBodyStyle::Definition
        } else {
            FunctionBodyStyle::Declaration
        };

        if !function.generics.is_empty() {
            diagnostics.push(unsupported(
                function.span,
                "LLVM IR backend foundation does not support generic functions yet",
            ));
        }
        if function.is_async {
            diagnostics.push(unsupported(
                function.span,
                "LLVM IR backend foundation does not support `async fn` yet",
            ));
        }
        if function.is_unsafe && body_style == FunctionBodyStyle::Definition {
            diagnostics.push(unsupported(
                function.span,
                "LLVM IR backend foundation does not support `unsafe fn` bodies yet",
            ));
        }
        if body_style == FunctionBodyStyle::Definition {
            match function.abi.as_deref() {
                Some("c") | None => {}
                Some(_) => diagnostics.push(unsupported(
                    function.span,
                    "LLVM IR backend foundation only supports `extern \"c\"` function definitions yet",
                )),
            }
        }
        if body_style == FunctionBodyStyle::Declaration {
            match function.abi.as_deref() {
                Some("c") => {}
                Some(_) => diagnostics.push(unsupported(
                    function.span,
                    "LLVM IR backend foundation only supports `extern \"c\"` declarations yet",
                )),
                None => diagnostics.push(
                    Diagnostic::error(format!("function `{}` has no body to lower", function.name))
                        .with_label(Label::new(function.span)),
                ),
            }
        }

        let mut params = Vec::new();
        for param in &function.params {
            match param {
                Param::Regular(param) => {
                    let ty = lower_type(self.input.hir, self.input.resolution, param.ty);
                    match lower_llvm_type(&ty, param.name_span, "parameter type") {
                        Ok(llvm_ty) => params.push(ParamSignature {
                            name: param.name.clone(),
                            ty,
                            llvm_ty,
                        }),
                        Err(error) => diagnostics.push(error),
                    }
                }
                Param::Receiver(receiver) => diagnostics.push(unsupported(
                    receiver.span,
                    "LLVM IR backend foundation does not support receiver methods yet",
                )),
            }
        }

        let return_ty = function
            .return_type
            .map(|type_id| lower_type(self.input.hir, self.input.resolution, type_id))
            .unwrap_or_else(void_ty);
        let return_llvm_ty = match lower_llvm_type(&return_ty, function.span, "return type") {
            Ok(llvm_ty) => llvm_ty,
            Err(error) => {
                diagnostics.push(error);
                String::new()
            }
        };

        if is_entry {
            if body_style != FunctionBodyStyle::Definition {
                diagnostics.push(
                    Diagnostic::error(
                        "entry function `main` must have a body in the P4 backend foundation",
                    )
                    .with_label(Label::new(function.span)),
                );
            }
            if !params.is_empty() {
                diagnostics.push(Diagnostic::error(
                    "entry function `main` must not take parameters in the P4 backend foundation",
                )
                .with_label(Label::new(function.span))
                .with_note("future phases will extend `ql build` to package-aware entry signatures"));
            }
            if function.abi.is_some() {
                diagnostics.push(Diagnostic::error(
                    "entry function `main` must use the default Qlang ABI in the current native build pipeline",
                )
                .with_label(Label::new(function.span))
                .with_note("use a separate `extern \"c\"` helper when you need a stable exported C symbol"));
            }
            if !matches!(
                return_ty,
                Ty::Builtin(BuiltinType::Int) | Ty::Builtin(BuiltinType::Void)
            ) {
                diagnostics.push(Diagnostic::error(
                    "entry function `main` must return `Int` or `Void` in the P4 backend foundation",
                )
                .with_label(Label::new(function.span)));
            }
        }

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        Ok(FunctionSignature {
            function_ref,
            name: function.name.clone(),
            llvm_name: match body_style {
                FunctionBodyStyle::Definition => match function.abi.as_deref() {
                    Some("c") => sanitize_symbol(&function.name),
                    _ => llvm_symbol_name(
                        self.input.hir.function_owner_item(function_ref),
                        &function.name,
                    ),
                },
                FunctionBodyStyle::Declaration => sanitize_symbol(&function.name),
            },
            span: function.span,
            return_ty,
            return_llvm_ty,
            params,
            body_style,
        })
    }

    fn prepare_function(
        &self,
        function_ref: FunctionRef,
    ) -> Result<PreparedFunction, Vec<Diagnostic>> {
        let signature = self
            .signatures
            .get(&function_ref)
            .cloned()
            .expect("signatures should be built before preparation");
        debug_assert_eq!(signature.body_style, FunctionBodyStyle::Definition);
        let body = self
            .input
            .mir
            .body_for_owner(BodyOwner::Item(
                self.input.hir.function_owner_item(function_ref),
            ))
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(format!(
                        "function `{}` has no MIR body to lower",
                        signature.name
                    ))
                    .with_label(Label::new(signature.span)),
                ]
            })?;

        let mut diagnostics = Vec::new();
        let mut local_types = self.seed_local_types(body, &signature, &mut diagnostics);

        for block in body.blocks() {
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                match &statement.kind {
                    StatementKind::Assign { place, value } => {
                        self.require_direct_place(statement.span, place, &mut diagnostics);
                        if let Some(ty) = self.infer_rvalue_type(
                            body,
                            value,
                            &local_types,
                            &mut diagnostics,
                            statement.span,
                        ) {
                            local_types.entry(place.base).or_insert(ty);
                        }
                    }
                    StatementKind::BindPattern {
                        pattern, source, ..
                    } => {
                        self.require_binding_pattern(*pattern, statement.span, &mut diagnostics);
                        let _ = self.infer_operand_type(
                            body,
                            source,
                            &local_types,
                            &mut diagnostics,
                            statement.span,
                        );
                    }
                    StatementKind::Eval { value } => {
                        let _ = self.infer_rvalue_type(
                            body,
                            value,
                            &local_types,
                            &mut diagnostics,
                            statement.span,
                        );
                    }
                    StatementKind::StorageLive { .. } | StatementKind::StorageDead { .. } => {}
                    StatementKind::RegisterCleanup { .. } | StatementKind::RunCleanup { .. } => {
                        diagnostics.push(unsupported(
                            statement.span,
                            "LLVM IR backend foundation does not support cleanup lowering yet",
                        ));
                    }
                }
            }

            match &block.terminator.kind {
                TerminatorKind::Goto { .. }
                | TerminatorKind::Return
                | TerminatorKind::Terminate => {}
                TerminatorKind::Branch { condition, .. } => {
                    let Some(condition_ty) = self.infer_operand_type(
                        body,
                        condition,
                        &local_types,
                        &mut diagnostics,
                        block.terminator.span,
                    ) else {
                        continue;
                    };
                    if !condition_ty.is_bool() {
                        diagnostics.push(unsupported(
                            block.terminator.span,
                            "LLVM IR backend foundation currently requires branch conditions to lower to `Bool`",
                        ));
                    }
                }
                TerminatorKind::Match { .. } => diagnostics.push(unsupported(
                    block.terminator.span,
                    "LLVM IR backend foundation does not support `match` lowering yet",
                )),
                TerminatorKind::ForLoop { .. } => diagnostics.push(unsupported(
                    block.terminator.span,
                    "LLVM IR backend foundation does not support `for` lowering yet",
                )),
            }
        }

        let should_validate_local_types = diagnostics.is_empty();
        for local_id in body.local_ids() {
            let Some(ty) = local_types.get(&local_id) else {
                diagnostics.push(Diagnostic::error(format!(
                    "could not infer LLVM type for MIR local `{}`",
                    body.local(local_id).name
                ))
                .with_label(Label::new(body.local(local_id).span))
                .with_note("this usually means the current MIR shape is not part of the P4 backend foundation support matrix"));
                continue;
            };

            if !should_validate_local_types || is_void_ty(ty) {
                continue;
            }
            if let Err(error) = lower_llvm_type(ty, body.local(local_id).span, "local type") {
                diagnostics.push(error);
            }
        }

        if diagnostics.is_empty() {
            Ok(PreparedFunction {
                signature,
                local_types,
            })
        } else {
            Err(diagnostics)
        }
    }

    fn seed_local_types(
        &self,
        body: &mir::MirBody,
        signature: &FunctionSignature,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> HashMap<mir::LocalId, Ty> {
        let mut local_types = HashMap::new();

        for local_id in body.local_ids() {
            let local = body.local(local_id);
            let ty = match &local.origin {
                LocalOrigin::ReturnSlot => Some(signature.return_ty.clone()),
                LocalOrigin::Param { index } => {
                    signature.params.get(*index).map(|param| param.ty.clone())
                }
                LocalOrigin::Binding(hir_local) => self.input.typeck.local_ty(*hir_local).cloned(),
                LocalOrigin::Receiver => {
                    diagnostics.push(unsupported(
                        local.span,
                        "LLVM IR backend foundation does not support receiver locals yet",
                    ));
                    None
                }
                LocalOrigin::Temp { .. } => None,
            };

            if let Some(ty) = ty {
                local_types.insert(local_id, ty);
            }
        }

        local_types
    }

    fn infer_rvalue_type(
        &self,
        body: &mir::MirBody,
        value: &Rvalue,
        local_types: &HashMap<mir::LocalId, Ty>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        match value {
            Rvalue::Use(operand) => {
                self.infer_operand_type(body, operand, local_types, diagnostics, span)
            }
            Rvalue::Call { callee, args } => {
                for arg in args {
                    if arg.name.is_some() {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support named call arguments yet",
                        ));
                    }
                    let _ =
                        self.infer_operand_type(body, &arg.value, local_types, diagnostics, span);
                }

                let Operand::Constant(Constant::Function { function, .. }) = callee else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports direct resolved function calls",
                    ));
                    return None;
                };

                self.signatures
                    .get(function)
                    .map(|signature| signature.return_ty.clone())
                    .or_else(|| {
                        diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation could not resolve the direct callee declaration",
                    ));
                        None
                    })
            }
            Rvalue::Binary { left, op, right } => {
                let left_ty =
                    self.infer_operand_type(body, left, local_types, diagnostics, span)?;
                let right_ty =
                    self.infer_operand_type(body, right, local_types, diagnostics, span)?;
                self.validate_binary_operands(*op, &left_ty, &right_ty, span, diagnostics)
            }
            Rvalue::Unary { op, operand } => {
                let operand_ty =
                    self.infer_operand_type(body, operand, local_types, diagnostics, span)?;
                match op {
                    UnaryOp::Neg => {
                        if is_numeric_ty(&operand_ty) {
                            Some(operand_ty)
                        } else {
                            diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation only supports numeric negation",
                            ));
                            None
                        }
                    }
                    UnaryOp::Await => {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support `await` yet",
                        ));
                        None
                    }
                    UnaryOp::Spawn => {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support `spawn` yet",
                        ));
                        None
                    }
                }
            }
            Rvalue::Tuple(_) => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support tuple values yet",
                ));
                None
            }
            Rvalue::Array(_) => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support array values yet",
                ));
                None
            }
            Rvalue::AggregateStruct { .. } => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support struct values yet",
                ));
                None
            }
            Rvalue::Closure { .. } => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support closure values yet",
                ));
                None
            }
            Rvalue::Question(_) => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support `?` lowering yet",
                ));
                None
            }
            Rvalue::OpaqueExpr(_) => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered an opaque expression that still needs MIR elaboration",
                ));
                None
            }
        }
    }

    fn infer_operand_type(
        &self,
        body: &mir::MirBody,
        operand: &Operand,
        local_types: &HashMap<mir::LocalId, Ty>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        match operand {
            Operand::Place(place) => {
                self.require_direct_place(span, place, diagnostics);
                local_types.get(&place.base).cloned().or_else(|| {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "could not resolve LLVM type for local `{}`",
                            body.local(place.base).name
                        ))
                        .with_label(Label::new(span)),
                    );
                    None
                })
            }
            Operand::Constant(constant) => match constant {
                Constant::Integer(_) => Some(Ty::Builtin(BuiltinType::Int)),
                Constant::Bool(_) => Some(Ty::Builtin(BuiltinType::Bool)),
                Constant::Void => Some(void_ty()),
                Constant::Function { .. } => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support first-class function values yet",
                    ));
                    None
                }
                Constant::Item { .. } => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support item values here",
                    ));
                    None
                }
                Constant::String { .. } => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support string literals yet",
                    ));
                    None
                }
                Constant::None => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support `none` yet",
                    ));
                    None
                }
                Constant::Import(_) => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support imported value lowering yet",
                    ));
                    None
                }
                Constant::UnresolvedName(_) => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation cannot lower unresolved names",
                    ));
                    None
                }
            },
        }
    }

    fn validate_binary_operands(
        &self,
        op: BinaryOp,
        left_ty: &Ty,
        right_ty: &Ty,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Ty> {
        if left_ty != right_ty {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation currently requires binary operands to have the same lowered type",
            ));
            return None;
        }

        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if is_numeric_ty(left_ty) {
                    Some(left_ty.clone())
                } else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports numeric arithmetic",
                    ));
                    None
                }
            }
            BinaryOp::EqEq
            | BinaryOp::BangEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
            | BinaryOp::Lt
            | BinaryOp::LtEq => {
                if is_comparable_ty(left_ty) {
                    Some(Ty::Builtin(BuiltinType::Bool))
                } else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports integer, float, or bool comparisons",
                    ));
                    None
                }
            }
            BinaryOp::Assign => {
                diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support assignment expressions yet",
                ));
                None
            }
        }
    }

    fn require_direct_place(&self, span: Span, place: &Place, diagnostics: &mut Vec<Diagnostic>) {
        if !place.projections.is_empty() {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation does not support field or index projections yet",
            ));
        }
    }

    fn require_binding_pattern(
        &self,
        pattern: hir::PatternId,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !matches!(
            pattern_kind(self.input.hir, pattern),
            PatternKind::Binding(_)
        ) {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation only supports single-name binding patterns",
            ));
        }
    }

    fn lookup_function(
        &self,
        function_ref: FunctionRef,
    ) -> Result<&hir::Function, Vec<Diagnostic>> {
        match function_ref {
            FunctionRef::Item(item_id) => match &self.input.hir.item(item_id).kind {
                ItemKind::Function(function) => Ok(function),
                _ => Err(vec![
                    Diagnostic::error(
                        "LLVM IR backend foundation can only lower free function declarations",
                    )
                    .with_label(Label::new(self.input.hir.item(item_id).span)),
                ]),
            },
            FunctionRef::ExternBlockMember { block, index } => {
                match &self.input.hir.item(block).kind {
                    ItemKind::ExternBlock(extern_block) => Ok(extern_block
                        .functions
                        .get(index)
                        .expect("extern function index should be valid")),
                    _ => Err(vec![
                    Diagnostic::error(
                        "LLVM IR backend foundation expected an extern block function declaration",
                    )
                    .with_label(Label::new(self.input.hir.item(block).span)),
                ]),
                }
            }
        }
    }

    fn render_module(
        &self,
        reachable: &[FunctionRef],
        functions: &[PreparedFunction],
        entry: Option<FunctionRef>,
    ) -> String {
        let mut output = String::new();
        let _ = writeln!(
            output,
            "; generated by ql-codegen-llvm P4 backend foundation"
        );
        let _ = writeln!(output, "; ModuleID = '{}'", self.input.module_name);
        let _ = writeln!(
            output,
            "source_filename = \"{}.ql\"",
            self.input.module_name
        );
        let _ = writeln!(output, "target triple = \"{}\"", default_target_triple());

        for function_ref in reachable {
            output.push('\n');
            if let Some(function) = functions
                .iter()
                .find(|function| function.signature.function_ref == *function_ref)
            {
                self.render_function(&mut output, function);
                continue;
            }

            let signature = self
                .signatures
                .get(function_ref)
                .expect("reachable functions should have signatures");
            self.render_declaration(&mut output, signature);
        }

        if let Some(entry) = entry
            && let Some(entry_function) = functions
                .iter()
                .find(|function| function.signature.function_ref == entry)
        {
            output.push('\n');
            self.render_host_entry_wrapper(&mut output, entry_function);
        }

        output
    }

    fn render_host_entry_wrapper(&self, output: &mut String, entry: &PreparedFunction) {
        output.push_str("define i32 @main() {\n");
        output.push_str("entry:\n");

        if is_void_ty(&entry.signature.return_ty) {
            let _ = writeln!(
                output,
                "  call {} @{}()",
                entry.signature.return_llvm_ty, entry.signature.llvm_name
            );
            output.push_str("  ret i32 0\n");
        } else {
            let _ = writeln!(
                output,
                "  %entry_ret = call {} @{}()",
                entry.signature.return_llvm_ty, entry.signature.llvm_name
            );
            output.push_str("  %entry_exit = trunc i64 %entry_ret to i32\n");
            output.push_str("  ret i32 %entry_exit\n");
        }

        output.push_str("}\n");
    }

    fn render_declaration(&self, output: &mut String, function: &FunctionSignature) {
        let params = function
            .params
            .iter()
            .map(|param| param.llvm_ty.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "declare {} @{}({params})",
            function.return_llvm_ty, function.llvm_name
        );
    }

    fn render_function(&self, output: &mut String, function: &PreparedFunction) {
        let body = self
            .input
            .mir
            .body_for_owner(BodyOwner::Item(
                self.input
                    .hir
                    .function_owner_item(function.signature.function_ref),
            ))
            .expect("prepared function should still have a MIR body");
        let params = function
            .signature
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let _ = &param.name;
                format!("{} %arg{index}", param.llvm_ty)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(
            output,
            "define {} @{}({params}) {{",
            function.signature.return_llvm_ty, function.signature.llvm_name
        );
        output.push_str("entry:\n");

        for local_id in body.local_ids() {
            let ty = function
                .local_types
                .get(&local_id)
                .expect("prepared locals should all have types");
            if is_void_ty(ty) {
                continue;
            }
            let llvm_ty = lower_llvm_type(ty, body.local(local_id).span, "local type")
                .expect("prepared local types should already be supported");
            let _ = writeln!(
                output,
                "  {} = alloca {}",
                llvm_slot_name(body, local_id),
                llvm_ty
            );
        }

        for local_id in body.local_ids() {
            let local = body.local(local_id);
            let LocalOrigin::Param { index } = local.origin else {
                continue;
            };
            let param = &function.signature.params[index];
            let _ = writeln!(
                output,
                "  store {} %arg{}, ptr {}",
                param.llvm_ty,
                index,
                llvm_slot_name(body, local_id)
            );
        }

        let _ = writeln!(output, "  br label %bb{}", body.entry.index());

        let mut renderer = FunctionRenderer {
            emitter: self,
            body,
            prepared: function,
            next_temp: 0,
        };

        for block_id in body.block_ids() {
            let block = body.block(block_id);
            let _ = writeln!(output, "bb{}:", block_id.index());
            for statement_id in &block.statements {
                renderer.render_statement(output, body.statement(*statement_id));
            }
            renderer.render_terminator(output, &block.terminator);
        }

        output.push_str("}\n");
    }
}

struct FunctionRenderer<'a, 'b> {
    emitter: &'a ModuleEmitter<'b>,
    body: &'a mir::MirBody,
    prepared: &'a PreparedFunction,
    next_temp: usize,
}

impl<'a, 'b> FunctionRenderer<'a, 'b> {
    fn render_statement(&mut self, output: &mut String, statement: &mir::Statement) {
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                if let Some(rendered) = self.render_rvalue(output, value, statement.span)
                    && !is_void_ty(&rendered.ty)
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {}",
                        rendered.llvm_ty,
                        rendered.repr,
                        llvm_slot_name(self.body, place.base)
                    );
                }
            }
            StatementKind::BindPattern {
                pattern, source, ..
            } => {
                let PatternKind::Binding(local) = pattern_kind(self.emitter.input.hir, *pattern)
                else {
                    panic!("prepared patterns should only contain bindings");
                };
                let rendered = self.render_operand(output, source, statement.span);
                let binding_local = self
                    .body
                    .local_ids()
                    .find(|candidate| {
                        matches!(
                            self.body.local(*candidate).origin,
                            LocalOrigin::Binding(hir_local) if hir_local == *local
                        )
                    })
                    .expect("binding local should exist in MIR body");
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {}",
                    rendered.llvm_ty,
                    rendered.repr,
                    llvm_slot_name(self.body, binding_local)
                );
            }
            StatementKind::Eval { value } => {
                let _ = self.render_rvalue(output, value, statement.span);
            }
            StatementKind::StorageLive { .. } | StatementKind::StorageDead { .. } => {}
            StatementKind::RegisterCleanup { .. } | StatementKind::RunCleanup { .. } => {
                panic!("prepared functions should not contain cleanup statements")
            }
        }
    }

    fn render_terminator(&mut self, output: &mut String, terminator: &mir::Terminator) {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                let _ = writeln!(output, "  br label %bb{}", target.index());
            }
            TerminatorKind::Branch {
                condition,
                then_target,
                else_target,
            } => {
                let rendered = self.render_operand(output, condition, terminator.span);
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %bb{}, label %bb{}",
                    rendered.repr,
                    then_target.index(),
                    else_target.index()
                );
            }
            TerminatorKind::Return => {
                if is_void_ty(&self.prepared.signature.return_ty) {
                    output.push_str("  ret void\n");
                } else {
                    let temp = self.fresh_temp();
                    let slot = llvm_slot_name(self.body, self.body.return_local);
                    let _ = writeln!(
                        output,
                        "  {temp} = load {}, ptr {}",
                        self.prepared.signature.return_llvm_ty, slot
                    );
                    let _ = writeln!(
                        output,
                        "  ret {} {temp}",
                        self.prepared.signature.return_llvm_ty
                    );
                }
            }
            TerminatorKind::Terminate => {
                output.push_str("  unreachable\n");
            }
            TerminatorKind::Match { .. } | TerminatorKind::ForLoop { .. } => {
                panic!("prepared functions should not contain unsupported terminators")
            }
        }
    }

    fn render_rvalue(
        &mut self,
        output: &mut String,
        value: &Rvalue,
        span: Span,
    ) -> Option<LoweredValue> {
        match value {
            Rvalue::Use(operand) => Some(self.render_operand(output, operand, span)),
            Rvalue::Call { callee, args } => {
                let Operand::Constant(Constant::Function { function, .. }) = callee else {
                    panic!("prepared calls should only contain direct resolved callees");
                };
                let signature = self
                    .emitter
                    .signatures
                    .get(function)
                    .expect("callee signatures should exist");
                let rendered_args = args
                    .iter()
                    .map(|arg| {
                        let value = self.render_operand(output, &arg.value, span);
                        format!("{} {}", value.llvm_ty, value.repr)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                if is_void_ty(&signature.return_ty) {
                    let _ = writeln!(
                        output,
                        "  call {} @{}({rendered_args})",
                        signature.return_llvm_ty, signature.llvm_name
                    );
                    None
                } else {
                    let temp = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {temp} = call {} @{}({rendered_args})",
                        signature.return_llvm_ty, signature.llvm_name
                    );
                    Some(LoweredValue {
                        ty: signature.return_ty.clone(),
                        llvm_ty: signature.return_llvm_ty.clone(),
                        repr: temp,
                    })
                }
            }
            Rvalue::Binary { left, op, right } => {
                let left = self.render_operand(output, left, span);
                let right = self.render_operand(output, right, span);
                self.render_binary(output, *op, left, right)
            }
            Rvalue::Unary { op, operand } => {
                let operand = self.render_operand(output, operand, span);
                match op {
                    UnaryOp::Neg => {
                        let temp = self.fresh_temp();
                        if is_float_ty(&operand.ty) {
                            let _ = writeln!(
                                output,
                                "  {temp} = fneg {} {}",
                                operand.llvm_ty, operand.repr
                            );
                        } else {
                            let _ = writeln!(
                                output,
                                "  {temp} = sub {} 0, {}",
                                operand.llvm_ty, operand.repr
                            );
                        }
                        Some(LoweredValue {
                            ty: operand.ty,
                            llvm_ty: operand.llvm_ty,
                            repr: temp,
                        })
                    }
                    UnaryOp::Await | UnaryOp::Spawn => {
                        panic!("prepared functions should not contain unsupported unary ops")
                    }
                }
            }
            Rvalue::Tuple(_)
            | Rvalue::Array(_)
            | Rvalue::AggregateStruct { .. }
            | Rvalue::Closure { .. }
            | Rvalue::Question(_)
            | Rvalue::OpaqueExpr(_) => {
                panic!("prepared functions should not contain unsupported rvalues")
            }
        }
    }

    fn render_binary(
        &mut self,
        output: &mut String,
        op: BinaryOp,
        left: LoweredValue,
        right: LoweredValue,
    ) -> Option<LoweredValue> {
        let temp = self.fresh_temp();

        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let opcode = arithmetic_opcode(op, &left.ty);
                let _ = writeln!(
                    output,
                    "  {temp} = {opcode} {} {}, {}",
                    left.llvm_ty, left.repr, right.repr
                );
                Some(LoweredValue {
                    ty: left.ty,
                    llvm_ty: left.llvm_ty,
                    repr: temp,
                })
            }
            BinaryOp::EqEq
            | BinaryOp::BangEq
            | BinaryOp::Gt
            | BinaryOp::GtEq
            | BinaryOp::Lt
            | BinaryOp::LtEq => {
                let opcode = compare_opcode(op, &left.ty);
                let _ = writeln!(
                    output,
                    "  {temp} = {opcode} {} {}, {}",
                    left.llvm_ty, left.repr, right.repr
                );
                Some(LoweredValue {
                    ty: Ty::Builtin(BuiltinType::Bool),
                    llvm_ty: "i1".to_owned(),
                    repr: temp,
                })
            }
            BinaryOp::Assign => {
                panic!("prepared functions should not contain assignment expressions")
            }
        }
    }

    fn render_operand(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> LoweredValue {
        match operand {
            Operand::Place(place) => {
                let ty = self
                    .prepared
                    .local_types
                    .get(&place.base)
                    .cloned()
                    .unwrap_or_else(|| panic!("prepared place at {span:?} should have a type"));
                let llvm_ty = lower_llvm_type(&ty, span, "operand type")
                    .expect("prepared operand types should already be supported");
                let temp = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {temp} = load {}, ptr {}",
                    llvm_ty,
                    llvm_slot_name(self.body, place.base)
                );
                LoweredValue {
                    ty,
                    llvm_ty,
                    repr: temp,
                }
            }
            Operand::Constant(constant) => match constant {
                Constant::Integer(value) => LoweredValue {
                    ty: Ty::Builtin(BuiltinType::Int),
                    llvm_ty: "i64".to_owned(),
                    repr: value.clone(),
                },
                Constant::Bool(value) => LoweredValue {
                    ty: Ty::Builtin(BuiltinType::Bool),
                    llvm_ty: "i1".to_owned(),
                    repr: if *value { "true" } else { "false" }.to_owned(),
                },
                Constant::Void => LoweredValue {
                    ty: void_ty(),
                    llvm_ty: "void".to_owned(),
                    repr: "void".to_owned(),
                },
                Constant::Function { .. } => {
                    panic!(
                        "prepared non-call operands should not materialize function declarations"
                    )
                }
                Constant::Item { .. } => {
                    panic!("prepared operands should not materialize unsupported item values")
                }
                Constant::String { .. }
                | Constant::None
                | Constant::Import(_)
                | Constant::UnresolvedName(_) => {
                    panic!("prepared operands should not contain unsupported constants")
                }
            },
        }
    }

    fn fresh_temp(&mut self) -> String {
        let index = self.next_temp;
        self.next_temp += 1;
        format!("%t{index}")
    }
}

fn pattern_kind(module: &hir::Module, pattern: hir::PatternId) -> &PatternKind {
    &module.pattern(pattern).kind
}

fn lower_llvm_type(ty: &Ty, span: Span, context: &str) -> Result<String, Diagnostic> {
    match ty {
        Ty::Builtin(BuiltinType::Bool) => Ok("i1".to_owned()),
        Ty::Builtin(BuiltinType::Void) => Ok("void".to_owned()),
        Ty::Builtin(BuiltinType::Int) | Ty::Builtin(BuiltinType::I64) => Ok("i64".to_owned()),
        Ty::Builtin(BuiltinType::UInt) | Ty::Builtin(BuiltinType::U64) => Ok("i64".to_owned()),
        Ty::Builtin(BuiltinType::I32) | Ty::Builtin(BuiltinType::U32) => Ok("i32".to_owned()),
        Ty::Builtin(BuiltinType::I16) | Ty::Builtin(BuiltinType::U16) => Ok("i16".to_owned()),
        Ty::Builtin(BuiltinType::I8) | Ty::Builtin(BuiltinType::U8) => Ok("i8".to_owned()),
        Ty::Builtin(BuiltinType::ISize) | Ty::Builtin(BuiltinType::USize) => Ok("i64".to_owned()),
        Ty::Builtin(BuiltinType::F32) => Ok("float".to_owned()),
        Ty::Builtin(BuiltinType::F64) => Ok("double".to_owned()),
        _ => Err(Diagnostic::error(format!(
            "LLVM IR backend foundation does not support {context} `{ty}` yet"
        ))
        .with_label(Label::new(span))),
    }
}

fn arithmetic_opcode(op: BinaryOp, ty: &Ty) -> &'static str {
    match (op, is_float_ty(ty), integer_signedness(ty)) {
        (BinaryOp::Add, true, _) => "fadd",
        (BinaryOp::Sub, true, _) => "fsub",
        (BinaryOp::Mul, true, _) => "fmul",
        (BinaryOp::Div, true, _) => "fdiv",
        (BinaryOp::Rem, true, _) => "frem",
        (BinaryOp::Add, false, _) => "add",
        (BinaryOp::Sub, false, _) => "sub",
        (BinaryOp::Mul, false, _) => "mul",
        (BinaryOp::Div, false, Some(Signedness::Signed)) => "sdiv",
        (BinaryOp::Div, false, Some(Signedness::Unsigned)) => "udiv",
        (BinaryOp::Rem, false, Some(Signedness::Signed)) => "srem",
        (BinaryOp::Rem, false, Some(Signedness::Unsigned)) => "urem",
        _ => unreachable!("validated arithmetic should only use supported numeric types"),
    }
}

fn compare_opcode(op: BinaryOp, ty: &Ty) -> &'static str {
    if is_float_ty(ty) {
        match op {
            BinaryOp::EqEq => "fcmp oeq",
            BinaryOp::BangEq => "fcmp one",
            BinaryOp::Gt => "fcmp ogt",
            BinaryOp::GtEq => "fcmp oge",
            BinaryOp::Lt => "fcmp olt",
            BinaryOp::LtEq => "fcmp ole",
            _ => unreachable!("validated compares should only use comparison operators"),
        }
    } else {
        match (op, integer_signedness(ty)) {
            (BinaryOp::EqEq, _) => "icmp eq",
            (BinaryOp::BangEq, _) => "icmp ne",
            (BinaryOp::Gt, Some(Signedness::Signed)) => "icmp sgt",
            (BinaryOp::GtEq, Some(Signedness::Signed)) => "icmp sge",
            (BinaryOp::Lt, Some(Signedness::Signed)) => "icmp slt",
            (BinaryOp::LtEq, Some(Signedness::Signed)) => "icmp sle",
            (BinaryOp::Gt, Some(Signedness::Unsigned)) => "icmp ugt",
            (BinaryOp::GtEq, Some(Signedness::Unsigned)) => "icmp uge",
            (BinaryOp::Lt, Some(Signedness::Unsigned)) => "icmp ult",
            (BinaryOp::LtEq, Some(Signedness::Unsigned)) => "icmp ule",
            _ => unreachable!("validated compares should only use supported numeric or bool types"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Signedness {
    Signed,
    Unsigned,
}

fn integer_signedness(ty: &Ty) -> Option<Signedness> {
    match ty {
        Ty::Builtin(
            BuiltinType::Int
            | BuiltinType::I8
            | BuiltinType::I16
            | BuiltinType::I32
            | BuiltinType::I64
            | BuiltinType::ISize,
        ) => Some(Signedness::Signed),
        Ty::Builtin(
            BuiltinType::UInt
            | BuiltinType::U8
            | BuiltinType::U16
            | BuiltinType::U32
            | BuiltinType::U64
            | BuiltinType::USize,
        )
        | Ty::Builtin(BuiltinType::Bool) => Some(Signedness::Unsigned),
        _ => None,
    }
}

fn is_float_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Builtin(BuiltinType::F32 | BuiltinType::F64))
}

fn is_numeric_ty(ty: &Ty) -> bool {
    integer_signedness(ty).is_some() || is_float_ty(ty)
}

fn is_comparable_ty(ty: &Ty) -> bool {
    is_numeric_ty(ty) || matches!(ty, Ty::Builtin(BuiltinType::Bool))
}

fn is_void_ty(ty: &Ty) -> bool {
    matches!(ty, Ty::Builtin(BuiltinType::Void))
}

fn void_ty() -> Ty {
    Ty::Builtin(BuiltinType::Void)
}

fn unsupported(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(message).with_label(Label::new(span))
}

fn default_target_triple() -> &'static str {
    match (ARCH, OS) {
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("x86_64", "linux") => "x86_64-pc-linux-gnu",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        _ => "unknown-unknown-unknown",
    }
}

fn llvm_symbol_name(item_id: ItemId, name: &str) -> String {
    format!("ql_{}_{}", item_id.index(), sanitize_symbol(name))
}

fn function_sort_key(function_ref: FunctionRef) -> (usize, usize) {
    match function_ref {
        FunctionRef::Item(item_id) => (item_id.index(), 0),
        FunctionRef::ExternBlockMember { block, index } => (block.index(), index + 1),
    }
}

fn llvm_slot_name(body: &mir::MirBody, local_id: mir::LocalId) -> String {
    format!(
        "%l{}_{}",
        local_id.index(),
        sanitize_symbol(&body.local(local_id).name)
    )
}

fn sanitize_symbol(raw: &str) -> String {
    let mut output = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "unnamed".to_owned()
    } else {
        output
    }
}

#[cfg(test)]
mod tests {
    use ql_analysis::analyze_source;

    use super::{CodegenInput, CodegenMode, emit_module};

    fn emit(source: &str) -> String {
        emit_with_mode(source, CodegenMode::Program)
    }

    fn emit_library(source: &str) -> String {
        emit_with_mode(source, CodegenMode::Library)
    }

    fn emit_with_mode(source: &str, mode: CodegenMode) -> String {
        let analysis = analyze_source(source).expect("source should analyze");
        assert!(
            !analysis.has_errors(),
            "test source should not contain semantic diagnostics"
        );

        emit_module(CodegenInput {
            module_name: "test_module",
            mode,
            hir: analysis.hir(),
            mir: analysis.mir(),
            resolution: analysis.resolution(),
            typeck: analysis.typeck(),
        })
        .expect("codegen should succeed")
    }

    fn emit_error(source: &str) -> Vec<String> {
        let analysis = analyze_source(source).expect("source should analyze");
        assert!(
            !analysis.has_errors(),
            "test source should not contain semantic diagnostics"
        );

        emit_module(CodegenInput {
            module_name: "test_module",
            mode: CodegenMode::Program,
            hir: analysis.hir(),
            mir: analysis.mir(),
            resolution: analysis.resolution(),
            typeck: analysis.typeck(),
        })
        .expect_err("codegen should fail")
        .into_diagnostics()
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
    }

    #[test]
    fn emits_llvm_ir_for_direct_calls_and_arithmetic() {
        let rendered = emit(
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    let value = add_one(41)
    return value
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_add_one(i64 %arg0)"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call i64 @ql_0_add_one(i64 41)"));
        assert!(rendered.contains("call i64 @ql_1_main()"));
        assert!(rendered.contains("add i64"));
    }

    #[test]
    fn emits_branches_for_bool_conditions() {
        let rendered = emit(
            r#"
fn main() -> Int {
    if true {
        return 1
    }
    return 0
}
"#,
        );

        assert!(rendered.contains("br i1 true"));
        assert!(rendered.contains("ret i64"));
        assert!(!rendered.contains("store void void"));
    }

    #[test]
    fn emits_zero_exit_code_wrapper_for_void_main() {
        let rendered = emit(
            r#"
fn main() -> Void {
    return
}
"#,
        );

        assert!(rendered.contains("define void @ql_0_main()"));
        assert!(rendered.contains("call void @ql_0_main()"));
        assert!(rendered.contains("ret i32 0"));
    }

    #[test]
    fn library_mode_exports_free_functions_without_host_main_wrapper() {
        let rendered = emit_library(
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn add_two(value: Int) -> Int {
    return add_one(add_one(value))
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_add_one(i64 %arg0)"));
        assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
        assert!(!rendered.contains("define i32 @main()"));
    }

    #[test]
    fn emits_extern_c_declarations_for_direct_calls() {
        let rendered = emit(
            r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn main() -> Int {
    return q_add(1, 2)
}
"#,
        );

        assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
        assert!(rendered.contains("call i64 @q_add(i64 1, i64 2)"));
    }

    #[test]
    fn library_mode_keeps_extern_block_declarations_for_direct_calls() {
        let rendered = emit_library(
            r#"
extern "c" {
    fn q_add(left: Int, right: Int) -> Int
}

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );

        assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
        assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
        assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
    }

    #[test]
    fn library_mode_keeps_top_level_extern_declarations_for_direct_calls() {
        let rendered = emit_library(
            r#"
extern "c" fn q_add(left: Int, right: Int) -> Int

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );

        assert!(rendered.contains("declare i64 @q_add(i64, i64)"));
        assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
        assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
    }

    #[test]
    fn emits_extern_c_function_definitions_with_stable_symbol_names() {
        let rendered = emit(
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#,
        );

        assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
        assert!(rendered.contains("define i64 @ql_1_main()"));
        assert!(rendered.contains("call i64 @q_add(i64 1, i64 2)"));
        assert!(!rendered.contains("define i64 @ql_0_q_add"));
    }

    #[test]
    fn library_mode_keeps_extern_c_function_definitions_exported() {
        let rendered = emit_library(
            r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn add_two(value: Int) -> Int {
    return q_add(value, 2)
}
"#,
        );

        assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
        assert!(rendered.contains("define i64 @ql_1_add_two(i64 %arg0)"));
        assert!(rendered.contains("call i64 @q_add(i64 %t0, i64 2)"));
        assert!(!rendered.contains("define i64 @main("));
    }

    #[test]
    fn rejects_non_c_extern_declarations() {
        let messages = emit_error(
            r#"
extern "rust" fn q_add(left: Int, right: Int) -> Int

fn main() -> Int {
    return q_add(1, 2)
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation only supports `extern \"c\"` declarations yet"
        }));
    }

    #[test]
    fn rejects_non_c_extern_definitions() {
        let messages = emit_error(
            r#"
extern "rust" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}

fn main() -> Int {
    return q_add(1, 2)
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation only supports `extern \"c\"` function definitions yet"
        }));
    }

    #[test]
    fn rejects_extern_c_entry_main_definitions() {
        let messages = emit_error(
            r#"
extern "c" fn main() -> Int {
    return 0
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message
                == "entry function `main` must use the default Qlang ABI in the current native build pipeline"
        }));
    }

    #[test]
    fn rejects_first_class_function_values() {
        let messages = emit_error(
            r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    let f = add_one
    return 0
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support first-class function values yet"
        }));
    }

    #[test]
    fn rejects_unsupported_closure_values() {
        let messages = emit_error(
            r#"
fn main() -> Int {
    let capture = () => 1
    return 0
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support closure values yet"
        }));
    }

    #[test]
    fn rejects_unsupported_match_lowering() {
        let messages = emit_error(
            r#"
fn main() -> Int {
    let flag = true
    return match flag {
        true => 1,
        false => 0,
    }
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support `match` lowering yet"
        }));
    }

    #[test]
    fn rejects_unsupported_for_lowering() {
        let messages = emit_error(
            r#"
fn main() -> Int {
    for value in 0 {
        break
    }
    return 0
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support `for` lowering yet"
        }));
    }
}
