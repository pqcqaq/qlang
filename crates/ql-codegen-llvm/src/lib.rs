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
use ql_runtime::{RuntimeHook, RuntimeHookSignature};
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
    pub runtime_hooks: &'a [RuntimeHookSignature],
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
    body_return_ty: Ty,
    body_return_llvm_ty: String,
    return_ty: Ty,
    return_llvm_ty: String,
    params: Vec<ParamSignature>,
    body_style: FunctionBodyStyle,
    is_async: bool,
    async_body_llvm_name: Option<String>,
    async_frame_layout: Option<AsyncFrameLayout>,
    async_result_layout: Option<AsyncTaskResultLayout>,
}

#[derive(Clone, Debug)]
struct ParamSignature {
    name: String,
    ty: Ty,
    llvm_ty: String,
}

#[derive(Clone, Debug)]
struct AsyncFrameLayout {
    llvm_ty: String,
    size: u64,
    align: u64,
    fields: Vec<AsyncFrameField>,
}

#[derive(Clone, Debug)]
struct AsyncFrameField {
    param_index: usize,
    llvm_ty: String,
}

#[derive(Clone, Debug)]
enum AsyncTaskResultLayout {
    Void,
    Scalar {
        llvm_ty: String,
        size: u64,
        align: u64,
    },
}

impl AsyncTaskResultLayout {
    fn body_llvm_ty(&self) -> &str {
        match self {
            Self::Void => "void",
            Self::Scalar { llvm_ty, .. } => llvm_ty,
        }
    }
}

#[derive(Clone, Debug)]
struct PreparedFunction {
    signature: FunctionSignature,
    local_types: HashMap<mir::LocalId, Ty>,
    async_task_handles: HashMap<mir::LocalId, AsyncTaskHandleInfo>,
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

#[derive(Clone, Copy, Debug)]
struct ScalarAbiLayout {
    size: u64,
    align: u64,
}

#[derive(Clone, Debug)]
struct AsyncTaskHandleInfo {
    result_ty: Ty,
    result_layout: AsyncTaskResultLayout,
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
            return Err(CodegenError::new(dedupe_diagnostics(diagnostics)));
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
            return Err(CodegenError::new(dedupe_diagnostics(diagnostics)));
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

        let llvm_name = match body_style {
            FunctionBodyStyle::Definition => match function.abi.as_deref() {
                Some("c") => sanitize_symbol(&function.name),
                _ => llvm_symbol_name(
                    self.input.hir.function_owner_item(function_ref),
                    &function.name,
                ),
            },
            FunctionBodyStyle::Declaration => sanitize_symbol(&function.name),
        };

        let body_return_ty = function
            .return_type
            .map(|type_id| lower_type(self.input.hir, self.input.resolution, type_id))
            .unwrap_or_else(void_ty);
        let (body_return_llvm_ty, async_result_layout) =
            if function.is_async && body_style == FunctionBodyStyle::Definition {
                match build_async_task_result_layout(&body_return_ty, function.span) {
                    Ok(layout) => (layout.body_llvm_ty().to_owned(), Some(layout)),
                    Err(error) => {
                        diagnostics.push(error);
                        (String::new(), None)
                    }
                }
            } else {
                match lower_llvm_type(&body_return_ty, function.span, "return type") {
                    Ok(llvm_ty) => (llvm_ty, None),
                    Err(error) => {
                        diagnostics.push(error);
                        (String::new(), None)
                    }
                }
            };
        let mut return_ty = body_return_ty.clone();
        let mut return_llvm_ty = body_return_llvm_ty.clone();
        let mut async_body_llvm_name = None;
        let mut async_frame_layout = None;

