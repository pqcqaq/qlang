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
use ql_resolve::{BuiltinType, ResolutionMap, TypeResolution, ValueResolution};
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
    pub inline_runtime_support: bool,
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
    Loadable {
        llvm_ty: String,
        _size: u64,
        align: u64,
    },
}

impl AsyncTaskResultLayout {
    fn body_llvm_ty(&self) -> &str {
        match self {
            Self::Void => "void",
            Self::Loadable { llvm_ty, .. } => llvm_ty,
        }
    }

    fn storage_size(&self) -> u64 {
        match self {
            Self::Void => 1,
            Self::Loadable { _size, .. } => (*_size).max(1),
        }
    }
}

#[derive(Clone, Debug)]
struct PreparedFunction {
    signature: FunctionSignature,
    local_types: HashMap<mir::LocalId, Ty>,
    async_task_handles: HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    task_handle_place_aliases: HashMap<mir::LocalId, mir::Place>,
    supported_for_loops: HashMap<mir::BasicBlockId, SupportedForLoopLowering>,
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

#[derive(Clone, Debug)]
struct SupportedForLoopLowering {
    iterable_place: Place,
    item_local: mir::LocalId,
    element_ty: Ty,
    array_len: usize,
    body_target: mir::BasicBlockId,
}

#[derive(Clone, Copy, Debug)]
struct ScalarAbiLayout {
    size: u64,
    align: u64,
}

#[derive(Clone, Copy, Debug)]
struct LoadableAbiLayout {
    size: u64,
    align: u64,
}

#[derive(Clone, Debug)]
struct StructFieldLowering {
    name: String,
    ty: Ty,
    llvm_ty: String,
}

#[derive(Clone, Debug)]
struct AsyncTaskHandleInfo {
    result_ty: Ty,
    result_layout: AsyncTaskResultLayout,
}

#[derive(Clone, Debug)]
enum ResolvedProjectionStep {
    Field { index: usize, ty: Ty },
    TupleIndex { index: usize, ty: Ty },
    ArrayIndex { ty: Ty },
}

impl ResolvedProjectionStep {
    fn output_ty(&self) -> Ty {
        match self {
            Self::Field { ty, .. } | Self::TupleIndex { ty, .. } | Self::ArrayIndex { ty } => {
                ty.clone()
            }
        }
    }
}

struct TypeInferenceContext<'a> {
    body: &'a mir::MirBody,
    local_types: &'a HashMap<mir::LocalId, Ty>,
    async_task_handles: &'a HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    diagnostics: &'a mut Vec<Diagnostic>,
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
                    match self.lower_llvm_type(&ty, param.name_span, "parameter type") {
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
                match self.build_async_task_result_layout(&body_return_ty, function.span) {
                    Ok(layout) => (layout.body_llvm_ty().to_owned(), Some(layout)),
                    Err(error) => {
                        diagnostics.push(error);
                        (String::new(), None)
                    }
                }
            } else {
                match self.lower_llvm_type(&body_return_ty, function.span, "return type") {
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
                    // async fn main is now supported in program mode: the host entry wrapper
                    // (render_host_entry_wrapper) drives the full task lifecycle
                    // (task_create → executor_spawn → task_await → load result →
                    // task_result_release) and returns an i32 exit code to the OS.
                    // These extra hooks are only required when lowering the host entry.
                    if is_entry
                        && self.input.mode == CodegenMode::Program
                        && (self.has_runtime_hook(RuntimeHook::TaskAwait)
                            || self.has_runtime_hook(RuntimeHook::TaskResultRelease))
                    {
                        if !self.has_runtime_hook(RuntimeHook::ExecutorSpawn) {
                            diagnostics.push(unsupported(
                                function.span,
                                "LLVM IR backend foundation requires the `executor-spawn` runtime hook before lowering `async fn main`",
                            ));
                        }
                        if !self.has_runtime_hook(RuntimeHook::TaskAwait) {
                            diagnostics.push(unsupported(
                                function.span,
                                "LLVM IR backend foundation requires the `task-await` runtime hook before lowering `async fn main`",
                            ));
                        }
                        if !self.has_runtime_hook(RuntimeHook::TaskResultRelease) {
                            diagnostics.push(unsupported(
                                function.span,
                                "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `async fn main`",
                            ));
                        }
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
                        match self.build_async_frame_layout(&params, function.span) {
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
        let task_handle_place_aliases =
            self.collect_task_handle_place_aliases(body, &local_types, &mut diagnostics);
        let mut supported_for_loops = HashMap::new();
        self.propagate_expected_temp_local_types(body, &mut local_types, &async_task_handles);

        for block_id in body.block_ids() {
            let block = body.block(block_id);
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                match &statement.kind {
                    StatementKind::Assign { place, value } => {
                        let expected_ty = self.assignment_place_type(
                            body,
                            place,
                            &local_types,
                            &async_task_handles,
                            statement.span,
                            &mut diagnostics,
                        );
                        let mut infer = TypeInferenceContext {
                            body,
                            local_types: &local_types,
                            async_task_handles: &async_task_handles,
                            diagnostics: &mut diagnostics,
                        };
                        if let Some(ty) = self.infer_rvalue_type(
                            value,
                            expected_ty.as_ref(),
                            &mut infer,
                            statement.span,
                        ) {
                            if place.projections.is_empty() {
                                local_types.entry(place.base).or_insert(ty);
                            }
                        }
                    }
                    StatementKind::BindPattern {
                        pattern, source, ..
                    } => {
                        self.require_binding_pattern(*pattern, statement.span, &mut diagnostics);
                        let source_ty = self.infer_operand_type(
                            body,
                            source,
                            &local_types,
                            &async_task_handles,
                            &mut diagnostics,
                            statement.span,
                        );
                        if let (Some(binding_local), Some(source_ty)) =
                            (self.binding_local_for_pattern(body, *pattern), source_ty)
                        {
                            seed_inferred_local_type(&mut local_types, binding_local, source_ty);
                        }
                    }
                    StatementKind::Eval { value } => {
                        let mut infer = TypeInferenceContext {
                            body,
                            local_types: &local_types,
                            async_task_handles: &async_task_handles,
                            diagnostics: &mut diagnostics,
                        };
                        let _ = self.infer_rvalue_type(value, None, &mut infer, statement.span);
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
                TerminatorKind::ForLoop {
                    iterable,
                    item_local,
                    is_await,
                    body_target,
                    ..
                } => {
                    if !*is_await {
                        diagnostics.push(unsupported(
                            block.terminator.span,
                            "LLVM IR backend foundation does not support `for` lowering yet",
                        ));
                        continue;
                    }

                    let mut scratch = Vec::new();
                    let iterable_ty = self.infer_operand_type(
                        body,
                        iterable,
                        &local_types,
                        &async_task_handles,
                        &mut scratch,
                        block.terminator.span,
                    );
                    let (Operand::Place(iterable_place), Some(Ty::Array { element, len })) =
                        (iterable, iterable_ty)
                    else {
                        diagnostics.push(unsupported(
                            block.terminator.span,
                            "LLVM IR backend foundation does not support `for await` lowering yet",
                        ));
                        continue;
                    };
                    let element_ty = element.as_ref().clone();
                    seed_inferred_local_type(&mut local_types, *item_local, element_ty.clone());
                    supported_for_loops.insert(
                        block_id,
                        SupportedForLoopLowering {
                            iterable_place: iterable_place.clone(),
                            item_local: *item_local,
                            element_ty,
                            array_len: len,
                            body_target: *body_target,
                        },
                    );
                }
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
            if let Err(error) = self.lower_llvm_type(ty, body.local(local_id).span, "local type") {
                diagnostics.push(error);
            }
        }

        if diagnostics.is_empty() {
            Ok(PreparedFunction {
                signature,
                local_types,
                async_task_handles,
                task_handle_place_aliases,
                supported_for_loops,
            })
        } else {
            Err(dedupe_diagnostics(diagnostics))
        }
    }

    fn propagate_expected_temp_local_types(
        &self,
        body: &mir::MirBody,
        local_types: &mut HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    ) {
        let max_passes = body.locals().len().max(1);
        for _ in 0..max_passes {
            let before = local_types.len();
            for block in body.blocks() {
                for statement_id in &block.statements {
                    let statement = body.statement(*statement_id);
                    self.propagate_expected_temp_types_from_statement(
                        body,
                        statement,
                        local_types,
                        async_task_handles,
                    );
                }
            }
            if local_types.len() == before {
                break;
            }
        }
    }

    fn propagate_expected_temp_types_from_statement(
        &self,
        body: &mir::MirBody,
        statement: &mir::Statement,
        local_types: &mut HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    ) {
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                let mut diagnostics = Vec::new();
                if let Some(expected_ty) = self.infer_place_type(
                    body,
                    place,
                    local_types,
                    async_task_handles,
                    &mut diagnostics,
                    statement.span,
                ) {
                    self.seed_expected_temp_from_rvalue(body, value, &expected_ty, local_types);
                }
                self.seed_expected_temp_from_call_args(body, value, local_types);
            }
            StatementKind::BindPattern {
                pattern, source, ..
            } => {
                if let Some(expected_ty) = self
                    .binding_local_for_pattern(body, *pattern)
                    .and_then(|binding_local| local_types.get(&binding_local).cloned())
                {
                    self.seed_expected_temp_from_operand(body, source, &expected_ty, local_types);
                }
            }
            StatementKind::Eval { value } => {
                self.seed_expected_temp_from_call_args(body, value, local_types);
            }
            StatementKind::StorageLive { .. }
            | StatementKind::StorageDead { .. }
            | StatementKind::RegisterCleanup { .. }
            | StatementKind::RunCleanup { .. } => {}
        }
    }

    fn seed_expected_temp_from_rvalue(
        &self,
        body: &mir::MirBody,
        value: &Rvalue,
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        match value {
            Rvalue::Use(operand) => {
                self.seed_expected_temp_from_operand(body, operand, expected_ty, local_types);
            }
            Rvalue::Tuple(items) => {
                self.seed_expected_temp_from_tuple_items(body, items, expected_ty, local_types);
            }
            Rvalue::Array(items) => {
                self.seed_expected_temp_from_array_items(body, items, expected_ty, local_types);
            }
            Rvalue::AggregateStruct { fields, .. } => {
                self.seed_expected_temp_from_struct_fields(body, fields, expected_ty, local_types);
            }
            Rvalue::Call { .. }
            | Rvalue::Binary { .. }
            | Rvalue::Unary { .. }
            | Rvalue::Closure { .. }
            | Rvalue::Question(_)
            | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn seed_expected_temp_from_call_args(
        &self,
        body: &mir::MirBody,
        value: &Rvalue,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        let Rvalue::Call { callee, args } = value else {
            return;
        };
        let Operand::Constant(Constant::Function { function, .. }) = callee else {
            return;
        };
        let Some(signature) = self.signatures.get(function) else {
            return;
        };

        for (arg, param) in args.iter().zip(signature.params.iter()) {
            self.seed_expected_temp_from_operand(body, &arg.value, &param.ty, local_types);
        }
    }

    fn seed_expected_temp_from_operand(
        &self,
        body: &mir::MirBody,
        operand: &Operand,
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        let Operand::Place(place) = operand else {
            return;
        };
        if !place.projections.is_empty() {
            return;
        }
        self.seed_direct_temp_local_type(body, place.base, expected_ty, local_types);
    }

    fn seed_expected_temp_from_tuple_items(
        &self,
        body: &mir::MirBody,
        items: &[Operand],
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        let Ty::Tuple(expected_items) = expected_ty else {
            return;
        };
        for (item, expected_item_ty) in items.iter().zip(expected_items.iter()) {
            self.seed_expected_temp_from_operand(body, item, expected_item_ty, local_types);
        }
    }

    fn seed_expected_temp_from_array_items(
        &self,
        body: &mir::MirBody,
        items: &[Operand],
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        let Ty::Array { element, .. } = expected_ty else {
            return;
        };
        for item in items {
            self.seed_expected_temp_from_operand(body, item, element, local_types);
        }
    }

    fn seed_expected_temp_from_struct_fields(
        &self,
        body: &mir::MirBody,
        fields: &[mir::AggregateField],
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        let Ok(field_layouts) =
            self.struct_field_lowerings(expected_ty, Span::default(), "expected struct value type")
        else {
            return;
        };
        for field in fields {
            let Some(expected_field_ty) = field_layouts
                .iter()
                .find(|info| info.name == field.name)
                .map(|info| &info.ty)
            else {
                continue;
            };
            self.seed_expected_temp_from_operand(
                body,
                &field.value,
                expected_field_ty,
                local_types,
            );
        }
    }

    fn seed_direct_temp_local_type(
        &self,
        body: &mir::MirBody,
        local_id: mir::LocalId,
        expected_ty: &Ty,
        local_types: &mut HashMap<mir::LocalId, Ty>,
    ) {
        if !Self::is_useful_expected_temp_type(expected_ty) {
            return;
        }
        if local_types.contains_key(&local_id) {
            return;
        }
        if matches!(body.local(local_id).origin, LocalOrigin::Temp { .. }) {
            local_types.insert(local_id, expected_ty.clone());
        }
    }

    fn is_useful_expected_temp_type(ty: &Ty) -> bool {
        match ty {
            Ty::Unknown | Ty::Generic(_) => false,
            Ty::Builtin(_) => true,
            Ty::Array { element, .. } => Self::is_useful_expected_temp_type(element),
            Ty::Item { args, .. } | Ty::Import { args, .. } | Ty::Named { args, .. } => {
                args.iter().all(Self::is_useful_expected_temp_type)
            }
            Ty::Pointer { inner, .. } | Ty::TaskHandle(inner) => {
                Self::is_useful_expected_temp_type(inner)
            }
            Ty::Tuple(items) => items.iter().all(Self::is_useful_expected_temp_type),
            Ty::Callable { params, ret } => {
                params.iter().all(Self::is_useful_expected_temp_type)
                    && Self::is_useful_expected_temp_type(ret)
            }
        }
    }

    fn binding_local_for_pattern(
        &self,
        body: &mir::MirBody,
        pattern: hir::PatternId,
    ) -> Option<mir::LocalId> {
        let PatternKind::Binding(local) = pattern_kind(self.input.hir, pattern) else {
            return None;
        };
        body.local_ids().find(|candidate| {
            matches!(
                body.local(*candidate).origin,
                LocalOrigin::Binding(hir_local) if hir_local == *local
            )
        })
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

    fn collect_task_handle_place_aliases(
        &self,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> HashMap<mir::LocalId, mir::Place> {
        let mut aliases = HashMap::new();

        for block in body.blocks() {
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                let StatementKind::BindPattern {
                    pattern,
                    source: Operand::Place(source_place),
                    mutable,
                } = &statement.kind
                else {
                    continue;
                };
                if *mutable {
                    continue;
                }
                let Some(binding_local) = self.binding_local_for_pattern(body, *pattern) else {
                    continue;
                };
                let Some(binding_ty) = local_types.get(&binding_local) else {
                    continue;
                };
                if !self.ty_contains_task_handles(binding_ty, statement.span, diagnostics) {
                    continue;
                }
                aliases.insert(binding_local, source_place.clone());
            }
        }

        aliases
    }

    fn ty_contains_task_handles(
        &self,
        ty: &Ty,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        match ty {
            Ty::TaskHandle(_) => true,
            Ty::Array { element, .. } | Ty::Pointer { inner: element, .. } => {
                self.ty_contains_task_handles(element, span, diagnostics)
            }
            Ty::Tuple(items) => items
                .iter()
                .any(|item| self.ty_contains_task_handles(item, span, diagnostics)),
            Ty::Item { .. } => {
                match self.struct_field_lowerings(ty, span, "task-handle alias analysis") {
                    Ok(fields) => fields
                        .iter()
                        .any(|field| self.ty_contains_task_handles(&field.ty, span, diagnostics)),
                    Err(error) => {
                        diagnostics.push(error);
                        false
                    }
                }
            }
            _ => false,
        }
    }

    fn infer_rvalue_type(
        &self,
        value: &Rvalue,
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        match value {
            Rvalue::Use(operand) => self.infer_operand_type(
                ctx.body,
                operand,
                ctx.local_types,
                ctx.async_task_handles,
                ctx.diagnostics,
                span,
            ),
            Rvalue::Call { callee, args } => {
                for arg in args {
                    if arg.name.is_some() {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support named call arguments yet",
                        ));
                    }
                    let _ = self.infer_operand_type(
                        ctx.body,
                        &arg.value,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    );
                }

                let Operand::Constant(Constant::Function { function, .. }) = callee else {
                    ctx.diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports direct resolved function calls",
                    ));
                    return None;
                };

                let Some(signature) = self.signatures.get(function) else {
                    ctx.diagnostics.push(unsupported(
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
                    ctx.body,
                    left,
                    ctx.local_types,
                    ctx.async_task_handles,
                    ctx.diagnostics,
                    span,
                )?;
                let right_ty = self.infer_operand_type(
                    ctx.body,
                    right,
                    ctx.local_types,
                    ctx.async_task_handles,
                    ctx.diagnostics,
                    span,
                )?;
                self.validate_binary_operands(*op, &left_ty, &right_ty, span, ctx.diagnostics)
            }
            Rvalue::Unary { op, operand } => match op {
                UnaryOp::Neg => {
                    let operand_ty = self.infer_operand_type(
                        ctx.body,
                        operand,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    )?;
                    if is_numeric_ty(&operand_ty) {
                        Some(operand_ty)
                    } else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation only supports numeric negation",
                        ));
                        None
                    }
                }
                UnaryOp::Await => {
                    if !self.has_runtime_hook(RuntimeHook::TaskAwait) {
                        ctx.diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation requires the `task-await` runtime hook before lowering `await` expressions",
                            ));
                        return None;
                    }
                    if !self.has_runtime_hook(RuntimeHook::TaskResultRelease) {
                        ctx.diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering `await` expressions",
                            ));
                        return None;
                    }

                    let Operand::Place(place) = operand else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation currently requires `await` operands to lower through a task-handle place",
                        ));
                        return None;
                    };
                    let Some(operand_ty) = self.infer_operand_type(
                        ctx.body,
                        operand,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    ) else {
                        return None;
                    };
                    if !place.projections.is_empty() && !matches!(operand_ty, Ty::TaskHandle(_)) {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support field or index projections yet",
                        ));
                        return None;
                    }

                    match operand_ty {
                        Ty::TaskHandle(result_ty) => Some(*result_ty),
                        _ => {
                            ctx.diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation could not resolve the async task handle consumed by `await`",
                            ));
                            None
                        }
                    }
                }
                UnaryOp::Spawn => {
                    if !self.has_runtime_hook(RuntimeHook::ExecutorSpawn) {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation requires the `executor-spawn` runtime hook before lowering `spawn` expressions",
                        ));
                        return None;
                    }

                    let Operand::Place(place) = operand else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation currently requires `spawn` operands to lower through a task-handle place",
                        ));
                        return None;
                    };
                    let Some(operand_ty) = self.infer_operand_type(
                        ctx.body,
                        operand,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    ) else {
                        return None;
                    };
                    if !place.projections.is_empty() && !matches!(operand_ty, Ty::TaskHandle(_)) {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support field or index projections yet",
                        ));
                        return None;
                    }

                    match operand_ty {
                        Ty::TaskHandle(result_ty) => Some(Ty::TaskHandle(result_ty)),
                        _ => {
                            ctx.diagnostics.push(unsupported(
                                span,
                                "LLVM IR backend foundation could not resolve the async task handle consumed by `spawn`",
                            ));
                            None
                        }
                    }
                }
            },
            Rvalue::Tuple(items) => {
                let mut item_types = Vec::with_capacity(items.len());
                for item in items {
                    item_types.push(self.infer_operand_type(
                        ctx.body,
                        item,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    )?);
                }
                Some(Ty::Tuple(item_types))
            }
            Rvalue::Array(items) => self.infer_array_rvalue_type(items, expected_ty, ctx, span),
            Rvalue::AggregateStruct { path, fields } => {
                self.infer_struct_rvalue_type(path, fields, expected_ty, ctx, span)
            }
            Rvalue::Closure { .. } => {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support closure values yet",
                ));
                None
            }
            Rvalue::Question(_) => {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation does not support `?` lowering yet",
                ));
                None
            }
            Rvalue::OpaqueExpr(_) => {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered an opaque expression that still needs MIR elaboration",
                ));
                None
            }
        }
    }

    fn infer_struct_rvalue_type(
        &self,
        path: &ql_ast::Path,
        fields: &[mir::AggregateField],
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        let struct_ty = expected_ty
            .filter(|ty| matches!(ty, Ty::Item { .. }))
            .cloned()
            .or_else(|| self.resolve_local_struct_path(path));
        let Some(struct_ty) = struct_ty else {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation could not resolve the struct type for this aggregate value",
            ));
            return None;
        };

        let field_layouts = match self.struct_field_lowerings(&struct_ty, span, "struct value type")
        {
            Ok(layouts) => layouts,
            Err(error) => {
                ctx.diagnostics.push(error);
                return None;
            }
        };

        for field in fields {
            let Some(expected_field) = field_layouts.iter().find(|info| info.name == field.name)
            else {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered a struct field that was not present in the lowered declaration",
                ));
                continue;
            };
            let Some(actual_ty) = self.infer_operand_type(
                ctx.body,
                &field.value,
                ctx.local_types,
                ctx.async_task_handles,
                ctx.diagnostics,
                span,
            ) else {
                continue;
            };
            if !expected_field.ty.compatible_with(&actual_ty) {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered a struct field value whose lowered type did not match the declaration",
                ));
            }
        }

        Some(struct_ty)
    }

    fn infer_array_rvalue_type(
        &self,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        let expected_array = match expected_ty {
            Some(Ty::Array { element, len }) => Some((element.as_ref().clone(), *len)),
            _ => None,
        };
        if let Some((_, expected_len)) = expected_array.as_ref()
            && items.len() != *expected_len
        {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation encountered an array literal whose length no longer matches the expected array type",
            ));
            return None;
        }
        if items.is_empty() && expected_array.is_none() {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation cannot infer the element type of an empty array literal without an expected array type",
            ));
            return None;
        }

        let mut element_ty = expected_array
            .as_ref()
            .map(|(element, _)| element.clone())
            .unwrap_or(Ty::Unknown);
        for item in items {
            let item_ty = self.infer_operand_type(
                ctx.body,
                item,
                ctx.local_types,
                ctx.async_task_handles,
                ctx.diagnostics,
                span,
            )?;

            if expected_array.is_none() && element_ty.is_unknown() && !item_ty.is_unknown() {
                element_ty = item_ty;
                continue;
            }

            if item_ty.is_unknown() {
                continue;
            }

            let expected_element = expected_array
                .as_ref()
                .map(|(element, _)| element)
                .unwrap_or(&element_ty);
            if !expected_element.compatible_with(&item_ty) {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation currently requires array literals to lower to one compatible element type",
                ));
                return None;
            }
        }

        if element_ty.is_unknown() {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation could not resolve the element type of this array literal",
            ));
            return None;
        }

        Some(Ty::Array {
            element: Box::new(element_ty),
            len: items.len(),
        })
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
            Operand::Place(place) => self.infer_place_type(
                body,
                place,
                local_types,
                async_task_handles,
                diagnostics,
                span,
            ),
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
                Constant::Item { item, .. } => match &self.input.hir.item(*item).kind {
                    ItemKind::Const(global) => {
                        Some(lower_type(self.input.hir, self.input.resolution, global.ty))
                    }
                    _ => {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support item values here",
                        ));
                        None
                    }
                },
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

    fn infer_place_type(
        &self,
        body: &mir::MirBody,
        place: &Place,
        local_types: &HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        let mut current_ty = self.resolve_place_base_type(
            body,
            place,
            local_types,
            async_task_handles,
            diagnostics,
            span,
        )?;

        for projection in &place.projections {
            if let mir::ProjectionElem::Index(index) = projection {
                let index_ty = self.infer_operand_type(
                    body,
                    index,
                    local_types,
                    async_task_handles,
                    diagnostics,
                    span,
                )?;
                if !index_ty.is_unknown()
                    && !Ty::Builtin(BuiltinType::Int).compatible_with(&index_ty)
                {
                    diagnostics.push(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires index projections to use `Int`, found `{index_ty}`"
                        ),
                    ));
                    return None;
                }
            }

            let step = match self.resolve_projection_step(&current_ty, projection, span) {
                Ok(step) => step,
                Err(error) => {
                    diagnostics.push(error);
                    return None;
                }
            };
            current_ty = step.output_ty();
        }

        Some(current_ty)
    }

    fn resolve_place_base_type(
        &self,
        body: &mir::MirBody,
        place: &Place,
        local_types: &HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        local_types.get(&place.base).cloned().or_else(|| {
            if let Some(handle) = async_task_handles.get(&place.base) {
                return Some(Ty::TaskHandle(Box::new(handle.result_ty.clone())));
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

    fn resolve_projection_step(
        &self,
        current_ty: &Ty,
        projection: &mir::ProjectionElem,
        span: Span,
    ) -> Result<ResolvedProjectionStep, Diagnostic> {
        match projection {
            mir::ProjectionElem::Field(field) => {
                let (index, field) = self.struct_field_projection(current_ty, field, span)?;
                Ok(ResolvedProjectionStep::Field {
                    index,
                    ty: field.ty,
                })
            }
            mir::ProjectionElem::TupleIndex(index) => match current_ty {
                Ty::Tuple(items) => items
                    .get(*index)
                    .cloned()
                    .map(|ty| ResolvedProjectionStep::TupleIndex { index: *index, ty })
                    .ok_or_else(|| {
                        unsupported(
                            span,
                            format!(
                                "LLVM IR backend foundation could not project tuple index `{index}` from `{current_ty}`"
                            ),
                        )
                    }),
                _ => Err(unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation does not support tuple projection on `{current_ty}` yet"
                    ),
                )),
            },
            mir::ProjectionElem::Index(index) => match current_ty {
                Ty::Array { element, .. } => Ok(ResolvedProjectionStep::ArrayIndex {
                    ty: element.as_ref().clone(),
                }),
                Ty::Tuple(items) => {
                    let index = self.parse_tuple_projection_index(index, current_ty, span)?;
                    items.get(index).cloned().map_or_else(
                        || {
                            Err(unsupported(
                                span,
                                format!(
                                    "LLVM IR backend foundation could not project tuple index `{index}` from `{current_ty}`"
                                ),
                            ))
                        },
                        |ty| Ok(ResolvedProjectionStep::TupleIndex { index, ty }),
                    )
                }
                _ => Err(unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation does not support index projection on `{current_ty}` yet"
                    ),
                )),
            },
        }
    }

    fn struct_field_projection(
        &self,
        ty: &Ty,
        field: &str,
        span: Span,
    ) -> Result<(usize, StructFieldLowering), Diagnostic> {
        let fields = self.struct_field_lowerings(ty, span, "field projection")?;
        fields
            .into_iter()
            .enumerate()
            .find(|(_, candidate)| candidate.name == field)
            .ok_or_else(|| {
                unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation could not resolve field `{field}` on `{ty}`"
                    ),
                )
            })
    }

    fn parse_tuple_projection_index(
        &self,
        index: &Operand,
        current_ty: &Ty,
        span: Span,
    ) -> Result<usize, Diagnostic> {
        let Operand::Constant(Constant::Integer(raw)) = index else {
            return Err(unsupported(
                span,
                format!(
                    "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use an integer literal index"
                ),
            ));
        };

        ql_ast::parse_usize_literal(raw).ok_or_else(|| {
            unsupported(
                span,
                format!(
                    "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use a non-negative integer literal index"
                ),
            )
        })
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

    fn assignment_place_type(
        &self,
        body: &mir::MirBody,
        place: &Place,
        local_types: &HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Option<Ty> {
        if place.projections.is_empty() {
            return local_types.get(&place.base).cloned().or_else(|| {
                async_task_handles
                    .get(&place.base)
                    .map(|handle| Ty::TaskHandle(Box::new(handle.result_ty.clone())))
            });
        }

        let mut current_ty = self.resolve_place_base_type(
            body,
            place,
            local_types,
            async_task_handles,
            diagnostics,
            span,
        )?;

        for projection in &place.projections {
            if let mir::ProjectionElem::Index(index) = projection {
                let index_ty = self.infer_operand_type(
                    body,
                    index,
                    local_types,
                    async_task_handles,
                    diagnostics,
                    span,
                )?;
                if !index_ty.is_unknown()
                    && !Ty::Builtin(BuiltinType::Int).compatible_with(&index_ty)
                {
                    diagnostics.push(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires index projections to use `Int`, found `{index_ty}`"
                        ),
                    ));
                    return None;
                }
            }

            let step = match self.resolve_projection_step(&current_ty, projection, span) {
                Ok(step) => step,
                Err(error) => {
                    diagnostics.push(error);
                    return None;
                }
            };

            match (&projection, &step) {
                (mir::ProjectionElem::Field(_), ResolvedProjectionStep::Field { .. })
                | (mir::ProjectionElem::TupleIndex(_), ResolvedProjectionStep::TupleIndex { .. })
                | (mir::ProjectionElem::Index(_), ResolvedProjectionStep::TupleIndex { .. })
                | (mir::ProjectionElem::Index(_), ResolvedProjectionStep::ArrayIndex { .. }) => {}
                _ => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support this assignment place yet",
                    ));
                    return None;
                }
            }

            current_ty = step.output_ty();
        }

        Some(current_ty)
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
            self.render_runtime_heap_declarations(&mut output);
            self.render_program_runtime_support(&mut output);
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

        if entry.signature.is_async
            && self.has_runtime_hook(RuntimeHook::ExecutorSpawn)
            && self.has_runtime_hook(RuntimeHook::TaskAwait)
            && self.has_runtime_hook(RuntimeHook::TaskResultRelease)
        {
            // async fn main: drive the full task lifecycle to get the exit code.
            //
            // Sequence (per RuntimeHook contracts in ql-runtime/src/lib.rs):
            //   task    = @ql_N_main()                             ← task_create wrapper
            //   join    = qlrt_executor_spawn(null, task)          ← submit to default executor
            //   res_ptr = qlrt_task_await(join)                    ← block until done
            //   ret_val = load <RetTy>, ptr res_ptr                ← extract payload
            //             qlrt_task_result_release(res_ptr)        ← free payload buffer
            //             ret i32 (trunc ret_val / 0 for void)
            let spawn_hook = self
                .runtime_hook_signature(RuntimeHook::ExecutorSpawn)
                .expect("async fn main lowering should require the executor-spawn runtime hook");
            let await_hook = self
                .runtime_hook_signature(RuntimeHook::TaskAwait)
                .expect("async fn main lowering should require the task-await runtime hook");
            let release_hook = self
                .runtime_hook_signature(RuntimeHook::TaskResultRelease)
                .expect(
                    "async fn main lowering should require the task-result-release runtime hook",
                );

            // Call the task-create wrapper (returns opaque task ptr).
            let _ = writeln!(
                output,
                "  %async_main_task = call ptr @{}()",
                entry.signature.llvm_name
            );
            // Spawn into the default executor (null = implicit executor placeholder).
            let _ = writeln!(
                output,
                "  %async_main_join = call {} @{}(ptr null, ptr %async_main_task)",
                spawn_hook.return_type.llvm_ir(),
                spawn_hook.hook.symbol_name(),
            );
            // Await the join handle; returns result_ptr.
            let _ = writeln!(
                output,
                "  %async_main_res = call {} @{}(ptr %async_main_join)",
                await_hook.return_type.llvm_ir(),
                await_hook.hook.symbol_name(),
            );

            if is_void_ty(&entry.signature.body_return_ty) {
                // Void async main: release the (empty) payload and return 0.
                let _ = writeln!(
                    output,
                    "  call {} @{}(ptr %async_main_res)",
                    release_hook.return_type.llvm_ir(),
                    release_hook.hook.symbol_name(),
                );
                output.push_str("  ret i32 0\n");
            } else {
                // Load the result payload, release it, then return exit code.
                let _ = writeln!(
                    output,
                    "  %async_main_ret = load {}, ptr %async_main_res",
                    entry.signature.body_return_llvm_ty,
                );
                let _ = writeln!(
                    output,
                    "  call {} @{}(ptr %async_main_res)",
                    release_hook.return_type.llvm_ir(),
                    release_hook.hook.symbol_name(),
                );
                output.push_str("  %async_main_exit = trunc i64 %async_main_ret to i32\n");
                output.push_str("  ret i32 %async_main_exit\n");
            }
        } else if is_void_ty(&entry.signature.return_ty) {
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
            if self.should_inline_runtime_hook(signature.hook) {
                continue;
            }
            let _ = writeln!(output, "{}", signature.render_llvm_declaration());
        }
    }

    fn render_runtime_heap_declarations(&self, output: &mut String) {
        if self.input.runtime_hooks.is_empty() {
            return;
        }
        output.push_str("declare ptr @malloc(i64)\n");
        output.push_str("declare void @free(ptr)\n");
    }

    fn render_program_runtime_support(&self, output: &mut String) {
        if !self.should_inline_runtime_support() {
            return;
        }
        let needs_task_runtime = self.has_runtime_hook(RuntimeHook::AsyncTaskCreate)
            || self.has_runtime_hook(RuntimeHook::ExecutorSpawn)
            || self.has_runtime_hook(RuntimeHook::TaskAwait)
            || self.has_runtime_hook(RuntimeHook::TaskResultRelease);
        let needs_async_iter_stub = self.has_runtime_hook(RuntimeHook::AsyncIterNext);

        if !needs_task_runtime && !needs_async_iter_stub {
            return;
        }

        output.push('\n');

        if needs_task_runtime {
            output.push_str("define ptr @qlrt_async_frame_alloc(i64 %size, i64 %align) {\n");
            output.push_str("entry:\n");
            output.push_str("  %frame_is_zero = icmp eq i64 %size, 0\n");
            output.push_str("  %frame_alloc_size = select i1 %frame_is_zero, i64 1, i64 %size\n");
            output.push_str("  %frame = call ptr @malloc(i64 %frame_alloc_size)\n");
            output.push_str("  ret ptr %frame\n");
            output.push_str("}\n\n");

            output.push_str("define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame) {\n");
            output.push_str("entry:\n");
            output.push_str("  %task_end = getelementptr { ptr, ptr, ptr }, ptr null, i32 1\n");
            output.push_str("  %task_size = ptrtoint ptr %task_end to i64\n");
            output.push_str("  %task = call ptr @malloc(i64 %task_size)\n");
            output.push_str(
                "  %task_entry_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 0\n",
            );
            output.push_str("  store ptr %entry_fn, ptr %task_entry_ptr\n");
            output.push_str(
                "  %task_frame_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 1\n",
            );
            output.push_str("  store ptr %frame, ptr %task_frame_ptr\n");
            output.push_str(
                "  %task_result_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 2\n",
            );
            output.push_str("  store ptr null, ptr %task_result_ptr\n");
            output.push_str("  ret ptr %task\n");
            output.push_str("}\n\n");

            output.push_str("define ptr @qlrt_executor_spawn(ptr %executor, ptr %task) {\n");
            output.push_str("entry:\n");
            output.push_str(
                "  %task_entry_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 0\n",
            );
            output.push_str("  %task_entry = load ptr, ptr %task_entry_ptr\n");
            output.push_str("  %task_started = icmp eq ptr %task_entry, null\n");
            output.push_str("  br i1 %task_started, label %done, label %start\n\n");
            output.push_str("start:\n");
            output.push_str(
                "  %task_frame_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 1\n",
            );
            output.push_str("  %task_frame = load ptr, ptr %task_frame_ptr\n");
            output.push_str("  %task_result = call ptr %task_entry(ptr %task_frame)\n");
            output.push_str("  call void @free(ptr %task_frame)\n");
            output.push_str("  store ptr null, ptr %task_entry_ptr\n");
            output.push_str("  store ptr null, ptr %task_frame_ptr\n");
            output.push_str(
                "  %task_result_ptr = getelementptr inbounds { ptr, ptr, ptr }, ptr %task, i32 0, i32 2\n",
            );
            output.push_str("  store ptr %task_result, ptr %task_result_ptr\n");
            output.push_str("  br label %done\n\n");
            output.push_str("done:\n");
            output.push_str("  ret ptr %task\n");
            output.push_str("}\n\n");

            output.push_str("define ptr @qlrt_task_await(ptr %handle) {\n");
            output.push_str("entry:\n");
            output.push_str("  %started = call ptr @qlrt_executor_spawn(ptr null, ptr %handle)\n");
            output.push_str(
                "  %result_ptr_slot = getelementptr inbounds { ptr, ptr, ptr }, ptr %started, i32 0, i32 2\n",
            );
            output.push_str("  %result_ptr = load ptr, ptr %result_ptr_slot\n");
            output.push_str("  call void @free(ptr %started)\n");
            output.push_str("  ret ptr %result_ptr\n");
            output.push_str("}\n\n");

            output.push_str("define void @qlrt_task_result_release(ptr %result) {\n");
            output.push_str("entry:\n");
            output.push_str("  call void @free(ptr %result)\n");
            output.push_str("  ret void\n");
            output.push_str("}\n");
        }

        if needs_async_iter_stub {
            if needs_task_runtime {
                output.push('\n');
            }
            output.push_str("define ptr @qlrt_async_iter_next(ptr %iterator) {\n");
            output.push_str("entry:\n");
            output.push_str("  ret ptr null\n");
            output.push_str("}\n");
        }
    }

    fn has_runtime_hook(&self, hook: RuntimeHook) -> bool {
        self.input
            .runtime_hooks
            .iter()
            .any(|signature| signature.hook == hook)
    }

    fn should_inline_runtime_support(&self) -> bool {
        self.input.mode == CodegenMode::Program || self.input.inline_runtime_support
    }

    fn should_inline_runtime_hook(&self, hook: RuntimeHook) -> bool {
        self.should_inline_runtime_support()
            && matches!(
                hook,
                RuntimeHook::AsyncFrameAlloc
                    | RuntimeHook::AsyncTaskCreate
                    | RuntimeHook::ExecutorSpawn
                    | RuntimeHook::TaskAwait
                    | RuntimeHook::TaskResultRelease
                    | RuntimeHook::AsyncIterNext
            )
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
        let entry_name = format!("{}__async_entry", function.signature.llvm_name);
        self.render_function_body(output, function, body_name);
        output.push('\n');
        self.render_async_task_entry_thunk(output, function, body_name, &entry_name);
        output.push('\n');
        self.render_async_task_wrapper(output, function, &entry_name);
    }

    fn render_async_task_entry_thunk(
        &self,
        output: &mut String,
        function: &PreparedFunction,
        body_name: &str,
        entry_name: &str,
    ) {
        let result_layout = function
            .signature
            .async_result_layout
            .as_ref()
            .expect("async functions should precompute a task result layout");
        let _ = writeln!(output, "define ptr @{entry_name}(ptr %frame) {{");
        output.push_str("entry:\n");
        match result_layout {
            AsyncTaskResultLayout::Void => {
                let _ = writeln!(
                    output,
                    "  call {} @{}(ptr %frame)",
                    function.signature.body_return_llvm_ty, body_name
                );
                output.push_str("  %async_result_ptr = call ptr @malloc(i64 1)\n");
            }
            AsyncTaskResultLayout::Loadable { llvm_ty, align, .. } => {
                let _ = writeln!(
                    output,
                    "  %async_result = call {} @{}(ptr %frame)",
                    llvm_ty, body_name
                );
                let _ = writeln!(
                    output,
                    "  %async_result_ptr = call ptr @malloc(i64 {})",
                    result_layout.storage_size()
                );
                let _ = writeln!(
                    output,
                    "  store {} %async_result, ptr %async_result_ptr, align {}",
                    llvm_ty, align
                );
            }
        }
        output.push_str("  ret ptr %async_result_ptr\n");
        output.push_str("}\n");
    }

    fn render_async_task_wrapper(
        &self,
        output: &mut String,
        function: &PreparedFunction,
        entry_name: &str,
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
            entry_name
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
                Some(AsyncTaskResultLayout::Loadable {
                    llvm_ty,
                    _size: _,
                    align,
                }) => {
                    // Zero-sized fixed arrays and recursive aggregates containing only
                    // zero-sized fields are still valid loadable async results.
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
            let llvm_ty = self
                .lower_llvm_type(ty, body.local(local_id).span, "local type")
                .expect("prepared local types should already be supported");
            let _ = writeln!(
                output,
                "  {} = alloca {}",
                llvm_slot_name(body, local_id),
                llvm_ty
            );
        }
        for block_id in body.block_ids() {
            if function.supported_for_loops.contains_key(&block_id) {
                let _ = writeln!(
                    output,
                    "  {} = alloca i64",
                    for_await_index_slot_name(block_id)
                );
            }
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
        for block_id in body.block_ids() {
            if function.supported_for_loops.contains_key(&block_id) {
                let _ = writeln!(
                    output,
                    "  store i64 -1, ptr {}",
                    for_await_index_slot_name(block_id)
                );
            }
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
            renderer.render_terminator(output, block_id, &block.terminator);
        }
        for block_id in body.block_ids() {
            let Some(loop_lowering) = function.supported_for_loops.get(&block_id) else {
                continue;
            };
            renderer.render_for_await_setup_block(output, block_id, loop_lowering);
        }

        output.push_str("}\n");
    }

    fn build_async_task_result_layout(
        &self,
        ty: &Ty,
        span: Span,
    ) -> Result<AsyncTaskResultLayout, Diagnostic> {
        if is_void_ty(ty) {
            return Ok(AsyncTaskResultLayout::Void);
        }

        let llvm_ty = self.lower_llvm_type(ty, span, "async task result type")?;
        let layout = self.loadable_abi_layout(ty, span, "async task result type")?;

        Ok(AsyncTaskResultLayout::Loadable {
            llvm_ty,
            _size: layout.size,
            align: layout.align,
        })
    }

    fn build_async_frame_layout(
        &self,
        params: &[ParamSignature],
        span: Span,
    ) -> Result<AsyncFrameLayout, Diagnostic> {
        let mut fields = Vec::with_capacity(params.len());
        let mut field_types = Vec::with_capacity(params.len());
        let mut size = 0;
        let mut align = 1;

        for (index, param) in params.iter().enumerate() {
            let layout = self.loadable_abi_layout(&param.ty, span, "async fn frame field type")?;
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

    fn lower_llvm_type(&self, ty: &Ty, span: Span, context: &str) -> Result<String, Diagnostic> {
        match ty {
            Ty::Array { element, len } => {
                let element_llvm_ty = self.lower_llvm_type(element, span, context)?;
                Ok(format!("[{len} x {element_llvm_ty}]"))
            }
            Ty::Tuple(items) => {
                if items.iter().any(is_void_ty) {
                    return Err(Diagnostic::error(format!(
                        "LLVM IR backend foundation does not support {context} `{ty}` yet"
                    ))
                    .with_label(Label::new(span)));
                }

                let field_types = items
                    .iter()
                    .map(|item| self.lower_llvm_type(item, span, context))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(format!("{{ {} }}", field_types.join(", ")))
            }
            Ty::Item { .. } => {
                let fields = self.struct_field_lowerings(ty, span, context)?;
                Ok(format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|field| field.llvm_ty.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
            _ => lower_llvm_type(ty, span, context),
        }
    }

    fn loadable_abi_layout(
        &self,
        ty: &Ty,
        span: Span,
        context: &str,
    ) -> Result<LoadableAbiLayout, Diagnostic> {
        match ty {
            Ty::Array { element, len } => {
                let element = self.loadable_abi_layout(element, span, context)?;
                Ok(LoadableAbiLayout {
                    size: element.size * (*len as u64),
                    align: element.align,
                })
            }
            Ty::Tuple(items) => {
                if items.iter().any(is_void_ty) {
                    return Err(Diagnostic::error(format!(
                        "LLVM IR backend foundation does not support {context} `{ty}` yet"
                    ))
                    .with_label(Label::new(span)));
                }

                let mut size = 0;
                let mut align = 1;
                for item in items {
                    let layout = self.loadable_abi_layout(item, span, context)?;
                    size = align_to(size, layout.align);
                    size += layout.size;
                    align = align.max(layout.align);
                }
                Ok(LoadableAbiLayout {
                    size: align_to(size, align),
                    align,
                })
            }
            Ty::Item { .. } => {
                // Keep struct layout recursive so async payloads and projected reads share one
                // aggregate contract instead of growing per-shape special cases.
                let mut size = 0;
                let mut align = 1;
                for field in self.struct_field_lowerings(ty, span, context)? {
                    let layout = self.loadable_abi_layout(&field.ty, span, context)?;
                    size = align_to(size, layout.align);
                    size += layout.size;
                    align = align.max(layout.align);
                }
                Ok(LoadableAbiLayout {
                    size: align_to(size, align),
                    align,
                })
            }
            _ => {
                let layout = scalar_abi_layout(ty, span, context)?;
                Ok(LoadableAbiLayout {
                    size: layout.size,
                    align: layout.align,
                })
            }
        }
    }

    fn struct_field_lowerings(
        &self,
        ty: &Ty,
        span: Span,
        context: &str,
    ) -> Result<Vec<StructFieldLowering>, Diagnostic> {
        let Ty::Item { item_id, args, .. } = ty else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        if !args.is_empty() {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        }

        let item = self.input.hir.item(*item_id);
        let ItemKind::Struct(struct_decl) = &item.kind else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        if !struct_decl.generics.is_empty() {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        }

        struct_decl
            .fields
            .iter()
            .map(|field| {
                let ty = lower_type(self.input.hir, self.input.resolution, field.ty);
                if is_void_ty(&ty) {
                    return Err(Diagnostic::error(format!(
                        "LLVM IR backend foundation does not support {context} `{ty}` yet"
                    ))
                    .with_label(Label::new(span)));
                }
                let llvm_ty = self.lower_llvm_type(&ty, span, context)?;
                Ok(StructFieldLowering {
                    name: field.name.clone(),
                    ty,
                    llvm_ty,
                })
            })
            .collect()
    }

    fn resolve_local_struct_path(&self, path: &ql_ast::Path) -> Option<Ty> {
        let [name] = path.segments.as_slice() else {
            return None;
        };
        let mut candidates = HashSet::new();

        for item_id in self.input.hir.items.iter().copied() {
            if matches!(
                &self.input.hir.item(item_id).kind,
                ItemKind::Struct(struct_decl) if struct_decl.name == *name
            ) {
                candidates.insert(item_id);
            }
        }

        for scope in self.input.resolution.scopes.scopes() {
            for binding in &scope.type_bindings {
                if binding.name != *name {
                    continue;
                }
                match &binding.resolution {
                    TypeResolution::Item(item_id) => {
                        candidates.insert(*item_id);
                    }
                    TypeResolution::Import(import_binding) => {
                        let Some(target_name) = import_binding.path.segments.last() else {
                            continue;
                        };
                        for item_id in self.input.hir.items.iter().copied() {
                            if matches!(
                                &self.input.hir.item(item_id).kind,
                                ItemKind::Struct(struct_decl) if struct_decl.name == *target_name
                            ) {
                                candidates.insert(item_id);
                            }
                        }
                    }
                    TypeResolution::Builtin(_) | TypeResolution::Generic(_) => {}
                }
            }
        }

        let mut candidates = candidates.into_iter();
        let item_id = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }

        match &self.input.hir.item(item_id).kind {
            ItemKind::Struct(struct_decl) => Some(Ty::Item {
                item_id,
                name: struct_decl.name.clone(),
                args: Vec::new(),
            }),
            _ => None,
        }
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
                let expected_ty = self.prepared_place_type(place, statement.span);
                let target_is_void = expected_ty.as_ref().is_some_and(is_void_ty);
                if let Some(rendered) =
                    self.render_rvalue(output, value, expected_ty.as_ref(), statement.span)
                    && !target_is_void
                    && !is_void_ty(&rendered.ty)
                {
                    let target_ptr = if place.projections.is_empty() {
                        llvm_slot_name(self.body, place.base)
                    } else {
                        self.render_place_pointer(output, place, statement.span).0
                    };
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {}",
                        rendered.llvm_ty, rendered.repr, target_ptr
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
                let _ = self.render_rvalue(output, value, None, statement.span);
            }
            StatementKind::StorageLive { .. } | StatementKind::StorageDead { .. } => {}
            StatementKind::RegisterCleanup { .. } | StatementKind::RunCleanup { .. } => {
                panic!("prepared functions should not contain cleanup statements")
            }
        }
    }

    fn render_terminator(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        terminator: &mir::Terminator,
    ) {
        match &terminator.kind {
            TerminatorKind::Goto { target } => {
                if self.prepared.supported_for_loops.contains_key(target) {
                    let current = self.fresh_temp();
                    let next = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {current} = load i64, ptr {}",
                        for_await_index_slot_name(*target)
                    );
                    let _ = writeln!(output, "  {next} = add i64 {current}, 1");
                    let _ = writeln!(
                        output,
                        "  store i64 {next}, ptr {}",
                        for_await_index_slot_name(*target)
                    );
                }
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
            TerminatorKind::Match { .. } => {
                panic!("prepared functions should not contain unsupported terminators")
            }
            TerminatorKind::ForLoop { exit_target, .. } => {
                let loop_lowering = self
                    .prepared
                    .supported_for_loops
                    .get(&block_id)
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared `for await` at block {:?} should have lowering metadata",
                            block_id
                        )
                    });
                let index = self.fresh_temp();
                let continue_flag = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {index} = load i64, ptr {}",
                    for_await_index_slot_name(block_id)
                );
                let _ = writeln!(
                    output,
                    "  {continue_flag} = icmp ult i64 {index}, {}",
                    loop_lowering.array_len
                );
                let _ = writeln!(
                    output,
                    "  br i1 {continue_flag}, label %{}, label %bb{}",
                    for_await_setup_block_name(block_id),
                    exit_target.index()
                );
            }
        }
    }

    fn render_rvalue(
        &mut self,
        output: &mut String,
        value: &Rvalue,
        expected_ty: Option<&Ty>,
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
                    let ty = if signature.is_async {
                        Ty::TaskHandle(Box::new(signature.body_return_ty.clone()))
                    } else {
                        signature.return_ty.clone()
                    };
                    Some(LoweredValue {
                        ty,
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
                UnaryOp::Spawn => self.render_spawn(output, operand, span),
            },
            Rvalue::Tuple(items) => self.render_tuple_rvalue(output, items, expected_ty, span),
            Rvalue::Array(items) => self.render_array_rvalue(output, items, expected_ty, span),
            Rvalue::AggregateStruct { path, fields } => {
                self.render_struct_rvalue(output, path, fields, expected_ty, span)
            }
            Rvalue::Closure { .. } | Rvalue::Question(_) | Rvalue::OpaqueExpr(_) => {
                panic!("prepared functions should not contain unsupported rvalues")
            }
        }
    }

    fn render_tuple_rvalue(
        &mut self,
        output: &mut String,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let rendered_items = items
            .iter()
            .map(|item| self.render_operand(output, item, span))
            .collect::<Vec<_>>();
        let ty = match expected_ty {
            Some(Ty::Tuple(expected_items)) if expected_items.len() == rendered_items.len() => {
                Ty::Tuple(expected_items.clone())
            }
            _ => Ty::Tuple(rendered_items.iter().map(|item| item.ty.clone()).collect()),
        };
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&ty, span, "tuple value")
            .expect("prepared tuple values should already have supported LLVM types");

        if rendered_items.is_empty() {
            return Some(LoweredValue {
                ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            });
        }

        let mut aggregate = "undef".to_owned();
        for (index, item) in rendered_items.iter().enumerate() {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                item.llvm_ty, item.repr, index
            );
            aggregate = next;
        }

        Some(LoweredValue {
            ty,
            llvm_ty,
            repr: aggregate,
        })
    }

    fn render_array_rvalue(
        &mut self,
        output: &mut String,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let rendered_items = items
            .iter()
            .map(|item| self.render_operand(output, item, span))
            .collect::<Vec<_>>();
        let ty = if let Some(Ty::Array { .. }) = expected_ty {
            expected_ty.cloned().expect("checked array expected type")
        } else {
            let mut element_ty = Ty::Unknown;
            for item in &rendered_items {
                if element_ty.is_unknown() && !item.ty.is_unknown() {
                    element_ty = item.ty.clone();
                    continue;
                }
                if item.ty.is_unknown() {
                    continue;
                }
                assert!(
                    element_ty.compatible_with(&item.ty),
                    "prepared array literal at {span:?} should contain one compatible element type"
                );
            }
            assert!(
                !element_ty.is_unknown(),
                "prepared array literal at {span:?} should have a resolved element type"
            );
            Ty::Array {
                element: Box::new(element_ty),
                len: rendered_items.len(),
            }
        };
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&ty, span, "array value")
            .expect("prepared array values should already have supported LLVM types");

        if rendered_items.is_empty() {
            return Some(LoweredValue {
                ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            });
        }

        let mut aggregate = "undef".to_owned();
        for (index, item) in rendered_items.iter().enumerate() {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                item.llvm_ty, item.repr, index
            );
            aggregate = next;
        }

        Some(LoweredValue {
            ty,
            llvm_ty,
            repr: aggregate,
        })
    }

    fn render_struct_rvalue(
        &mut self,
        output: &mut String,
        _path: &ql_ast::Path,
        fields: &[mir::AggregateField],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let struct_ty = expected_ty.cloned().or_else(|| {
            panic!("prepared struct aggregate at {span:?} should have an expected lowered type")
        })?;
        let field_layouts = self
            .emitter
            .struct_field_lowerings(&struct_ty, span, "struct value")
            .unwrap_or_else(|_| panic!("prepared struct aggregate at {span:?} should have a loadable declaration layout"));
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&struct_ty, span, "struct value")
            .unwrap_or_else(|_| {
                panic!("prepared struct aggregate at {span:?} should have a lowered LLVM type")
            });

        let mut rendered_fields = HashMap::with_capacity(fields.len());
        for field in fields {
            let rendered = self.render_operand(output, &field.value, span);
            rendered_fields.insert(field.name.clone(), rendered);
        }

        if field_layouts.is_empty() {
            return Some(LoweredValue {
                ty: struct_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            });
        }

        let mut aggregate = "undef".to_owned();
        for (index, field) in field_layouts.iter().enumerate() {
            let rendered = rendered_fields.remove(&field.name).unwrap_or_else(|| {
                panic!("prepared struct aggregate at {span:?} should provide every declared field")
            });
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                rendered.llvm_ty, rendered.repr, index
            );
            aggregate = next;
        }

        Some(LoweredValue {
            ty: struct_ty,
            llvm_ty,
            repr: aggregate,
        })
    }

    fn task_handle_info_for_local(
        &self,
        local_id: mir::LocalId,
        span: Span,
    ) -> AsyncTaskHandleInfo {
        if let Some(handle) = self.prepared.async_task_handles.get(&local_id) {
            return handle.clone();
        }

        match self.prepared.local_types.get(&local_id) {
            Some(Ty::TaskHandle(result_ty)) => AsyncTaskHandleInfo {
                result_ty: (**result_ty).clone(),
                result_layout: self.emitter.build_async_task_result_layout(result_ty, span).unwrap_or_else(
                    |_| panic!("prepared task-handle local at {span:?} should have a loadable async result layout"),
                ),
            },
            _ => panic!("prepared local at {span:?} should be an async task handle"),
        }
    }

    fn task_handle_info_for_place(&self, place: &Place, span: Span) -> AsyncTaskHandleInfo {
        if place.projections.is_empty() {
            return self.task_handle_info_for_local(place.base, span);
        }

        let mut diagnostics = Vec::new();
        let place_ty = self
            .emitter
            .infer_place_type(
                self.body,
                place,
                &self.prepared.local_types,
                &self.prepared.async_task_handles,
                &mut diagnostics,
                span,
            )
            .unwrap_or_else(|| {
                panic!("prepared task-handle place at {span:?} should have a resolved type")
            });

        match place_ty {
            Ty::TaskHandle(result_ty) => AsyncTaskHandleInfo {
                result_ty: (*result_ty).clone(),
                result_layout: self
                    .emitter
                    .build_async_task_result_layout(&result_ty, span)
                    .unwrap_or_else(|_| {
                        panic!(
                            "prepared projected task-handle place at {span:?} should have a loadable async result layout"
                        )
                    }),
            },
            other => panic!(
                "prepared projected task-handle place at {span:?} should resolve to a task handle, found `{other}`"
            ),
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
        let handle_info = self.task_handle_info_for_place(place, span);
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
            AsyncTaskResultLayout::Loadable { llvm_ty, .. } => {
                // INVARIANT (RuntimeHook::TaskAwait contract): result_ptr points to a
                // contiguous, naturally aligned payload of the async return type.  The
                // backend may immediately load the value before calling
                // qlrt_task_result_release.  Any runtime implementation of qlrt_task_await
                // must uphold this layout guarantee.  See RuntimeHook::TaskAwait in
                // ql-runtime/src/lib.rs for the authoritative contract documentation.
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

    fn render_spawn(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> Option<LoweredValue> {
        let spawn_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::ExecutorSpawn)
            .expect("prepared spawn lowering should require the executor-spawn runtime hook");
        let Operand::Place(place) = operand else {
            panic!("prepared spawn operands should lower through task-handle places");
        };
        let handle_info = self.task_handle_info_for_place(place, span);
        let task = self.render_operand(output, operand, span);
        let submitted = self.fresh_temp();
        // `null` is the current placeholder for the ambient/default executor contract.
        let _ = writeln!(
            output,
            "  {submitted} = call {} @{}(ptr null, {} {})",
            spawn_hook.return_type.llvm_ir(),
            spawn_hook.hook.symbol_name(),
            task.llvm_ty,
            task.repr
        );
        Some(LoweredValue {
            ty: Ty::TaskHandle(Box::new(handle_info.result_ty)),
            llvm_ty: "ptr".to_owned(),
            repr: submitted,
        })
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

    fn render_item_constant(
        &mut self,
        output: &mut String,
        item_id: ItemId,
        span: Span,
    ) -> LoweredValue {
        let ItemKind::Const(global) = &self.emitter.input.hir.item(item_id).kind else {
            panic!("prepared const item lowering at {span:?} should only materialize const items");
        };
        let ty = lower_type(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            global.ty,
        );
        self.render_const_expr(output, global.value, Some(&ty), span)
    }

    fn render_const_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> LoweredValue {
        let expr = self.emitter.input.hir.expr(expr_id);
        match &expr.kind {
            hir::ExprKind::Integer(value) => LoweredValue {
                ty: Ty::Builtin(BuiltinType::Int),
                llvm_ty: "i64".to_owned(),
                repr: value.clone(),
            },
            hir::ExprKind::Bool(value) => LoweredValue {
                ty: Ty::Builtin(BuiltinType::Bool),
                llvm_ty: "i1".to_owned(),
                repr: if *value { "true" } else { "false" }.to_owned(),
            },
            hir::ExprKind::Name(_) => {
                match self.emitter.input.resolution.expr_resolution(expr_id) {
                    Some(ValueResolution::Item(item_id)) => {
                        self.render_item_constant(output, *item_id, span)
                    }
                    _ => panic!(
                        "prepared const item lowering at {span:?} should only reference resolved const items"
                    ),
                }
            }
            hir::ExprKind::Block(block) | hir::ExprKind::Unsafe(block) => {
                let tail = self
                    .emitter
                    .input
                    .hir
                    .block(*block)
                    .tail
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared const item lowering at {span:?} should only use block constants with tails"
                        )
                    });
                self.render_const_expr(output, tail, expected_ty, span)
            }
            hir::ExprKind::Question(inner) => {
                self.render_const_expr(output, *inner, expected_ty, span)
            }
            hir::ExprKind::Tuple(items) => {
                self.render_const_tuple_expr(output, items, expected_ty, expr.span)
            }
            hir::ExprKind::Array(items) => {
                self.render_const_array_expr(output, items, expected_ty, expr.span)
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                self.render_const_struct_expr(output, fields, expected_ty, expr.span)
            }
            _ => panic!(
                "prepared const item lowering at {span:?} should not contain unsupported const expressions"
            ),
        }
    }

    fn render_const_tuple_expr(
        &mut self,
        output: &mut String,
        items: &[hir::ExprId],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> LoweredValue {
        let tuple_ty = match expected_ty {
            Some(Ty::Tuple(items)) => Ty::Tuple(items.clone()),
            Some(other) => panic!(
                "prepared const tuple lowering at {span:?} should have tuple expected type, found `{other}`"
            ),
            None => {
                panic!("prepared const tuple lowering at {span:?} should have an expected type")
            }
        };
        let Ty::Tuple(expected_items) = &tuple_ty else {
            unreachable!();
        };
        assert_eq!(
            expected_items.len(),
            items.len(),
            "prepared const tuple lowering at {span:?} should preserve tuple arity"
        );
        let rendered_items = items
            .iter()
            .zip(expected_items.iter())
            .map(|(item, item_ty)| self.render_const_expr(output, *item, Some(item_ty), span))
            .collect::<Vec<_>>();
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&tuple_ty, span, "tuple value")
            .expect("prepared const tuple values should already have supported LLVM types");

        if rendered_items.is_empty() {
            return LoweredValue {
                ty: tuple_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            };
        }

        let mut aggregate = "undef".to_owned();
        for (index, item) in rendered_items.iter().enumerate() {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                item.llvm_ty, item.repr, index
            );
            aggregate = next;
        }

        LoweredValue {
            ty: tuple_ty,
            llvm_ty,
            repr: aggregate,
        }
    }

    fn render_const_array_expr(
        &mut self,
        output: &mut String,
        items: &[hir::ExprId],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> LoweredValue {
        let array_ty = match expected_ty {
            Some(Ty::Array { element, len }) => {
                assert_eq!(
                    *len,
                    items.len(),
                    "prepared const array lowering at {span:?} should preserve array length"
                );
                Ty::Array {
                    element: element.clone(),
                    len: *len,
                }
            }
            Some(other) => panic!(
                "prepared const array lowering at {span:?} should have array expected type, found `{other}`"
            ),
            None => {
                panic!("prepared const array lowering at {span:?} should have an expected type")
            }
        };
        let Ty::Array { element, .. } = &array_ty else {
            unreachable!();
        };
        let rendered_items = items
            .iter()
            .map(|item| self.render_const_expr(output, *item, Some(element.as_ref()), span))
            .collect::<Vec<_>>();
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&array_ty, span, "array value")
            .expect("prepared const array values should already have supported LLVM types");

        if rendered_items.is_empty() {
            return LoweredValue {
                ty: array_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            };
        }

        let mut aggregate = "undef".to_owned();
        for (index, item) in rendered_items.iter().enumerate() {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                item.llvm_ty, item.repr, index
            );
            aggregate = next;
        }

        LoweredValue {
            ty: array_ty,
            llvm_ty,
            repr: aggregate,
        }
    }

    fn render_const_struct_expr(
        &mut self,
        output: &mut String,
        fields: &[hir::StructLiteralField],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> LoweredValue {
        let struct_ty = expected_ty.cloned().unwrap_or_else(|| {
            panic!("prepared const struct lowering at {span:?} should have an expected type")
        });
        let field_layouts = self
            .emitter
            .struct_field_lowerings(&struct_ty, span, "struct value")
            .unwrap_or_else(|_| {
                panic!("prepared const struct lowering at {span:?} should have a loadable declaration layout")
            });
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&struct_ty, span, "struct value")
            .unwrap_or_else(|_| {
                panic!("prepared const struct lowering at {span:?} should have a lowered LLVM type")
            });

        let mut rendered_fields = HashMap::with_capacity(fields.len());
        for field in fields {
            let field_ty = field_layouts
                .iter()
                .find(|layout| layout.name == field.name)
                .map(|layout| &layout.ty)
                .unwrap_or_else(|| {
                    panic!(
                        "prepared const struct lowering at {span:?} should provide declared field `{}`",
                        field.name
                    )
                });
            let rendered = self.render_const_expr(output, field.value, Some(field_ty), span);
            rendered_fields.insert(field.name.clone(), rendered);
        }

        if field_layouts.is_empty() {
            return LoweredValue {
                ty: struct_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            };
        }

        let mut aggregate = "undef".to_owned();
        for (index, field) in field_layouts.iter().enumerate() {
            let rendered = rendered_fields.remove(&field.name).unwrap_or_else(|| {
                panic!(
                    "prepared const struct lowering at {span:?} should provide every declared field"
                )
            });
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                rendered.llvm_ty, rendered.repr, index
            );
            aggregate = next;
        }

        LoweredValue {
            ty: struct_ty,
            llvm_ty,
            repr: aggregate,
        }
    }

    fn render_operand(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> LoweredValue {
        match operand {
            Operand::Place(place) => self.render_place_operand(output, place, span),
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
                Constant::Item { item, .. } => self.render_item_constant(output, *item, span),
                Constant::String { .. }
                | Constant::None
                | Constant::Import(_)
                | Constant::UnresolvedName(_) => {
                    panic!("prepared operands should not contain unsupported constants")
                }
            },
        }
    }

    fn render_place_operand(
        &mut self,
        output: &mut String,
        place: &Place,
        span: Span,
    ) -> LoweredValue {
        if place.projections.is_empty()
            && self.prepared.async_task_handles.contains_key(&place.base)
        {
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

        let (ptr, ty) = self.render_place_pointer(output, place, span);
        if is_void_ty(&ty) {
            return LoweredValue {
                ty,
                llvm_ty: "void".to_owned(),
                repr: "void".to_owned(),
            };
        }

        let llvm_ty = self
            .emitter
            .lower_llvm_type(&ty, span, "operand type")
            .expect("prepared operand types should already be supported");
        let temp = self.fresh_temp();
        let _ = writeln!(output, "  {temp} = load {llvm_ty}, ptr {ptr}");
        LoweredValue {
            ty,
            llvm_ty,
            repr: temp,
        }
    }

    fn prepared_place_type(&self, place: &Place, span: Span) -> Option<Ty> {
        let mut diagnostics = Vec::new();
        let ty = self.emitter.infer_place_type(
            self.body,
            place,
            &self.prepared.local_types,
            &self.prepared.async_task_handles,
            &mut diagnostics,
            span,
        );
        assert!(
            diagnostics.is_empty(),
            "prepared place at {span:?} should have a resolved type without diagnostics: {diagnostics:?}"
        );
        ty
    }

    fn render_place_pointer(
        &mut self,
        output: &mut String,
        place: &Place,
        span: Span,
    ) -> (String, Ty) {
        let place = self.canonical_task_handle_alias_place(place);
        let mut current_ptr = llvm_slot_name(self.body, place.base);
        let mut current_ty = self
            .prepared
            .local_types
            .get(&place.base)
            .cloned()
            .unwrap_or_else(|| panic!("prepared place at {span:?} should have a type"));

        for projection in &place.projections {
            // Project through the slot pointer directly so nested struct/tuple/array reads all
            // reuse one lowering path and we do not have to materialize intermediate aggregates.
            let aggregate_llvm_ty = self
                .emitter
                .lower_llvm_type(&current_ty, span, "projection base type")
                .unwrap_or_else(|_| {
                    panic!("prepared projection base at {span:?} should have a lowered LLVM type")
                });
            let step = self
                .emitter
                .resolve_projection_step(&current_ty, projection, span)
                .unwrap_or_else(|_| {
                    panic!("prepared projection at {span:?} should have a supported place type")
                });

            match (projection, &step) {
                (mir::ProjectionElem::Field(_), ResolvedProjectionStep::Field { index, .. })
                | (
                    mir::ProjectionElem::TupleIndex(_),
                    ResolvedProjectionStep::TupleIndex { index, .. },
                )
                | (
                    mir::ProjectionElem::Index(_),
                    ResolvedProjectionStep::TupleIndex { index, .. },
                ) => {
                    let next = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i32 0, i32 {index}"
                    );
                    current_ptr = next;
                }
                (mir::ProjectionElem::Index(index), ResolvedProjectionStep::ArrayIndex { .. }) => {
                    let rendered_index = self.render_operand(output, index, span);
                    assert!(
                        rendered_index
                            .ty
                            .compatible_with(&Ty::Builtin(BuiltinType::Int)),
                        "prepared array index at {span:?} should have type Int"
                    );
                    let next = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i64 0, {} {}",
                        rendered_index.llvm_ty, rendered_index.repr
                    );
                    current_ptr = next;
                }
                _ => {
                    panic!("prepared projection at {span:?} should lower through a matching step")
                }
            }

            current_ty = step.output_ty();
        }

        (current_ptr, current_ty)
    }

    fn canonical_task_handle_alias_place(&self, place: &Place) -> Place {
        let mut canonical = place.clone();
        while let Some(source_place) = self.prepared.task_handle_place_aliases.get(&canonical.base)
        {
            let mut projections = source_place.projections.clone();
            projections.extend(canonical.projections);
            canonical = Place {
                base: source_place.base,
                projections,
            };
        }
        canonical
    }

    fn fresh_temp(&mut self) -> String {
        let index = self.next_temp;
        self.next_temp += 1;
        format!("%t{index}")
    }

    fn render_for_await_setup_block(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        loop_lowering: &SupportedForLoopLowering,
    ) {
        let span = self.body.block(block_id).terminator.span;
        let _ = writeln!(output, "{}:", for_await_setup_block_name(block_id));
        let index = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {index} = load i64, ptr {}",
            for_await_index_slot_name(block_id)
        );
        let (iterable_ptr, iterable_ty) =
            self.render_place_pointer(output, &loop_lowering.iterable_place, span);
        let array_llvm_ty = self
            .emitter
            .lower_llvm_type(&iterable_ty, span, "for-await iterable type")
            .expect("prepared for-await iterable should have a lowered LLVM type");
        let element_ptr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {element_ptr} = getelementptr inbounds {array_llvm_ty}, ptr {iterable_ptr}, i64 0, i64 {index}"
        );
        let element_llvm_ty = self
            .emitter
            .lower_llvm_type(&loop_lowering.element_ty, span, "for-await item type")
            .expect("prepared for-await item should have a lowered LLVM type");
        let element = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {element} = load {element_llvm_ty}, ptr {element_ptr}"
        );
        let _ = writeln!(
            output,
            "  store {element_llvm_ty} {element}, ptr {}",
            llvm_slot_name(self.body, loop_lowering.item_local)
        );
        let _ = writeln!(
            output,
            "  br label %bb{}",
            loop_lowering.body_target.index()
        );
    }
}

fn pattern_kind(module: &hir::Module, pattern: hir::PatternId) -> &PatternKind {
    &module.pattern(pattern).kind
}

fn lower_llvm_type(ty: &Ty, span: Span, context: &str) -> Result<String, Diagnostic> {
    match ty {
        Ty::Array { element, len } => {
            let element_llvm_ty = lower_llvm_type(element, span, context)?;
            Ok(format!("[{len} x {element_llvm_ty}]"))
        }
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
        Ty::TaskHandle(_) => Ok("ptr".to_owned()),
        Ty::Tuple(items) => {
            if items.iter().any(is_void_ty) {
                return Err(Diagnostic::error(format!(
                    "LLVM IR backend foundation does not support {context} `{ty}` yet"
                ))
                .with_label(Label::new(span)));
            }

            let field_types = items
                .iter()
                .map(|item| lower_llvm_type(item, span, context))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(format!("{{ {} }}", field_types.join(", ")))
        }
        _ => Err(Diagnostic::error(format!(
            "LLVM IR backend foundation does not support {context} `{ty}` yet"
        ))
        .with_label(Label::new(span))),
    }
}

#[cfg(test)]
fn build_async_task_result_layout(
    ty: &Ty,
    span: Span,
) -> Result<AsyncTaskResultLayout, Diagnostic> {
    if is_void_ty(ty) {
        return Ok(AsyncTaskResultLayout::Void);
    }

    let llvm_ty = lower_llvm_type(ty, span, "async task result type")?;
    let layout = loadable_abi_layout(ty, span, "async task result type")?;

    Ok(AsyncTaskResultLayout::Loadable {
        llvm_ty,
        _size: layout.size,
        align: layout.align,
    })
}