        if function.is_async {
            match body_style {
                FunctionBodyStyle::Definition => {
                    if is_entry {
                        diagnostics.push(unsupported(
                            function.span,
                            "LLVM IR backend foundation does not support `async fn main` yet",
                        ));
                    }
                    if !self.has_runtime_hook(RuntimeHook::AsyncTaskCreate) {
                        diagnostics.push(unsupported(
                            function.span,
                            "LLVM IR backend foundation requires the `async-task-create` runtime hook before lowering `async fn` bodies",
                        ));
                    }
                    if !params.is_empty() && !self.has_runtime_hook(RuntimeHook::AsyncFrameAlloc) {
                        diagnostics.push(unsupported(
                            function.span,
                            "LLVM IR backend foundation requires the `async-frame-alloc` runtime hook before lowering parameterized `async fn` bodies",
                        ));
                    }
                    if !params.is_empty() {
                        match build_async_frame_layout(&params, function.span) {
                            Ok(layout) => async_frame_layout = Some(layout),
                            Err(error) => diagnostics.push(error),
                        }
                    }

                    return_ty = Ty::Unknown;
                    return_llvm_ty = "ptr".to_owned();
                    async_body_llvm_name = Some(format!("{llvm_name}__async_body"));
                }
                FunctionBodyStyle::Declaration => diagnostics.push(unsupported(
                    function.span,
                    "LLVM IR backend foundation does not support `async fn` declarations yet",
                )),
            }
        }

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
                body_return_ty,
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
            llvm_name,
            span: function.span,
            body_return_ty,
            body_return_llvm_ty,
            return_ty,
            return_llvm_ty,
            params,
            body_style,
            is_async: function.is_async,
            async_body_llvm_name,
            async_frame_layout,
            async_result_layout,
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
        let async_task_handles = self.collect_async_task_handles(body);
        let unsupported_for_iterable_locals = collect_unsupported_for_iterable_locals(body);

        for block in body.blocks() {
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                match &statement.kind {
                    StatementKind::Assign { place, value } => {
                        self.require_direct_place(statement.span, place, &mut diagnostics);
                        if unsupported_for_iterable_locals.contains(&place.base) {
                            continue;
                        }
                        if let Some(ty) = self.infer_rvalue_type(
                            body,
                            value,
                            &local_types,
                            &async_task_handles,
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
                            &async_task_handles,
                            &mut diagnostics,
                            statement.span,
                        );
                    }
                    StatementKind::Eval { value } => {
                        let _ = self.infer_rvalue_type(
                            body,
                            value,
                            &local_types,
                            &async_task_handles,
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
                        &async_task_handles,
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
                TerminatorKind::ForLoop { is_await, .. } => diagnostics.push(unsupported(
                    block.terminator.span,
                    if *is_await {
                        "LLVM IR backend foundation does not support `for await` lowering yet"
                    } else {
                        "LLVM IR backend foundation does not support `for` lowering yet"
                    },
                )),
            }
        }

        let should_validate_local_types = diagnostics.is_empty();
        for local_id in body.local_ids() {
            if async_task_handles.contains_key(&local_id) {
                continue;
            }
            let Some(ty) = local_types.get(&local_id) else {
                if diagnostics.is_empty() {
                    diagnostics.push(Diagnostic::error(format!(
                        "could not infer LLVM type for MIR local `{}`",
                        body.local(local_id).name
                    ))
                    .with_label(Label::new(body.local(local_id).span))
                    .with_note("this usually means the current MIR shape is not part of the P4 backend foundation support matrix"));
                }
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
                async_task_handles,
            })
        } else {
            Err(dedupe_diagnostics(diagnostics))
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
                LocalOrigin::ReturnSlot => Some(signature.body_return_ty.clone()),
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

    fn collect_async_task_handles(
        &self,
        body: &mir::MirBody,
    ) -> HashMap<mir::LocalId, AsyncTaskHandleInfo> {
        let mut handles = HashMap::new();

        for block in body.blocks() {
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                let StatementKind::Assign { place, value } = &statement.kind else {
                    continue;
                };
                if !place.projections.is_empty() {
                    continue;
                }
                let Rvalue::Call { callee, .. } = value else {
                    continue;
                };
                let Operand::Constant(Constant::Function { function, .. }) = callee else {
                    continue;
                };
                let Some(signature) = self.signatures.get(function) else {
                    continue;
                };
                if !signature.is_async {
                    continue;
                }
                let Some(result_layout) = signature.async_result_layout.clone() else {
                    continue;
                };

                handles.insert(
                    place.base,
                    AsyncTaskHandleInfo {
                        result_ty: signature.body_return_ty.clone(),
                        result_layout,
                    },
                );
            }
        }

        handles
    }

    fn infer_rvalue_type(
        &self,
        body: &mir::MirBody,
        value: &Rvalue,
        local_types: &HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        match value {
            Rvalue::Use(operand) => self.infer_operand_type(
                body,
                operand,
                local_types,
                async_task_handles,
                diagnostics,
                span,
            ),
            Rvalue::Call { callee, args } => {
                for arg in args {
                    if arg.name.is_some() {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support named call arguments yet",
                        ));
                    }
                    let _ = self.infer_operand_type(
                        body,
                        &arg.value,
                        local_types,
                        async_task_handles,
                        diagnostics,
                        span,
                    );
                }

                let Operand::Constant(Constant::Function { function, .. }) = callee else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports direct resolved function calls",
                    ));
                    return None;
                };

                let Some(signature) = self.signatures.get(function) else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation could not resolve the direct callee declaration",
                    ));
                    return None;
                };

                if signature.is_async {
                    None
                } else {
                    Some(signature.return_ty.clone())
                }
            }
            Rvalue::Binary { left, op, right } => {
                let left_ty = self.infer_operand_type(
                    body,
                    left,
                    local_types,
                    async_task_handles,
                    diagnostics,
                    span,
                )?;
                let right_ty = self.infer_operand_type(
                    body,
                    right,
                    local_types,
                    async_task_handles,
                    diagnostics,
                    span,
                )?;
                self.validate_binary_operands(*op, &left_ty, &right_ty, span, diagnostics)
            }
            Rvalue::Unary { op, operand } => match op {
                UnaryOp::Neg => {
                    let operand_ty = self.infer_operand_type(
                        body,
                        operand,
                        local_types,
                        async_task_handles,
                        diagnostics,
                        span,
                    )?;
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
                    if !self.has_runtime_hook(RuntimeHook::TaskAwait) {
                        diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation requires the `task-await` runtime hook before lowering `await` expressions",
                            ));
                        return None;
                    }
                    if !self.has_runtime_hook(RuntimeHook::TaskResultRelease) {
                        diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `await` expressions",
                            ));
                        return None;
                    }

                    let Operand::Place(place) = operand else {
                        diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation currently requires `await` operands to lower through a task-handle place",
                            ));
                        return None;
                    };
                    self.require_direct_place(span, place, diagnostics);

                    async_task_handles
                            .get(&place.base)
                            .map(|handle| handle.result_ty.clone())
                            .or_else(|| {
                                diagnostics.push(unsupported(
                                    span,
                                    "LLVM IR backend foundation could not resolve the async task handle consumed by `await`",
                                ));
                                None
                            })
                }
                UnaryOp::Spawn => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support `spawn` yet",
                    ));
                    None
                }
            },
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
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        match operand {
            Operand::Place(place) => {
                self.require_direct_place(span, place, diagnostics);
                local_types.get(&place.base).cloned().or_else(|| {
                    if async_task_handles.contains_key(&place.base) {
                        return Some(Ty::Unknown);
                    }
                    if diagnostics.is_empty() {
                        diagnostics.push(
                            Diagnostic::error(format!(
                                "could not resolve LLVM type for local `{}`",
                                body.local(place.base).name
                            ))
                            .with_label(Label::new(span)),
                        );
                    }
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

        if !self.input.runtime_hooks.is_empty() {
            output.push('\n');
            self.render_runtime_hook_declarations(&mut output);
        }

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

    fn render_runtime_hook_declarations(&self, output: &mut String) {
        for signature in self.input.runtime_hooks {
            let _ = writeln!(output, "{}", signature.render_llvm_declaration());
        }
    }

    fn has_runtime_hook(&self, hook: RuntimeHook) -> bool {
        self.input
            .runtime_hooks
            .iter()
            .any(|signature| signature.hook == hook)
    }

    fn runtime_hook_signature(&self, hook: RuntimeHook) -> Option<RuntimeHookSignature> {
        self.input
            .runtime_hooks
            .iter()
            .copied()
            .find(|signature| signature.hook == hook)
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
        if function.signature.is_async {
            self.render_async_function(output, function);
            return;
        }
        self.render_function_body(output, function, &function.signature.llvm_name);
    }

    fn render_async_function(&self, output: &mut String, function: &PreparedFunction) {
        let body_name = function
            .signature
            .async_body_llvm_name
            .as_deref()
            .expect("async functions should have a dedicated body symbol");
        self.render_function_body(output, function, body_name);
        output.push('\n');
        self.render_async_task_wrapper(output, function, body_name);
    }

    fn render_async_task_wrapper(
        &self,
        output: &mut String,
        function: &PreparedFunction,
        body_name: &str,
    ) {
        let task_create_hook = self
            .runtime_hook_signature(RuntimeHook::AsyncTaskCreate)
            .expect("async body lowering should require the async-task-create runtime hook");
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
        let frame = if let Some(layout) = &function.signature.async_frame_layout {
            let frame_alloc_hook = self
                .runtime_hook_signature(RuntimeHook::AsyncFrameAlloc)
                .expect(
                    "parameterized async body lowering should require the async-frame-alloc runtime hook",
                );
            output.push_str(&format!(
                "  %async_frame = call {} @{}(i64 {}, i64 {})\n",
                frame_alloc_hook.return_type.llvm_ir(),
                frame_alloc_hook.hook.symbol_name(),
                layout.size,
                layout.align
            ));
            for field in &layout.fields {
                output.push_str(&format!(
                    "  %async_frame_field{} = getelementptr inbounds {}, ptr %async_frame, i32 0, i32 {}\n",
                    field.param_index, layout.llvm_ty, field.param_index
                ));
                output.push_str(&format!(
                    "  store {} %arg{}, ptr %async_frame_field{}\n",
                    field.llvm_ty, field.param_index, field.param_index
                ));
            }
            "%async_frame"
        } else {
            "null"
        };
        let temp = "%async_task";
        let _ = writeln!(
            output,
            "  {temp} = call {} @{}(ptr @{}, ptr {frame})",
            task_create_hook.return_type.llvm_ir(),
            task_create_hook.hook.symbol_name(),
            body_name
        );
        let _ = writeln!(output, "  ret {} {temp}", function.signature.return_llvm_ty);
        output.push_str("}\n");
    }

    fn render_function_body(
        &self,
        output: &mut String,
        function: &PreparedFunction,
        llvm_name: &str,
    ) {
        let body = self
            .input
            .mir
            .body_for_owner(BodyOwner::Item(
                self.input
                    .hir
                    .function_owner_item(function.signature.function_ref),
            ))
            .expect("prepared function should still have a MIR body");
        let params = if function.signature.is_async {
            "ptr %frame".to_owned()
        } else {
            function
                .signature
                .params
                .iter()
                .enumerate()
                .map(|(index, param)| {
                    let _ = &param.name;
                    format!("{} %arg{index}", param.llvm_ty)
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let _ = writeln!(
            output,
            "define {} @{}({params}) {{",
            function.signature.body_return_llvm_ty, llvm_name
        );
        output.push_str("entry:\n");
        if function.signature.is_async {
            match function.signature.async_result_layout.as_ref() {
                Some(AsyncTaskResultLayout::Void) => {
                    debug_assert!(is_void_ty(&function.signature.body_return_ty));
                }
                Some(AsyncTaskResultLayout::Scalar {
                    llvm_ty,
                    size,
                    align,
                }) => {
                    debug_assert!(*size > 0);
                    debug_assert!(*align > 0);
                    debug_assert_eq!(llvm_ty, &function.signature.body_return_llvm_ty);
                }
                None => debug_assert!(false, "async definitions should precompute a result layout"),
            }
        }

        for local_id in body.local_ids() {
            if function.async_task_handles.contains_key(&local_id) {
                let _ = writeln!(output, "  {} = alloca ptr", llvm_slot_name(body, local_id));
                continue;
            }
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
            if let Some(layout) = &function.signature.async_frame_layout {
                let field = layout
                    .fields
                    .get(index)
                    .expect("async frame layout should cover every lowered parameter");
                let _ = writeln!(
                    output,
                    "  %async_body_frame_field{index} = getelementptr inbounds {}, ptr %frame, i32 0, i32 {}",
                    layout.llvm_ty, field.param_index
                );
                let _ = writeln!(
                    output,
                    "  %async_body_frame_value{index} = load {}, ptr %async_body_frame_field{index}",
                    field.llvm_ty
                );
                let _ = writeln!(
                    output,
                    "  store {} %async_body_frame_value{index}, ptr {}",
                    field.llvm_ty,
                    llvm_slot_name(body, local_id)
                );
                continue;
            }
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
                if is_void_ty(&self.prepared.signature.body_return_ty) {
                    output.push_str("  ret void\n");
                } else {
                    let temp = self.fresh_temp();
                    let slot = llvm_slot_name(self.body, self.body.return_local);
                    let _ = writeln!(
                        output,
                        "  {temp} = load {}, ptr {}",
                        self.prepared.signature.body_return_llvm_ty, slot
                    );
                    let _ = writeln!(
                        output,
                        "  ret {} {temp}",
                        self.prepared.signature.body_return_llvm_ty
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
            Rvalue::Unary { op, operand } => match op {
                UnaryOp::Neg => {
                    let operand = self.render_operand(output, operand, span);
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
                UnaryOp::Await => self.render_await(output, operand, span),
                UnaryOp::Spawn => {
                    panic!("prepared functions should not contain unsupported unary ops")
                }
            },
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

    fn render_await(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> Option<LoweredValue> {
        let await_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::TaskAwait)
            .expect("prepared await lowering should require the task-await runtime hook");
        let release_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::TaskResultRelease)
            .expect("prepared await lowering should require the task-result-release runtime hook");
        let Operand::Place(place) = operand else {
            panic!("prepared await operands should lower through task-handle places");
        };
        let handle_info = self
            .prepared
            .async_task_handles
            .get(&place.base)
            .unwrap_or_else(|| {
                panic!("prepared await operand at {span:?} should be a task handle")
            });
        let handle = self.render_operand(output, operand, span);
        let result_ptr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {result_ptr} = call {} @{}({} {})",
            await_hook.return_type.llvm_ir(),
            await_hook.hook.symbol_name(),
            handle.llvm_ty,
            handle.repr
        );

        match &handle_info.result_layout {
            AsyncTaskResultLayout::Void => {
                let _ = writeln!(
                    output,
                    "  call {} @{}(ptr {result_ptr})",
                    release_hook.return_type.llvm_ir(),
                    release_hook.hook.symbol_name()
                );
                Some(LoweredValue {
                    ty: void_ty(),
                    llvm_ty: "void".to_owned(),
                    repr: "void".to_owned(),
                })
            }
            AsyncTaskResultLayout::Scalar { llvm_ty, .. } => {
                let loaded = self.fresh_temp();
                let _ = writeln!(output, "  {loaded} = load {llvm_ty}, ptr {result_ptr}");
                let _ = writeln!(
                    output,
                    "  call {} @{}(ptr {result_ptr})",
                    release_hook.return_type.llvm_ir(),
                    release_hook.hook.symbol_name()
                );
                Some(LoweredValue {
                    ty: handle_info.result_ty.clone(),
                    llvm_ty: llvm_ty.clone(),
                    repr: loaded,
                })
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
                if self.prepared.async_task_handles.contains_key(&place.base) {
                    let temp = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {temp} = load ptr, ptr {}",
                        llvm_slot_name(self.body, place.base)
                    );
                    return LoweredValue {
                        ty: Ty::Unknown,
                        llvm_ty: "ptr".to_owned(),
                        repr: temp,
                    };
                }

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

fn build_async_frame_layout(
    params: &[ParamSignature],
    span: Span,
) -> Result<AsyncFrameLayout, Diagnostic> {
    let mut fields = Vec::with_capacity(params.len());
    let mut field_types = Vec::with_capacity(params.len());
    let mut size = 0;
    let mut align = 1;

    for (index, param) in params.iter().enumerate() {
        let layout = scalar_abi_layout(&param.ty, span, "async fn frame field type")?;
        size = align_to(size, layout.align);
        size += layout.size;
        align = align.max(layout.align);
        field_types.push(param.llvm_ty.clone());
        fields.push(AsyncFrameField {
            param_index: index,
            llvm_ty: param.llvm_ty.clone(),
        });
    }

    Ok(AsyncFrameLayout {
        llvm_ty: format!("{{ {} }}", field_types.join(", ")),
        size: align_to(size, align),
        align,
        fields,
    })
}

fn build_async_task_result_layout(
    ty: &Ty,
    span: Span,
) -> Result<AsyncTaskResultLayout, Diagnostic> {
    if is_void_ty(ty) {
        return Ok(AsyncTaskResultLayout::Void);
    }

    let llvm_ty = lower_llvm_type(ty, span, "async task result type")?;
    let layout = scalar_abi_layout(ty, span, "async task result type")?;

    Ok(AsyncTaskResultLayout::Scalar {
        llvm_ty,
        size: layout.size,
        align: layout.align,
    })
}

fn scalar_abi_layout(ty: &Ty, span: Span, context: &str) -> Result<ScalarAbiLayout, Diagnostic> {
    match ty {
        Ty::Builtin(BuiltinType::Bool)
        | Ty::Builtin(BuiltinType::I8)
        | Ty::Builtin(BuiltinType::U8) => Ok(ScalarAbiLayout { size: 1, align: 1 }),
        Ty::Builtin(BuiltinType::I16) | Ty::Builtin(BuiltinType::U16) => {
            Ok(ScalarAbiLayout { size: 2, align: 2 })
        }
        Ty::Builtin(BuiltinType::I32)
        | Ty::Builtin(BuiltinType::U32)
        | Ty::Builtin(BuiltinType::F32) => Ok(ScalarAbiLayout { size: 4, align: 4 }),
        Ty::Builtin(BuiltinType::Int)
        | Ty::Builtin(BuiltinType::UInt)
        | Ty::Builtin(BuiltinType::I64)
        | Ty::Builtin(BuiltinType::U64)
        | Ty::Builtin(BuiltinType::ISize)
        | Ty::Builtin(BuiltinType::USize)
        | Ty::Builtin(BuiltinType::F64) => Ok(ScalarAbiLayout { size: 8, align: 8 }),
        _ => Err(Diagnostic::error(format!(
            "LLVM IR backend foundation does not support {context} `{ty}` yet"
        ))
        .with_label(Label::new(span))),
    }
}

fn align_to(value: u64, align: u64) -> u64 {
    debug_assert!(align > 0);
    let remainder = value % align;
    if remainder == 0 {
        value
    } else {
        value + (align - remainder)
    }
}

fn collect_unsupported_for_iterable_locals(body: &mir::MirBody) -> HashSet<mir::LocalId> {
    let mut locals = HashSet::new();
    for block in body.blocks() {
        let TerminatorKind::ForLoop { iterable, .. } = &block.terminator.kind else {
            continue;
        };
        if let Operand::Place(place) = iterable {
            locals.insert(place.base);
        }
    }
    locals
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

fn dedupe_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut unique = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics {
        if !unique.contains(&diagnostic) {
            unique.push(diagnostic);
        }
    }
    unique
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
    use ql_runtime::{
        RuntimeCapability, RuntimeHook, RuntimeHookSignature, collect_runtime_hook_signatures,
        runtime_hook_signature,
    };

    use ql_resolve::BuiltinType;
    use ql_span::Span;
    use ql_typeck::Ty;

    use super::{
        AsyncTaskResultLayout, CodegenInput, CodegenMode, build_async_task_result_layout,
        emit_module,
    };

    fn emit(source: &str) -> String {
        emit_with_mode(source, CodegenMode::Program)
    }

    fn emit_library(source: &str) -> String {
        emit_with_mode(source, CodegenMode::Library)
    }

    fn emit_with_mode(source: &str, mode: CodegenMode) -> String {
        emit_with_runtime_hooks(source, mode, &[])
    }

    fn emit_with_runtime_hooks(
        source: &str,
        mode: CodegenMode,
        runtime_hooks: &[RuntimeHookSignature],
    ) -> String {
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
            runtime_hooks,
        })
        .expect("codegen should succeed")
    }

    fn emit_error(source: &str) -> Vec<String> {
        emit_error_with_runtime_hooks(source, CodegenMode::Program, &[])
    }

    fn emit_error_with_runtime_hooks(
        source: &str,
        mode: CodegenMode,
        runtime_hooks: &[RuntimeHookSignature],
    ) -> Vec<String> {
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
            runtime_hooks,
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
    fn emits_runtime_hook_declarations_from_shared_abi_contract() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
            RuntimeCapability::AsyncFunctionBodies,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
fn main() -> Int {
    return 0
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        let async_frame_alloc = "declare ptr @qlrt_async_frame_alloc(i64, i64)";
        let async_task_create = "declare ptr @qlrt_async_task_create(ptr, ptr)";
        let executor_spawn = "declare ptr @qlrt_executor_spawn(ptr, ptr)";
        let task_await = "declare ptr @qlrt_task_await(ptr)";
        let task_result_release = "declare void @qlrt_task_result_release(ptr)";
        let entry_definition = "define i64 @ql_0_main()";

        assert!(rendered.contains(async_frame_alloc));
        assert!(rendered.contains(async_task_create));
        assert!(rendered.contains(executor_spawn));
        assert!(rendered.contains(task_await));
        assert!(rendered.contains(task_result_release));
        assert!(
            rendered
                .find(async_frame_alloc)
                .expect("runtime declaration should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(async_task_create)
                .expect("runtime declaration should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(executor_spawn)
                .expect("runtime declaration should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(task_await)
                .expect("runtime declaration should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(task_result_release)
                .expect("runtime declaration should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
    }

    #[test]
    fn emits_async_task_create_wrapper_for_parameterless_async_body() {
        let runtime_hooks =
            collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("declare ptr @qlrt_async_frame_alloc(i64, i64)"));
        assert!(rendered.contains("declare ptr @qlrt_async_task_create(ptr, ptr)"));
        assert!(rendered.contains("define i64 @ql_0_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("define ptr @ql_0_worker()"));
        assert!(
            rendered.contains(
                "call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_body, ptr null)"
            )
        );
    }

    #[test]
    fn emits_async_task_create_wrapper_with_heap_frame_for_parameterized_async_body() {
        let runtime_hooks =
            collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(flag: Bool, value: Int) -> Int {
    if flag {
        return value
    }
    return 0
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("declare ptr @qlrt_async_frame_alloc(i64, i64)"));
        assert!(rendered.contains("define i64 @ql_0_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("define ptr @ql_0_worker(i1 %arg0, i64 %arg1)"));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 16, i64 8)"));
        assert!(
            rendered.contains("getelementptr inbounds { i1, i64 }, ptr %async_frame, i32 0, i32 0")
        );
        assert!(
            rendered.contains("getelementptr inbounds { i1, i64 }, ptr %async_frame, i32 0, i32 1")
        );
        assert!(rendered.contains("store i1 %arg0, ptr %async_frame_field0"));
        assert!(rendered.contains("store i64 %arg1, ptr %async_frame_field1"));
        assert!(rendered.contains(
            "call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_body, ptr %async_frame)"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field0 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 0"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field1 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 1"
        ));
    }

    #[test]
    fn builds_async_task_result_layouts_for_void_and_scalar_results() {
        let void_layout =
            build_async_task_result_layout(&Ty::Builtin(BuiltinType::Void), Span::new(0, 0))
                .expect("void async result layout should be supported");
        assert!(matches!(void_layout, AsyncTaskResultLayout::Void));
        assert_eq!(void_layout.body_llvm_ty(), "void");

        let int_layout =
            build_async_task_result_layout(&Ty::Builtin(BuiltinType::Int), Span::new(0, 0))
                .expect("scalar async result layout should be supported");
        match int_layout {
            AsyncTaskResultLayout::Scalar {
                llvm_ty,
                size,
                align,
            } => {
                assert_eq!(llvm_ty, "i64");
                assert_eq!(size, 8);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => panic!("expected scalar layout for Int"),
        }
    }

    #[test]
    fn rejects_parameterized_async_function_bodies_without_async_frame_alloc_hook() {
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}
"#,
            CodegenMode::Library,
            &[runtime_hook_signature(RuntimeHook::AsyncTaskCreate)],
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation requires the `async-frame-alloc` runtime hook before lowering parameterized `async fn` bodies"
        }));
    }

    #[test]
    fn rejects_async_function_bodies_without_async_task_create_hook() {
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}
"#,
            CodegenMode::Library,
            &[],
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation requires the `async-task-create` runtime hook before lowering `async fn` bodies"
        }));
    }

    #[test]
    fn rejects_unsupported_async_task_result_types_before_await_lowering() {
        let runtime_hooks =
            collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn worker() -> (Int, Int) {
    return (1, 2)
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation does not support async task result type `(Int, Int)` yet"
        }));
        assert!(
            messages.iter().all(|message| {
                !message.contains("does not support return type `(Int, Int)` yet")
            })
        );
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
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
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
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
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
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn emits_await_lowering_for_scalar_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("declare ptr @qlrt_task_await(ptr)"));
        assert!(rendered.contains("declare void @qlrt_task_result_release(ptr)"));
        assert!(rendered.contains("define i64 @ql_1_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_void_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Void {
    return
}

async fn helper() -> Void {
    await worker()
    return
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define void @ql_1_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
        assert!(!rendered.contains("load void"));
    }

    #[test]
    fn rejects_await_lowering_without_task_result_release_hook() {
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    return await worker()
}
"#,
            CodegenMode::Library,
            &[
                runtime_hook_signature(RuntimeHook::AsyncTaskCreate),
                runtime_hook_signature(RuntimeHook::TaskAwait),
            ],
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `await` expressions"
        }));
    }

    #[test]
    fn rejects_unsupported_spawn_lowering_in_async_library_body() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
        ]);
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    spawn worker()
    return 0
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support `spawn` yet"
        }));
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn rejects_unsupported_for_await_lowering_without_iterable_noise() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::AsyncIteration,
        ]);
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn helper() -> Int {
    for await value in [1, 2, 3] {
        break
    }
    return 0
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(messages.iter().any(|message| {
            message == "LLVM IR backend foundation does not support `for await` lowering yet"
        }));
        assert!(messages.iter().all(|message| {
            message != "LLVM IR backend foundation does not support `for` lowering yet"
                && message != "LLVM IR backend foundation does not support array values yet"
                && !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_cleanup_lowering_diagnostics() {
        let messages = emit_error(
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    return 0
}
"#,
        );

        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support cleanup lowering yet"
                })
                .count(),
            1
        );
    }
}