#[cfg(test)]
fn loadable_abi_layout(
    ty: &Ty,
    span: Span,
    context: &str,
) -> Result<LoadableAbiLayout, Diagnostic> {
    match ty {
        Ty::Array { element, len } => {
            let element = loadable_abi_layout(element, span, context)?;
            Ok(LoadableAbiLayout {
                size: element.size * (*len as u64),
                align: element.align,
            })
        }
        Ty::Tuple(items) => {
            if items.iter().any(is_void_ty) {
                return Err(Diagnostic::error(format!(
                    "LLVM IR backend foundation does not support {context} `{ty}` yet"
                ))
                .with_label(Label::new(span)));
            }

            let mut size = 0;
            let mut align = 1;
            for item in items {
                let layout = loadable_abi_layout(item, span, context)?;
                size = align_to(size, layout.align);
                size += layout.size;
                align = align.max(layout.align);
            }
            Ok(LoadableAbiLayout {
                size: align_to(size, align),
                align,
            })
        }
        _ => {
            let layout = scalar_abi_layout(ty, span, context)?;
            Ok(LoadableAbiLayout {
                size: layout.size,
                align: layout.align,
            })
        }
    }
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
        | Ty::Builtin(BuiltinType::F64)
        | Ty::TaskHandle(_) => Ok(ScalarAbiLayout { size: 8, align: 8 }),
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

fn seed_inferred_local_type(
    local_types: &mut HashMap<mir::LocalId, Ty>,
    local_id: mir::LocalId,
    ty: Ty,
) {
    if ty.is_unknown() {
        return;
    }
    match local_types.get(&local_id) {
        Some(existing) if !existing.is_unknown() => {}
        _ => {
            local_types.insert(local_id, ty);
        }
    }
}

fn for_await_index_slot_name(block_id: mir::BasicBlockId) -> String {
    format!("%for_await_index_bb{}", block_id.index())
}

fn for_await_setup_block_name(block_id: mir::BasicBlockId) -> String {
    format!("bb{}_for_await_setup", block_id.index())
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
            inline_runtime_support: false,
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
            inline_runtime_support: false,
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

    fn emit_with_runtime_hooks_and_inline_support(
        source: &str,
        mode: CodegenMode,
        runtime_hooks: &[RuntimeHookSignature],
        inline_runtime_support: bool,
    ) -> String {
        let analysis = analyze_source(source).expect("source should analyze");
        assert!(
            !analysis.has_errors(),
            "test source should not contain semantic diagnostics"
        );

        emit_module(CodegenInput {
            module_name: "test_module",
            mode,
            inline_runtime_support,
            hir: analysis.hir(),
            mir: analysis.mir(),
            resolution: analysis.resolution(),
            typeck: analysis.typeck(),
            runtime_hooks,
        })
        .expect("codegen should succeed")
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

        let async_frame_alloc = "define ptr @qlrt_async_frame_alloc(i64 %size, i64 %align)";
        let async_task_create = "define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)";
        let executor_spawn = "define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)";
        let task_await = "define ptr @qlrt_task_await(ptr %handle)";
        let task_result_release = "define void @qlrt_task_result_release(ptr %result)";
        let entry_definition = "define i64 @ql_0_main()";

        assert!(rendered.contains(async_frame_alloc));
        assert!(rendered.contains(async_task_create));
        assert!(rendered.contains(executor_spawn));
        assert!(rendered.contains(task_await));
        assert!(rendered.contains(task_result_release));
        assert!(
            rendered
                .find(async_frame_alloc)
                .expect("runtime definition should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(async_task_create)
                .expect("runtime definition should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(executor_spawn)
                .expect("runtime definition should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(task_await)
                .expect("runtime definition should exist")
                < rendered
                    .find(entry_definition)
                    .expect("entry function should exist")
        );
        assert!(
            rendered
                .find(task_result_release)
                .expect("runtime definition should exist")
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
        assert!(rendered.contains("define ptr @ql_0_worker__async_entry(ptr %frame)"));
        assert!(rendered.contains("call i64 @ql_0_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @malloc(i64 8)"));
        assert!(rendered.contains("define ptr @ql_0_worker()"));
        assert!(
            rendered.contains(
                "call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_entry, ptr null)"
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
        assert!(rendered.contains("define ptr @ql_0_worker__async_entry(ptr %frame)"));
        assert!(rendered.contains("call i64 @ql_0_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @malloc(i64 8)"));
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
            "call ptr @qlrt_async_task_create(ptr @ql_0_worker__async_entry, ptr %async_frame)"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field0 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 0"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field1 = getelementptr inbounds { i1, i64 }, ptr %frame, i32 0, i32 1"
        ));
    }

    #[test]
    fn emits_async_task_create_wrapper_with_recursive_aggregate_frame_fields() {
        let runtime_hooks =
            collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i64 @ql_1_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
        assert!(rendered.contains(
            "getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %async_frame, i32 0, i32 0"
        ));
        assert!(rendered.contains(
            "getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %async_frame, i32 0, i32 1"
        ));
        assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
        assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
        assert!(
            rendered.contains(
                "%async_body_frame_field0 = getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %frame, i32 0, i32 0"
            )
        );
        assert!(
            rendered.contains(
                "%async_body_frame_field1 = getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr %frame, i32 0, i32 1"
            )
        );
    }

    #[test]
    fn emits_async_task_create_wrapper_with_zero_sized_recursive_aggregate_frame_fields() {
        let runtime_hooks =
            collect_runtime_hook_signatures([RuntimeCapability::AsyncFunctionBodies]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 0
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i64 @ql_1_worker__async_body(ptr %frame)"));
        assert!(rendered.contains(
            "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
        ));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
        assert!(rendered.contains(
            "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 0"
        ));
        assert!(rendered.contains(
            "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 1"
        ));
        assert!(rendered.contains(
            "getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %async_frame, i32 0, i32 2"
        ));
        assert!(rendered.contains("store [0 x i64] %arg0, ptr %async_frame_field0"));
        assert!(rendered.contains("store { [0 x i64] } %arg1, ptr %async_frame_field1"));
        assert!(rendered.contains("store [1 x [0 x i64]] %arg2, ptr %async_frame_field2"));
        assert!(rendered.contains(
            "%async_body_frame_field0 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 0"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field1 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 1"
        ));
        assert!(rendered.contains(
            "%async_body_frame_field2 = getelementptr inbounds { [0 x i64], { [0 x i64] }, [1 x [0 x i64]] }, ptr %frame, i32 0, i32 2"
        ));
    }

    #[test]
    fn builds_async_task_result_layouts_for_void_scalar_task_handle_tuple_and_array_results() {
        let void_layout =
            build_async_task_result_layout(&Ty::Builtin(BuiltinType::Void), Span::new(0, 0))
                .expect("void async result layout should be supported");
        assert!(matches!(void_layout, AsyncTaskResultLayout::Void));
        assert_eq!(void_layout.body_llvm_ty(), "void");

        let int_layout =
            build_async_task_result_layout(&Ty::Builtin(BuiltinType::Int), Span::new(0, 0))
                .expect("scalar async result layout should be supported");
        match int_layout {
            AsyncTaskResultLayout::Loadable {
                llvm_ty,
                _size,
                align,
            } => {
                assert_eq!(llvm_ty, "i64");
                assert_eq!(_size, 8);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => panic!("expected scalar layout for Int"),
        }

        let task_handle_layout = build_async_task_result_layout(
            &Ty::TaskHandle(Box::new(Ty::Builtin(BuiltinType::Int))),
            Span::new(0, 0),
        )
        .expect("task-handle async result layout should be supported");
        match task_handle_layout {
            AsyncTaskResultLayout::Loadable {
                llvm_ty,
                _size,
                align,
            } => {
                assert_eq!(llvm_ty, "ptr");
                assert_eq!(_size, 8);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => {
                panic!("expected loadable layout for task-handle result")
            }
        }

        let tuple_layout = build_async_task_result_layout(
            &Ty::Tuple(vec![
                Ty::Builtin(BuiltinType::Bool),
                Ty::Builtin(BuiltinType::Int),
            ]),
            Span::new(0, 0),
        )
        .expect("tuple async result layout should be supported");
        match tuple_layout {
            AsyncTaskResultLayout::Loadable {
                llvm_ty,
                _size,
                align,
            } => {
                assert_eq!(llvm_ty, "{ i1, i64 }");
                assert_eq!(_size, 16);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => panic!("expected loadable layout for tuple result"),
        }

        let array_layout = build_async_task_result_layout(
            &Ty::Array {
                element: Box::new(Ty::Builtin(BuiltinType::Int)),
                len: 3,
            },
            Span::new(0, 0),
        )
        .expect("array async result layout should be supported");
        match array_layout {
            AsyncTaskResultLayout::Loadable {
                llvm_ty,
                _size,
                align,
            } => {
                assert_eq!(llvm_ty, "[3 x i64]");
                assert_eq!(_size, 24);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => panic!("expected loadable layout for array result"),
        }
    }

    #[test]
    fn builds_async_task_result_layouts_for_zero_sized_arrays() {
        let array_layout = build_async_task_result_layout(
            &Ty::Array {
                element: Box::new(Ty::Builtin(BuiltinType::Int)),
                len: 0,
            },
            Span::new(0, 0),
        )
        .expect("zero-sized array async result layout should be supported");
        match array_layout {
            AsyncTaskResultLayout::Loadable {
                llvm_ty,
                _size,
                align,
            } => {
                assert_eq!(llvm_ty, "[0 x i64]");
                assert_eq!(_size, 0);
                assert_eq!(align, 8);
            }
            AsyncTaskResultLayout::Void => {
                panic!("expected loadable layout for zero-sized array result")
            }
        }
    }

    #[test]
    fn emits_scalar_struct_value_lowering_in_declaration_order() {
        let rendered = emit_library(
            r#"
struct Pair {
    left: Bool,
    right: Int,
}

fn pair() -> Pair {
    return Pair { right: 42, left: true }
}
"#,
        );

        assert!(rendered.contains("define { i1, i64 } @ql_1_pair()"));
        assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
        assert!(rendered.contains("insertvalue { i1, i64 }"));
        assert!(rendered.contains("i64 42, 1"));
        assert!(rendered.contains("ret { i1, i64 }"));
    }

    #[test]
    fn emits_fixed_array_value_lowering() {
        let rendered = emit_library(
            r#"
fn values() -> [Int; 3] {
    return [1, 2, 3]
}
"#,
        );

        assert!(rendered.contains("define [3 x i64] @ql_0_values()"));
        assert!(rendered.contains("insertvalue [3 x i64] undef, i64 1, 0"));
        assert!(rendered.contains("i64 2, 1"));
        assert!(rendered.contains("i64 3, 2"));
        assert!(rendered.contains("ret [3 x i64]"));
    }

    #[test]
    fn emits_empty_array_value_lowering_when_return_type_is_known() {
        let rendered = emit_library(
            r#"
fn values() -> [Int; 0] {
    return []
}
"#,
        );

        assert!(
            rendered.contains("define [0 x i64] @ql_0_values()"),
            "{rendered}"
        );
        assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
        assert!(rendered.contains("ret [0 x i64]"), "{rendered}");
    }

    #[test]
    fn emits_empty_array_argument_lowering_when_callee_param_type_is_known() {
        let rendered = emit_library(
            r#"
fn take(values: [Int; 0]) -> Int {
    return 0
}

fn call() -> Int {
    return take([])
}
"#,
        );

        assert!(
            rendered.contains("define i64 @ql_0_take([0 x i64] %arg0)"),
            "{rendered}"
        );
        assert!(
            rendered.contains("call i64 @ql_0_take(") && rendered.contains("[0 x i64]"),
            "{rendered}"
        );
    }

    #[test]
    fn emits_empty_array_lowering_inside_expected_tuple_items() {
        let rendered = emit_library(
            r#"
fn pair() -> ([Int; 0], Int) {
    return ([], 1)
}
"#,
        );

        assert!(
            rendered.contains("define { [0 x i64], i64 } @ql_0_pair()"),
            "{rendered}"
        );
        assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
        assert!(
            rendered.contains("insertvalue { [0 x i64], i64 }"),
            "{rendered}"
        );
    }

    #[test]
    fn emits_empty_array_lowering_inside_expected_struct_fields() {
        let rendered = emit_library(
            r#"
struct Wrap {
    values: [Int; 0],
}

fn build() -> Wrap {
    return Wrap { values: [] }
}
"#,
        );

        assert!(
            rendered.contains("define { [0 x i64] } @ql_1_build()"),
            "{rendered}"
        );
        assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
        assert!(rendered.contains("insertvalue { [0 x i64] }"), "{rendered}");
    }

    #[test]
    fn emits_empty_array_lowering_inside_expected_nested_arrays() {
        let rendered = emit_library(
            r#"
fn values() -> [[Int; 0]; 1] {
    return [[]]
}
"#,
        );

        assert!(
            rendered.contains("define [1 x [0 x i64]] @ql_0_values()"),
            "{rendered}"
        );
        assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
        assert!(
            rendered.contains("insertvalue [1 x [0 x i64]]"),
            "{rendered}"
        );
    }

    #[test]
    fn rejects_empty_array_value_without_expected_type() {
        let messages = emit_error(
            r#"
fn main() -> Int {
    let values = []
    return 0
}
"#,
        );

        assert!(messages.iter().any(|message| {
            message
                == "LLVM IR backend foundation cannot infer the element type of an empty array literal without an expected array type"
        }));
    }

    #[test]
    fn emits_struct_literal_lowering_through_same_file_import_alias() {
        let rendered = emit_library(
            r#"
use Pair as P

struct Pair {
    left: Bool,
    right: Int,
}

fn pair() -> Pair {
    return P { right: 42, left: true }
}
"#,
        );

        assert!(rendered.contains("define { i1, i64 } @ql_1_pair()"));
        assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
        assert!(rendered.contains("i64 42, 1"));
    }

    #[test]
    fn emits_struct_field_projection_reads() {
        let rendered = emit_library(
            r#"
struct Pair {
    left: Bool,
    right: Int,
}

fn right(pair: Pair) -> Int {
    return pair.right
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_1_right({ i1, i64 } %arg0)"));
        assert!(rendered.contains("getelementptr inbounds { i1, i64 }, ptr"));
        assert!(rendered.contains("i32 0, i32 1"));
        assert!(rendered.contains("load i64, ptr"));
    }

    #[test]
    fn emits_tuple_index_projection_reads() {
        let rendered = emit_library(
            r#"
fn second(pair: (Bool, Int)) -> Int {
    return pair[1]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_second({ i1, i64 } %arg0)"));
        assert!(rendered.contains("getelementptr inbounds { i1, i64 }, ptr"));
        assert!(rendered.contains("i32 0, i32 1"));
        assert!(rendered.contains("load i64, ptr"));
    }

    #[test]
    fn emits_array_index_projection_reads() {
        let rendered = emit_library(
            r#"
fn pick(values: [Int; 3], index: Int) -> Int {
    return values[index]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_pick([3 x i64] %arg0, i64 %arg1)"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(rendered.contains("i64 0, i64 %t"));
        assert!(rendered.contains("load i64, ptr"));
    }

    #[test]
    fn emits_nested_projection_reads_through_recursive_aggregates() {
        let rendered = emit_library(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

struct Outer {
    pair: Pair,
    values: [Int; 2],
}

fn pick_pair(outer: Outer) -> Int {
    return outer.pair.right
}

fn pick_array(outer: Outer, index: Int) -> Int {
    return outer.values[index]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_2_pick_pair({ { i64, i64 }, [2 x i64] } %arg0)"));
        assert!(rendered.contains("getelementptr inbounds { { i64, i64 }, [2 x i64] }, ptr"));
        assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
        assert!(rendered.contains("getelementptr inbounds [2 x i64], ptr"));
    }

    #[test]
    fn emits_struct_field_projection_writes() {
        let rendered = emit_library(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

fn write_right() -> Int {
    var pair = Pair { left: 1, right: 2 }
    pair.right = 7
    return pair.right
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_1_write_right()"));
        assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
        assert!(rendered.contains("i32 0, i32 1"));
        assert!(rendered.contains("store i64 7, ptr %t"));
    }

    #[test]
    fn emits_tuple_index_projection_writes() {
        let rendered = emit_library(
            r#"
fn write_first() -> Int {
    var pair = (1, 2)
    pair[0] = 9
    return pair[0]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_write_first()"));
        assert!(rendered.contains("getelementptr inbounds { i64, i64 }, ptr"));
        assert!(rendered.contains("i32 0, i32 0"));
        assert!(rendered.contains("store i64 9, ptr %t"));
    }

    #[test]
    fn emits_array_index_projection_writes() {
        let rendered = emit_library(
            r#"
fn write_first() -> Int {
    var values = [1, 2, 3]
    values[0] = 9
    return values[0]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_write_first()"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(rendered.contains("i64 0, i64 0"));
        assert!(rendered.contains("store i64 9, ptr %t"));
    }

    #[test]
    fn emits_dynamic_array_index_projection_writes() {
        let rendered = emit_library(
            r#"
fn write_at(index: Int) -> Int {
    var values = [1, 2, 3]
    values[index] = 9
    return values[index]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_write_at(i64 %arg0)"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(rendered.contains("i64 0, i64 %t"));
        assert!(rendered.contains("store i64 9, ptr %t"));
    }

    #[test]
    fn emits_nested_dynamic_array_index_projection_writes() {
        let rendered = emit_library(
            r#"
fn write_cell(row: Int, col: Int) -> Int {
    var matrix = [[1, 2, 3], [4, 5, 6]]
    matrix[row][col] = 9
    return matrix[row][col]
}
"#,
        );

        assert!(rendered.contains("define i64 @ql_0_write_cell(i64 %arg0, i64 %arg1)"));
        assert!(rendered.contains("getelementptr inbounds [2 x [3 x i64]], ptr"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(rendered.contains("store i64 9, ptr %t"));
    }

    #[test]
    fn emits_dynamic_task_handle_array_index_projection_writes() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(index: Int) -> Wrap {
    var tasks = [worker(), worker()]
    tasks[index] = worker()
    return await tasks[0]
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.contains("i64 0, i64 %t"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
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
    fn emits_async_struct_task_result_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Bool,
    right: Int,
}

async fn worker() -> Pair {
    return Pair { right: 42, left: true }
}

async fn helper() -> Pair {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define { i1, i64 } @ql_1_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
        assert!(rendered.contains("i64 42, 1"));
        assert!(rendered.contains("define { i1, i64 } @ql_2_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load { i1, i64 }, ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_async_array_task_result_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> [Int; 3] {
    return [1, 2, 3]
}

async fn helper() -> [Int; 3] {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define [3 x i64] @ql_0_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("insertvalue [3 x i64] undef, i64 1, 0"));
        assert!(rendered.contains("i64 2, 1"));
        assert!(rendered.contains("i64 3, 2"));
        assert!(rendered.contains("define [3 x i64] @ql_1_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load [3 x i64], ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_async_zero_sized_array_task_result_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> [Int; 0] {
    return []
}

async fn helper() -> [Int; 0] {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define [0 x i64] @ql_0_worker__async_body(ptr %frame)"));
        assert!(
            rendered.contains("store [0 x i64] zeroinitializer"),
            "{rendered}"
        );
        assert!(rendered.contains("define [0 x i64] @ql_1_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load [0 x i64], ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_async_zero_sized_recursive_aggregate_task_result_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define { [0 x i64] } @ql_1_worker__async_body(ptr %frame)"));
        assert!(
            rendered.contains("store [0 x i64] zeroinitializer"),
            "{rendered}"
        );
        assert!(rendered.contains("define { [0 x i64] } @ql_2_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_async_zero_sized_recursive_aggregate_param_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn helper() -> Int {
    return await worker([], Wrap { values: [] }, [[]])
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains(
            "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
        ));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
        assert!(rendered.contains("call ptr @ql_1_worker("));
        assert!(rendered.contains("[0 x i64] zeroinitializer"), "{rendered}");
        assert!(rendered.contains("[1 x [0 x i64]]"), "{rendered}");
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load i64, ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_async_recursive_aggregate_task_result_lowering() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

async fn helper() -> (Pair, [Int; 2]) {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(
            rendered.contains(
                "define { { i64, i64 }, [2 x i64] } @ql_1_worker__async_body(ptr %frame)"
            )
        );
        assert!(rendered.contains("insertvalue { i64, i64 } undef, i64 1, 0"));
        assert!(rendered.contains("insertvalue [2 x i64] undef, i64 3, 0"));
        assert!(
            rendered.contains(
                "define { { i64, i64 }, [2 x i64] } @ql_2_helper__async_body(ptr %frame)"
            )
        );
        assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_scalar_tuple_value_lowering() {
        let rendered = emit_library(
            r#"
fn pair() -> (Bool, Int) {
    return (true, 42)
}
"#,
        );

        assert!(rendered.contains("define { i1, i64 } @ql_0_pair()"));
        assert!(rendered.contains("insertvalue { i1, i64 } undef, i1 true, 0"));
        assert!(rendered.contains("i64 42, 1"));
        assert!(rendered.contains("ret { i1, i64 }"));
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
    fn emits_await_lowering_for_bound_direct_async_handles() {
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
    let task = worker()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    return await schedule()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("call ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_chained_await_lowering_for_nested_task_handle_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let next = await outer()
    return await next
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("call ptr @ql_1_outer()"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load ptr, ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_chained_await_lowering_for_tuple_task_handle_aggregate_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn helper() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_2_outer()"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
        assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_chained_await_lowering_for_struct_task_handle_aggregate_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

async fn helper() -> Wrap {
    let pending = await outer()
    await pending.first
    return await pending.second
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
        assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_chained_await_lowering_for_array_task_handle_aggregate_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn helper() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_2_outer()"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load [2 x ptr], ptr %t"));
        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_chained_await_lowering_for_nested_aggregate_task_handle_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn helper() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load [2 x { ptr, i64 }], ptr %t"));
        assert!(rendered.contains("getelementptr inbounds [2 x { ptr, i64 }], ptr"));
        assert!(rendered.contains("getelementptr inbounds { ptr, i64 }, ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_await_lowering_for_bound_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = schedule()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @ql_1_schedule()"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_local_returned_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn helper() -> Int {
    return await schedule()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_zero_sized_recursive_aggregate_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    return await schedule()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @ql_1_worker()"));
        assert!(rendered.contains("call ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_bound_zero_sized_recursive_aggregate_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = schedule()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @ql_2_schedule()"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_local_returned_zero_sized_recursive_aggregate_task_handles() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

async fn helper() -> Wrap {
    return await schedule()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @ql_1_worker()"));
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("call ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_forwarded_task_handle_arguments() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn helper() -> Int {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_forward(ptr %arg0)"));
        assert!(rendered.matches("_forward(").count() >= 2);
        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
    }

    #[test]
    fn emits_await_lowering_for_forwarded_zero_sized_recursive_aggregate_task_handles() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn helper() -> Wrap {
    let task = worker()
    let forwarded = forward(task)
    return await forwarded
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_forward(ptr %arg0)"));
        assert!(rendered.matches("_forward(").count() >= 2);
        assert!(rendered.contains("call ptr @ql_1_worker()"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
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
    fn emits_await_lowering_for_tuple_async_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> (Bool, Int) {
    return (true, 1)
}

async fn helper() -> (Bool, Int) {
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define { i1, i64 } @ql_1_helper__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { i1, i64 }, ptr %t"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %t"));
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
    fn emits_fire_and_forget_spawn_lowering_in_async_library_body() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
        ]);
        let rendered = emit_with_runtime_hooks(
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

        assert!(rendered.contains("declare ptr @qlrt_executor_spawn(ptr, ptr)"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
    }

    #[test]
    fn emits_spawn_handle_lowering_and_awaits_spawned_task_in_async_library_body() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = spawn worker()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("declare ptr @qlrt_executor_spawn(ptr, ptr)"));
        assert!(rendered.contains("declare ptr @qlrt_task_await(ptr)"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    }

    #[test]
    fn emits_spawn_lowering_for_bound_task_handles() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_0_worker()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    }

    #[test]
    fn emits_spawn_lowering_for_bound_zero_sized_recursive_aggregate_task_handles() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let task = worker()
    let running = spawn task
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @ql_1_worker()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = spawn schedule()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.matches("_schedule(").count() >= 2);
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    }

    #[test]
    fn emits_spawn_lowering_for_bound_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    return worker()
}

async fn helper() -> Int {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @ql_1_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    }

    #[test]
    fn emits_spawn_lowering_for_zero_sized_recursive_aggregate_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = spawn schedule()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.matches("_schedule(").count() >= 2);
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_conditional_zero_sized_recursive_aggregate_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_reverse_branch_conditional_zero_sized_recursive_aggregate_task_handle_helpers()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_conditional_zero_sized_recursive_aggregate_async_calls() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_reverse_branch_conditional_zero_sized_recursive_aggregate_async_calls()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_branch_join_reinitializing_zero_sized_recursive_aggregate_async_calls()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        return await running
    } else {
        task = fresh_worker()
    }
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_reverse_branch_join_reinitializing_zero_sized_recursive_aggregate_async_calls()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper(flag: Bool) -> Wrap {
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task
        task = fresh_worker()
        return await running
    }
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_bound_zero_sized_recursive_aggregate_task_handle_helpers() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

async fn helper() -> Wrap {
    let task = schedule()
    let running = spawn task
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @ql_2_schedule()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_spawn_lowering_for_forwarded_task_handle_arguments() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn helper() -> Int {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_forward(ptr %arg0)"));
        assert!(rendered.matches("_forward(").count() >= 2);
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
    }

    #[test]
    fn emits_spawn_lowering_for_forwarded_zero_sized_recursive_aggregate_task_handles() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

async fn helper() -> Wrap {
    let task = worker()
    let running = spawn forward(task)
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_2_forward(ptr %arg0)"));
        assert!(rendered.matches("_forward(").count() >= 2);
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
    }

    #[test]
    fn emits_await_lowering_for_projected_zero_sized_task_handle_tuple_elements() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = (worker(), worker())
    return await pair[0]
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_await_lowering_for_projected_zero_sized_task_handle_array_elements() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = [worker(), worker()]
    return await pair[0]
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_spawn_lowering_for_projected_zero_sized_task_handle_tuple_elements() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = (worker(), worker())
    let running = spawn pair[0]
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_spawn_lowering_for_projected_zero_sized_task_handle_array_elements() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = [worker(), worker()]
    let running = spawn pair[0]
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_await_lowering_for_projected_zero_sized_task_handle_struct_fields() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    return await pair.task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_spawn_lowering_for_projected_zero_sized_task_handle_struct_fields() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    task: Task[Wrap],
    value: Int,
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let pair = TaskPair { task: worker(), value: 1 }
    let running = spawn pair.task
    return await running
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_spawn_handle_lowering_for_zero_sized_recursive_aggregate_results() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn helper() -> Wrap {
    let task = spawn worker()
    return await task
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("define { [0 x i64] } @ql_1_worker__async_body(ptr %frame)"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %t"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr"));
    }

    #[test]
    fn emits_for_await_lowering_for_fixed_array_async_library_bodies() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::AsyncIteration,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn helper() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
        );

        assert!(rendered.contains("declare ptr @qlrt_async_iter_next(ptr)"));
        assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
        assert!(rendered.contains("icmp ult i64"));
        assert!(rendered.contains("for_await_setup"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(!rendered.contains("does not support `for await` lowering yet"));
    }

    #[test]
    fn inlines_runtime_support_for_async_dynamic_library_builds() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskAwait,
            RuntimeCapability::AsyncIteration,
        ]);
        let rendered = emit_with_runtime_hooks_and_inline_support(
            r#"
async fn seed_total() -> Int {
    var total = 0
    for await value in [20, 22] {
        total = total + value
    }
    return total
}

async fn helper() -> Int {
    return await seed_total()
}

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
            true,
        );

        assert!(!rendered.contains("declare ptr @qlrt_async_task_create(ptr, ptr)"));
        assert!(!rendered.contains("declare ptr @qlrt_task_await(ptr)"));
        assert!(!rendered.contains("declare void @qlrt_task_result_release(ptr)"));
        assert!(!rendered.contains("declare ptr @qlrt_async_iter_next(ptr)"));
        assert!(rendered.contains("define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)"));
        assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
        assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));
        assert!(rendered.contains("define ptr @qlrt_async_iter_next(ptr %iterator)"));
        assert!(rendered.contains("define i64 @q_add(i64 %arg0, i64 %arg1)"));
    }

    #[test]
    fn rejects_unsupported_for_await_lowering_for_non_array_iterables_without_iterable_noise() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::AsyncIteration,
        ]);
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn helper() -> Int {
    for await value in (1, 2, 3) {
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

    #[test]
    fn dedupes_cleanup_and_for_lowering_diagnostics() {
        let messages = emit_error(
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    for value in 0 {
        break
    }
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
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `for` lowering yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_cleanup_and_match_lowering_diagnostics() {
        let messages = emit_error(
            r#"
extern "c" fn first()

fn main() -> Int {
    let flag = true
    defer first()
    return match flag {
        true => 1,
        false => 0,
    }
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
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `match` lowering yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_match_and_question_mark_lowering_diagnostics() {
        let messages = emit_error(
            r#"
fn helper() -> Int {
    let flag = true
    return match flag {
        true => 1,
        false => 0,
    }
}

fn main() -> Int {
    return helper()?
}
"#,
        );

        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation encountered an opaque expression that still needs MIR elaboration"
                })
                .count(),
            1
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `match` lowering yet"
                })
                .count(),
            1
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation only supports single-name binding patterns"
                })
                .count(),
            2
        );
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `?` lowering yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_cleanup_and_question_mark_lowering_diagnostics() {
        let messages = emit_error(
            r#"
extern "c" fn first()

fn helper() -> Int {
    return 1
}

fn main() -> Int {
    defer first()
    return helper()?
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
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `?` lowering yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_cleanup_and_closure_value_lowering_diagnostics() {
        let messages = emit_error(
            r#"
extern "c" fn first()

fn main() -> Int {
    defer first()
    let capture = () => 1
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
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support closure values yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    #[test]
    fn dedupes_cleanup_and_for_await_lowering_diagnostics() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::AsyncIteration,
        ]);
        let messages = emit_error_with_runtime_hooks(
            r#"
extern "c" fn first()

async fn helper() -> Int {
    defer first()
    for await value in (1, 2, 3) {
        break
    }
    return 0
}
"#,
            CodegenMode::Library,
            &runtime_hooks,
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
        assert_eq!(
            messages
                .iter()
                .filter(|message| {
                    message.as_str()
                        == "LLVM IR backend foundation does not support `for await` lowering yet"
                })
                .count(),
            1
        );
        assert!(messages.iter().all(|message| {
            message != "LLVM IR backend foundation does not support `for` lowering yet"
                && message != "LLVM IR backend foundation does not support array values yet"
                && !message.contains("could not resolve LLVM type for local")
                && !message.contains("could not infer LLVM type for MIR local")
        }));
    }

    // -----------------------------------------------------------------------
    // async fn main — program-mode entry lifecycle tests (P7.4)
    // -----------------------------------------------------------------------

    #[test]
    fn emits_async_main_entry_lifecycle_in_program_mode() {
        // async fn main drives the full task lifecycle from the C `@main` entry:
        //   task    = @ql_N_main()                        (task-create wrapper)
        //   join    = qlrt_executor_spawn(null, task)
        //   res_ptr = qlrt_task_await(join)
        //   ret_val = load i64, ptr res_ptr
        //             qlrt_task_result_release(res_ptr)
        //   exit    = trunc i64 ret_val to i32
        //             ret i32 exit
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    return await worker()
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        // Runtime hooks must be defined in-program so native executable links succeed.
        assert!(rendered.contains("define ptr @qlrt_async_task_create(ptr %entry_fn, ptr %frame)"));
        assert!(rendered.contains("define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)"));
        assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
        assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));

        // The async body and task-create wrapper must be emitted.
        assert!(
            rendered.contains("define ptr @ql_1_main__async_body(ptr %frame)")
                || rendered.contains("define ptr @ql_0_main__async_body(ptr %frame)")
                || rendered.contains("__async_body")
        );
        assert!(rendered.contains("__async_entry"));
        assert!(rendered.contains("define ptr @ql_") && rendered.contains("@main("));

        // The C entry point must drive the lifecycle.
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("load i64, ptr %async_main_res"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.contains("trunc i64 %async_main_ret to i32"));
        assert!(rendered.contains("ret i32 %async_main_exit"));
    }

    #[test]
    fn emits_async_void_main_entry_lifecycle_in_program_mode() {
        // async fn main returning Void: the host entry skips the load and returns 0.
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn main() -> Void {
    return
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.contains("ret i32 0"));
        // No result load for void.
        assert!(!rendered.contains("load i64, ptr %async_main_res"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_fixed_array_for_await_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
            RuntimeCapability::AsyncIteration,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn main() -> Int {
    var total = 0
    for await value in [1, 2, 3] {
        total = total + value
    }
    return total
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @qlrt_executor_spawn(ptr %executor, ptr %task)"));
        assert!(rendered.contains("define ptr @qlrt_task_await(ptr %handle)"));
        assert!(rendered.contains("define void @qlrt_task_result_release(ptr %result)"));
        assert!(rendered.contains("define ptr @qlrt_async_iter_next(ptr %iterator)"));
        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.contains("store i64 -1, ptr %for_await_index_bb"));
        assert!(rendered.contains("icmp ult i64"));
        assert!(rendered.contains("for_await_setup"));
        assert!(rendered.contains("getelementptr inbounds [3 x i64], ptr"));
        assert!(!rendered.contains("does not support `for await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_nested_task_handle_payload_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn outer() -> Task[Int] {
    return worker()
}

async fn main() -> Int {
    let next = await outer()
    return await next
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %t"));
        assert!(rendered.contains("load ptr, ptr %t"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_tuple_task_handle_payload_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn main() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
        assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_array_task_handle_payload_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Task[Int]; 2] {
    return [left(), right()]
}

async fn main() -> Int {
    let tasks = await outer()
    let first = await tasks[0]
    let second = await tasks[1]
    return first + second
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load [2 x ptr], ptr %t"));
        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_nested_aggregate_task_handle_payload_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    task: Task[Int],
    value: Int,
}

async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> [Pending; 2] {
    return [
        Pending { task: left(), value: 10 },
        Pending { task: right(), value: 20 },
    ]
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending[0].task
    let second = await pending[1].task
    return first + second + pending[0].value + pending[1].value
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load [2 x { ptr, i64 }], ptr %t"));
        assert!(rendered.contains("getelementptr inbounds [2 x { ptr, i64 }], ptr"));
        assert!(rendered.contains("getelementptr inbounds { ptr, i64 }, ptr"));
        assert!(rendered.matches("load ptr, ptr").count() >= 2);
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_helper_task_handle_flows_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn other() -> Int {
    return 2
}

fn schedule() -> Task[Int] {
    return worker()
}

fn forward(task: Task[Int]) -> Task[Int] {
    return task
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return direct + bound_value + spawned_value + forwarded_value + running_value
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("_schedule(").count() >= 3);
        assert!(rendered.matches("_forward(").count() >= 3);
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
        assert!(rendered.matches("@qlrt_task_await").count() >= 6);
        assert!(!rendered.contains("does not support `await` lowering yet"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_helper_task_handle_flows_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn other() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    return worker()
}

fn forward(task: Task[Wrap]) -> Task[Wrap] {
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let direct = await schedule()

    let bound = schedule()
    let bound_value = await bound

    let spawned = spawn schedule()
    let spawned_value = await spawned

    let task = other()
    let forwarded = forward(task)
    let forwarded_value = await forwarded

    let next = worker()
    let running = spawn forward(next)
    let running_value = await running

    return score(direct)
        + score(bound_value)
        + score(spawned_value)
        + score(forwarded_value)
        + score(running_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("_schedule(").count() >= 3);
        assert!(rendered.matches("_forward(").count() >= 3);
        assert!(rendered.matches("_score(").count() >= 6);
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
        assert!(rendered.matches("@qlrt_task_await").count() >= 6);
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_local_returned_task_handle_helpers_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

fn schedule() -> Task[Int] {
    let task = worker()
    return task
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return first + second
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("_schedule(").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_direct_task_handles_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first = await first_task
    let second = await second_task
    return first + second
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.matches("store ptr %t").count() >= 2);
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("call ptr @qlrt_task_await(ptr %t").count() >= 2);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_spawned_bound_task_handles_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first_task = worker(1)
    let second_task = worker(2)
    let first_running = spawn first_task
    let second_running = spawn second_task
    let first = await first_running
    let second = await second_running
    return first + second
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.matches("store ptr %t").count() >= 2);
        assert!(
            rendered
                .matches("call ptr @qlrt_executor_spawn(ptr null, ptr %t")
                .count()
                >= 2
        );
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_local_returned_zero_sized_task_handle_helpers_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn schedule() -> Task[Wrap] {
    let task = worker()
    return task
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await schedule()
    let second = await schedule()
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("_schedule(").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_aggregate_results_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

fn score(values: [Int; 0], value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await empty_values()
    let second = await wrapped()
    return score(first, second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.contains("load [0 x i64], ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_spawned_zero_sized_aggregate_results_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let task = spawn worker()
    let first = await task
    return score(first)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aggregate_results_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 1)
}

async fn array_worker() -> [Int; 3] {
    return [2, 3, 4]
}

async fn pair_worker() -> Pair {
    return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn score_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
    return pair.left + pair.right
}

async fn main() -> Int {
    let tuple_value = await tuple_worker()
    let array_value = await array_worker()
    let pair_value = await pair_worker()
    return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load { i1, i64 }, ptr %t"));
        assert!(rendered.contains("load [3 x i64], ptr %t"));
        assert!(rendered.contains("load { i64, i64 }, ptr %t"));
        assert!(rendered.matches("_score_").count() >= 4);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_spawned_aggregate_results_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn tuple_worker() -> (Bool, Int) {
    return (true, 1)
}

async fn array_worker() -> [Int; 3] {
    return [2, 3, 4]
}

async fn pair_worker() -> Pair {
    return Pair { left: 5, right: 6 }
}

fn score_tuple(pair: (Bool, Int)) -> Int {
    if pair[0] {
        return pair[1]
    }
    return 0
}

fn score_array(values: [Int; 3]) -> Int {
    return values[0] + values[1] + values[2]
}

fn score_pair(pair: Pair) -> Int {
    return pair.left + pair.right
}

async fn main() -> Int {
    let tuple_task = spawn tuple_worker()
    let array_task = spawn array_worker()
    let pair_task = spawn pair_worker()
    let tuple_value = await tuple_task
    let array_value = await array_task
    let pair_value = await pair_task
    return score_tuple(tuple_value) + score_array(array_value) + score_pair(pair_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load { i1, i64 }, ptr %t"));
        assert!(rendered.contains("load [3 x i64], ptr %t"));
        assert!(rendered.contains("load { i64, i64 }, ptr %t"));
        assert!(rendered.matches("_score_").count() >= 4);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_recursive_aggregate_results_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let value = await worker()
    return score(value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_spawned_recursive_aggregate_results_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker() -> (Pair, [Int; 2]) {
    return (Pair { left: 1, right: 2 }, [3, 4])
}

fn score(result: (Pair, [Int; 2])) -> Int {
    return result[0].left + result[0].right + result[1][0] + result[1][1]
}

async fn main() -> Int {
    let task = spawn worker()
    let value = await task
    return score(value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load { { i64, i64 }, [2 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_recursive_aggregate_param_lowering_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn main() -> Int {
    return await worker(Pair { left: 1, right: 2 }, [3, 4])
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
        assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
        assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
        assert!(rendered.contains("call ptr @ql_1_worker("));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load i64, ptr"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_spawned_recursive_aggregate_param_lowering_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pair {
    left: Int,
    right: Int,
}

async fn worker(pair: Pair, values: [Int; 2]) -> Int {
    return pair.right + values[1]
}

async fn main() -> Int {
    let task = spawn worker(Pair { left: 1, right: 2 }, [3, 4])
    return await task
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define ptr @ql_1_worker({ i64, i64 } %arg0, [2 x i64] %arg1)"));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 32, i64 8)"));
        assert!(rendered.contains("store { i64, i64 } %arg0, ptr %async_frame_field0"));
        assert!(rendered.contains("store [2 x i64] %arg1, ptr %async_frame_field1"));
        assert!(rendered.contains("call ptr @ql_1_worker("));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load i64, ptr"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_zero_sized_recursive_aggregate_param_lowering_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn main() -> Int {
    return await worker([], Wrap { values: [] }, [[]])
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains(
            "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
        ));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
        assert!(rendered.contains("call ptr @ql_1_worker("));
        assert!(rendered.contains("[0 x i64] zeroinitializer"));
        assert!(rendered.contains("[1 x [0 x i64]]"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr"));
        assert!(rendered.contains("load i64, ptr"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_spawned_zero_sized_recursive_aggregate_param_lowering_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker(values: [Int; 0], wrap: Wrap, nested: [[Int; 0]; 1]) -> Int {
    return 7
}

async fn main() -> Int {
    let task = spawn worker([], Wrap { values: [] }, [[]])
    return await task
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains(
            "define ptr @ql_1_worker([0 x i64] %arg0, { [0 x i64] } %arg1, [1 x [0 x i64]] %arg2)"
        ));
        assert!(rendered.contains("call ptr @qlrt_async_frame_alloc(i64 0, i64 8)"));
        assert!(rendered.contains("call ptr @ql_1_worker("));
        assert!(rendered.contains("[0 x i64] zeroinitializer"));
        assert!(rendered.contains("[1 x [0 x i64]]"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("load i64, ptr"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_task_handle_awaits_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    let tuple_second = await tuple[1]

    let array = [worker(3), worker(4)]
    let array_first = await array[0]
    let array_second = await array[1]

    let pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    let struct_second = await pair.right

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 4
        );
        assert!(rendered.matches("_score(").count() >= 6);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_task_handle_spawns_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let tuple = (worker(1), worker(2))
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(3), worker(4)]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.matches("load i64, ptr %t").count() >= 3);
        assert!(rendered.matches("_score(").count() >= 4);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_task_handle_reinit_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct TaskPair {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tuple = (worker(1), worker(2))
    let tuple_first = await tuple[0]
    tuple[0] = worker(7)
    let tuple_second = await tuple[0]

    var array = [worker(3), worker(4)]
    let array_first = await array[0]
    array[0] = worker(8)
    let array_second = await array[0]

    var pair = TaskPair { left: worker(5), right: worker(6) }
    let struct_first = await pair.left
    pair.left = worker(9)
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.matches("store ptr").count() >= 9);
        assert!(rendered.matches("load i64, ptr %t").count() >= 6);
        assert!(rendered.matches("_score(").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_task_handle_conditional_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(1), worker(2)]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker(7)
    }
    let final_value = await tasks[0]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_dynamic_task_handle_reinit_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    let first = await tasks[slot.value]
    tasks[slot.value] = worker(first + 1)
    let second = await tasks[slot.value]
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 3
        );
        assert!(rendered.matches("store ptr").count() >= 4);
        assert!(rendered.matches("load i64, ptr %t").count() >= 3);
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_dynamic_task_handle_conditional_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    if flag {
        let first = await tasks[slot.value]
        tasks[slot.value] = worker(first + 1)
    }
    let final_value = await tasks[slot.value]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 3
        );
        assert!(rendered.matches("store ptr").count() >= 4);
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_guard_refined_dynamic_task_handle_literal_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn helper(index: Int) -> Int {
    var tasks = [worker(1), worker(2)]
    if index == 0 {
        let first = await tasks[index]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    return score(final_value)
}

async fn main() -> Int {
    return await helper(0)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 4);
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_guard_refined_projected_dynamic_task_handle_literal_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var tasks = [worker(1), worker(2)]
    let slot = Slot { value: 0 }
    if slot.value == 0 {
        let first = await tasks[slot.value]
        tasks[0] = worker(first + 1)
    }
    let final_value = await tasks[0]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 1
        );
        assert!(rendered.matches("store ptr").count() >= 4);
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_root_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(1), worker(2)],
    }
    let slot = Slot { value: 0 }
    let first = await pending.tasks[slot.value]
    pending.tasks[slot.value] = worker(first + 1)
    return await pending.tasks[slot.value]
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 3
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 3
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 3
        );
        assert!(rendered.matches("store ptr").count() >= 6);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_composed_dynamic_task_handle_reinit_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let row = choose()
    var tasks = [worker(1), worker(2)]
    let slots = [row, row]
    let first = await tasks[slots[row]]
    tasks[slots[row]] = worker(first + 1)
    let final_value = await tasks[slots[row]]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x i64], ptr")
                .count()
                >= 3
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 3
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_alias_sourced_composed_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn choose() -> Int {
    return 0
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let row = choose()
    var tasks = [worker(1), worker(2)]
    let slots = [row, row]
    let alias = slots
    let first = await tasks[alias[row]]
    tasks[slots[row]] = worker(first + 1)
    let final_value = await tasks[alias[row]]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x i64], ptr")
                .count()
                >= 3
        );
        assert!(rendered.matches("store [2 x i64]").count() >= 2);
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_projected_root_const_backed_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let first = await pending.tasks[INDEX]
    pending.tasks[0] = worker(first + 3)
    let second = await pending.tasks[INDEX]
    let tail = await pending.tasks[1]
    return second + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 3
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(5), worker(8)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let second = await alias[slot.value]
    let tail = await pending.tasks[1]
    return second + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_const_backed_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(6), worker(9)],
    }
    let alias = pending.tasks
    let first = await alias[INDEX]
    pending.tasks[0] = worker(first + 2)
    let second = await alias[INDEX]
    let tail = await pending.tasks[1]
    return second + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_projected_root_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(7), worker(11)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 3)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_dynamic_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(13)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 4)
    }
    let second = await alias[0]
    let tail = await pending.tasks[1]
    return second + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 5);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_tuple_repackage_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 3)
    let pair = (alias[slot.value], worker(5))
    let second = await pair[0]
    let extra = await pair[1]
    let tail = await pending.tasks[1]
    return second + extra + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 6);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_struct_repackage_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 3)
    let bundle = Bundle {
        left: alias[slot.value],
        right: worker(6),
    }
    let second = await bundle.left
    let extra = await bundle.right
    let tail = await pending.tasks[1]
    return second + extra + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 6);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_nested_repackage_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 3)
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(7),
        },
        tail: pending.tasks[1],
    }
    let second = await env.bundle.left
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_projected_root_task_handle_nested_repackage_spawn_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(9), worker(14)],
    }
    let slot = Slot { value: 0 }
    let alias = pending.tasks
    let first = await alias[slot.value]
    pending.tasks[slot.value] = worker(first + 4)
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(7),
        },
        tail: pending.tasks[1],
    }
    let running = spawn env.bundle.left
    let second = await running
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(
            rendered
                .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_aliased_guard_refined_const_backed_projected_root_task_handle_nested_repackage_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
}

struct Slot {
    value: Int,
}

struct Bundle {
    left: Task[Int],
    right: Task[Int],
}

struct Envelope {
    bundle: Bundle,
    tail: Task[Int],
}

const INDEX: Int = 0

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pending = Pending {
        tasks: [worker(8), worker(14)],
    }
    let alias = pending.tasks
    let slot = Slot { value: INDEX }
    if slot.value == 0 {
        let first = await alias[slot.value]
        pending.tasks[0] = worker(first + 5)
    }
    let env = Envelope {
        bundle: Bundle {
            left: alias[slot.value],
            right: worker(9),
        },
        tail: pending.tasks[1],
    }
    let second = await env.bundle.left
    let extra = await env.bundle.right
    let tail = await env.tail
    return second + extra + tail
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(
            rendered
                .matches("getelementptr inbounds { [2 x ptr] }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { { ptr, ptr }, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(
            rendered
                .matches("getelementptr inbounds { i64 }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_dynamic_task_handle_array_index_assignment_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var index = 0
    var tasks = [worker(1), worker(2)]
    tasks[index] = worker(3)
    let value = await tasks[0]
    return score(value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("store ptr").count() >= 4);
        assert!(rendered.matches("load i64, ptr %t").count() >= 1);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_dynamic_task_handle_spawn_and_sibling_task_use_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Pending {
    tasks: [Task[Int]; 2],
    fallback: Task[Int],
}

async fn worker(value: Int) -> Int {
    return value
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var index = 0
    let pending = Pending {
        tasks: [worker(1), worker(2)],
        fallback: worker(7),
    }
    let running = spawn pending.tasks[index]
    let first = await running
    let second = await pending.fallback
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 1
        );
        assert!(rendered.matches("load i64, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_nested_task_handle_payload_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Task[Wrap] {
    return worker()
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let next = await outer()
    let value = await next
    return score(value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(rendered.contains("load ptr, ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_struct_task_handle_payload_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct Pending {
    first: Task[Wrap],
    second: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn outer() -> Pending {
    return Pending { first: worker(), second: worker() }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let pending = await outer()
    let first = await pending.first
    let second = await pending.second
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(rendered.contains("load { ptr, ptr }, ptr %t"));
        assert!(rendered.contains("getelementptr inbounds { ptr, ptr }, ptr"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_awaits_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    let tuple_second = await tuple[1]

    let array = [worker(), worker()]
    let array_first = await array[0]
    let array_second = await array[1]

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    let struct_second = await pair.right

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 6);
        assert!(rendered.matches("_score(").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(!rendered.contains("does not support `await` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_spawns_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let tuple = (worker(), worker())
    let tuple_running = spawn tuple[0]
    let tuple_value = await tuple_running

    let array = [worker(), worker()]
    let array_running = spawn array[0]
    let array_value = await array_running

    let pair = TaskPair { left: worker(), right: worker() }
    let struct_running = spawn pair.left
    let struct_value = await struct_running

    return score(tuple_value) + score(array_value) + score(struct_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 4);
        assert!(rendered.matches("@qlrt_task_await").count() >= 4);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("getelementptr inbounds [2 x ptr], ptr"));
        assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 3);
        assert!(rendered.matches("_score(").count() >= 4);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

struct TaskPair {
    left: Task[Wrap],
    right: Task[Wrap],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    var tuple = (worker(), worker())
    let tuple_first = await tuple[0]
    tuple[0] = worker()
    let tuple_second = await tuple[0]

    var array = [worker(), worker()]
    let array_first = await array[0]
    array[0] = worker()
    let array_second = await array[0]

    var pair = TaskPair { left: worker(), right: worker() }
    let struct_first = await pair.left
    pair.left = worker()
    let struct_second = await pair.left

    return score(tuple_first)
        + score(tuple_second)
        + score(array_first)
        + score(array_second)
        + score(struct_first)
        + score(struct_second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(
            rendered
                .matches("getelementptr inbounds { ptr, ptr }, ptr")
                .count()
                >= 4
        );
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 6);
        assert!(rendered.matches("_score(").count() >= 7);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_projected_task_handle_conditional_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var tasks = [worker(), worker()]
    if flag {
        let first = await tasks[0]
        tasks[0] = worker()
    }
    let final_value = await tasks[0]
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_task_await").count() >= 3);
        assert!(
            rendered
                .matches("getelementptr inbounds [2 x ptr], ptr")
                .count()
                >= 2
        );
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.matches("load { [0 x i64] }, ptr %t").count() >= 2);
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support field or index projections yet"));
        assert!(
            !rendered.contains("does not support assignment to field or index projections yet")
        );
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_branch_spawned_reinit_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn fresh_worker() -> Int {
    return 2
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    } else {
        task = fresh_worker()
    }
    let final_value = await task
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("load i64, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_branch_spawned_reinit_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    } else {
        task = fresh_worker()
    }
    let final_value = await task
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_reverse_branch_spawned_reinit_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn fresh_worker() -> Wrap {
    return Wrap { values: [] }
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let flag = true
    var task = worker()
    if flag {
        task = fresh_worker()
    } else {
        let running = spawn task
        task = fresh_worker()
        let first = await running
        return score(first)
    }
    let final_value = await task
    return score(final_value)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 2);
        assert!(rendered.matches("@qlrt_task_await").count() >= 2);
        assert!(rendered.contains("store ptr %t"));
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 2);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_conditional_async_call_spawns_in_program_mode() {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool) -> Int {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Int {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(rendered.contains("load i64, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_conditional_async_call_spawns_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool) -> Wrap {
    if flag {
        let running = spawn worker();
        return await running
    }
    return await worker()
}

async fn choose_reverse(flag: Bool) -> Wrap {
    if flag {
        return await worker()
    }
    let running = spawn worker();
    return await running
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await choose(true)
    let second = await choose_reverse(false)
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 5);
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_conditional_helper_task_handle_spawns_in_program_mode()
    {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
async fn worker() -> Int {
    return 1
}

async fn choose(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Int]) -> Int {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Int {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Int {
    return await choose_reverse(flag, worker())
}

fn score(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(rendered.contains("load i64, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn emits_async_main_entry_lifecycle_with_zero_sized_conditional_helper_task_handle_spawns_in_program_mode()
     {
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            RuntimeCapability::TaskSpawn,
            RuntimeCapability::TaskAwait,
        ]);
        let rendered = emit_with_runtime_hooks(
            r#"
struct Wrap {
    values: [Int; 0],
}

async fn worker() -> Wrap {
    return Wrap { values: [] }
}

async fn choose(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        let running = spawn task
        return await running
    }
    return await task
}

async fn choose_reverse(flag: Bool, task: Task[Wrap]) -> Wrap {
    if flag {
        return await task
    }
    let running = spawn task
    return await running
}

async fn helper(flag: Bool) -> Wrap {
    return await choose(flag, worker())
}

async fn helper_reverse(flag: Bool) -> Wrap {
    return await choose_reverse(flag, worker())
}

fn score(value: Wrap) -> Int {
    return 1
}

async fn main() -> Int {
    let first = await helper(true)
    let second = await helper_reverse(false)
    return score(first) + score(second)
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(rendered.contains("define i32 @main()"));
        assert!(rendered.contains("call ptr @qlrt_executor_spawn(ptr null, ptr %async_main_task)"));
        assert!(rendered.contains("call ptr @qlrt_task_await(ptr %async_main_join)"));
        assert!(rendered.contains("call void @qlrt_task_result_release(ptr %async_main_res)"));
        assert!(rendered.matches("@qlrt_executor_spawn").count() >= 3);
        assert!(rendered.matches("@qlrt_task_await").count() >= 7);
        assert!(rendered.contains("load { [0 x i64] }, ptr %t"));
        assert!(rendered.matches("_score(").count() >= 3);
        assert!(!rendered.contains("does not support `spawn` lowering yet"));
    }

    #[test]
    fn rejects_async_main_without_required_executor_spawn_hook() {
        // async fn main requires the executor-spawn hook; omitting it must error.
        let runtime_hooks = collect_runtime_hook_signatures([
            RuntimeCapability::AsyncFunctionBodies,
            // TaskSpawn (executor-spawn) intentionally absent.
            RuntimeCapability::TaskAwait,
        ]);
        let messages = emit_error_with_runtime_hooks(
            r#"
async fn main() -> Int {
    return 1
}
"#,
            CodegenMode::Program,
            &runtime_hooks,
        );

        assert!(
            messages.iter().any(|m| m.contains("executor-spawn")),
            "expected executor-spawn hook error, got: {messages:?}"
        );
    }
}
