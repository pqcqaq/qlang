mod error;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::env::consts::{ARCH, OS};
use std::fmt::Write;

use ql_ast::{BinaryOp, UnaryOp};
use ql_diagnostics::{Diagnostic, Label};
use ql_hir::{self as hir, FunctionRef, ItemId, ItemKind, Param, PatternKind};
use ql_mir::{
    self as mir, BodyOwner, Constant, LocalOrigin, Operand, Place, Rvalue, StatementKind,
    TerminatorKind,
};
use ql_resolve::{BuiltinType, ImportBinding, ResolutionMap, TypeResolution, ValueResolution};
use ql_runtime::{RuntimeHook, RuntimeHookSignature};
use ql_span::Span;
use ql_typeck::{Ty, TyArrayLen, TypeckResult, lower_type};

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

fn cleanup_call_result_ty(signature: &FunctionSignature) -> Ty {
    if signature.is_async {
        Ty::TaskHandle(Box::new(signature.body_return_ty.clone()))
    } else {
        signature.return_ty.clone()
    }
}

fn callable_ty_from_signature(signature: &FunctionSignature) -> Ty {
    Ty::Callable {
        params: signature
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect(),
        ret: Box::new(cleanup_call_result_ty(signature)),
    }
}

fn closure_llvm_name(parent_llvm_name: &str, closure: mir::ClosureId) -> String {
    format!("{parent_llvm_name}__closure{}", closure.index())
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
    body: mir::MirBody,
    local_types: HashMap<mir::LocalId, Ty>,
    async_task_handles: HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    task_handle_place_aliases: HashMap<mir::LocalId, mir::Place>,
    supported_for_loops: HashMap<mir::BasicBlockId, SupportedForLoopLowering>,
    supported_matches: HashMap<mir::BasicBlockId, SupportedMatchLowering>,
    param_binding_locals: Vec<mir::LocalId>,
    direct_local_capturing_closures: HashMap<mir::LocalId, mir::ClosureId>,
    ordinary_control_flow_capturing_closure_calls:
        HashMap<(mir::BasicBlockId, mir::LocalId), SupportedOrdinaryCapturingClosureCall>,
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
struct GuardBindingValue {
    local: hir::LocalId,
    value: LoweredValue,
}

#[derive(Clone, Debug)]
enum CleanupCapturingClosureBindingValue {
    Direct(mir::ClosureId),
    IfBranch {
        condition: Option<LoweredValue>,
        then_closure: mir::ClosureId,
        else_closure: mir::ClosureId,
    },
    TaggedMatch {
        tag: Option<LoweredValue>,
        closures: Vec<mir::ClosureId>,
    },
    BoolMatch {
        scrutinee: Option<LoweredValue>,
        true_closure: mir::ClosureId,
        false_closure: mir::ClosureId,
    },
    IntegerMatch {
        scrutinee: Option<LoweredValue>,
        arms: Vec<CleanupIntegerCapturingClosureMatchArm>,
        fallback_closure: mir::ClosureId,
    },
}

impl CleanupCapturingClosureBindingValue {
    fn direct_closure_id(&self) -> Option<mir::ClosureId> {
        match self {
            Self::Direct(closure_id) => Some(*closure_id),
            Self::IfBranch { .. }
            | Self::TaggedMatch { .. }
            | Self::BoolMatch { .. }
            | Self::IntegerMatch { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
struct CleanupIntegerCapturingClosureMatchArm {
    value: String,
    closure_id: mir::ClosureId,
}

#[derive(Clone, Debug)]
struct CleanupCapturingClosureBinding {
    local: hir::LocalId,
    value: CleanupCapturingClosureBindingValue,
}

impl CleanupCapturingClosureBinding {
    fn direct(local: hir::LocalId, closure_id: mir::ClosureId) -> Self {
        Self {
            local,
            value: CleanupCapturingClosureBindingValue::Direct(closure_id),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CleanupCapturingClosureAssignment {
    local: hir::LocalId,
    closure_id: mir::ClosureId,
}

#[derive(Clone, Copy, Debug)]
struct SupportedCleanupSharedLocalIfBinding {
    target_local: hir::LocalId,
    condition_expr: hir::ExprId,
    then_branch: hir::BlockId,
    else_expr: hir::ExprId,
    then_closure: mir::ClosureId,
    else_closure: mir::ClosureId,
}

#[derive(Clone, Debug)]
enum SupportedCleanupSharedLocalMatchBinding {
    Tagged {
        target_local: hir::LocalId,
        expr_id: hir::ExprId,
        closures: Vec<mir::ClosureId>,
    },
    Bool {
        target_local: hir::LocalId,
        expr_id: hir::ExprId,
        true_closure: mir::ClosureId,
        false_closure: mir::ClosureId,
    },
    Integer {
        target_local: hir::LocalId,
        expr_id: hir::ExprId,
        arms: Vec<CleanupIntegerCapturingClosureMatchArm>,
        fallback_closure: mir::ClosureId,
    },
}

#[derive(Clone, Copy, Debug)]
enum CleanupDirectCapturingClosureExpr {
    Direct(mir::ClosureId),
    Assignment(CleanupCapturingClosureAssignment),
}

impl CleanupDirectCapturingClosureExpr {
    fn closure_id(self) -> mir::ClosureId {
        match self {
            Self::Direct(closure_id) => closure_id,
            Self::Assignment(binding) => binding.closure_id,
        }
    }
}

#[derive(Clone, Debug)]
struct SupportedForLoopLowering {
    iterable_root: SupportedForLoopIterableRoot,
    item_local: mir::LocalId,
    element_ty: Ty,
    item_ty: Ty,
    auto_await_task_elements: bool,
    iterable_kind: SupportedForLoopIterableKind,
    iterable_len: usize,
    body_target: mir::BasicBlockId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupportedForLoopIterableKind {
    Array,
    Tuple,
}

#[derive(Clone, Debug)]
enum SupportedForLoopIterableRoot {
    Place(Place),
    Item(ItemId),
}

fn cleanup_for_iterable_shape(ty: &Ty) -> Option<(SupportedForLoopIterableKind, Ty, usize)> {
    match ty {
        Ty::Array { element, len } if !is_void_ty(element.as_ref()) => Some((
            SupportedForLoopIterableKind::Array,
            element.as_ref().clone(),
            known_array_len(len)?,
        )),
        Ty::Tuple(items)
            if !items.is_empty()
                && !items.iter().any(is_void_ty)
                && items.iter().skip(1).all(|item| {
                    item.compatible_with(&items[0]) && items[0].compatible_with(item)
                }) =>
        {
            Some((
                SupportedForLoopIterableKind::Tuple,
                items[0].clone(),
                items.len(),
            ))
        }
        _ => None,
    }
}

fn known_array_len(len: &TyArrayLen) -> Option<usize> {
    len.as_known()
}

#[derive(Clone, Debug)]
enum SupportedMatchLowering {
    Bool {
        true_target: mir::BasicBlockId,
        false_target: mir::BasicBlockId,
    },
    GuardOnly {
        arms: Vec<SupportedGuardOnlyMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    BoolGuarded {
        arms: Vec<SupportedBoolMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    Integer {
        arms: Vec<SupportedIntegerMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    IntegerGuarded {
        arms: Vec<SupportedGuardedIntegerMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    String {
        arms: Vec<SupportedStringMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    StringGuarded {
        arms: Vec<SupportedGuardedStringMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
    Enum {
        arms: Vec<SupportedEnumMatchArm>,
        fallback_target: mir::BasicBlockId,
    },
}

#[derive(Clone, Debug)]
struct SupportedGuardOnlyMatchArm {
    pattern: hir::PatternId,
    guard: SupportedBoolGuard,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug)]
struct SupportedBoolMatchArm {
    pattern: SupportedBoolMatchPattern,
    binding_local: Option<hir::LocalId>,
    guard: SupportedBoolGuard,
    target: mir::BasicBlockId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupportedBoolMatchPattern {
    True,
    False,
    CatchAll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SupportedBoolGuard {
    Always,
    Dynamic(hir::ExprId),
}

#[derive(Clone, Debug)]
struct SupportedIntegerMatchArm {
    value: String,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug)]
struct SupportedGuardedIntegerMatchArm {
    pattern: SupportedIntegerMatchPattern,
    binding_local: Option<hir::LocalId>,
    guard: SupportedBoolGuard,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug)]
enum SupportedIntegerMatchPattern {
    Literal(String),
    CatchAll,
}

#[derive(Clone, Debug)]
struct SupportedStringMatchArm {
    value: String,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug)]
struct SupportedGuardedStringMatchArm {
    pattern: SupportedStringMatchPattern,
    binding_local: Option<hir::LocalId>,
    guard: SupportedBoolGuard,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SupportedStringMatchPattern {
    Literal(String),
    CatchAll,
}

#[derive(Clone, Debug)]
struct SupportedEnumMatchArm {
    variant_index: Option<usize>,
    pattern: hir::PatternId,
    guard: SupportedBoolGuard,
    target: mir::BasicBlockId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SupportedOrdinaryCapturingClosureCall {
    Branch {
        condition: Operand,
        then_closure: mir::ClosureId,
        else_closure: mir::ClosureId,
    },
    BoolMatch {
        scrutinee: Operand,
        true_closure: mir::ClosureId,
        false_closure: mir::ClosureId,
    },
    IntegerMatch {
        scrutinee: Operand,
        arms: Vec<SupportedOrdinaryIntegerCapturingClosureCallArm>,
        fallback_closure: mir::ClosureId,
    },
    StringMatch {
        scrutinee: Operand,
        arms: Vec<SupportedOrdinaryStringCapturingClosureCallArm>,
        fallback_closure: mir::ClosureId,
    },
    StringGuardedMatch {
        scrutinee: Operand,
        arms: Vec<SupportedOrdinaryGuardedStringCapturingClosureCallArm>,
        fallback_closure: mir::ClosureId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SupportedOrdinaryIntegerCapturingClosureCallArm {
    value: String,
    closure_id: mir::ClosureId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SupportedOrdinaryStringCapturingClosureCallArm {
    value: String,
    closure_id: mir::ClosureId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SupportedOrdinaryGuardedStringCapturingClosureCallArm {
    pattern: SupportedStringMatchPattern,
    binding_local: Option<hir::LocalId>,
    guard: SupportedBoolGuard,
    closure_id: mir::ClosureId,
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
struct EnumVariantFieldLowering {
    name: Option<String>,
    ty: Ty,
    llvm_ty: String,
}

#[derive(Clone, Debug)]
struct EnumVariantLowering {
    name: String,
    fields: Vec<EnumVariantFieldLowering>,
    payload_llvm_ty: Option<String>,
}

#[derive(Clone, Debug)]
struct EnumPayloadStorageLowering {
    llvm_ty: String,
    size: u64,
    align: u64,
}

#[derive(Clone, Debug)]
struct EnumLowering {
    llvm_ty: String,
    layout: LoadableAbiLayout,
    storage: Option<EnumPayloadStorageLowering>,
    variants: Vec<EnumVariantLowering>,
}

#[derive(Clone, Debug)]
struct AsyncTaskHandleInfo {
    result_ty: Ty,
    result_layout: AsyncTaskResultLayout,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct StringLiteralKey {
    value: String,
    is_format: bool,
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
    const_closure_llvm_names: HashMap<hir::ExprId, String>,
    string_literal_llvm_names: HashMap<StringLiteralKey, String>,
}

impl<'a> ModuleEmitter<'a> {
    fn new(input: CodegenInput<'a>) -> Self {
        Self {
            input,
            signatures: HashMap::new(),
            const_closure_llvm_names: HashMap::new(),
            string_literal_llvm_names: HashMap::new(),
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

        let mut prepared_closures = Vec::new();
        for function in &prepared {
            self.collect_prepared_closures(
                function.signature.function_ref,
                &function.signature.llvm_name,
                function,
                &mut prepared_closures,
                &mut diagnostics,
            );
        }

        let mut const_closure_targets = VecDeque::new();
        let mut seen_const_closures = HashSet::new();
        for function in &prepared {
            self.collect_const_closure_targets_in_body(
                &function.body,
                &mut const_closure_targets,
                &mut seen_const_closures,
            );
        }
        for closure in &prepared_closures {
            self.collect_const_closure_targets_in_body(
                &closure.body,
                &mut const_closure_targets,
                &mut seen_const_closures,
            );
        }

        while let Some((item_id, closure_expr)) = const_closure_targets.pop_front() {
            match self.prepare_const_closure(item_id, closure_expr) {
                Ok(prepared_const_closure) => {
                    self.const_closure_llvm_names.insert(
                        closure_expr,
                        prepared_const_closure.signature.llvm_name.clone(),
                    );

                    let previous_len = prepared_closures.len();
                    self.collect_prepared_closures(
                        FunctionRef::Item(item_id),
                        &prepared_const_closure.signature.llvm_name,
                        &prepared_const_closure,
                        &mut prepared_closures,
                        &mut diagnostics,
                    );
                    self.collect_const_closure_targets_in_body(
                        &prepared_const_closure.body,
                        &mut const_closure_targets,
                        &mut seen_const_closures,
                    );
                    for closure in &prepared_closures[previous_len..] {
                        self.collect_const_closure_targets_in_body(
                            &closure.body,
                            &mut const_closure_targets,
                            &mut seen_const_closures,
                        );
                    }
                    prepared_closures.push(prepared_const_closure);
                }
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }

        if !diagnostics.is_empty() {
            return Err(CodegenError::new(dedupe_diagnostics(diagnostics)));
        }

        self.string_literal_llvm_names = self.collect_string_literal_llvm_names();

        Ok(self.render_module(&reachable, &prepared, &prepared_closures, entry))
    }

    fn collect_string_literal_llvm_names(&self) -> HashMap<StringLiteralKey, String> {
        let mut literals = HashMap::new();
        let mut next_index = 0usize;

        for &item_id in &self.input.hir.items {
            self.collect_string_literals_in_item(item_id, &mut literals, &mut next_index);
        }

        literals
    }

    fn collect_string_literals_in_item(
        &self,
        item_id: ItemId,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        match &self.input.hir.item(item_id).kind {
            ItemKind::Function(function) => {
                self.collect_string_literals_in_function(function, literals, next_index);
            }
            ItemKind::Const(global) | ItemKind::Static(global) => {
                self.collect_string_literals_in_expr(global.value, literals, next_index);
            }
            ItemKind::Struct(struct_decl) => {
                for field in &struct_decl.fields {
                    if let Some(default) = field.default {
                        self.collect_string_literals_in_expr(default, literals, next_index);
                    }
                }
            }
            ItemKind::Enum(enum_decl) => {
                for variant in &enum_decl.variants {
                    let hir::VariantFields::Struct(fields) = &variant.fields else {
                        continue;
                    };
                    for field in fields {
                        if let Some(default) = field.default {
                            self.collect_string_literals_in_expr(default, literals, next_index);
                        }
                    }
                }
            }
            ItemKind::Trait(trait_decl) => {
                for method in &trait_decl.methods {
                    self.collect_string_literals_in_function(method, literals, next_index);
                }
            }
            ItemKind::Impl(impl_decl) => {
                for method in &impl_decl.methods {
                    self.collect_string_literals_in_function(method, literals, next_index);
                }
            }
            ItemKind::Extend(extend_decl) => {
                for method in &extend_decl.methods {
                    self.collect_string_literals_in_function(method, literals, next_index);
                }
            }
            ItemKind::TypeAlias(_) | ItemKind::ExternBlock(_) => {}
        }
    }

    fn collect_string_literals_in_function(
        &self,
        function: &hir::Function,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        let Some(body) = function.body else {
            return;
        };
        self.collect_string_literals_in_block(body, literals, next_index);
    }

    fn collect_string_literals_in_block(
        &self,
        block_id: hir::BlockId,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        let block = self.input.hir.block(block_id);
        for &statement_id in &block.statements {
            self.collect_string_literals_in_stmt(statement_id, literals, next_index);
        }
        if let Some(tail) = block.tail {
            self.collect_string_literals_in_expr(tail, literals, next_index);
        }
    }

    fn collect_string_literals_in_stmt(
        &self,
        statement_id: hir::StmtId,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        match &self.input.hir.stmt(statement_id).kind {
            hir::StmtKind::Let { value, .. } | hir::StmtKind::Defer(value) => {
                self.collect_string_literals_in_expr(*value, literals, next_index);
            }
            hir::StmtKind::Return(value) => {
                if let Some(value) = value {
                    self.collect_string_literals_in_expr(*value, literals, next_index);
                }
            }
            hir::StmtKind::While { condition, body } => {
                self.collect_string_literals_in_expr(*condition, literals, next_index);
                self.collect_string_literals_in_block(*body, literals, next_index);
            }
            hir::StmtKind::Loop { body } => {
                self.collect_string_literals_in_block(*body, literals, next_index);
            }
            hir::StmtKind::For { iterable, body, .. } => {
                self.collect_string_literals_in_expr(*iterable, literals, next_index);
                self.collect_string_literals_in_block(*body, literals, next_index);
            }
            hir::StmtKind::Expr { expr, .. } => {
                self.collect_string_literals_in_expr(*expr, literals, next_index);
            }
            hir::StmtKind::Break | hir::StmtKind::Continue => {}
        }
    }

    fn collect_string_literals_in_expr(
        &self,
        expr_id: hir::ExprId,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::String { value, is_format } => {
                let key = StringLiteralKey {
                    value: value.clone(),
                    is_format: *is_format,
                };
                literals.entry(key).or_insert_with(|| {
                    let llvm_name = format!("ql_str_{}", *next_index);
                    *next_index += 1;
                    llvm_name
                });
            }
            hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => {
                for &item in items {
                    self.collect_string_literals_in_expr(item, literals, next_index);
                }
            }
            hir::ExprKind::RepeatArray { value, .. } => {
                self.collect_string_literals_in_expr(*value, literals, next_index);
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                self.collect_string_literals_in_block(*block_id, literals, next_index);
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_string_literals_in_expr(*condition, literals, next_index);
                self.collect_string_literals_in_block(*then_branch, literals, next_index);
                if let Some(other) = else_branch {
                    self.collect_string_literals_in_expr(*other, literals, next_index);
                }
            }
            hir::ExprKind::Match { value, arms } => {
                self.collect_string_literals_in_expr(*value, literals, next_index);
                for arm in arms {
                    self.collect_string_literals_in_pattern(arm.pattern, literals, next_index);
                    if let Some(guard) = arm.guard {
                        self.collect_string_literals_in_expr(guard, literals, next_index);
                    }
                    self.collect_string_literals_in_expr(arm.body, literals, next_index);
                }
            }
            hir::ExprKind::Closure { body, .. } | hir::ExprKind::Question(body) => {
                self.collect_string_literals_in_expr(*body, literals, next_index);
            }
            hir::ExprKind::Call { callee, args } => {
                self.collect_string_literals_in_expr(*callee, literals, next_index);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(value) => {
                            self.collect_string_literals_in_expr(*value, literals, next_index);
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_string_literals_in_expr(*value, literals, next_index);
                        }
                    }
                }
            }
            hir::ExprKind::Member { object, .. } => {
                self.collect_string_literals_in_expr(*object, literals, next_index);
            }
            hir::ExprKind::Bracket { target, items } => {
                self.collect_string_literals_in_expr(*target, literals, next_index);
                for &item in items {
                    self.collect_string_literals_in_expr(item, literals, next_index);
                }
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.collect_string_literals_in_expr(field.value, literals, next_index);
                }
            }
            hir::ExprKind::Binary { left, right, .. } => {
                self.collect_string_literals_in_expr(*left, literals, next_index);
                self.collect_string_literals_in_expr(*right, literals, next_index);
            }
            hir::ExprKind::Unary { expr, .. } => {
                self.collect_string_literals_in_expr(*expr, literals, next_index);
            }
            hir::ExprKind::Name(_)
            | hir::ExprKind::Integer(_)
            | hir::ExprKind::Bool(_)
            | hir::ExprKind::NoneLiteral => {}
        }
    }

    fn collect_string_literals_in_pattern(
        &self,
        pattern_id: hir::PatternId,
        literals: &mut HashMap<StringLiteralKey, String>,
        next_index: &mut usize,
    ) {
        match pattern_kind(self.input.hir, pattern_id) {
            PatternKind::String(value) => {
                let key = StringLiteralKey {
                    value: value.clone(),
                    is_format: false,
                };
                literals.entry(key).or_insert_with(|| {
                    let llvm_name = format!("ql_str_{}", *next_index);
                    *next_index += 1;
                    llvm_name
                });
            }
            PatternKind::Tuple(items)
            | PatternKind::Array(items)
            | PatternKind::TupleStruct { items, .. } => {
                for item in items {
                    self.collect_string_literals_in_pattern(*item, literals, next_index);
                }
            }
            PatternKind::Struct { fields, .. } => {
                for field in fields {
                    self.collect_string_literals_in_pattern(field.pattern, literals, next_index);
                }
            }
            PatternKind::Binding(_)
            | PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::Bool(_)
            | PatternKind::NoneLiteral
            | PatternKind::Wildcard => {}
        }
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
            .flat_map(|item_id| library_function_refs(self.input.hir, item_id))
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
            let Some(body) = self
                .input
                .mir
                .body_for_owner(function_body_owner(function_ref))
            else {
                continue;
            };

            for block in body.blocks() {
                for statement_id in &block.statements {
                    match &body.statement(*statement_id).kind {
                        StatementKind::Assign { value, .. } | StatementKind::Eval { value } => {
                            self.collect_rvalue_callees(value, &mut queue);
                        }
                        StatementKind::BindPattern { source, .. } => {
                            self.collect_operand_function_values(source, &mut queue);
                        }
                        StatementKind::RegisterCleanup { cleanup } => {
                            let cleanup = body.cleanup(*cleanup);
                            match &cleanup.kind {
                                mir::CleanupKind::Defer { expr } => {
                                    self.collect_guard_expr_callees(*expr, &mut queue);
                                }
                            }
                        }
                        StatementKind::StorageLive { .. }
                        | StatementKind::StorageDead { .. }
                        | StatementKind::RunCleanup { .. } => {}
                    }
                }
                if let TerminatorKind::ForLoop { iterable, .. } = &block.terminator.kind {
                    let item_id = match iterable {
                        Operand::Constant(Constant::Item { item, .. }) => Some(*item),
                        Operand::Constant(Constant::Import(path)) => {
                            local_item_for_import_path(self.input.hir, path)
                        }
                        _ => None,
                    };
                    if let Some(item_id) = item_id
                        && let Some((value_expr, _)) = runtime_task_iterable_item_value(
                            self.input.hir,
                            self.input.resolution,
                            item_id,
                        )
                    {
                        self.collect_guard_expr_callees(value_expr, &mut queue);
                    }
                }
                if let TerminatorKind::Match { arms, .. } = &block.terminator.kind {
                    for arm in arms {
                        if let Some(guard) = arm.guard {
                            self.collect_guard_expr_callees(guard, &mut queue);
                        }
                    }
                }
            }
        }

        ordered.sort_by_key(|function_ref| function_sort_key(*function_ref));
        ordered
    }

    fn collect_rvalue_callees(&self, value: &Rvalue, queue: &mut VecDeque<FunctionRef>) {
        match value {
            Rvalue::Use(operand) | Rvalue::Question(operand) => {
                self.collect_operand_function_values(operand, queue);
            }
            Rvalue::Tuple(items) | Rvalue::Array(items) => {
                for item in items {
                    self.collect_operand_function_values(item, queue);
                }
            }
            Rvalue::RepeatArray { value, .. } => {
                self.collect_operand_function_values(value, queue);
            }
            Rvalue::Call { callee, args } => {
                if let Some(function) = self.resolve_direct_callee_function(callee) {
                    queue.push_back(function);
                }
                self.collect_operand_function_values(callee, queue);
                for arg in args {
                    self.collect_operand_function_values(&arg.value, queue);
                }
            }
            Rvalue::Binary { left, right, .. } => {
                self.collect_operand_function_values(left, queue);
                self.collect_operand_function_values(right, queue);
            }
            Rvalue::Unary { operand, .. } => {
                self.collect_operand_function_values(operand, queue);
            }
            Rvalue::AggregateTupleStruct { items, .. } => {
                for item in items {
                    self.collect_operand_function_values(item, queue);
                }
            }
            Rvalue::AggregateStruct { fields, .. } => {
                for field in fields {
                    self.collect_operand_function_values(&field.value, queue);
                }
            }
            Rvalue::Closure { .. } | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn collect_operand_function_values(
        &self,
        operand: &Operand,
        queue: &mut VecDeque<FunctionRef>,
    ) {
        match operand {
            Operand::Constant(Constant::Function { function, .. }) => queue.push_back(*function),
            Operand::Constant(Constant::Item { item, .. }) => {
                if let Some((value_expr, _)) =
                    runtime_task_backed_item_value(self.input.hir, self.input.resolution, *item)
                {
                    self.collect_guard_expr_callees(value_expr, queue);
                    return;
                }
                match &self.input.hir.item(*item).kind {
                    ItemKind::Function(_) => {
                        queue.push_back(FunctionRef::Item(*item));
                    }
                    ItemKind::Const(global) | ItemKind::Static(global) => {
                        self.collect_guard_expr_callees(global.value, queue);
                    }
                    _ => {}
                }
            }
            Operand::Constant(Constant::Import(path)) => {
                let Some(item_id) = local_item_for_import_path(self.input.hir, path) else {
                    return;
                };
                if let Some((value_expr, _)) =
                    runtime_task_backed_item_value(self.input.hir, self.input.resolution, item_id)
                {
                    self.collect_guard_expr_callees(value_expr, queue);
                    return;
                }
                if matches!(&self.input.hir.item(item_id).kind, ItemKind::Function(_)) {
                    queue.push_back(FunctionRef::Item(item_id));
                    return;
                }
                match &self.input.hir.item(item_id).kind {
                    ItemKind::Const(global) | ItemKind::Static(global) => {
                        self.collect_guard_expr_callees(global.value, queue);
                    }
                    _ => {}
                }
            }
            Operand::Place(_)
            | Operand::Constant(
                Constant::Integer(_)
                | Constant::String { .. }
                | Constant::Bool(_)
                | Constant::None
                | Constant::Void
                | Constant::UnresolvedName(_),
            ) => {}
        }
    }

    fn collect_guard_expr_callees(&self, expr_id: hir::ExprId, queue: &mut VecDeque<FunctionRef>) {
        let mut visited_items = HashSet::new();
        self.collect_guard_expr_callees_with_const_items(expr_id, queue, &mut visited_items);
    }

    fn collect_guard_expr_callees_with_const_items(
        &self,
        expr_id: hir::ExprId,
        queue: &mut VecDeque<FunctionRef>,
        visited_items: &mut HashSet<ItemId>,
    ) {
        let mut visited = HashSet::new();
        if let Some(function) = const_expr_sync_function_ref(
            self.input.hir,
            self.input.resolution,
            expr_id,
            &mut visited,
        ) {
            queue.push_back(function);
        }

        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                if let Some(function) =
                    guard_direct_callee_function(self.input.hir, self.input.resolution, *callee)
                {
                    queue.push_back(function);
                }
                self.collect_guard_expr_callees_with_const_items(*callee, queue, visited_items);
                for arg in args {
                    self.collect_guard_expr_callees_with_const_items(
                        guard_call_arg_expr(arg),
                        queue,
                        visited_items,
                    );
                }
            }
            hir::ExprKind::Unary { expr, .. } => {
                self.collect_guard_expr_callees_with_const_items(*expr, queue, visited_items)
            }
            hir::ExprKind::Binary { left, right, .. } => {
                self.collect_guard_expr_callees_with_const_items(*left, queue, visited_items);
                self.collect_guard_expr_callees_with_const_items(*right, queue, visited_items);
            }
            hir::ExprKind::Member { object, .. } => {
                self.collect_guard_expr_callees_with_const_items(*object, queue, visited_items);
            }
            hir::ExprKind::Bracket { target, items } => {
                self.collect_guard_expr_callees_with_const_items(*target, queue, visited_items);
                for item in items {
                    self.collect_guard_expr_callees_with_const_items(*item, queue, visited_items);
                }
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                self.collect_guard_block_callees(*block_id, queue);
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_guard_expr_callees_with_const_items(*condition, queue, visited_items);
                self.collect_guard_block_callees(*then_branch, queue);
                if let Some(other) = else_branch {
                    self.collect_guard_expr_callees_with_const_items(*other, queue, visited_items);
                }
            }
            hir::ExprKind::Match { value, arms } => {
                self.collect_guard_expr_callees_with_const_items(*value, queue, visited_items);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.collect_guard_expr_callees_with_const_items(
                            guard,
                            queue,
                            visited_items,
                        );
                    }
                    self.collect_guard_expr_callees_with_const_items(
                        arm.body,
                        queue,
                        visited_items,
                    );
                }
            }
            hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => {
                for item in items {
                    self.collect_guard_expr_callees_with_const_items(*item, queue, visited_items);
                }
            }
            hir::ExprKind::RepeatArray { value, .. } => {
                self.collect_guard_expr_callees_with_const_items(*value, queue, visited_items);
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.collect_guard_expr_callees_with_const_items(
                        field.value,
                        queue,
                        visited_items,
                    );
                }
            }
            hir::ExprKind::Closure { body, .. } | hir::ExprKind::Question(body) => {
                self.collect_guard_expr_callees_with_const_items(*body, queue, visited_items);
            }
            hir::ExprKind::Name(_) => {
                if let Some(item_id) =
                    task_backed_item_root_expr(self.input.hir, self.input.resolution, expr_id)
                    && let Some((value_expr, _)) = runtime_task_backed_item_value(
                        self.input.hir,
                        self.input.resolution,
                        item_id,
                    )
                {
                    self.collect_guard_expr_callees_with_const_items(
                        value_expr,
                        queue,
                        visited_items,
                    );
                    return;
                }
                if let Some(value_resolution) = self.input.resolution.expr_resolution(expr_id)
                    && let Some(item_id) =
                        local_item_for_value_resolution(self.input.hir, value_resolution)
                    && visited_items.insert(item_id)
                {
                    match &self.input.hir.item(item_id).kind {
                        ItemKind::Function(_) => queue.push_back(FunctionRef::Item(item_id)),
                        ItemKind::Const(global) | ItemKind::Static(global) => {
                            self.collect_guard_expr_callees_with_const_items(
                                global.value,
                                queue,
                                visited_items,
                            );
                        }
                        _ => {}
                    }
                    visited_items.remove(&item_id);
                }
            }
            hir::ExprKind::Integer(_)
            | hir::ExprKind::String { .. }
            | hir::ExprKind::Bool(_)
            | hir::ExprKind::NoneLiteral => {}
        }
    }

    fn collect_guard_block_callees(
        &self,
        block_id: hir::BlockId,
        queue: &mut VecDeque<FunctionRef>,
    ) {
        let block = self.input.hir.block(block_id);
        for statement_id in &block.statements {
            self.collect_guard_statement_callees(*statement_id, queue);
        }
        if let Some(tail) = block.tail {
            self.collect_guard_expr_callees(tail, queue);
        }
    }

    fn collect_guard_statement_callees(
        &self,
        statement_id: hir::StmtId,
        queue: &mut VecDeque<FunctionRef>,
    ) {
        match &self.input.hir.stmt(statement_id).kind {
            hir::StmtKind::Expr { expr, .. } => self.collect_guard_expr_callees(*expr, queue),
            hir::StmtKind::Let { value, .. } => self.collect_guard_expr_callees(*value, queue),
            hir::StmtKind::While { condition, body } => {
                self.collect_guard_expr_callees(*condition, queue);
                self.collect_guard_block_callees(*body, queue);
            }
            hir::StmtKind::Loop { body } => {
                self.collect_guard_block_callees(*body, queue);
            }
            hir::StmtKind::For { iterable, body, .. } => {
                self.collect_guard_expr_callees(*iterable, queue);
                self.collect_guard_block_callees(*body, queue);
            }
            hir::StmtKind::Return(_)
            | hir::StmtKind::Defer(_)
            | hir::StmtKind::Break
            | hir::StmtKind::Continue => {}
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
                Param::Receiver(receiver) => {
                    let Some(ty) =
                        receiver_param_type(self.input.hir, self.input.resolution, function_ref)
                    else {
                        diagnostics.push(unsupported(
                            receiver.span,
                            "LLVM IR backend foundation only supports receiver methods with a concrete impl/extend target type",
                        ));
                        continue;
                    };
                    match self.lower_llvm_type(&ty, receiver.span, "receiver parameter type") {
                        Ok(llvm_ty) => params.push(ParamSignature {
                            name: "self".to_owned(),
                            ty,
                            llvm_ty,
                        }),
                        Err(error) => diagnostics.push(error),
                    }
                }
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
            .body_for_owner(function_body_owner(function_ref))
            .ok_or_else(|| {
                vec![
                    Diagnostic::error(format!(
                        "function `{}` has no MIR body to lower",
                        signature.name
                    ))
                    .with_label(Label::new(signature.span)),
                ]
            })?;

        self.prepare_body(signature, body)
    }

    fn prepare_closure(
        &self,
        root_function_ref: FunctionRef,
        parent_llvm_name: &str,
        parent: &PreparedFunction,
        closure_id: mir::ClosureId,
        closure: &mir::ClosureDecl,
    ) -> Result<PreparedFunction, Vec<Diagnostic>> {
        let body = closure.lowered_body.as_deref().ok_or_else(|| {
            vec![unsupported(
                closure.span,
                "LLVM IR backend foundation currently only supports a narrow non-`move` capturing-closure subset with immutable same-function scalar, `String`, and task-handle captures",
            )]
        })?;
        let closure_ty = self
            .input
            .typeck
            .expr_ty(closure.expr)
            .cloned()
            .ok_or_else(|| {
                vec![unsupported(
                    closure.span,
                    "LLVM IR backend foundation could not resolve the callable type for this closure value",
                )]
            })?;
        let Ty::Callable { params, ret } = closure_ty else {
            return Err(vec![unsupported(
                closure.span,
                "LLVM IR backend foundation expected closures to lower as callable values",
            )]);
        };
        if params.len() != closure.param_locals.len() {
            return Err(vec![unsupported(
                closure.span,
                "LLVM IR backend foundation encountered a closure whose lowered parameter list no longer matches the inferred callable arity",
            )]);
        }

        let return_ty = ret.as_ref().clone();
        let return_llvm_ty =
            match self.lower_llvm_type(&return_ty, closure.span, "closure return type") {
                Ok(llvm_ty) => llvm_ty,
                Err(error) => return Err(vec![error]),
            };
        let mut param_signatures =
            Vec::with_capacity(closure.capture_binding_locals.len() + params.len());
        for (capture_local, hir_local) in closure
            .captures
            .iter()
            .zip(closure.capture_binding_locals.iter())
        {
            let Some(ty) = parent.local_types.get(&capture_local.local) else {
                return Err(vec![unsupported(
                    closure.span,
                    "LLVM IR backend foundation could not resolve a captured local type for this closure value",
                )]);
            };
            let local = self.input.hir.local(*hir_local);
            let llvm_ty = match self.lower_llvm_type(ty, local.span, "closure capture type") {
                Ok(llvm_ty) => llvm_ty,
                Err(error) => return Err(vec![error]),
            };
            param_signatures.push(ParamSignature {
                name: format!("capture_{}", local.name),
                ty: ty.clone(),
                llvm_ty,
            });
        }
        for (local_id, ty) in closure.param_locals.iter().zip(params.iter()) {
            let local = self.input.hir.local(*local_id);
            let llvm_ty = match self.lower_llvm_type(ty, local.span, "closure parameter type") {
                Ok(llvm_ty) => llvm_ty,
                Err(error) => return Err(vec![error]),
            };
            param_signatures.push(ParamSignature {
                name: local.name.clone(),
                ty: ty.clone(),
                llvm_ty,
            });
        }

        let mut prepared = self.prepare_body(
            FunctionSignature {
                function_ref: root_function_ref,
                name: format!("{}::closure{}", parent_llvm_name, closure_id.index()),
                llvm_name: closure_llvm_name(parent_llvm_name, closure_id),
                span: closure.span,
                body_return_ty: return_ty.clone(),
                body_return_llvm_ty: return_llvm_ty.clone(),
                return_ty,
                return_llvm_ty,
                params: param_signatures,
                body_style: FunctionBodyStyle::Definition,
                is_async: false,
                async_body_llvm_name: None,
                async_frame_layout: None,
                async_result_layout: None,
            },
            body,
        )?;
        prepared.param_binding_locals = closure
            .capture_binding_locals
            .iter()
            .chain(closure.param_locals.iter())
            .filter_map(|local_id| mir_local_for_hir_local(&prepared.body, *local_id))
            .collect();
        Ok(prepared)
    }

    fn prepare_const_closure(
        &self,
        item_id: ItemId,
        closure_expr: hir::ExprId,
    ) -> Result<PreparedFunction, Vec<Diagnostic>> {
        let (ItemKind::Const(global) | ItemKind::Static(global)) =
            &self.input.hir.item(item_id).kind
        else {
            return Err(vec![unsupported(
                self.input.hir.item(item_id).span,
                "LLVM IR backend foundation expected closure-backed callable const/static items",
            )]);
        };
        let hir::ExprKind::Closure { params, body, .. } = &self.input.hir.expr(closure_expr).kind
        else {
            return Err(vec![unsupported(
                global.span,
                "LLVM IR backend foundation expected closure-backed callable const/static items",
            )]);
        };
        let closure_ty = self
            .input
            .typeck
            .expr_ty(closure_expr)
            .cloned()
            .ok_or_else(|| {
                vec![unsupported(
                    global.span,
                    "LLVM IR backend foundation could not resolve the callable type for this const/static closure value",
                )]
            })?;
        let Ty::Callable {
            params: param_tys,
            ret,
        } = closure_ty
        else {
            return Err(vec![unsupported(
                global.span,
                "LLVM IR backend foundation expected callable const/static closure values to lower as callable values",
            )]);
        };
        if param_tys.len() != params.len() {
            return Err(vec![unsupported(
                global.span,
                "LLVM IR backend foundation encountered a const/static closure whose lowered parameter list no longer matches the inferred callable arity",
            )]);
        }

        let return_ty = ret.as_ref().clone();
        let return_llvm_ty =
            match self.lower_llvm_type(&return_ty, global.span, "closure return type") {
                Ok(llvm_ty) => llvm_ty,
                Err(error) => return Err(vec![error]),
            };
        let mut param_signatures = Vec::with_capacity(params.len());
        for (&local_id, ty) in params.iter().zip(param_tys.iter()) {
            let local = self.input.hir.local(local_id);
            let llvm_ty = match self.lower_llvm_type(ty, local.span, "closure parameter type") {
                Ok(llvm_ty) => llvm_ty,
                Err(error) => return Err(vec![error]),
            };
            param_signatures.push(ParamSignature {
                name: local.name.clone(),
                ty: ty.clone(),
                llvm_ty,
            });
        }

        let llvm_name = format!("{}__closure0", llvm_symbol_name(item_id, &global.name));
        let body = ql_mir::lower_standalone_non_capturing_closure_body(
            self.input.hir,
            self.input.resolution,
            self.input.typeck,
            BodyOwner::Item(item_id),
            format!("{}::closure0", global.name),
            self.input.hir.expr(closure_expr).span,
            params.clone(),
            *body,
        );
        let mut prepared = self.prepare_body(
            FunctionSignature {
                function_ref: FunctionRef::Item(item_id),
                name: format!("{}::closure0", global.name),
                llvm_name,
                span: self.input.hir.expr(closure_expr).span,
                body_return_ty: return_ty.clone(),
                body_return_llvm_ty: return_llvm_ty.clone(),
                return_ty,
                return_llvm_ty,
                params: param_signatures,
                body_style: FunctionBodyStyle::Definition,
                is_async: false,
                async_body_llvm_name: None,
                async_frame_layout: None,
                async_result_layout: None,
            },
            &body,
        )?;
        prepared.param_binding_locals = params
            .iter()
            .filter_map(|local_id| mir_local_for_hir_local(&prepared.body, *local_id))
            .collect();
        Ok(prepared)
    }

    fn collect_prepared_closures(
        &self,
        root_function_ref: FunctionRef,
        parent_llvm_name: &str,
        parent: &PreparedFunction,
        prepared: &mut Vec<PreparedFunction>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for closure_id in parent.body.closure_ids() {
            let closure = parent.body.closure(closure_id);
            let Some(_) = closure.lowered_body.as_ref() else {
                continue;
            };
            match self.prepare_closure(
                root_function_ref,
                parent_llvm_name,
                parent,
                closure_id,
                closure,
            ) {
                Ok(prepared_closure) => {
                    self.collect_prepared_closures(
                        root_function_ref,
                        &prepared_closure.signature.llvm_name,
                        &prepared_closure,
                        prepared,
                        diagnostics,
                    );
                    prepared.push(prepared_closure);
                }
                Err(mut errors) => diagnostics.append(&mut errors),
            }
        }
    }

    fn capturing_closure_diagnostic(&self, span: Span) -> Diagnostic {
        unsupported(
            span,
            "LLVM IR backend foundation currently only supports a narrow non-`move` capturing-closure subset: immutable same-function scalar, `String`, and task-handle captures through the currently shipped ordinary/control-flow and cleanup/guard-call roots",
        )
    }

    fn collect_supported_direct_local_capturing_closures(
        &self,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
        supported_matches: &HashMap<mir::BasicBlockId, SupportedMatchLowering>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> (
        HashMap<mir::LocalId, mir::ClosureId>,
        HashMap<(mir::BasicBlockId, mir::LocalId), SupportedOrdinaryCapturingClosureCall>,
    ) {
        let mut staged = HashMap::new();
        let mut supported = HashMap::new();
        let mut closure_spans = HashMap::new();
        let mut conflicting_temps = HashSet::new();

        for statement in body.statements() {
            let StatementKind::Assign { place, value } = &statement.kind else {
                continue;
            };
            if !place.projections.is_empty() {
                continue;
            }
            let Rvalue::Closure { closure } = value else {
                continue;
            };
            let closure_decl = body.closure(*closure);
            if closure_decl.capture_binding_locals.is_empty() {
                continue;
            }

            let closure_supported = closure_decl.captures.iter().all(|capture| {
                local_types
                    .get(&capture.local)
                    .is_some_and(is_supported_capture_ty)
            });
            if !closure_supported {
                diagnostics.push(self.capturing_closure_diagnostic(closure_decl.span));
                continue;
            }

            if staged.insert(place.base, *closure).is_some() {
                diagnostics.push(self.capturing_closure_diagnostic(closure_decl.span));
                continue;
            }
            closure_spans.insert(place.base, closure_decl.span);
        }

        for statement in body.statements() {
            match &statement.kind {
                StatementKind::Assign { place, value } => {
                    let forwarded_closure = if place.projections.is_empty() {
                        direct_local_capturing_closure_assignment_source(
                            value,
                            &staged,
                            &supported,
                            &closure_spans,
                        )
                    } else {
                        None
                    };
                    let temp_forwarded_closure =
                        if matches!(body.local(place.base).origin, LocalOrigin::Temp { .. }) {
                            forwarded_closure
                        } else {
                            None
                        };
                    let same_supported_forwarded_closure = matches!(
                        forwarded_closure,
                        Some((closure_id, _))
                            if supported
                                .get(&place.base)
                                .is_some_and(|current| *current == closure_id)
                    );
                    let supported_forwarded_closure = if temp_forwarded_closure.is_none() {
                        forwarded_closure.filter(|_| supported.contains_key(&place.base))
                    } else {
                        None
                    };
                    if supported.contains_key(&place.base)
                        && !matches!(value, Rvalue::Closure { .. })
                        && !same_supported_forwarded_closure
                        && temp_forwarded_closure.is_none()
                        && supported_forwarded_closure.is_none()
                    {
                        diagnostics.push(
                            self.capturing_closure_diagnostic(
                                *closure_spans
                                    .get(&place.base)
                                    .expect("capturing closure local should preserve its span"),
                            ),
                        );
                    }
                    if same_supported_forwarded_closure {
                        continue;
                    }
                    if let Some((closure_id, closure_span)) = temp_forwarded_closure {
                        match supported.get(&place.base).copied() {
                            Some(current) if current != closure_id => {
                                supported.remove(&place.base);
                                conflicting_temps.insert(place.base);
                                closure_spans.entry(place.base).or_insert(closure_span);
                            }
                            Some(_) => {}
                            None if !conflicting_temps.contains(&place.base) => {
                                supported.insert(place.base, closure_id);
                                closure_spans.insert(place.base, closure_span);
                            }
                            None => {}
                        }
                        continue;
                    }
                    if let Some((closure_id, closure_span)) = supported_forwarded_closure {
                        supported.insert(place.base, closure_id);
                        closure_spans.insert(place.base, closure_span);
                        continue;
                    }
                    self.validate_direct_local_capturing_closure_rvalue(
                        value,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                StatementKind::BindPattern {
                    pattern, source, ..
                } => {
                    if let Operand::Place(place) = source
                        && place.projections.is_empty()
                    {
                        if let Some(closure_id) = staged
                            .get(&place.base)
                            .copied()
                            .or_else(|| supported.get(&place.base).copied())
                        {
                            let closure_span = *closure_spans.get(&place.base).expect(
                                "capturing closure source local should preserve its source span",
                            );
                            let Some(binding_local) =
                                self.binding_local_for_pattern(body, *pattern)
                            else {
                                diagnostics.push(self.capturing_closure_diagnostic(closure_span));
                                continue;
                            };
                            if supported.insert(binding_local, closure_id).is_some() {
                                diagnostics.push(self.capturing_closure_diagnostic(closure_span));
                            } else {
                                closure_spans.insert(binding_local, closure_span);
                            }
                            continue;
                        }
                    }
                    self.validate_direct_local_capturing_closure_operand(
                        source,
                        false,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                StatementKind::Eval { value } => {
                    self.validate_direct_local_capturing_closure_rvalue(
                        value,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                StatementKind::RegisterCleanup { cleanup }
                | StatementKind::RunCleanup { cleanup } => {
                    let mir::CleanupKind::Defer { expr } = &body.cleanup(*cleanup).kind;
                    if self.cleanup_expr_mentions_supported_capturing_closure_local(
                        *expr, body, &supported,
                    ) {
                        diagnostics
                            .push(self.capturing_closure_diagnostic(body.cleanup(*cleanup).span));
                    }
                }
                StatementKind::StorageLive { .. } | StatementKind::StorageDead { .. } => {}
            }
        }

        for block in body.blocks() {
            match &block.terminator.kind {
                TerminatorKind::Branch { condition, .. } => {
                    self.validate_direct_local_capturing_closure_operand(
                        condition,
                        false,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::Match { scrutinee, .. } => {
                    self.validate_direct_local_capturing_closure_operand(
                        scrutinee,
                        false,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::ForLoop { iterable, .. } => {
                    self.validate_direct_local_capturing_closure_operand(
                        iterable,
                        false,
                        &supported,
                        &closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::Goto { .. }
                | TerminatorKind::Return
                | TerminatorKind::Terminate => {}
            }
        }

        let ordinary_control_flow_capturing_closure_calls = self
            .collect_supported_ordinary_control_flow_capturing_closure_calls(
                body,
                supported_matches,
                &staged,
                &supported,
                &mut closure_spans,
                &mut conflicting_temps,
            );
        let mut ordinary_control_flow_capturing_closure_calls =
            ordinary_control_flow_capturing_closure_calls;
        self.propagate_supported_ordinary_control_flow_capturing_closure_calls(
            body,
            &mut ordinary_control_flow_capturing_closure_calls,
            &mut conflicting_temps,
            &mut closure_spans,
        );
        self.validate_ordinary_control_flow_capturing_closure_candidates(
            body,
            &ordinary_control_flow_capturing_closure_calls,
            &conflicting_temps,
            &closure_spans,
            diagnostics,
        );

        (supported, ordinary_control_flow_capturing_closure_calls)
    }

    fn validate_direct_local_capturing_closure_rvalue(
        &self,
        value: &Rvalue,
        supported: &HashMap<mir::LocalId, mir::ClosureId>,
        closure_spans: &HashMap<mir::LocalId, Span>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match value {
            Rvalue::Use(operand) | Rvalue::Question(operand) | Rvalue::Unary { operand, .. } => {
                self.validate_direct_local_capturing_closure_operand(
                    operand,
                    false,
                    supported,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::Tuple(items) | Rvalue::Array(items) => {
                for item in items {
                    self.validate_direct_local_capturing_closure_operand(
                        item,
                        false,
                        supported,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::RepeatArray { value, .. } => {
                self.validate_direct_local_capturing_closure_operand(
                    value,
                    false,
                    supported,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::AggregateTupleStruct { items, .. } => {
                for item in items {
                    self.validate_direct_local_capturing_closure_operand(
                        item,
                        false,
                        supported,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::AggregateStruct { fields, .. } => {
                for field in fields {
                    self.validate_direct_local_capturing_closure_operand(
                        &field.value,
                        false,
                        supported,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::Call { callee, args } => {
                self.validate_direct_local_capturing_closure_operand(
                    callee,
                    true,
                    supported,
                    closure_spans,
                    diagnostics,
                );
                for arg in args {
                    self.validate_direct_local_capturing_closure_operand(
                        &arg.value,
                        false,
                        supported,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::Binary { left, right, .. } => {
                self.validate_direct_local_capturing_closure_operand(
                    left,
                    false,
                    supported,
                    closure_spans,
                    diagnostics,
                );
                self.validate_direct_local_capturing_closure_operand(
                    right,
                    false,
                    supported,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::Closure { .. } | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn validate_direct_local_capturing_closure_operand(
        &self,
        operand: &Operand,
        allow_direct_call_callee: bool,
        supported: &HashMap<mir::LocalId, mir::ClosureId>,
        closure_spans: &HashMap<mir::LocalId, Span>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Operand::Place(place) = operand else {
            return;
        };
        if place.projections.is_empty()
            && supported.contains_key(&place.base)
            && !allow_direct_call_callee
        {
            diagnostics.push(
                self.capturing_closure_diagnostic(
                    *closure_spans
                        .get(&place.base)
                        .expect("capturing closure local should preserve its span"),
                ),
            );
        }
        for projection in &place.projections {
            if let mir::ProjectionElem::Index(index) = projection {
                self.validate_direct_local_capturing_closure_operand(
                    index,
                    false,
                    supported,
                    closure_spans,
                    diagnostics,
                );
            }
        }
    }

    fn collect_supported_ordinary_control_flow_capturing_closure_calls(
        &self,
        body: &mir::MirBody,
        supported_matches: &HashMap<mir::BasicBlockId, SupportedMatchLowering>,
        staged: &HashMap<mir::LocalId, mir::ClosureId>,
        supported: &HashMap<mir::LocalId, mir::ClosureId>,
        closure_spans: &mut HashMap<mir::LocalId, Span>,
        conflicting_temps: &mut HashSet<mir::LocalId>,
    ) -> HashMap<(mir::BasicBlockId, mir::LocalId), SupportedOrdinaryCapturingClosureCall> {
        let mut ordinary_calls = HashMap::new();

        for block_id in body.block_ids() {
            let block = body.block(block_id);
            match &block.terminator.kind {
                TerminatorKind::Branch {
                    condition,
                    then_target,
                    else_target,
                } => {
                    let Some((join_target, local, then_closure)) =
                        control_flow_capturing_closure_block_assignment(
                            body,
                            *then_target,
                            staged,
                            supported,
                            closure_spans,
                        )
                    else {
                        continue;
                    };
                    let Some((other_join_target, other_local, else_closure)) =
                        control_flow_capturing_closure_block_assignment(
                            body,
                            *else_target,
                            staged,
                            supported,
                            closure_spans,
                        )
                    else {
                        continue;
                    };
                    if join_target != other_join_target
                        || local != other_local
                        || !conflicting_temps.contains(&local)
                    {
                        continue;
                    }
                    self.record_supported_ordinary_control_flow_capturing_closure_call(
                        body,
                        join_target,
                        local,
                        SupportedOrdinaryCapturingClosureCall::Branch {
                            condition: condition.clone(),
                            then_closure,
                            else_closure,
                        },
                        &mut ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                    );
                }
                TerminatorKind::Match { scrutinee, .. } => {
                    let Some(match_lowering) = supported_matches.get(&block_id) else {
                        continue;
                    };
                    match match_lowering {
                        SupportedMatchLowering::Bool {
                            true_target,
                            false_target,
                        } => {
                            let Some((join_target, local, true_closure)) =
                                control_flow_capturing_closure_block_assignment(
                                    body,
                                    *true_target,
                                    staged,
                                    supported,
                                    closure_spans,
                                )
                            else {
                                continue;
                            };
                            let Some((other_join_target, other_local, false_closure)) =
                                control_flow_capturing_closure_block_assignment(
                                    body,
                                    *false_target,
                                    staged,
                                    supported,
                                    closure_spans,
                                )
                            else {
                                continue;
                            };
                            if join_target != other_join_target
                                || local != other_local
                                || !conflicting_temps.contains(&local)
                            {
                                continue;
                            }
                            self.record_supported_ordinary_control_flow_capturing_closure_call(
                                body,
                                join_target,
                                local,
                                SupportedOrdinaryCapturingClosureCall::BoolMatch {
                                    scrutinee: scrutinee.clone(),
                                    true_closure,
                                    false_closure,
                                },
                                &mut ordinary_calls,
                                conflicting_temps,
                                closure_spans,
                            );
                        }
                        SupportedMatchLowering::Integer {
                            arms,
                            fallback_target,
                        } => {
                            let Some((join_target, local, fallback_closure)) =
                                control_flow_capturing_closure_block_assignment(
                                    body,
                                    *fallback_target,
                                    staged,
                                    supported,
                                    closure_spans,
                                )
                            else {
                                continue;
                            };
                            if !conflicting_temps.contains(&local) {
                                continue;
                            }

                            let mut lowered_arms = Vec::with_capacity(arms.len());
                            let mut supported_join = true;
                            for arm in arms {
                                let Some((arm_join_target, arm_local, arm_closure)) =
                                    control_flow_capturing_closure_block_assignment(
                                        body,
                                        arm.target,
                                        staged,
                                        supported,
                                        closure_spans,
                                    )
                                else {
                                    supported_join = false;
                                    break;
                                };
                                if arm_join_target != join_target || arm_local != local {
                                    supported_join = false;
                                    break;
                                }
                                lowered_arms.push(
                                    SupportedOrdinaryIntegerCapturingClosureCallArm {
                                        value: arm.value.clone(),
                                        closure_id: arm_closure,
                                    },
                                );
                            }
                            if !supported_join {
                                continue;
                            }

                            self.record_supported_ordinary_control_flow_capturing_closure_call(
                                body,
                                join_target,
                                local,
                                SupportedOrdinaryCapturingClosureCall::IntegerMatch {
                                    scrutinee: scrutinee.clone(),
                                    arms: lowered_arms,
                                    fallback_closure,
                                },
                                &mut ordinary_calls,
                                conflicting_temps,
                                closure_spans,
                            );
                        }
                        SupportedMatchLowering::String {
                            arms,
                            fallback_target,
                        } => {
                            let Some((join_target, local, fallback_closure)) =
                                control_flow_capturing_closure_block_assignment(
                                    body,
                                    *fallback_target,
                                    staged,
                                    supported,
                                    closure_spans,
                                )
                            else {
                                continue;
                            };
                            if !conflicting_temps.contains(&local) {
                                continue;
                            }

                            let mut lowered_arms = Vec::with_capacity(arms.len());
                            let mut supported_join = true;
                            for arm in arms {
                                let Some((arm_join_target, arm_local, arm_closure)) =
                                    control_flow_capturing_closure_block_assignment(
                                        body,
                                        arm.target,
                                        staged,
                                        supported,
                                        closure_spans,
                                    )
                                else {
                                    supported_join = false;
                                    break;
                                };
                                if arm_join_target != join_target || arm_local != local {
                                    supported_join = false;
                                    break;
                                }
                                lowered_arms.push(SupportedOrdinaryStringCapturingClosureCallArm {
                                    value: arm.value.clone(),
                                    closure_id: arm_closure,
                                });
                            }
                            if !supported_join {
                                continue;
                            }

                            self.record_supported_ordinary_control_flow_capturing_closure_call(
                                body,
                                join_target,
                                local,
                                SupportedOrdinaryCapturingClosureCall::StringMatch {
                                    scrutinee: scrutinee.clone(),
                                    arms: lowered_arms,
                                    fallback_closure,
                                },
                                &mut ordinary_calls,
                                conflicting_temps,
                                closure_spans,
                            );
                        }
                        SupportedMatchLowering::StringGuarded {
                            arms,
                            fallback_target,
                        } => {
                            let Some((join_target, local, fallback_closure)) =
                                control_flow_capturing_closure_block_assignment(
                                    body,
                                    *fallback_target,
                                    staged,
                                    supported,
                                    closure_spans,
                                )
                            else {
                                continue;
                            };
                            if !conflicting_temps.contains(&local) {
                                continue;
                            }

                            let mut lowered_arms = Vec::with_capacity(arms.len());
                            let mut supported_join = true;
                            for arm in arms {
                                let Some((arm_join_target, arm_local, arm_closure)) =
                                    control_flow_capturing_closure_block_assignment(
                                        body,
                                        arm.target,
                                        staged,
                                        supported,
                                        closure_spans,
                                    )
                                else {
                                    supported_join = false;
                                    break;
                                };
                                if arm_join_target != join_target || arm_local != local {
                                    supported_join = false;
                                    break;
                                }
                                lowered_arms.push(
                                    SupportedOrdinaryGuardedStringCapturingClosureCallArm {
                                        pattern: arm.pattern.clone(),
                                        binding_local: arm.binding_local,
                                        guard: arm.guard,
                                        closure_id: arm_closure,
                                    },
                                );
                            }
                            if !supported_join {
                                continue;
                            }

                            self.record_supported_ordinary_control_flow_capturing_closure_call(
                                body,
                                join_target,
                                local,
                                SupportedOrdinaryCapturingClosureCall::StringGuardedMatch {
                                    scrutinee: scrutinee.clone(),
                                    arms: lowered_arms,
                                    fallback_closure,
                                },
                                &mut ordinary_calls,
                                conflicting_temps,
                                closure_spans,
                            );
                        }
                        SupportedMatchLowering::GuardOnly { .. }
                        | SupportedMatchLowering::BoolGuarded { .. }
                        | SupportedMatchLowering::IntegerGuarded { .. }
                        | SupportedMatchLowering::Enum { .. } => {}
                    }
                }
                TerminatorKind::Goto { .. }
                | TerminatorKind::Return
                | TerminatorKind::Terminate
                | TerminatorKind::ForLoop { .. } => {}
            }
        }

        ordinary_calls
    }

    fn record_supported_ordinary_control_flow_capturing_closure_call(
        &self,
        body: &mir::MirBody,
        join_target: mir::BasicBlockId,
        local: mir::LocalId,
        lowering: SupportedOrdinaryCapturingClosureCall,
        ordinary_calls: &mut HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &mut HashSet<mir::LocalId>,
        closure_spans: &mut HashMap<mir::LocalId, Span>,
    ) {
        ordinary_calls.insert((join_target, local), lowering.clone());

        let Some(closure_span) = closure_spans.get(&local).copied() else {
            return;
        };
        for binding_local in
            self.ordinary_control_flow_capturing_closure_binding_locals(body, join_target, local)
        {
            ordinary_calls.insert((join_target, binding_local), lowering.clone());
            conflicting_temps.insert(binding_local);
            closure_spans.insert(binding_local, closure_span);
        }
    }

    fn ordinary_control_flow_capturing_closure_binding_locals(
        &self,
        body: &mir::MirBody,
        block_id: mir::BasicBlockId,
        source_local: mir::LocalId,
    ) -> Vec<mir::LocalId> {
        let mut binding_locals = Vec::new();
        for statement_id in &body.block(block_id).statements {
            let statement = body.statement(*statement_id);
            let StatementKind::BindPattern {
                pattern, source, ..
            } = &statement.kind
            else {
                continue;
            };
            let Operand::Place(place) = source else {
                continue;
            };
            if !place.projections.is_empty() || place.base != source_local {
                continue;
            }
            let Some(binding_local) = self.binding_local_for_pattern(body, *pattern) else {
                continue;
            };
            binding_locals.push(binding_local);
        }
        binding_locals
    }

    fn validate_ordinary_control_flow_capturing_closure_candidates(
        &self,
        body: &mir::MirBody,
        ordinary_calls: &HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &HashSet<mir::LocalId>,
        closure_spans: &HashMap<mir::LocalId, Span>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for block_id in body.block_ids() {
            let block = body.block(block_id);
            for statement_id in &block.statements {
                let statement = body.statement(*statement_id);
                match &statement.kind {
                    StatementKind::Assign { value, .. } | StatementKind::Eval { value } => {
                        self.validate_ordinary_control_flow_capturing_closure_rvalue(
                            block_id,
                            value,
                            ordinary_calls,
                            conflicting_temps,
                            closure_spans,
                            diagnostics,
                        );
                    }
                    StatementKind::BindPattern {
                        pattern, source, ..
                    } => {
                        let supported_binding = if let Operand::Place(place) = source {
                            place.projections.is_empty()
                                && conflicting_temps.contains(&place.base)
                                && self.binding_local_for_pattern(body, *pattern).is_some_and(
                                    |binding_local| {
                                        ordinary_calls.contains_key(&(block_id, binding_local))
                                    },
                                )
                        } else {
                            false
                        };
                        if supported_binding {
                            continue;
                        }
                        self.validate_ordinary_control_flow_capturing_closure_operand(
                            block_id,
                            source,
                            false,
                            ordinary_calls,
                            conflicting_temps,
                            closure_spans,
                            diagnostics,
                        );
                    }
                    StatementKind::RegisterCleanup { .. }
                    | StatementKind::RunCleanup { .. }
                    | StatementKind::StorageLive { .. }
                    | StatementKind::StorageDead { .. } => {}
                }
            }

            match &block.terminator.kind {
                TerminatorKind::Branch { condition, .. } => {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        condition,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::Match { scrutinee, .. } => {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        scrutinee,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::ForLoop { iterable, .. } => {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        iterable,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
                TerminatorKind::Goto { .. }
                | TerminatorKind::Return
                | TerminatorKind::Terminate => {}
            }
        }
    }

    fn propagate_supported_ordinary_control_flow_capturing_closure_calls(
        &self,
        body: &mir::MirBody,
        ordinary_calls: &mut HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &mut HashSet<mir::LocalId>,
        closure_spans: &mut HashMap<mir::LocalId, Span>,
    ) {
        let predecessors = ordinary_control_flow_capturing_closure_predecessors(body);
        if ordinary_calls.is_empty() {
            return;
        }

        loop {
            let mut changed = false;
            let locals = ordinary_calls
                .keys()
                .map(|(_, local)| *local)
                .collect::<HashSet<_>>();
            for block_id in body.block_ids() {
                let Some(block_predecessors) = predecessors.get(&block_id) else {
                    continue;
                };
                if block_predecessors.is_empty() {
                    continue;
                }

                for local in &locals {
                    if ordinary_calls.contains_key(&(block_id, *local))
                        || ordinary_control_flow_capturing_closure_block_reassigns_local(
                            self, body, block_id, *local,
                        )
                    {
                        continue;
                    }

                    let mut propagated = None;
                    for predecessor in block_predecessors {
                        let Some(lowering) = ordinary_calls.get(&(*predecessor, *local)) else {
                            propagated = None;
                            break;
                        };
                        match &propagated {
                            Some(current) if current != lowering => {
                                propagated = None;
                                break;
                            }
                            Some(_) => {}
                            None => propagated = Some(lowering.clone()),
                        }
                    }

                    if let Some(lowering) = propagated {
                        ordinary_calls.insert((block_id, *local), lowering);
                        changed = true;
                    }
                }
            }

            for block_id in body.block_ids() {
                if self.propagate_supported_ordinary_control_flow_capturing_closure_block_bindings(
                    body,
                    block_id,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                ) {
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }
    }

    fn propagate_supported_ordinary_control_flow_capturing_closure_block_bindings(
        &self,
        body: &mir::MirBody,
        block_id: mir::BasicBlockId,
        ordinary_calls: &mut HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &mut HashSet<mir::LocalId>,
        closure_spans: &mut HashMap<mir::LocalId, Span>,
    ) -> bool {
        let mut changed = false;

        loop {
            let mut block_changed = false;
            for statement_id in &body.block(block_id).statements {
                let statement = body.statement(*statement_id);
                let StatementKind::BindPattern {
                    pattern, source, ..
                } = &statement.kind
                else {
                    continue;
                };
                let Operand::Place(place) = source else {
                    continue;
                };
                if !place.projections.is_empty() {
                    continue;
                }
                let Some(lowering) = ordinary_calls.get(&(block_id, place.base)).cloned() else {
                    continue;
                };
                let Some(binding_local) = self.binding_local_for_pattern(body, *pattern) else {
                    continue;
                };
                if ordinary_calls.contains_key(&(block_id, binding_local)) {
                    continue;
                }

                ordinary_calls.insert((block_id, binding_local), lowering);
                conflicting_temps.insert(binding_local);
                if let Some(closure_span) = closure_spans.get(&place.base).copied() {
                    closure_spans.insert(binding_local, closure_span);
                }
                block_changed = true;
                changed = true;
            }

            if !block_changed {
                break;
            }
        }

        changed
    }

    fn validate_ordinary_control_flow_capturing_closure_rvalue(
        &self,
        block_id: mir::BasicBlockId,
        value: &Rvalue,
        ordinary_calls: &HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &HashSet<mir::LocalId>,
        closure_spans: &HashMap<mir::LocalId, Span>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        match value {
            Rvalue::Use(operand) | Rvalue::Question(operand) | Rvalue::Unary { operand, .. } => {
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    operand,
                    false,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::Tuple(items) | Rvalue::Array(items) => {
                for item in items {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        item,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::RepeatArray { value, .. } => {
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    value,
                    false,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::AggregateTupleStruct { items, .. } => {
                for item in items {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        item,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::AggregateStruct { fields, .. } => {
                for field in fields {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        &field.value,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::Call { callee, args } => {
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    callee,
                    true,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
                for arg in args {
                    self.validate_ordinary_control_flow_capturing_closure_operand(
                        block_id,
                        &arg.value,
                        false,
                        ordinary_calls,
                        conflicting_temps,
                        closure_spans,
                        diagnostics,
                    );
                }
            }
            Rvalue::Binary { left, right, .. } => {
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    left,
                    false,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    right,
                    false,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
            }
            Rvalue::Closure { .. } | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn validate_ordinary_control_flow_capturing_closure_operand(
        &self,
        block_id: mir::BasicBlockId,
        operand: &Operand,
        allow_direct_call_callee: bool,
        ordinary_calls: &HashMap<
            (mir::BasicBlockId, mir::LocalId),
            SupportedOrdinaryCapturingClosureCall,
        >,
        conflicting_temps: &HashSet<mir::LocalId>,
        closure_spans: &HashMap<mir::LocalId, Span>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Operand::Place(place) = operand else {
            return;
        };
        if place.projections.is_empty() && conflicting_temps.contains(&place.base) {
            let supported_direct_call =
                allow_direct_call_callee && ordinary_calls.contains_key(&(block_id, place.base));
            if !supported_direct_call {
                diagnostics.push(
                    self.capturing_closure_diagnostic(
                        *closure_spans
                            .get(&place.base)
                            .expect("capturing closure local should preserve its span"),
                    ),
                );
            }
        }
        for projection in &place.projections {
            if let mir::ProjectionElem::Index(index) = projection {
                self.validate_ordinary_control_flow_capturing_closure_operand(
                    block_id,
                    index,
                    false,
                    ordinary_calls,
                    conflicting_temps,
                    closure_spans,
                    diagnostics,
                );
            }
        }
    }

    fn cleanup_expr_mentions_supported_capturing_closure_local(
        &self,
        expr_id: hir::ExprId,
        body: &mir::MirBody,
        supported: &HashMap<mir::LocalId, mir::ClosureId>,
    ) -> bool {
        let body_binding_locals = supported
            .keys()
            .filter_map(|local_id| match body.local(*local_id).origin {
                LocalOrigin::Binding(hir_local) => Some(hir_local),
                _ => None,
            })
            .collect::<HashSet<_>>();
        cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
            self.input.hir,
            self.input.resolution,
            body,
            supported,
            expr_id,
            &body_binding_locals,
            &[],
        )
    }

    fn collect_const_closure_targets_in_body(
        &self,
        body: &mir::MirBody,
        queue: &mut VecDeque<(ItemId, hir::ExprId)>,
        seen: &mut HashSet<hir::ExprId>,
    ) {
        for block in body.blocks() {
            for statement_id in &block.statements {
                match &body.statement(*statement_id).kind {
                    StatementKind::Assign { value, .. } | StatementKind::Eval { value } => {
                        self.collect_const_closure_targets_in_rvalue(value, queue, seen);
                    }
                    StatementKind::BindPattern { source, .. } => {
                        self.collect_const_closure_targets_in_operand(source, queue, seen);
                    }
                    StatementKind::RegisterCleanup { cleanup } => {
                        let mir::CleanupKind::Defer { expr } = &body.cleanup(*cleanup).kind;
                        let mut visited_items = HashSet::new();
                        self.collect_const_closure_targets_in_expr(
                            *expr,
                            queue,
                            seen,
                            &mut visited_items,
                        );
                    }
                    StatementKind::StorageLive { .. }
                    | StatementKind::StorageDead { .. }
                    | StatementKind::RunCleanup { .. } => {}
                }
            }
            if let TerminatorKind::Match { arms, .. } = &block.terminator.kind {
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        let mut visited_items = HashSet::new();
                        self.collect_const_closure_targets_in_expr(
                            guard,
                            queue,
                            seen,
                            &mut visited_items,
                        );
                    }
                }
            }
        }
    }

    fn collect_const_closure_targets_in_rvalue(
        &self,
        value: &Rvalue,
        queue: &mut VecDeque<(ItemId, hir::ExprId)>,
        seen: &mut HashSet<hir::ExprId>,
    ) {
        match value {
            Rvalue::Use(operand) | Rvalue::Question(operand) => {
                self.collect_const_closure_targets_in_operand(operand, queue, seen);
            }
            Rvalue::Tuple(items) | Rvalue::Array(items) => {
                for item in items {
                    self.collect_const_closure_targets_in_operand(item, queue, seen);
                }
            }
            Rvalue::RepeatArray { value, .. } => {
                self.collect_const_closure_targets_in_operand(value, queue, seen);
            }
            Rvalue::Call { callee, args } => {
                self.collect_const_closure_targets_in_operand(callee, queue, seen);
                for arg in args {
                    self.collect_const_closure_targets_in_operand(&arg.value, queue, seen);
                }
            }
            Rvalue::Binary { left, right, .. } => {
                self.collect_const_closure_targets_in_operand(left, queue, seen);
                self.collect_const_closure_targets_in_operand(right, queue, seen);
            }
            Rvalue::Unary { operand, .. } => {
                self.collect_const_closure_targets_in_operand(operand, queue, seen);
            }
            Rvalue::AggregateTupleStruct { items, .. } => {
                for item in items {
                    self.collect_const_closure_targets_in_operand(item, queue, seen);
                }
            }
            Rvalue::AggregateStruct { fields, .. } => {
                for field in fields {
                    self.collect_const_closure_targets_in_operand(&field.value, queue, seen);
                }
            }
            Rvalue::Closure { .. } | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn collect_const_closure_targets_in_operand(
        &self,
        operand: &Operand,
        queue: &mut VecDeque<(ItemId, hir::ExprId)>,
        seen: &mut HashSet<hir::ExprId>,
    ) {
        match operand {
            Operand::Constant(Constant::Item { item, .. }) => {
                let mut visited_items = HashSet::new();
                if let Some((item_id, closure_expr)) = const_or_static_callable_closure_target(
                    self.input.hir,
                    self.input.resolution,
                    *item,
                    &mut visited_items,
                ) && seen.insert(closure_expr)
                {
                    queue.push_back((item_id, closure_expr));
                }
            }
            Operand::Constant(Constant::Import(path)) => {
                let Some(item_id) = local_item_for_import_path(self.input.hir, path) else {
                    return;
                };
                let mut visited_items = HashSet::new();
                if let Some((item_id, closure_expr)) = const_or_static_callable_closure_target(
                    self.input.hir,
                    self.input.resolution,
                    item_id,
                    &mut visited_items,
                ) && seen.insert(closure_expr)
                {
                    queue.push_back((item_id, closure_expr));
                }
            }
            Operand::Place(_)
            | Operand::Constant(
                Constant::Integer(_)
                | Constant::String { .. }
                | Constant::Bool(_)
                | Constant::None
                | Constant::Void
                | Constant::Function { .. }
                | Constant::UnresolvedName(_),
            ) => {}
        }
    }

    fn collect_const_closure_targets_in_expr(
        &self,
        expr_id: hir::ExprId,
        queue: &mut VecDeque<(ItemId, hir::ExprId)>,
        seen: &mut HashSet<hir::ExprId>,
        visited_items: &mut HashSet<ItemId>,
    ) {
        let expr = self.input.hir.expr(expr_id);
        match &expr.kind {
            hir::ExprKind::Name(_) => {
                let item_id = match self.input.resolution.expr_resolution(expr_id) {
                    Some(ValueResolution::Item(item_id)) => Some(*item_id),
                    Some(ValueResolution::Import(import_binding)) => {
                        local_item_for_import_binding(self.input.hir, import_binding)
                    }
                    _ => None,
                };
                if let Some(item_id) = item_id {
                    if let Some((item_id, closure_expr)) = const_or_static_callable_closure_target(
                        self.input.hir,
                        self.input.resolution,
                        item_id,
                        visited_items,
                    ) && seen.insert(closure_expr)
                    {
                        queue.push_back((item_id, closure_expr));
                    }
                }
            }
            hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => {
                for &item in items {
                    self.collect_const_closure_targets_in_expr(item, queue, seen, visited_items);
                }
            }
            hir::ExprKind::RepeatArray { value, .. } => {
                self.collect_const_closure_targets_in_expr(*value, queue, seen, visited_items);
            }
            hir::ExprKind::Block(block) | hir::ExprKind::Unsafe(block) => {
                self.collect_const_closure_targets_in_block(*block, queue, seen, visited_items)
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.collect_const_closure_targets_in_expr(*condition, queue, seen, visited_items);
                self.collect_const_closure_targets_in_block(
                    *then_branch,
                    queue,
                    seen,
                    visited_items,
                );
                if let Some(other) = else_branch {
                    self.collect_const_closure_targets_in_expr(*other, queue, seen, visited_items);
                }
            }
            hir::ExprKind::Match { value, arms } => {
                self.collect_const_closure_targets_in_expr(*value, queue, seen, visited_items);
                for arm in arms {
                    if let Some(guard) = arm.guard {
                        self.collect_const_closure_targets_in_expr(
                            guard,
                            queue,
                            seen,
                            visited_items,
                        );
                    }
                    self.collect_const_closure_targets_in_expr(
                        arm.body,
                        queue,
                        seen,
                        visited_items,
                    );
                }
            }
            hir::ExprKind::Closure { body, .. } | hir::ExprKind::Question(body) => {
                self.collect_const_closure_targets_in_expr(*body, queue, seen, visited_items);
            }
            hir::ExprKind::Call { callee, args } => {
                self.collect_const_closure_targets_in_expr(*callee, queue, seen, visited_items);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(value) => self
                            .collect_const_closure_targets_in_expr(
                                *value,
                                queue,
                                seen,
                                visited_items,
                            ),
                        hir::CallArg::Named { value, .. } => self
                            .collect_const_closure_targets_in_expr(
                                *value,
                                queue,
                                seen,
                                visited_items,
                            ),
                    }
                }
            }
            hir::ExprKind::Member { object, .. } => {
                self.collect_const_closure_targets_in_expr(*object, queue, seen, visited_items);
            }
            hir::ExprKind::Bracket { target, items } => {
                self.collect_const_closure_targets_in_expr(*target, queue, seen, visited_items);
                for &item in items {
                    self.collect_const_closure_targets_in_expr(item, queue, seen, visited_items);
                }
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                for field in fields {
                    self.collect_const_closure_targets_in_expr(
                        field.value,
                        queue,
                        seen,
                        visited_items,
                    );
                }
            }
            hir::ExprKind::Binary { left, right, .. } => {
                self.collect_const_closure_targets_in_expr(*left, queue, seen, visited_items);
                self.collect_const_closure_targets_in_expr(*right, queue, seen, visited_items);
            }
            hir::ExprKind::Unary { expr, .. } => {
                self.collect_const_closure_targets_in_expr(*expr, queue, seen, visited_items);
            }
            hir::ExprKind::Integer(_)
            | hir::ExprKind::String { .. }
            | hir::ExprKind::Bool(_)
            | hir::ExprKind::NoneLiteral => {}
        }
    }

    fn collect_const_closure_targets_in_block(
        &self,
        block_id: hir::BlockId,
        queue: &mut VecDeque<(ItemId, hir::ExprId)>,
        seen: &mut HashSet<hir::ExprId>,
        visited_items: &mut HashSet<ItemId>,
    ) {
        let block = self.input.hir.block(block_id);
        for &stmt_id in &block.statements {
            match &self.input.hir.stmt(stmt_id).kind {
                hir::StmtKind::Let { value, .. } | hir::StmtKind::Defer(value) => {
                    self.collect_const_closure_targets_in_expr(*value, queue, seen, visited_items);
                }
                hir::StmtKind::Return(expr) => {
                    if let Some(expr) = expr {
                        self.collect_const_closure_targets_in_expr(
                            *expr,
                            queue,
                            seen,
                            visited_items,
                        );
                    }
                }
                hir::StmtKind::While { condition, body } => {
                    self.collect_const_closure_targets_in_expr(
                        *condition,
                        queue,
                        seen,
                        visited_items,
                    );
                    self.collect_const_closure_targets_in_block(*body, queue, seen, visited_items);
                }
                hir::StmtKind::Loop { body } => {
                    self.collect_const_closure_targets_in_block(*body, queue, seen, visited_items);
                }
                hir::StmtKind::For { iterable, body, .. } => {
                    self.collect_const_closure_targets_in_expr(
                        *iterable,
                        queue,
                        seen,
                        visited_items,
                    );
                    self.collect_const_closure_targets_in_block(*body, queue, seen, visited_items);
                }
                hir::StmtKind::Expr { expr, .. } => {
                    self.collect_const_closure_targets_in_expr(*expr, queue, seen, visited_items);
                }
                hir::StmtKind::Break | hir::StmtKind::Continue => {}
            }
        }
        if let Some(tail) = block.tail {
            self.collect_const_closure_targets_in_expr(tail, queue, seen, visited_items);
        }
    }

    fn prepare_body(
        &self,
        signature: FunctionSignature,
        body: &mir::MirBody,
    ) -> Result<PreparedFunction, Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        let mut local_types = self.seed_local_types(body, &signature);
        let async_task_handles = self.collect_async_task_handles(body);
        let immutable_place_aliases = collect_immutable_place_aliases(self.input.hir, body);
        let task_handle_place_aliases =
            self.collect_task_handle_place_aliases(body, &local_types, &mut diagnostics);
        let mut supported_for_loops = HashMap::new();
        let mut supported_matches = HashMap::new();
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
                        self.require_supported_bind_pattern(
                            *pattern,
                            statement.span,
                            &mut diagnostics,
                        );
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
                    StatementKind::RegisterCleanup { cleanup } => {
                        if !self.supports_cleanup_action(body.cleanup(*cleanup), body, &local_types)
                        {
                            diagnostics.push(unsupported(
                                statement.span,
                                "LLVM IR backend foundation does not support cleanup lowering yet",
                            ));
                        }
                    }
                    StatementKind::RunCleanup { .. } => {}
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
                TerminatorKind::Match {
                    scrutinee,
                    arms,
                    else_target,
                } => {
                    let mut scratch = Vec::new();
                    let scrutinee_ty = self.infer_operand_type(
                        body,
                        scrutinee,
                        &local_types,
                        &async_task_handles,
                        &mut scratch,
                        block.terminator.span,
                    );
                    let Some(match_lowering) = (match scrutinee_ty {
                        Some(Ty::Builtin(BuiltinType::Bool)) => {
                            let mut true_target = None;
                            let mut false_target = None;
                            let mut ordered_arms = Vec::new();
                            let mut guaranteed_true = false;
                            let mut guaranteed_false = false;
                            let mut dynamic_guard_seen = false;
                            let mut supported = true;
                            for arm in arms {
                                let guard = match arm.guard {
                                    None => SupportedBoolGuard::Always,
                                    Some(guard) => match supported_bool_guard(
                                        self.input.hir,
                                        self.input.resolution,
                                        self.input.typeck,
                                        &self.signatures,
                                        body,
                                        &local_types,
                                        &immutable_place_aliases,
                                        scrutinee,
                                        arm.pattern,
                                        guard,
                                    ) {
                                        Some(SupportedBoolGuardAnalysis::Always) => {
                                            SupportedBoolGuard::Always
                                        }
                                        Some(SupportedBoolGuardAnalysis::Skip) => continue,
                                        Some(SupportedBoolGuardAnalysis::Dynamic(expr_id)) => {
                                            dynamic_guard_seen = true;
                                            SupportedBoolGuard::Dynamic(expr_id)
                                        }
                                        None => {
                                            supported = false;
                                            break;
                                        }
                                    },
                                };
                                let Some(pattern) = supported_bool_match_pattern(
                                    self.input.hir,
                                    self.input.resolution,
                                    arm.pattern,
                                ) else {
                                    supported = false;
                                    break;
                                };

                                if matches!(guard, SupportedBoolGuard::Always) {
                                    match pattern {
                                        SupportedBoolMatchPattern::True => {
                                            true_target.get_or_insert(arm.target);
                                            guaranteed_true = true;
                                        }
                                        SupportedBoolMatchPattern::False => {
                                            false_target.get_or_insert(arm.target);
                                            guaranteed_false = true;
                                        }
                                        SupportedBoolMatchPattern::CatchAll => {
                                            true_target.get_or_insert(arm.target);
                                            false_target.get_or_insert(arm.target);
                                            guaranteed_true = true;
                                            guaranteed_false = true;
                                        }
                                    }
                                }

                                ordered_arms.push(SupportedBoolMatchArm {
                                    pattern,
                                    binding_local: match pattern_kind(self.input.hir, arm.pattern) {
                                        PatternKind::Binding(local) => Some(*local),
                                        _ => None,
                                    },
                                    guard,
                                    target: arm.target,
                                });

                                if dynamic_guard_seen {
                                    if guaranteed_true && guaranteed_false {
                                        break;
                                    }
                                } else if true_target.is_some() && false_target.is_some() {
                                    break;
                                }
                            }

                            if supported {
                                if dynamic_guard_seen {
                                    Some(SupportedMatchLowering::BoolGuarded {
                                        arms: ordered_arms,
                                        fallback_target: *else_target,
                                    })
                                } else {
                                    Some(SupportedMatchLowering::Bool {
                                        true_target: true_target.unwrap_or(*else_target),
                                        false_target: false_target.unwrap_or(*else_target),
                                    })
                                }
                            } else {
                                None
                            }
                        }
                        Some(Ty::Builtin(BuiltinType::Int)) => {
                            let mut lowered_arms = Vec::new();
                            let mut ordered_arms = Vec::new();
                            let mut fallback_target = *else_target;
                            let mut dynamic_guard_seen = false;
                            let mut supported = true;

                            for arm in arms {
                                let guard = match arm.guard {
                                    None => SupportedBoolGuard::Always,
                                    Some(guard) => match supported_bool_guard(
                                        self.input.hir,
                                        self.input.resolution,
                                        self.input.typeck,
                                        &self.signatures,
                                        body,
                                        &local_types,
                                        &immutable_place_aliases,
                                        scrutinee,
                                        arm.pattern,
                                        guard,
                                    ) {
                                        Some(SupportedBoolGuardAnalysis::Always) => {
                                            SupportedBoolGuard::Always
                                        }
                                        Some(SupportedBoolGuardAnalysis::Skip) => continue,
                                        Some(SupportedBoolGuardAnalysis::Dynamic(expr_id)) => {
                                            dynamic_guard_seen = true;
                                            SupportedBoolGuard::Dynamic(expr_id)
                                        }
                                        None => {
                                            supported = false;
                                            break;
                                        }
                                    },
                                };
                                match pattern_kind(self.input.hir, arm.pattern) {
                                    PatternKind::Integer(_) | PatternKind::Path(_) => {
                                        let Some(value) = supported_integer_match_pattern(
                                            self.input.hir,
                                            self.input.resolution,
                                            arm.pattern,
                                        ) else {
                                            supported = false;
                                            break;
                                        };
                                        ordered_arms.push(SupportedGuardedIntegerMatchArm {
                                            pattern: SupportedIntegerMatchPattern::Literal(value),
                                            binding_local: None,
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    PatternKind::Binding(local) => {
                                        if matches!(guard, SupportedBoolGuard::Always) {
                                            fallback_target = arm.target;
                                            break;
                                        }
                                        ordered_arms.push(SupportedGuardedIntegerMatchArm {
                                            pattern: SupportedIntegerMatchPattern::CatchAll,
                                            binding_local: Some(*local),
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    PatternKind::Wildcard => {
                                        if matches!(guard, SupportedBoolGuard::Always) {
                                            fallback_target = arm.target;
                                            break;
                                        }
                                        ordered_arms.push(SupportedGuardedIntegerMatchArm {
                                            pattern: SupportedIntegerMatchPattern::CatchAll,
                                            binding_local: None,
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    _ => {
                                        supported = false;
                                        break;
                                    }
                                }
                            }

                            if supported {
                                if dynamic_guard_seen {
                                    Some(SupportedMatchLowering::IntegerGuarded {
                                        arms: ordered_arms,
                                        fallback_target,
                                    })
                                } else {
                                    lowered_arms.extend(ordered_arms.into_iter().map(|arm| {
                                        SupportedIntegerMatchArm {
                                            value: match arm.pattern {
                                                SupportedIntegerMatchPattern::Literal(value) => value,
                                                SupportedIntegerMatchPattern::CatchAll => unreachable!(
                                                    "non-dynamic integer match lowering should stop at the first unguarded catch-all arm"
                                                ),
                                            },
                                            target: arm.target,
                                        }
                                    }));
                                    Some(SupportedMatchLowering::Integer {
                                        arms: lowered_arms,
                                        fallback_target,
                                    })
                                }
                            } else {
                                None
                            }
                        }
                        Some(Ty::Builtin(BuiltinType::String)) => {
                            let mut lowered_arms = Vec::new();
                            let mut ordered_arms = Vec::new();
                            let mut fallback_target = *else_target;
                            let mut dynamic_guard_seen = false;
                            let mut supported = true;

                            for arm in arms {
                                let guard = match arm.guard {
                                    None => SupportedBoolGuard::Always,
                                    Some(guard) => match supported_bool_guard(
                                        self.input.hir,
                                        self.input.resolution,
                                        self.input.typeck,
                                        &self.signatures,
                                        body,
                                        &local_types,
                                        &immutable_place_aliases,
                                        scrutinee,
                                        arm.pattern,
                                        guard,
                                    ) {
                                        Some(SupportedBoolGuardAnalysis::Always) => {
                                            SupportedBoolGuard::Always
                                        }
                                        Some(SupportedBoolGuardAnalysis::Skip) => continue,
                                        Some(SupportedBoolGuardAnalysis::Dynamic(expr_id)) => {
                                            dynamic_guard_seen = true;
                                            SupportedBoolGuard::Dynamic(expr_id)
                                        }
                                        None => {
                                            supported = false;
                                            break;
                                        }
                                    },
                                };
                                match pattern_kind(self.input.hir, arm.pattern) {
                                    PatternKind::String(_) | PatternKind::Path(_) => {
                                        let Some(value) = supported_string_match_pattern(
                                            self.input.hir,
                                            self.input.resolution,
                                            arm.pattern,
                                        ) else {
                                            supported = false;
                                            break;
                                        };
                                        ordered_arms.push(SupportedGuardedStringMatchArm {
                                            pattern: SupportedStringMatchPattern::Literal(value),
                                            binding_local: None,
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    PatternKind::Binding(local) => {
                                        if matches!(guard, SupportedBoolGuard::Always) {
                                            fallback_target = arm.target;
                                            break;
                                        }
                                        ordered_arms.push(SupportedGuardedStringMatchArm {
                                            pattern: SupportedStringMatchPattern::CatchAll,
                                            binding_local: Some(*local),
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    PatternKind::Wildcard => {
                                        if matches!(guard, SupportedBoolGuard::Always) {
                                            fallback_target = arm.target;
                                            break;
                                        }
                                        ordered_arms.push(SupportedGuardedStringMatchArm {
                                            pattern: SupportedStringMatchPattern::CatchAll,
                                            binding_local: None,
                                            guard,
                                            target: arm.target,
                                        });
                                    }
                                    _ => {
                                        supported = false;
                                        break;
                                    }
                                }
                            }

                            if supported {
                                if dynamic_guard_seen {
                                    Some(SupportedMatchLowering::StringGuarded {
                                        arms: ordered_arms,
                                        fallback_target,
                                    })
                                } else {
                                    lowered_arms.extend(ordered_arms.into_iter().map(|arm| {
                                        SupportedStringMatchArm {
                                            value: match arm.pattern {
                                                SupportedStringMatchPattern::Literal(value) => value,
                                                SupportedStringMatchPattern::CatchAll => unreachable!(
                                                    "non-dynamic string match lowering should stop at the first unguarded catch-all arm"
                                                ),
                                            },
                                            target: arm.target,
                                        }
                                    }));
                                    Some(SupportedMatchLowering::String {
                                        arms: lowered_arms,
                                        fallback_target,
                                    })
                                }
                            } else {
                                None
                            }
                        }
                        Some(scrutinee_ty) => {
                            if matches!(
                                scrutinee_ty,
                                Ty::Item { item_id, .. }
                                    if matches!(&self.input.hir.item(item_id).kind, ItemKind::Enum(_))
                            ) {
                                let mut ordered_arms = Vec::new();
                                let mut fallback_target = *else_target;
                                let mut supported = true;

                                for arm in arms {
                                    let guard = match arm.guard {
                                        None => SupportedBoolGuard::Always,
                                        Some(guard) => match supported_bool_guard(
                                            self.input.hir,
                                            self.input.resolution,
                                            self.input.typeck,
                                            &self.signatures,
                                            body,
                                            &local_types,
                                            &immutable_place_aliases,
                                            scrutinee,
                                            arm.pattern,
                                            guard,
                                        ) {
                                            Some(SupportedBoolGuardAnalysis::Always) => {
                                                SupportedBoolGuard::Always
                                            }
                                            Some(SupportedBoolGuardAnalysis::Skip) => continue,
                                            Some(SupportedBoolGuardAnalysis::Dynamic(expr_id)) => {
                                                SupportedBoolGuard::Dynamic(expr_id)
                                            }
                                            None => {
                                                supported = false;
                                                break;
                                            }
                                        },
                                    };

                                    match pattern_kind(self.input.hir, arm.pattern) {
                                        PatternKind::Binding(_) | PatternKind::Wildcard => {
                                            if matches!(guard, SupportedBoolGuard::Always) {
                                                fallback_target = arm.target;
                                                break;
                                            }
                                            ordered_arms.push(SupportedEnumMatchArm {
                                                variant_index: None,
                                                pattern: arm.pattern,
                                                guard,
                                                target: arm.target,
                                            });
                                        }
                                        PatternKind::Path(_)
                                        | PatternKind::TupleStruct { .. }
                                        | PatternKind::Struct { .. } => {
                                            let Some(variant_index) =
                                                enum_variant_index_for_pattern(
                                                    self.input.hir,
                                                    self.input.resolution,
                                                    &scrutinee_ty,
                                                    arm.pattern,
                                                )
                                            else {
                                                supported = false;
                                                break;
                                            };
                                            ordered_arms.push(SupportedEnumMatchArm {
                                                variant_index: Some(variant_index),
                                                pattern: arm.pattern,
                                                guard,
                                                target: arm.target,
                                            });
                                        }
                                        _ => {
                                            supported = false;
                                            break;
                                        }
                                    }
                                }

                                supported.then_some(SupportedMatchLowering::Enum {
                                    arms: ordered_arms,
                                    fallback_target,
                                })
                            } else {
                                let mut ordered_arms = Vec::new();
                                let mut fallback_target = *else_target;
                                let mut supported = true;

                                for arm in arms {
                                    let guard = match arm.guard {
                                        None => SupportedBoolGuard::Always,
                                        Some(guard) => match supported_bool_guard(
                                            self.input.hir,
                                            self.input.resolution,
                                            self.input.typeck,
                                            &self.signatures,
                                            body,
                                            &local_types,
                                            &immutable_place_aliases,
                                            scrutinee,
                                            arm.pattern,
                                            guard,
                                        ) {
                                            Some(SupportedBoolGuardAnalysis::Always) => {
                                                SupportedBoolGuard::Always
                                            }
                                            Some(SupportedBoolGuardAnalysis::Skip) => continue,
                                            Some(SupportedBoolGuardAnalysis::Dynamic(expr_id)) => {
                                                SupportedBoolGuard::Dynamic(expr_id)
                                            }
                                            None => {
                                                supported = false;
                                                break;
                                            }
                                        },
                                    };

                                    if self.supports_match_catch_all_pattern(
                                        arm.pattern,
                                        &scrutinee_ty,
                                    ) {
                                        if matches!(guard, SupportedBoolGuard::Always) {
                                            fallback_target = arm.target;
                                            break;
                                        }
                                        ordered_arms.push(SupportedGuardOnlyMatchArm {
                                            pattern: arm.pattern,
                                            guard,
                                            target: arm.target,
                                        });
                                    } else {
                                        supported = false;
                                        break;
                                    }
                                }

                                supported.then_some(SupportedMatchLowering::GuardOnly {
                                    arms: ordered_arms,
                                    fallback_target,
                                })
                            }
                        }
                        None => None,
                    }) else {
                        diagnostics.push(unsupported(
                            block.terminator.span,
                            "LLVM IR backend foundation does not support `match` lowering yet",
                        ));
                        continue;
                    };

                    supported_matches.insert(block_id, match_lowering);
                }
                TerminatorKind::ForLoop {
                    iterable,
                    item_local,
                    is_await,
                    body_target,
                    ..
                } => {
                    let mut scratch = Vec::new();
                    let iterable_ty = self.infer_operand_type(
                        body,
                        iterable,
                        &local_types,
                        &async_task_handles,
                        &mut scratch,
                        block.terminator.span,
                    );
                    let unsupported_message = if *is_await {
                        "LLVM IR backend foundation does not support `for await` lowering yet"
                    } else {
                        "LLVM IR backend foundation does not support `for` lowering yet"
                    };
                    let Some((iterable_root, iterable_kind, element_ty, iterable_len)) =
                        (match (iterable, iterable_ty) {
                            (Operand::Place(iterable_place), Some(Ty::Array { element, len })) => {
                                known_array_len(&len).map(|len| {
                                    (
                                        SupportedForLoopIterableRoot::Place(iterable_place.clone()),
                                        SupportedForLoopIterableKind::Array,
                                        element.as_ref().clone(),
                                        len,
                                    )
                                })
                            }
                            (Operand::Place(iterable_place), Some(Ty::Tuple(items)))
                                if !items.is_empty()
                                    && !items.iter().any(is_void_ty)
                                    && items.iter().skip(1).all(|item| {
                                        item.compatible_with(&items[0])
                                            && items[0].compatible_with(item)
                                    }) =>
                            {
                                Some((
                                    SupportedForLoopIterableRoot::Place(iterable_place.clone()),
                                    SupportedForLoopIterableKind::Tuple,
                                    items[0].clone(),
                                    items.len(),
                                ))
                            }
                            (
                                Operand::Constant(Constant::Item { item, .. }),
                                Some(Ty::Array { element, len }),
                            ) if const_or_static_item_type(
                                self.input.hir,
                                self.input.resolution,
                                *item,
                            )
                            .is_some() =>
                            {
                                known_array_len(&len).map(|len| {
                                    (
                                        SupportedForLoopIterableRoot::Item(*item),
                                        SupportedForLoopIterableKind::Array,
                                        element.as_ref().clone(),
                                        len,
                                    )
                                })
                            }
                            (
                                Operand::Constant(Constant::Item { item, .. }),
                                Some(Ty::Tuple(items)),
                            ) if const_or_static_item_type(
                                self.input.hir,
                                self.input.resolution,
                                *item,
                            )
                            .is_some()
                                && !items.is_empty()
                                && !items.iter().any(is_void_ty)
                                && items.iter().skip(1).all(|item| {
                                    item.compatible_with(&items[0])
                                        && items[0].compatible_with(item)
                                }) =>
                            {
                                Some((
                                    SupportedForLoopIterableRoot::Item(*item),
                                    SupportedForLoopIterableKind::Tuple,
                                    items[0].clone(),
                                    items.len(),
                                ))
                            }
                            (
                                Operand::Constant(Constant::Import(path)),
                                Some(Ty::Array { element, len }),
                            ) => local_item_for_import_path(self.input.hir, path).and_then(
                                |item_id| {
                                    const_or_static_item_type(
                                        self.input.hir,
                                        self.input.resolution,
                                        item_id,
                                    )
                                    .and_then(|_| {
                                        let len = known_array_len(&len)?;
                                        Some((
                                            SupportedForLoopIterableRoot::Item(item_id),
                                            SupportedForLoopIterableKind::Array,
                                            element.as_ref().clone(),
                                            len,
                                        ))
                                    })
                                },
                            ),
                            (Operand::Constant(Constant::Import(path)), Some(Ty::Tuple(items)))
                                if !items.is_empty()
                                    && !items.iter().any(is_void_ty)
                                    && items.iter().skip(1).all(|item| {
                                        item.compatible_with(&items[0])
                                            && items[0].compatible_with(item)
                                    }) =>
                            {
                                local_item_for_import_path(self.input.hir, path).and_then(
                                    |item_id| {
                                        const_or_static_item_type(
                                            self.input.hir,
                                            self.input.resolution,
                                            item_id,
                                        )
                                        .map(|_| {
                                            (
                                                SupportedForLoopIterableRoot::Item(item_id),
                                                SupportedForLoopIterableKind::Tuple,
                                                items[0].clone(),
                                                items.len(),
                                            )
                                        })
                                    },
                                )
                            }
                            _ => None,
                        })
                    else {
                        diagnostics.push(unsupported(block.terminator.span, unsupported_message));
                        continue;
                    };
                    if *is_await && matches!(&element_ty, Ty::TaskHandle(_)) {
                        if !self.has_runtime_hook(RuntimeHook::TaskAwait) {
                            diagnostics.push(unsupported(
                                block.terminator.span,
                                "LLVM IR backend foundation requires the `task-await` runtime hook before lowering task-backed `for await` loops",
                            ));
                            continue;
                        }
                        if !self.has_runtime_hook(RuntimeHook::TaskResultRelease) {
                            diagnostics.push(unsupported(
                                block.terminator.span,
                                "LLVM IR backend foundation requires the `task-result-release` runtime hook before lowering task-backed `for await` loops",
                            ));
                            continue;
                        }
                    }
                    let (item_ty, auto_await_task_elements) = if *is_await {
                        match &element_ty {
                            Ty::TaskHandle(result_ty) => ((**result_ty).clone(), true),
                            _ => (element_ty.clone(), false),
                        }
                    } else {
                        (element_ty.clone(), false)
                    };
                    seed_inferred_local_type(&mut local_types, *item_local, item_ty.clone());
                    supported_for_loops.insert(
                        block_id,
                        SupportedForLoopLowering {
                            iterable_root,
                            item_local: *item_local,
                            element_ty,
                            item_ty,
                            auto_await_task_elements,
                            iterable_kind,
                            iterable_len,
                            body_target: *body_target,
                        },
                    );
                }
            }
        }

        let (direct_local_capturing_closures, ordinary_control_flow_capturing_closure_calls) = self
            .collect_supported_direct_local_capturing_closures(
                body,
                &local_types,
                &supported_matches,
                &mut diagnostics,
            );
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
                body: body.clone(),
                local_types,
                async_task_handles,
                task_handle_place_aliases,
                supported_for_loops,
                supported_matches,
                param_binding_locals: Vec::new(),
                direct_local_capturing_closures,
                ordinary_control_flow_capturing_closure_calls,
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
                    self.propagate_inferred_local_types_from_statement(
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
                self.seed_expected_temp_from_call_args(
                    body,
                    value,
                    local_types,
                    async_task_handles,
                    statement.span,
                );
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
                self.seed_expected_temp_from_call_args(
                    body,
                    value,
                    local_types,
                    async_task_handles,
                    statement.span,
                );
            }
            StatementKind::StorageLive { .. }
            | StatementKind::StorageDead { .. }
            | StatementKind::RegisterCleanup { .. }
            | StatementKind::RunCleanup { .. } => {}
        }
    }

    fn propagate_inferred_local_types_from_statement(
        &self,
        body: &mir::MirBody,
        statement: &mir::Statement,
        local_types: &mut HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
    ) {
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                let mut scratch = Vec::new();
                let expected_ty = self.assignment_place_type(
                    body,
                    place,
                    local_types,
                    async_task_handles,
                    statement.span,
                    &mut scratch,
                );
                let mut infer = TypeInferenceContext {
                    body,
                    local_types,
                    async_task_handles,
                    diagnostics: &mut scratch,
                };
                if let Some(ty) =
                    self.infer_rvalue_type(value, expected_ty.as_ref(), &mut infer, statement.span)
                {
                    if place.projections.is_empty() {
                        seed_inferred_local_type(local_types, place.base, ty);
                    }
                }
            }
            StatementKind::BindPattern {
                pattern, source, ..
            } => {
                let mut scratch = Vec::new();
                let source_ty = self.infer_operand_type(
                    body,
                    source,
                    local_types,
                    async_task_handles,
                    &mut scratch,
                    statement.span,
                );
                if let (Some(binding_local), Some(source_ty)) =
                    (self.binding_local_for_pattern(body, *pattern), source_ty)
                {
                    seed_inferred_local_type(local_types, binding_local, source_ty);
                }
            }
            StatementKind::Eval { .. }
            | StatementKind::StorageLive { .. }
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
            Rvalue::Question(operand) => {
                self.seed_expected_temp_from_operand(body, operand, expected_ty, local_types);
            }
            Rvalue::Tuple(items) => {
                self.seed_expected_temp_from_tuple_items(body, items, expected_ty, local_types);
            }
            Rvalue::Array(items) => {
                self.seed_expected_temp_from_array_items(body, items, expected_ty, local_types);
            }
            Rvalue::RepeatArray { value, .. } => {
                if let Ty::Array { element, .. } = expected_ty {
                    self.seed_expected_temp_from_operand(body, value, element, local_types);
                }
            }
            Rvalue::AggregateTupleStruct { .. } => {}
            Rvalue::AggregateStruct { fields, .. } => {
                self.seed_expected_temp_from_struct_fields(body, fields, expected_ty, local_types);
            }
            Rvalue::Call { .. }
            | Rvalue::Binary { .. }
            | Rvalue::Unary { .. }
            | Rvalue::Closure { .. }
            | Rvalue::OpaqueExpr(_) => {}
        }
    }

    fn seed_expected_temp_from_call_args(
        &self,
        body: &mir::MirBody,
        value: &Rvalue,
        local_types: &mut HashMap<mir::LocalId, Ty>,
        async_task_handles: &HashMap<mir::LocalId, AsyncTaskHandleInfo>,
        span: Span,
    ) {
        let Rvalue::Call { callee, args } = value else {
            return;
        };
        if let Some(function) = self.resolve_direct_callee_function(callee)
            && let Some(signature) = self.signatures.get(&function)
            && let Some(ordered_args) = self.ordered_call_args(args, signature)
        {
            for (arg, param) in ordered_args.into_iter().zip(signature.params.iter()) {
                self.seed_expected_temp_from_operand(body, &arg.value, &param.ty, local_types);
            }
            return;
        }

        let mut diagnostics = Vec::new();
        let Some(callee_ty) = self.infer_operand_type(
            body,
            callee,
            local_types,
            async_task_handles,
            &mut diagnostics,
            span,
        ) else {
            return;
        };
        let Ty::Callable { params, .. } = callee_ty else {
            return;
        };
        if args.iter().any(|arg| arg.name.is_some()) || args.len() != params.len() {
            return;
        }

        for (arg, param_ty) in args.iter().zip(params.iter()) {
            self.seed_expected_temp_from_operand(body, &arg.value, param_ty, local_types);
        }
    }

    fn resolve_direct_callee_function(&self, callee: &Operand) -> Option<FunctionRef> {
        match callee {
            Operand::Constant(Constant::Function { function, .. }) => Some(*function),
            Operand::Constant(Constant::Import(path)) => {
                let item_id = local_item_for_import_path(self.input.hir, path)?;
                match &self.input.hir.item(item_id).kind {
                    ItemKind::Function(_) => Some(FunctionRef::Item(item_id)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn infer_function_value_type(
        &self,
        function: FunctionRef,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        let Some(signature) = self.signatures.get(&function) else {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation could not resolve the function declaration for this value",
            ));
            return None;
        };
        Some(callable_ty_from_signature(signature))
    }

    fn infer_const_or_static_item_type(
        &self,
        item_id: ItemId,
        diagnostics: &mut Vec<Diagnostic>,
        span: Span,
    ) -> Option<Ty> {
        let (ItemKind::Const(global) | ItemKind::Static(global)) =
            &self.input.hir.item(item_id).kind
        else {
            return None;
        };
        let ty = lower_type(self.input.hir, self.input.resolution, global.ty);
        if !matches!(ty, Ty::Callable { .. }) {
            return Some(ty);
        }

        let mut visited = HashSet::new();
        let Some(function) = const_or_static_callable_function_ref(
            self.input.hir,
            self.input.resolution,
            item_id,
            &mut visited,
        ) else {
            if const_or_static_callable_closure_target(
                self.input.hir,
                self.input.resolution,
                item_id,
                &mut visited,
            )
            .is_some()
            {
                return Some(ty);
            }
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation does not support callable const/static values yet",
            ));
            return None;
        };

        self.infer_function_value_type(function, diagnostics, span)
    }

    fn ordered_call_args<'b>(
        &self,
        args: &'b [mir::CallArgument],
        signature: &FunctionSignature,
    ) -> Option<Vec<&'b mir::CallArgument>> {
        if args.iter().all(|arg| arg.name.is_none()) {
            return (args.len() == signature.params.len()).then(|| args.iter().collect());
        }

        let mut ordered = vec![None; signature.params.len()];
        let mut next_positional = 0usize;

        for arg in args {
            let index = if let Some(name) = arg.name.as_deref() {
                signature
                    .params
                    .iter()
                    .position(|param| param.name == name)?
            } else {
                while next_positional < ordered.len() && ordered[next_positional].is_some() {
                    next_positional += 1;
                }
                if next_positional == ordered.len() {
                    return None;
                }
                let index = next_positional;
                next_positional += 1;
                index
            };

            if ordered[index].is_some() {
                return None;
            }
            ordered[index] = Some(arg);
        }

        ordered.into_iter().collect()
    }

    fn supports_cleanup_action(
        &self,
        cleanup: &mir::CleanupAction,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        match &cleanup.kind {
            mir::CleanupKind::Defer { expr } => {
                self.supports_cleanup_expr(*expr, body, local_types)
            }
        }
    }

    fn supports_cleanup_expr(
        &self,
        expr_id: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        self.supports_cleanup_expr_with_loop(expr_id, false, body, local_types)
    }

    fn supports_cleanup_expr_with_loop(
        &self,
        expr_id: hir::ExprId,
        in_cleanup_loop: bool,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                self.supports_cleanup_call_expr(*callee, args, None, body, local_types)
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => self
                .supports_cleanup_block_expr_with_loop(
                    *block_id,
                    in_cleanup_loop,
                    body,
                    local_types,
                ),
            hir::ExprKind::Question(inner) => {
                self.supports_cleanup_expr_with_loop(*inner, in_cleanup_loop, body, local_types)
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                matches!(task_ty, Ty::TaskHandle(_))
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types)
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Spawn,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                matches!(task_ty, Ty::TaskHandle(_))
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types)
            }
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => self.supports_cleanup_assignment_expr(*left, *right, body, local_types),
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.supports_cleanup_bool_expr(*condition, body, local_types)
                    && self.supports_cleanup_block_expr_with_loop(
                        *then_branch,
                        in_cleanup_loop,
                        body,
                        local_types,
                    )
                    && else_branch.is_none_or(|expr| {
                        self.supports_cleanup_expr_with_loop(
                            expr,
                            in_cleanup_loop,
                            body,
                            local_types,
                        )
                    })
            }
            hir::ExprKind::Match { value, arms } => self.supports_cleanup_match_expr_with_loop(
                *value,
                arms,
                in_cleanup_loop,
                body,
                local_types,
            ),
            _ => false,
        }
    }

    fn supports_cleanup_block_expr_with_loop(
        &self,
        block_id: hir::BlockId,
        in_cleanup_loop: bool,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        self.supports_cleanup_block_with_tail(
            block_id,
            false,
            in_cleanup_loop,
            body,
            local_types,
            |this, tail, loop_scope, cleanup_body, cleanup_local_types| {
                this.supports_cleanup_expr_with_loop(
                    tail,
                    loop_scope,
                    cleanup_body,
                    cleanup_local_types,
                )
            },
        )
    }

    fn supports_cleanup_block_with_tail(
        &self,
        block_id: hir::BlockId,
        require_tail: bool,
        in_cleanup_loop: bool,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
        tail_support: impl Fn(
            &Self,
            hir::ExprId,
            bool,
            &mir::MirBody,
            &HashMap<mir::LocalId, Ty>,
        ) -> bool,
    ) -> bool {
        let block = self.input.hir.block(block_id);
        block.statements.iter().all(|statement_id| {
            self.supports_cleanup_statement_with_loop(
                *statement_id,
                in_cleanup_loop,
                body,
                local_types,
            )
        }) && match block.tail {
            Some(tail) => tail_support(self, tail, in_cleanup_loop, body, local_types),
            None => !require_tail,
        }
    }

    fn supports_cleanup_statement_with_loop(
        &self,
        statement_id: hir::StmtId,
        in_cleanup_loop: bool,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        match &self.input.hir.stmt(statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                self.supports_cleanup_let_pattern(*pattern)
                    && self
                        .cleanup_let_expected_ty(*pattern, *value)
                        .is_some_and(|expected_ty| {
                            self.supports_cleanup_value_expr(*value, expected_ty, body, local_types)
                        })
            }
            hir::StmtKind::Expr { expr, .. } => {
                if let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &self.input.hir.expr(*expr).kind
                    && self.supports_cleanup_callable_assignment_expr(*left, *right)
                {
                    return true;
                }
                self.supports_cleanup_expr_with_loop(*expr, in_cleanup_loop, body, local_types)
            }
            hir::StmtKind::While {
                condition,
                body: while_body,
            } => {
                self.supports_cleanup_bool_expr(*condition, body, local_types)
                    && self.supports_cleanup_block_expr_with_loop(
                        *while_body,
                        true,
                        body,
                        local_types,
                    )
            }
            hir::StmtKind::Loop { body: loop_body } => {
                self.supports_cleanup_block_expr_with_loop(*loop_body, true, body, local_types)
            }
            hir::StmtKind::For {
                is_await,
                pattern,
                iterable,
                body: for_body,
            } => {
                let supports_cleanup_for_await = if *is_await {
                    let Some(iterable_ty) = self.input.typeck.expr_ty(*iterable) else {
                        return false;
                    };
                    let Some((_, element_ty, _)) = cleanup_for_iterable_shape(iterable_ty) else {
                        return false;
                    };
                    !matches!(element_ty, Ty::TaskHandle(_))
                        || (self.has_runtime_hook(RuntimeHook::TaskAwait)
                            && self.has_runtime_hook(RuntimeHook::TaskResultRelease))
                } else {
                    true
                };
                supports_cleanup_for_await
                    && self.supports_cleanup_for_pattern(*pattern)
                    && self.supports_cleanup_for_iterable(*iterable, body, local_types)
                    && self.supports_cleanup_block_expr_with_loop(
                        *for_body,
                        true,
                        body,
                        local_types,
                    )
            }
            hir::StmtKind::Break | hir::StmtKind::Continue => in_cleanup_loop,
            hir::StmtKind::Return(_) | hir::StmtKind::Defer(_) => false,
        }
    }

    fn supports_cleanup_callable_assignment_expr(
        &self,
        target_expr: hir::ExprId,
        value_expr: hir::ExprId,
    ) -> bool {
        let Some(target_ty) = self.input.typeck.expr_ty(target_expr) else {
            return false;
        };
        if is_void_ty(target_ty) || !matches!(target_ty, Ty::Callable { .. }) {
            return false;
        }
        self.input
            .typeck
            .expr_ty(value_expr)
            .is_some_and(|value_ty| target_ty.compatible_with(value_ty))
    }

    fn supports_cleanup_let_pattern(&self, pattern: hir::PatternId) -> bool {
        match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            PatternKind::Tuple(items) => items
                .iter()
                .all(|item| self.supports_cleanup_let_pattern(*item)),
            PatternKind::Array(items) => items
                .iter()
                .all(|item| self.supports_cleanup_let_pattern(*item)),
            PatternKind::Struct { fields, .. } => fields
                .iter()
                .all(|field| self.supports_cleanup_let_pattern(field.pattern)),
            _ => false,
        }
    }

    fn cleanup_let_expected_ty(&self, pattern: hir::PatternId, value: hir::ExprId) -> Option<&Ty> {
        match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(local) => self.input.typeck.local_ty(*local),
            PatternKind::Tuple(_)
            | PatternKind::Array(_)
            | PatternKind::Struct { .. }
            | PatternKind::Wildcard => self.input.typeck.expr_ty(value),
            _ => None,
        }
    }

    fn supports_cleanup_for_pattern(&self, pattern: hir::PatternId) -> bool {
        match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            PatternKind::Tuple(items) => items
                .iter()
                .all(|item| self.supports_cleanup_for_pattern(*item)),
            PatternKind::Array(items) => items
                .iter()
                .all(|item| self.supports_cleanup_for_pattern(*item)),
            PatternKind::Struct { fields, .. } => fields
                .iter()
                .all(|field| self.supports_cleanup_for_pattern(field.pattern)),
            _ => false,
        }
    }

    fn supports_match_catch_all_pattern(&self, pattern: hir::PatternId, scrutinee_ty: &Ty) -> bool {
        match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            PatternKind::Tuple(items) => {
                let Ty::Tuple(item_tys) = scrutinee_ty else {
                    return false;
                };
                items.len() == item_tys.len()
                    && items.iter().zip(item_tys.iter()).all(|(item, item_ty)| {
                        self.supports_match_catch_all_pattern(*item, item_ty)
                    })
            }
            PatternKind::Array(items) => {
                let Ty::Array { element, len } = scrutinee_ty else {
                    return false;
                };
                known_array_len(len).is_some_and(|len| items.len() == len)
                    && items
                        .iter()
                        .all(|item| self.supports_match_catch_all_pattern(*item, element))
            }
            PatternKind::Struct { fields, .. } => {
                let Ok(field_layouts) = self.struct_field_lowerings(
                    scrutinee_ty,
                    Span::default(),
                    "cleanup match pattern",
                ) else {
                    return false;
                };
                fields.iter().all(|field| {
                    field_layouts
                        .iter()
                        .find(|layout| layout.name == field.name)
                        .is_some_and(|layout| {
                            self.supports_match_catch_all_pattern(field.pattern, &layout.ty)
                        })
                })
            }
            _ => false,
        }
    }

    fn supports_destructuring_bind_pattern(&self, pattern: hir::PatternId) -> bool {
        match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(_) | PatternKind::Wildcard => true,
            PatternKind::Tuple(items) => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::Array(items) => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::TupleStruct { items, .. } => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::Struct { fields, .. } => fields
                .iter()
                .all(|field| self.supports_destructuring_bind_pattern(field.pattern)),
            _ => false,
        }
    }

    fn literal_source_expr(&self, expr_id: hir::ExprId) -> Option<hir::ExprId> {
        let mut visited = HashSet::new();
        let source = guard_literal_source_expr(
            self.input.hir,
            self.input.resolution,
            expr_id,
            &mut visited,
        )?;
        (source != expr_id).then_some(source)
    }

    fn supports_cleanup_for_iterable(
        &self,
        expr_id: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        let Some(iterable_ty) = self.input.typeck.expr_ty(expr_id) else {
            return false;
        };
        let Some(_) = cleanup_for_iterable_shape(iterable_ty) else {
            return false;
        };
        if task_iterable_item_root_expr(self.input.hir, self.input.resolution, expr_id).is_some() {
            return self
                .lower_llvm_type(
                    iterable_ty,
                    self.input.hir.expr(expr_id).span,
                    "cleanup for iterable",
                )
                .is_ok();
        }
        self.supports_cleanup_value_expr(expr_id, iterable_ty, body, local_types)
    }

    fn supports_cleanup_match_expr_with_loop(
        &self,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        in_cleanup_loop: bool,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        let Some(scrutinee_ty) = self.input.typeck.expr_ty(value_expr) else {
            return false;
        };

        if scrutinee_ty.is_bool() {
            return self.supports_cleanup_bool_expr(value_expr, body, local_types)
                && arms.iter().all(|arm| {
                    supported_cleanup_bool_match_pattern(
                        self.input.hir,
                        self.input.resolution,
                        arm.pattern,
                    )
                    .is_some()
                        && arm.guard.is_none_or(|guard| {
                            self.supports_cleanup_bool_expr(guard, body, local_types)
                        })
                        && self.supports_cleanup_expr_with_loop(
                            arm.body,
                            in_cleanup_loop,
                            body,
                            local_types,
                        )
                });
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            return self.supports_cleanup_scalar_expr(value_expr, body, local_types)
                && arms.iter().all(|arm| {
                    supported_cleanup_integer_match_pattern(
                        self.input.hir,
                        self.input.resolution,
                        arm.pattern,
                    )
                    .is_some()
                        && arm.guard.is_none_or(|guard| {
                            self.supports_cleanup_bool_expr(guard, body, local_types)
                        })
                        && self.supports_cleanup_expr_with_loop(
                            arm.body,
                            in_cleanup_loop,
                            body,
                            local_types,
                        )
                });
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::String)) {
            return self.supports_cleanup_value_expr(value_expr, scrutinee_ty, body, local_types)
                && arms.iter().all(|arm| {
                    supported_cleanup_string_match_pattern(
                        self.input.hir,
                        self.input.resolution,
                        arm.pattern,
                    )
                    .is_some()
                        && arm.guard.is_none_or(|guard| {
                            self.supports_cleanup_bool_expr(guard, body, local_types)
                        })
                        && self.supports_cleanup_expr_with_loop(
                            arm.body,
                            in_cleanup_loop,
                            body,
                            local_types,
                        )
                });
        }

        self.supports_cleanup_value_expr(value_expr, scrutinee_ty, body, local_types)
            && arms.iter().all(|arm| {
                self.supports_match_catch_all_pattern(arm.pattern, scrutinee_ty)
                    && arm.guard.is_none_or(|guard| {
                        self.supports_cleanup_bool_expr(guard, body, local_types)
                    })
                    && self.supports_cleanup_expr_with_loop(
                        arm.body,
                        in_cleanup_loop,
                        body,
                        local_types,
                    )
            })
    }

    fn cleanup_assignment_target_ty(
        &self,
        target_expr: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> Option<Ty> {
        let (_, target_ty) = guard_expr_place_with_ty(
            self.input.hir,
            self.input.resolution,
            body,
            local_types,
            None,
            target_expr,
        )?;

        if is_void_ty(&target_ty)
            || self
                .lower_llvm_type(
                    &target_ty,
                    self.input.hir.expr(target_expr).span,
                    "cleanup assignment target",
                )
                .is_err()
        {
            return None;
        }

        Some(target_ty)
    }

    fn supports_cleanup_assignment_expr_as_scalar_kind(
        &self,
        target_expr: hir::ExprId,
        value_expr: hir::ExprId,
        expected_kind: GuardScalarKind,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        let Some(target_ty) = self.cleanup_assignment_target_ty(target_expr, body, local_types)
        else {
            return false;
        };
        if guard_scalar_kind_for_ty(&target_ty) != Some(expected_kind) {
            return false;
        }

        match expected_kind {
            GuardScalarKind::Bool => self.supports_cleanup_bool_expr(value_expr, body, local_types),
            GuardScalarKind::Int => {
                self.supports_cleanup_scalar_expr(value_expr, body, local_types)
            }
        }
    }

    fn supports_cleanup_assignment_expr(
        &self,
        target_expr: hir::ExprId,
        value_expr: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        let Some(target_ty) = self.cleanup_assignment_target_ty(target_expr, body, local_types)
        else {
            return false;
        };

        self.supports_cleanup_value_expr(value_expr, &target_ty, body, local_types)
    }

    fn supports_cleanup_call_expr(
        &self,
        callee_expr: hir::ExprId,
        args: &[hir::CallArg],
        expected_ty: Option<&Ty>,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        if let Some(function) =
            guard_direct_callee_function(self.input.hir, self.input.resolution, callee_expr)
        {
            let Some(signature) = self.signatures.get(&function) else {
                return false;
            };
            let actual_ty = cleanup_call_result_ty(signature);
            if let Some(expected_ty) = expected_ty
                && (is_void_ty(&actual_ty) || !expected_ty.compatible_with(&actual_ty))
            {
                return false;
            }
            let Some(ordered_args) = ordered_guard_call_args(args, signature) else {
                return false;
            };
            return ordered_args
                .into_iter()
                .zip(signature.params.iter())
                .all(|(arg, param)| {
                    self.supports_cleanup_value_expr(
                        guard_call_arg_expr(arg),
                        &param.ty,
                        body,
                        local_types,
                    )
                });
        }

        let Some(callee_ty) = self.input.typeck.expr_ty(callee_expr) else {
            return false;
        };
        let Ty::Callable { params, ret } = callee_ty else {
            return false;
        };
        if !self.supports_cleanup_value_expr(callee_expr, callee_ty, body, local_types) {
            return false;
        }
        if let Some(expected_ty) = expected_ty
            && (is_void_ty(ret.as_ref()) || !expected_ty.compatible_with(ret.as_ref()))
        {
            return false;
        }
        if args.len() != params.len()
            || args
                .iter()
                .any(|arg| matches!(arg, hir::CallArg::Named { .. }))
        {
            return false;
        }
        args.iter().zip(params.iter()).all(|(arg, param_ty)| {
            self.supports_cleanup_value_expr(guard_call_arg_expr(arg), param_ty, body, local_types)
        })
    }

    fn supports_cleanup_value_expr(
        &self,
        expr_id: hir::ExprId,
        expected_ty: &Ty,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        if is_void_ty(expected_ty) {
            return false;
        }

        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                return self.supports_cleanup_call_expr(
                    *callee,
                    args,
                    Some(expected_ty),
                    body,
                    local_types,
                );
            }
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let Some(target_ty) = self.cleanup_assignment_target_ty(*left, body, local_types)
                else {
                    return false;
                };
                return expected_ty.compatible_with(&target_ty)
                    && self.supports_cleanup_value_expr(*right, &target_ty, body, local_types);
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                if matches!(expected_ty, Ty::Callable { .. })
                    && let Some(tail) = callable_elided_block_tail_expr(
                        self.input.hir,
                        self.input.resolution,
                        *block_id,
                    )
                {
                    return self.supports_cleanup_value_expr(tail, expected_ty, body, local_types);
                }
                return self.supports_cleanup_block_with_tail(
                    *block_id,
                    true,
                    false,
                    body,
                    local_types,
                    |this: &Self, tail, _, _, _| {
                        this.supports_cleanup_value_expr(tail, expected_ty, body, local_types)
                    },
                );
            }
            hir::ExprKind::Question(inner) => {
                return self.supports_cleanup_value_expr(*inner, expected_ty, body, local_types);
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                let Ty::TaskHandle(result_ty) = task_ty else {
                    return false;
                };
                return expected_ty.compatible_with(result_ty.as_ref())
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types);
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Spawn,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                let Ty::TaskHandle(_) = task_ty else {
                    return false;
                };
                return expected_ty.compatible_with(task_ty)
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types);
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                return self.supports_cleanup_bool_expr(*condition, body, local_types)
                    && self.supports_cleanup_block_with_tail(
                        *then_branch,
                        true,
                        false,
                        body,
                        local_types,
                        |this, tail, _, cleanup_body, cleanup_local_types| {
                            this.supports_cleanup_value_expr(
                                tail,
                                expected_ty,
                                cleanup_body,
                                cleanup_local_types,
                            )
                        },
                    )
                    && else_branch.is_some_and(|other| {
                        self.supports_cleanup_value_expr(other, expected_ty, body, local_types)
                    });
            }
            hir::ExprKind::Match { value, arms } => {
                let Some(scrutinee_ty) = self.input.typeck.expr_ty(*value) else {
                    return false;
                };

                if scrutinee_ty.is_bool() {
                    return self.supports_cleanup_bool_expr(*value, body, local_types)
                        && arms.iter().all(|arm| {
                            supported_cleanup_bool_match_pattern(
                                self.input.hir,
                                self.input.resolution,
                                arm.pattern,
                            )
                            .is_some()
                                && arm.guard.is_none_or(|guard| {
                                    self.supports_cleanup_bool_expr(guard, body, local_types)
                                })
                                && self.supports_cleanup_value_expr(
                                    arm.body,
                                    expected_ty,
                                    body,
                                    local_types,
                                )
                        });
                }

                if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
                    return self.supports_cleanup_scalar_expr(*value, body, local_types)
                        && arms.iter().all(|arm| {
                            supported_cleanup_integer_match_pattern(
                                self.input.hir,
                                self.input.resolution,
                                arm.pattern,
                            )
                            .is_some()
                                && arm.guard.is_none_or(|guard| {
                                    self.supports_cleanup_bool_expr(guard, body, local_types)
                                })
                                && self.supports_cleanup_value_expr(
                                    arm.body,
                                    expected_ty,
                                    body,
                                    local_types,
                                )
                        });
                }

                if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::String)) {
                    return self.supports_cleanup_value_expr(
                        *value,
                        scrutinee_ty,
                        body,
                        local_types,
                    ) && arms.iter().all(|arm| {
                        supported_cleanup_string_match_pattern(
                            self.input.hir,
                            self.input.resolution,
                            arm.pattern,
                        )
                        .is_some()
                            && arm.guard.is_none_or(|guard| {
                                self.supports_cleanup_bool_expr(guard, body, local_types)
                            })
                            && self.supports_cleanup_value_expr(
                                arm.body,
                                expected_ty,
                                body,
                                local_types,
                            )
                    });
                }

                return self.supports_cleanup_value_expr(*value, scrutinee_ty, body, local_types)
                    && arms.iter().all(|arm| {
                        self.supports_match_catch_all_pattern(arm.pattern, scrutinee_ty)
                            && arm.guard.is_none_or(|guard| {
                                self.supports_cleanup_bool_expr(guard, body, local_types)
                            })
                            && self.supports_cleanup_value_expr(
                                arm.body,
                                expected_ty,
                                body,
                                local_types,
                            )
                    });
            }
            hir::ExprKind::Tuple(items) => {
                let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
                    return false;
                };
                let Ty::Tuple(expected_items) = actual_ty else {
                    return false;
                };
                return expected_ty.compatible_with(actual_ty)
                    && expected_items.len() == items.len()
                    && items
                        .iter()
                        .zip(expected_items.iter())
                        .all(|(item, item_ty)| {
                            self.supports_cleanup_value_expr(*item, item_ty, body, local_types)
                        });
            }
            hir::ExprKind::Array(items) => {
                let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
                    return false;
                };
                let Ty::Array { element, len } = actual_ty else {
                    return false;
                };
                return expected_ty.compatible_with(actual_ty)
                    && known_array_len(len).is_some_and(|len| len == items.len())
                    && items.iter().all(|item| {
                        self.supports_cleanup_value_expr(*item, element.as_ref(), body, local_types)
                    });
            }
            hir::ExprKind::RepeatArray { value, .. } => {
                let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
                    return false;
                };
                let Ty::Array { element, len } = actual_ty else {
                    return false;
                };
                return expected_ty.compatible_with(actual_ty)
                    && known_array_len(len).is_some()
                    && self.supports_cleanup_value_expr(
                        *value,
                        element.as_ref(),
                        body,
                        local_types,
                    );
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
                    return false;
                };
                let Ok(field_layouts) = self.struct_field_lowerings(
                    actual_ty,
                    self.input.hir.expr(expr_id).span,
                    "cleanup value",
                ) else {
                    return false;
                };
                return expected_ty.compatible_with(actual_ty)
                    && fields.iter().all(|field| {
                        field_layouts
                            .iter()
                            .find(|layout| layout.name == field.name)
                            .is_some_and(|layout| {
                                self.supports_cleanup_value_expr(
                                    field.value,
                                    &layout.ty,
                                    body,
                                    local_types,
                                )
                            })
                    });
            }
            _ => {}
        }

        if let Some(source_expr) = self.literal_source_expr(expr_id) {
            return self.supports_cleanup_value_expr(source_expr, expected_ty, body, local_types);
        }

        let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
            return false;
        };
        if !expected_ty.compatible_with(actual_ty)
            || self
                .lower_llvm_type(
                    actual_ty,
                    self.input.hir.expr(expr_id).span,
                    "cleanup value",
                )
                .is_err()
        {
            return false;
        }

        if expected_ty.is_bool() {
            self.supports_cleanup_bool_expr(expr_id, body, local_types)
        } else if expected_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            self.supports_cleanup_scalar_expr(expr_id, body, local_types)
        } else {
            matches!(
                &self.input.hir.expr(expr_id).kind,
                hir::ExprKind::Name(_)
                    | hir::ExprKind::Member { .. }
                    | hir::ExprKind::Bracket { .. }
                    | hir::ExprKind::Tuple(_)
                    | hir::ExprKind::Array(_)
                    | hir::ExprKind::StructLiteral { .. }
            )
        }
    }

    fn supports_cleanup_bool_expr(
        &self,
        expr_id: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        if let hir::ExprKind::Binary {
            left,
            op: BinaryOp::Assign,
            right,
        } = &self.input.hir.expr(expr_id).kind
        {
            return self.supports_cleanup_assignment_expr_as_scalar_kind(
                *left,
                *right,
                GuardScalarKind::Bool,
                body,
                local_types,
            );
        }

        let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
            return false;
        };
        if !actual_ty.is_bool() {
            return false;
        }

        if let Some(source_expr) = self.literal_source_expr(expr_id) {
            return self.supports_cleanup_bool_expr(source_expr, body, local_types);
        }

        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::Bool(_)
            | hir::ExprKind::Name(_)
            | hir::ExprKind::Member { .. }
            | hir::ExprKind::Bracket { .. } => true,
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => self.supports_cleanup_assignment_expr_as_scalar_kind(
                *left,
                *right,
                GuardScalarKind::Bool,
                body,
                local_types,
            ),
            hir::ExprKind::Call { callee, args } => {
                self.supports_cleanup_call_expr(*callee, args, Some(actual_ty), body, local_types)
            }
            hir::ExprKind::Binary { left, op, right } => match op {
                BinaryOp::AndAnd | BinaryOp::OrOr => {
                    self.supports_cleanup_bool_expr(*left, body, local_types)
                        && self.supports_cleanup_bool_expr(*right, body, local_types)
                }
                BinaryOp::EqEq
                | BinaryOp::BangEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq => {
                    self.supports_cleanup_scalar_expr(*left, body, local_types)
                        && self.supports_cleanup_scalar_expr(*right, body, local_types)
                }
                BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Rem
                | BinaryOp::Assign => false,
            },
            hir::ExprKind::Unary {
                op: UnaryOp::Not,
                expr,
            } => self.supports_cleanup_bool_expr(*expr, body, local_types),
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                let Ty::TaskHandle(result_ty) = task_ty else {
                    return false;
                };
                actual_ty.compatible_with(result_ty.as_ref())
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types)
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => self
                .supports_cleanup_block_with_tail(
                    *block_id,
                    true,
                    false,
                    body,
                    local_types,
                    |this: &Self, tail, _, _, _| {
                        this.supports_cleanup_bool_expr(tail, body, local_types)
                    },
                ),
            hir::ExprKind::Question(inner) => {
                self.supports_cleanup_bool_expr(*inner, body, local_types)
            }
            _ => false,
        }
    }

    fn supports_cleanup_scalar_expr(
        &self,
        expr_id: hir::ExprId,
        body: &mir::MirBody,
        local_types: &HashMap<mir::LocalId, Ty>,
    ) -> bool {
        if let hir::ExprKind::Binary {
            left,
            op: BinaryOp::Assign,
            right,
        } = &self.input.hir.expr(expr_id).kind
        {
            return self.supports_cleanup_assignment_expr_as_scalar_kind(
                *left,
                *right,
                GuardScalarKind::Int,
                body,
                local_types,
            );
        }

        let Some(actual_ty) = self.input.typeck.expr_ty(expr_id) else {
            return false;
        };
        if !actual_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            return false;
        }

        if let Some(source_expr) = self.literal_source_expr(expr_id) {
            return self.supports_cleanup_scalar_expr(source_expr, body, local_types);
        }

        match &self.input.hir.expr(expr_id).kind {
            hir::ExprKind::Integer(_)
            | hir::ExprKind::Name(_)
            | hir::ExprKind::Member { .. }
            | hir::ExprKind::Bracket { .. } => true,
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => self.supports_cleanup_assignment_expr_as_scalar_kind(
                *left,
                *right,
                GuardScalarKind::Int,
                body,
                local_types,
            ),
            hir::ExprKind::Call { callee, args } => {
                self.supports_cleanup_call_expr(*callee, args, Some(actual_ty), body, local_types)
            }
            hir::ExprKind::Binary { left, op, right } => match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                    self.supports_cleanup_scalar_expr(*left, body, local_types)
                        && self.supports_cleanup_scalar_expr(*right, body, local_types)
                }
                BinaryOp::AndAnd
                | BinaryOp::OrOr
                | BinaryOp::EqEq
                | BinaryOp::BangEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::Assign => false,
            },
            hir::ExprKind::Unary {
                op: UnaryOp::Neg,
                expr,
            } => self.supports_cleanup_scalar_expr(*expr, body, local_types),
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let Some(task_ty) = self.input.typeck.expr_ty(*expr) else {
                    return false;
                };
                let Ty::TaskHandle(result_ty) = task_ty else {
                    return false;
                };
                actual_ty.compatible_with(result_ty.as_ref())
                    && self.supports_cleanup_value_expr(*expr, task_ty, body, local_types)
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => self
                .supports_cleanup_block_with_tail(
                    *block_id,
                    true,
                    false,
                    body,
                    local_types,
                    |this: &Self, tail, _, _, _| {
                        this.supports_cleanup_scalar_expr(tail, body, local_types)
                    },
                ),
            hir::ExprKind::Question(inner) => {
                self.supports_cleanup_scalar_expr(*inner, body, local_types)
            }
            _ => false,
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
                LocalOrigin::Receiver => signature.params.first().map(|param| param.ty.clone()),
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
                let Some(function) = self.resolve_direct_callee_function(callee) else {
                    continue;
                };
                let Some(signature) = self.signatures.get(&function) else {
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
            Ty::Item { item_id, args, .. } => match &self.input.hir.item(*item_id).kind {
                ItemKind::Struct(_) => {
                    match self.struct_field_lowerings(ty, span, "task-handle alias analysis") {
                        Ok(fields) => fields.iter().any(|field| {
                            self.ty_contains_task_handles(&field.ty, span, diagnostics)
                        }),
                        Err(error) => {
                            diagnostics.push(error);
                            false
                        }
                    }
                }
                ItemKind::Enum(_) => {
                    match self.enum_lowering(ty, span, "task-handle alias analysis") {
                        Ok(lowering) => lowering
                            .variants
                            .iter()
                            .flat_map(|variant| variant.fields.iter())
                            .any(|field| {
                                self.ty_contains_task_handles(&field.ty, span, diagnostics)
                            }),
                        Err(error) => {
                            diagnostics.push(error);
                            false
                        }
                    }
                }
                ItemKind::TypeAlias(alias) if args.is_empty() => {
                    let target_ty = lower_type(self.input.hir, self.input.resolution, alias.ty);
                    self.ty_contains_task_handles(&target_ty, span, diagnostics)
                }
                _ => false,
            },
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
            Rvalue::Question(operand) => self.infer_operand_type(
                ctx.body,
                operand,
                ctx.local_types,
                ctx.async_task_handles,
                ctx.diagnostics,
                span,
            ),
            Rvalue::Call { callee, args } => {
                if let Some(function) = self.resolve_direct_callee_function(callee) {
                    let Some(signature) = self.signatures.get(&function) else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation could not resolve the direct callee declaration",
                        ));
                        return None;
                    };

                    let Some(ordered_args) = self.ordered_call_args(args, signature) else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation could not map call arguments to direct callee parameters",
                        ));
                        return None;
                    };

                    for arg in ordered_args {
                        let _ = self.infer_operand_type(
                            ctx.body,
                            &arg.value,
                            ctx.local_types,
                            ctx.async_task_handles,
                            ctx.diagnostics,
                            span,
                        );
                    }

                    if signature.is_async {
                        None
                    } else {
                        Some(signature.return_ty.clone())
                    }
                } else {
                    let Some(callee_ty) = self.infer_operand_type(
                        ctx.body,
                        callee,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    ) else {
                        return None;
                    };
                    let Ty::Callable { params, ret } = callee_ty else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation only supports direct resolved function calls or callable operands",
                        ));
                        return None;
                    };
                    if args.iter().any(|arg| arg.name.is_some()) || args.len() != params.len() {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation could not map call arguments to callable parameters",
                        ));
                        return None;
                    }

                    for (arg, param_ty) in args.iter().zip(params.iter()) {
                        let _ = self.infer_operand_type(
                            ctx.body,
                            &arg.value,
                            ctx.local_types,
                            ctx.async_task_handles,
                            ctx.diagnostics,
                            span,
                        );
                        let _ = param_ty;
                    }

                    Some(ret.as_ref().clone())
                }
            }
            Rvalue::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let Operand::Place(place) = left else {
                    ctx.diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation currently requires assignment expressions to target a mutable place",
                    ));
                    return None;
                };
                let target_ty = self.assignment_place_type(
                    ctx.body,
                    place,
                    ctx.local_types,
                    ctx.async_task_handles,
                    span,
                    ctx.diagnostics,
                )?;
                let value_ty = self.infer_operand_type(
                    ctx.body,
                    right,
                    ctx.local_types,
                    ctx.async_task_handles,
                    ctx.diagnostics,
                    span,
                )?;
                if !backend_value_compatible(
                    self.input.hir,
                    self.input.resolution,
                    &target_ty,
                    &value_ty,
                ) {
                    ctx.diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation currently requires assignment expressions to store a value compatible with the target place",
                    ));
                    return None;
                }
                Some(target_ty)
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
                UnaryOp::Not => {
                    let operand_ty = self.infer_operand_type(
                        ctx.body,
                        operand,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    )?;
                    if backend_value_is_bool(self.input.hir, self.input.resolution, &operand_ty) {
                        Some(Ty::Builtin(BuiltinType::Bool))
                    } else {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation only supports `!` on `Bool` operands",
                        ));
                        None
                    }
                }
                UnaryOp::Neg => {
                    let operand_ty = self.infer_operand_type(
                        ctx.body,
                        operand,
                        ctx.local_types,
                        ctx.async_task_handles,
                        ctx.diagnostics,
                        span,
                    )?;
                    let operand_ty = transparent_backend_value_ty(
                        self.input.hir,
                        self.input.resolution,
                        &operand_ty,
                    );
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
            Rvalue::RepeatArray { value, len } => {
                self.infer_repeat_array_rvalue_type(value, len, expected_ty, ctx, span)
            }
            Rvalue::AggregateTupleStruct { path, items } => {
                self.infer_tuple_struct_rvalue_type(path, items, expected_ty, ctx, span)
            }
            Rvalue::AggregateStruct { path, fields } => {
                self.infer_struct_rvalue_type(path, fields, expected_ty, ctx, span)
            }
            Rvalue::Closure { closure } => {
                let closure = ctx.body.closure(*closure);
                if closure.lowered_body.is_none() {
                    ctx.diagnostics
                        .push(self.capturing_closure_diagnostic(closure.span));
                    None
                } else {
                    self.input.typeck.expr_ty(closure.expr).cloned()
                }
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

        let field_layouts = match &struct_ty {
            Ty::Item { item_id, .. } => match &self.input.hir.item(*item_id).kind {
                ItemKind::Struct(_) => {
                    match self.struct_field_lowerings(&struct_ty, span, "struct value type") {
                        Ok(layouts) => layouts,
                        Err(error) => {
                            ctx.diagnostics.push(error);
                            return None;
                        }
                    }
                }
                ItemKind::Enum(_) => {
                    let enum_lowering =
                        match self.enum_lowering(&struct_ty, span, "enum value type") {
                            Ok(lowering) => lowering,
                            Err(error) => {
                                ctx.diagnostics.push(error);
                                return None;
                            }
                        };
                    let (_, variant) = match self.enum_variant_lowering_for_path(
                        &enum_lowering,
                        path,
                        &struct_ty,
                        span,
                        "enum value type",
                    ) {
                        Ok(variant) => variant,
                        Err(error) => {
                            ctx.diagnostics.push(error);
                            return None;
                        }
                    };
                    if variant.fields.iter().any(|field| field.name.is_none()) {
                        ctx.diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation only supports enum struct-variant literals here",
                        ));
                        return None;
                    }
                    variant
                        .fields
                        .iter()
                        .map(|field| StructFieldLowering {
                            name: field.name.clone().expect("checked named enum field"),
                            ty: field.ty.clone(),
                            llvm_ty: field.llvm_ty.clone(),
                        })
                        .collect()
                }
                _ => {
                    ctx.diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation could not resolve the struct type for this aggregate value",
                    ));
                    return None;
                }
            },
            _ => {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation could not resolve the struct type for this aggregate value",
                ));
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
            if !backend_value_compatible(
                self.input.hir,
                self.input.resolution,
                &expected_field.ty,
                &actual_ty,
            ) {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered a struct field value whose lowered type did not match the declaration",
                ));
            }
        }

        Some(struct_ty)
    }

    fn infer_tuple_struct_rvalue_type(
        &self,
        path: &ql_ast::Path,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        let enum_ty = expected_ty
            .filter(|ty| matches!(ty, Ty::Item { .. }))
            .cloned()
            .or_else(|| self.resolve_local_struct_path(path));
        let Some(enum_ty) = enum_ty else {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation could not resolve the enum type for this aggregate value",
            ));
            return None;
        };

        let enum_lowering = match self.enum_lowering(&enum_ty, span, "enum value type") {
            Ok(lowering) => lowering,
            Err(error) => {
                ctx.diagnostics.push(error);
                return None;
            }
        };
        let (_, variant) = match self.enum_variant_lowering_for_path(
            &enum_lowering,
            path,
            &enum_ty,
            span,
            "enum value type",
        ) {
            Ok(variant) => variant,
            Err(error) => {
                ctx.diagnostics.push(error);
                return None;
            }
        };
        if variant.fields.iter().any(|field| field.name.is_some()) {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation only supports enum unit/tuple-variant literals here",
            ));
            return None;
        }
        if items.len() != variant.fields.len() {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation encountered an enum variant literal with the wrong arity",
            ));
            return None;
        }

        for (item, expected_field) in items.iter().zip(variant.fields.iter()) {
            let Some(actual_ty) = self.infer_operand_type(
                ctx.body,
                item,
                ctx.local_types,
                ctx.async_task_handles,
                ctx.diagnostics,
                span,
            ) else {
                continue;
            };
            if !backend_value_compatible(
                self.input.hir,
                self.input.resolution,
                &expected_field.ty,
                &actual_ty,
            ) {
                ctx.diagnostics.push(unsupported(
                    span,
                    "LLVM IR backend foundation encountered an enum variant value whose lowered type did not match the declaration",
                ));
            }
        }

        Some(enum_ty)
    }

    fn infer_array_rvalue_type(
        &self,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        let expected_array = match expected_ty {
            Some(Ty::Array { element, len }) => Some((element.as_ref().clone(), len.clone())),
            _ => None,
        };
        if let Some((_, expected_len)) = expected_array.as_ref()
            && let Some(expected_len) = known_array_len(expected_len)
            && items.len() != expected_len
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
            if !backend_value_compatible(
                self.input.hir,
                self.input.resolution,
                expected_element,
                &item_ty,
            ) {
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
            len: TyArrayLen::Known(items.len()),
        })
    }

    fn infer_repeat_array_rvalue_type(
        &self,
        value: &Operand,
        len: &hir::ArrayLen,
        expected_ty: Option<&Ty>,
        ctx: &mut TypeInferenceContext<'_>,
        span: Span,
    ) -> Option<Ty> {
        let expected_array = match expected_ty {
            Some(Ty::Array { element, len }) => Some((element.as_ref().clone(), len.clone())),
            _ => None,
        };
        let repeat_len = match len {
            hir::ArrayLen::Known(len) => TyArrayLen::Known(*len),
            hir::ArrayLen::Generic(name) => TyArrayLen::Generic(name.clone()),
        };
        if let Some((_, expected_len)) = expected_array.as_ref()
            && !expected_len.compatible_with(&repeat_len)
        {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation encountered a repeat-array literal whose length no longer matches the expected array type",
            ));
            return None;
        }

        let value_ty = self.infer_operand_type(
            ctx.body,
            value,
            ctx.local_types,
            ctx.async_task_handles,
            ctx.diagnostics,
            span,
        )?;
        let element_ty = expected_array
            .as_ref()
            .map(|(element, _)| element.clone())
            .unwrap_or_else(|| value_ty.clone());
        if !backend_value_compatible(
            self.input.hir,
            self.input.resolution,
            &element_ty,
            &value_ty,
        ) {
            ctx.diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation currently requires repeat-array values to match the expected element type",
            ));
            return None;
        }

        Some(Ty::Array {
            element: Box::new(element_ty),
            len: repeat_len,
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
                Constant::String { .. } => Some(Ty::Builtin(BuiltinType::String)),
                Constant::Bool(_) => Some(Ty::Builtin(BuiltinType::Bool)),
                Constant::Void => Some(void_ty()),
                Constant::Function { function, .. } => {
                    self.infer_function_value_type(*function, diagnostics, span)
                }
                Constant::Item { item, .. } => match &self.input.hir.item(*item).kind {
                    ItemKind::Const(_) | ItemKind::Static(_) => {
                        self.infer_const_or_static_item_type(*item, diagnostics, span)
                    }
                    _ => {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support item values here",
                        ));
                        None
                    }
                },
                Constant::None => {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation does not support `none` yet",
                    ));
                    None
                }
                Constant::Import(path) => local_item_for_import_path(self.input.hir, path)
                    .and_then(|item_id| match &self.input.hir.item(item_id).kind {
                        ItemKind::Function(_) => self.infer_function_value_type(
                            FunctionRef::Item(item_id),
                            diagnostics,
                            span,
                        ),
                        ItemKind::Const(_) | ItemKind::Static(_) => {
                            self.infer_const_or_static_item_type(item_id, diagnostics, span)
                        }
                        _ => None,
                    })
                    .or_else(|| {
                        diagnostics.push(unsupported(
                            span,
                            "LLVM IR backend foundation does not support imported value lowering yet",
                        ));
                        None
                    }),
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
                    && !backend_value_compatible(
                        self.input.hir,
                        self.input.resolution,
                        &Ty::Builtin(BuiltinType::Int),
                        &index_ty,
                    )
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
        let raw = match index {
            Operand::Constant(Constant::Integer(raw)) => raw.as_str(),
            Operand::Constant(Constant::Item { item, .. }) => {
                let Some(value) = const_or_static_item_integer_literal(
                    self.input.hir,
                    self.input.resolution,
                    *item,
                ) else {
                    return Err(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use an integer literal index"
                        ),
                    ));
                };
                return ql_ast::parse_usize_literal(&value.to_string()).ok_or_else(|| {
                    unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use a non-negative integer literal index"
                        ),
                    )
                });
            }
            Operand::Constant(Constant::Import(path)) => {
                let Some(item_id) = local_item_for_import_path(self.input.hir, path) else {
                    return Err(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use an integer literal index"
                        ),
                    ));
                };
                let Some(value) = const_or_static_item_integer_literal(
                    self.input.hir,
                    self.input.resolution,
                    item_id,
                ) else {
                    return Err(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use an integer literal index"
                        ),
                    ));
                };
                return ql_ast::parse_usize_literal(&value.to_string()).ok_or_else(|| {
                    unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use a non-negative integer literal index"
                        ),
                    )
                });
            }
            _ => {
                return Err(unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation currently requires tuple projection on `{current_ty}` to use an integer literal index"
                    ),
                ));
            }
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
        let left_ty = transparent_backend_value_ty(self.input.hir, self.input.resolution, left_ty);
        let right_ty =
            transparent_backend_value_ty(self.input.hir, self.input.resolution, right_ty);

        if left_ty != right_ty {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation currently requires binary operands to have the same lowered type",
            ));
            return None;
        }

        match op {
            BinaryOp::OrOr | BinaryOp::AndAnd => {
                panic!("short-circuit boolean operators should lower structurally in MIR")
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                if is_numeric_ty(&left_ty) {
                    Some(left_ty)
                } else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports numeric arithmetic",
                    ));
                    None
                }
            }
            BinaryOp::EqEq | BinaryOp::BangEq => {
                if is_comparable_ty(&left_ty) || matches!(left_ty, Ty::Builtin(BuiltinType::String))
                {
                    Some(Ty::Builtin(BuiltinType::Bool))
                } else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports integer, float, bool, or string equality comparisons",
                    ));
                    None
                }
            }
            BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq => {
                if is_comparable_ty(&left_ty) || matches!(left_ty, Ty::Builtin(BuiltinType::String))
                {
                    Some(Ty::Builtin(BuiltinType::Bool))
                } else {
                    diagnostics.push(unsupported(
                        span,
                        "LLVM IR backend foundation only supports integer, float, bool, or string ordered comparisons",
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
                    && !backend_value_compatible(
                        self.input.hir,
                        self.input.resolution,
                        &Ty::Builtin(BuiltinType::Int),
                        &index_ty,
                    )
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

    fn require_supported_bind_pattern(
        &self,
        pattern: hir::PatternId,
        span: Span,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let supported = match pattern_kind(self.input.hir, pattern) {
            PatternKind::Binding(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::Wildcard => true,
            PatternKind::Path(_) => {
                pattern_literal_bool(self.input.hir, self.input.resolution, pattern).is_some()
                    || pattern_literal_int(self.input.hir, self.input.resolution, pattern).is_some()
                    || pattern_literal_string(self.input.hir, self.input.resolution, pattern)
                        .is_some()
                    || resolved_enum_variant_index_for_pattern(
                        self.input.hir,
                        self.input.resolution,
                        pattern,
                    )
                    .is_some()
            }
            PatternKind::Tuple(items) => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::Array(items) => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::TupleStruct { items, .. } => items
                .iter()
                .all(|item| self.supports_destructuring_bind_pattern(*item)),
            PatternKind::Struct { fields, .. } => fields
                .iter()
                .all(|field| self.supports_destructuring_bind_pattern(field.pattern)),
            _ => false,
        };

        if !supported {
            diagnostics.push(unsupported(
                span,
                "LLVM IR backend foundation only supports binding, wildcard, literal, or tuple/tuple-struct/struct/fixed-array destructuring binding patterns",
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
            FunctionRef::TraitMethod { item, index } => match &self.input.hir.item(item).kind {
                ItemKind::Trait(trait_decl) => Ok(trait_decl
                    .methods
                    .get(index)
                    .expect("trait method index should be valid")),
                _ => Err(vec![
                    Diagnostic::error(
                        "LLVM IR backend foundation expected a trait method declaration",
                    )
                    .with_label(Label::new(self.input.hir.item(item).span)),
                ]),
            },
            FunctionRef::ImplMethod { item, index } => match &self.input.hir.item(item).kind {
                ItemKind::Impl(impl_block) => Ok(impl_block
                    .methods
                    .get(index)
                    .expect("impl method index should be valid")),
                _ => Err(vec![
                    Diagnostic::error(
                        "LLVM IR backend foundation expected an impl method declaration",
                    )
                    .with_label(Label::new(self.input.hir.item(item).span)),
                ]),
            },
            FunctionRef::ExtendMethod { item, index } => match &self.input.hir.item(item).kind {
                ItemKind::Extend(extend_block) => Ok(extend_block
                    .methods
                    .get(index)
                    .expect("extend method index should be valid")),
                _ => Err(vec![
                    Diagnostic::error(
                        "LLVM IR backend foundation expected an extend method declaration",
                    )
                    .with_label(Label::new(self.input.hir.item(item).span)),
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
        closures: &[PreparedFunction],
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
        output.push('\n');
        output.push_str("declare i32 @memcmp(ptr, ptr, i64)\n");

        if !self.input.runtime_hooks.is_empty() {
            output.push('\n');
            self.render_runtime_hook_declarations(&mut output);
            self.render_runtime_heap_declarations(&mut output);
            self.render_program_runtime_support(&mut output);
        }

        if !self.string_literal_llvm_names.is_empty() {
            output.push('\n');
            self.render_string_literal_globals(&mut output);
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

        let mut closures = closures.iter().collect::<Vec<_>>();
        closures.sort_by(|left, right| left.signature.llvm_name.cmp(&right.signature.llvm_name));
        for closure in closures {
            output.push('\n');
            self.render_function(&mut output, closure);
        }

        output
    }

    fn render_string_literal_globals(&self, output: &mut String) {
        let mut literals = self.string_literal_llvm_names.iter().collect::<Vec<_>>();
        literals.sort_by(|left, right| left.1.cmp(right.1));

        for (key, llvm_name) in literals {
            let llvm_bytes = llvm_string_literal_bytes(&key.value);
            let len = key.value.as_bytes().len() + 1;
            let _ = writeln!(
                output,
                "@{llvm_name} = private unnamed_addr constant [{len} x i8] c\"{llvm_bytes}\""
            );
        }
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
        let body = &function.body;
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
        for block_id in body.block_ids() {
            let Some(loop_lowering) = function.supported_for_loops.get(&block_id) else {
                continue;
            };
            let SupportedForLoopIterableRoot::Item(item_id) = &loop_lowering.iterable_root else {
                continue;
            };
            let Some(ty) =
                const_or_static_item_type(self.input.hir, self.input.resolution, *item_id)
            else {
                panic!(
                    "prepared `for` lowering for block {:?} should only materialize const/static iterable roots",
                    block_id
                );
            };
            let llvm_ty = self
                .lower_llvm_type(
                    &ty,
                    body.block(block_id).terminator.span,
                    "for iterable type",
                )
                .expect("prepared const/static `for` iterable should have a lowered LLVM type");
            let _ = writeln!(
                output,
                "  {} = alloca {}",
                for_iterable_slot_name(block_id),
                llvm_ty
            );
        }

        for local_id in body.local_ids() {
            let local = body.local(local_id);
            let Some(index) = lowered_param_index(local) else {
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
        for (index, local_id) in function.param_binding_locals.iter().enumerate() {
            let param = &function.signature.params[index];
            let _ = writeln!(
                output,
                "  store {} %arg{}, ptr {}",
                param.llvm_ty,
                index,
                llvm_slot_name(body, *local_id)
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

        let mut renderer = FunctionRenderer {
            emitter: self,
            body,
            prepared: function,
            next_temp: 0,
            cleanup_loop_labels: Vec::new(),
            cleanup_path_open: true,
            cleanup_bindings: Vec::new(),
            cleanup_capturing_closure_bindings: Vec::new(),
        };
        for block_id in body.block_ids() {
            let Some(loop_lowering) = renderer
                .prepared
                .supported_for_loops
                .get(&block_id)
                .cloned()
            else {
                continue;
            };
            renderer.render_for_loop_iterable_initialization(output, block_id, &loop_lowering);
        }

        let _ = writeln!(output, "  br label %bb{}", body.entry.index());

        for block_id in body.block_ids() {
            let block = body.block(block_id);
            let _ = writeln!(output, "bb{}:", block_id.index());
            for statement_id in &block.statements {
                renderer.render_statement(output, block_id, body.statement(*statement_id));
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
                let Some(len) = known_array_len(len) else {
                    return Err(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation requires concrete array length for {context} `{ty}`"
                        ),
                    ));
                };
                Ok(format!("[{len} x {element_llvm_ty}]"))
            }
            Ty::Builtin(BuiltinType::String) => Ok(llvm_string_aggregate_ty().to_owned()),
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
            Ty::Item { item_id, args, .. } => match &self.input.hir.item(*item_id).kind {
                ItemKind::Struct(_) => {
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
                ItemKind::Enum(_) => Ok(self.enum_lowering(ty, span, context)?.llvm_ty),
                ItemKind::TypeAlias(alias) if args.is_empty() => {
                    let target_ty = lower_type(self.input.hir, self.input.resolution, alias.ty);
                    self.lower_llvm_type(&target_ty, span, context)
                }
                _ => lower_llvm_type(ty, span, context),
            },
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
                let Some(len) = known_array_len(len) else {
                    return Err(unsupported(
                        span,
                        format!(
                            "LLVM IR backend foundation requires concrete array length for {context} `{ty}`"
                        ),
                    ));
                };
                Ok(LoadableAbiLayout {
                    size: element.size * (len as u64),
                    align: element.align,
                })
            }
            Ty::Builtin(BuiltinType::String) => Ok(string_loadable_abi_layout()),
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
            Ty::Item { item_id, args, .. } => match &self.input.hir.item(*item_id).kind {
                ItemKind::Struct(_) => {
                    // Keep struct layout recursive so async payloads and projected reads share
                    // one aggregate contract instead of growing per-shape special cases.
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
                ItemKind::Enum(_) => Ok(self.enum_lowering(ty, span, context)?.layout),
                ItemKind::TypeAlias(alias) if args.is_empty() => {
                    let target_ty = lower_type(self.input.hir, self.input.resolution, alias.ty);
                    self.loadable_abi_layout(&target_ty, span, context)
                }
                _ => {
                    let layout = scalar_abi_layout(ty, span, context)?;
                    Ok(LoadableAbiLayout {
                        size: layout.size,
                        align: layout.align,
                    })
                }
            },
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
        let item = self.input.hir.item(*item_id);
        let ItemKind::Struct(struct_decl) = &item.kind else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        let substitutions =
            self.generic_type_substitutions(&struct_decl.generics, args, ty, span, context)?;

        struct_decl
            .fields
            .iter()
            .map(|field| {
                let ty = self.substitute_generic_ty(
                    lower_type(self.input.hir, self.input.resolution, field.ty),
                    &substitutions,
                );
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

    fn enum_lowering(
        &self,
        ty: &Ty,
        span: Span,
        context: &str,
    ) -> Result<EnumLowering, Diagnostic> {
        let Ty::Item { item_id, args, .. } = ty else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        let item = self.input.hir.item(*item_id);
        let ItemKind::Enum(enum_decl) = &item.kind else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        let substitutions =
            self.generic_type_substitutions(&enum_decl.generics, args, ty, span, context)?;

        let mut variants = Vec::with_capacity(enum_decl.variants.len());
        let mut max_payload_size = 0;
        let mut max_payload_align = 1;

        for variant in &enum_decl.variants {
            let mut fields = Vec::new();
            let mut payload_size = 0;
            let mut payload_align = 1;

            match &variant.fields {
                hir::VariantFields::Unit => {}
                hir::VariantFields::Tuple(items) => {
                    for &type_id in items {
                        let ty = self.substitute_generic_ty(
                            lower_type(self.input.hir, self.input.resolution, type_id),
                            &substitutions,
                        );
                        if is_void_ty(&ty) {
                            return Err(Diagnostic::error(format!(
                                "LLVM IR backend foundation does not support {context} `{ty}` yet"
                            ))
                            .with_label(Label::new(span)));
                        }
                        let llvm_ty = self.lower_llvm_type(&ty, span, context)?;
                        let layout = self.loadable_abi_layout(&ty, span, context)?;
                        payload_size = align_to(payload_size, layout.align);
                        payload_size += layout.size;
                        payload_align = payload_align.max(layout.align);
                        fields.push(EnumVariantFieldLowering {
                            name: None,
                            ty,
                            llvm_ty,
                        });
                    }
                }
                hir::VariantFields::Struct(named_fields) => {
                    for field in named_fields {
                        let ty = self.substitute_generic_ty(
                            lower_type(self.input.hir, self.input.resolution, field.ty),
                            &substitutions,
                        );
                        if is_void_ty(&ty) {
                            return Err(Diagnostic::error(format!(
                                "LLVM IR backend foundation does not support {context} `{ty}` yet"
                            ))
                            .with_label(Label::new(span)));
                        }
                        let llvm_ty = self.lower_llvm_type(&ty, span, context)?;
                        let layout = self.loadable_abi_layout(&ty, span, context)?;
                        payload_size = align_to(payload_size, layout.align);
                        payload_size += layout.size;
                        payload_align = payload_align.max(layout.align);
                        fields.push(EnumVariantFieldLowering {
                            name: Some(field.name.clone()),
                            ty,
                            llvm_ty,
                        });
                    }
                }
            }

            let payload_llvm_ty = (!fields.is_empty()).then(|| {
                format!(
                    "{{ {} }}",
                    fields
                        .iter()
                        .map(|field| field.llvm_ty.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
            if !fields.is_empty() {
                max_payload_size = max_payload_size.max(align_to(payload_size, payload_align));
                max_payload_align = max_payload_align.max(payload_align);
            }

            variants.push(EnumVariantLowering {
                name: variant.name.clone(),
                fields,
                payload_llvm_ty,
            });
        }

        let storage = if max_payload_size == 0 {
            None
        } else {
            Some(self.enum_payload_storage_lowering(
                max_payload_size,
                max_payload_align,
                span,
                context,
            )?)
        };

        let layout = if let Some(storage) = &storage {
            let size = align_to(8, storage.align) + storage.size;
            let align = 8u64.max(storage.align);
            LoadableAbiLayout {
                size: align_to(size, align),
                align,
            }
        } else {
            LoadableAbiLayout { size: 8, align: 8 }
        };
        let llvm_ty = if let Some(storage) = &storage {
            format!("{{ i64, {} }}", storage.llvm_ty)
        } else {
            "{ i64 }".to_owned()
        };

        Ok(EnumLowering {
            llvm_ty,
            layout,
            storage,
            variants,
        })
    }

    fn enum_payload_storage_lowering(
        &self,
        size: u64,
        align: u64,
        span: Span,
        context: &str,
    ) -> Result<EnumPayloadStorageLowering, Diagnostic> {
        let element_llvm_ty = match align {
            1 => "i8",
            2 => "i16",
            4 => "i32",
            8 => "i64",
            _ => {
                return Err(Diagnostic::error(format!(
                    "LLVM IR backend foundation does not support {context} payload alignment `{align}` yet"
                ))
                .with_label(Label::new(span)))
            }
        };
        let count = size.div_ceil(align);
        Ok(EnumPayloadStorageLowering {
            llvm_ty: format!("[{count} x {element_llvm_ty}]"),
            size: count * align,
            align,
        })
    }

    fn generic_type_substitutions(
        &self,
        generics: &[hir::GenericParam],
        args: &[Ty],
        ty: &Ty,
        span: Span,
        context: &str,
    ) -> Result<BTreeMap<String, Ty>, Diagnostic> {
        if generics.is_empty() && args.is_empty() {
            return Ok(BTreeMap::new());
        }
        if generics.len() != args.len() {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        }
        Ok(generics
            .iter()
            .zip(args.iter())
            .map(|(generic, arg)| (generic.name.clone(), arg.clone()))
            .collect())
    }

    fn substitute_generic_ty(&self, ty: Ty, substitutions: &BTreeMap<String, Ty>) -> Ty {
        if substitutions.is_empty() {
            return ty;
        }
        match ty {
            Ty::Generic(name) => substitutions
                .get(&name)
                .cloned()
                .unwrap_or(Ty::Generic(name)),
            Ty::Array { element, len } => Ty::Array {
                element: Box::new(self.substitute_generic_ty(*element, substitutions)),
                len,
            },
            Ty::Item {
                item_id,
                name,
                args,
            } => Ty::Item {
                item_id,
                name,
                args: args
                    .into_iter()
                    .map(|arg| self.substitute_generic_ty(arg, substitutions))
                    .collect(),
            },
            Ty::Import { path, args } => Ty::Import {
                path,
                args: args
                    .into_iter()
                    .map(|arg| self.substitute_generic_ty(arg, substitutions))
                    .collect(),
            },
            Ty::Named { path, args } => Ty::Named {
                path,
                args: args
                    .into_iter()
                    .map(|arg| self.substitute_generic_ty(arg, substitutions))
                    .collect(),
            },
            Ty::Pointer { is_const, inner } => Ty::Pointer {
                is_const,
                inner: Box::new(self.substitute_generic_ty(*inner, substitutions)),
            },
            Ty::Tuple(items) => Ty::Tuple(
                items
                    .into_iter()
                    .map(|item| self.substitute_generic_ty(item, substitutions))
                    .collect(),
            ),
            Ty::TaskHandle(output) => {
                Ty::TaskHandle(Box::new(self.substitute_generic_ty(*output, substitutions)))
            }
            Ty::Callable { params, ret } => Ty::Callable {
                params: params
                    .into_iter()
                    .map(|param| self.substitute_generic_ty(param, substitutions))
                    .collect(),
                ret: Box::new(self.substitute_generic_ty(*ret, substitutions)),
            },
            other => other,
        }
    }

    fn enum_variant_lowering_for_path<'b>(
        &self,
        enum_lowering: &'b EnumLowering,
        path: &ql_ast::Path,
        ty: &Ty,
        span: Span,
        context: &str,
    ) -> Result<(usize, &'b EnumVariantLowering), Diagnostic> {
        let Some(variant_name) = path.segments.last() else {
            return Err(Diagnostic::error(format!(
                "LLVM IR backend foundation does not support {context} `{ty}` yet"
            ))
            .with_label(Label::new(span)));
        };
        enum_lowering
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| variant.name == *variant_name)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "LLVM IR backend foundation could not resolve enum variant `{variant_name}` on `{ty}`"
                ))
                .with_label(Label::new(span))
            })
    }

    fn resolve_local_struct_path(&self, path: &ql_ast::Path) -> Option<Ty> {
        let name = path.segments.first()?;
        let mut candidates = HashSet::new();

        for item_id in self.input.hir.items.iter().copied() {
            if matches!(
                &self.input.hir.item(item_id).kind,
                ItemKind::Struct(struct_decl) if path.segments.len() == 1 && struct_decl.name == *name
            ) || matches!(
                &self.input.hir.item(item_id).kind,
                ItemKind::Enum(enum_decl) if enum_decl.name == *name
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
                            ) || matches!(
                                &self.input.hir.item(item_id).kind,
                                ItemKind::Enum(enum_decl) if enum_decl.name == *target_name
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
            ItemKind::Enum(enum_decl) => Some(Ty::Item {
                item_id,
                name: enum_decl.name.clone(),
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
    cleanup_loop_labels: Vec<CleanupLoopLabels>,
    cleanup_path_open: bool,
    cleanup_bindings: Vec<GuardBindingValue>,
    cleanup_capturing_closure_bindings: Vec<CleanupCapturingClosureBinding>,
}

struct CleanupLoopLabels {
    continue_label: String,
    break_label: String,
}

impl<'a, 'b> FunctionRenderer<'a, 'b> {
    fn cleanup_binding(&self, local: hir::LocalId) -> Option<&LoweredValue> {
        self.cleanup_bindings
            .iter()
            .rev()
            .find(|binding| binding.local == local)
            .map(|binding| &binding.value)
    }

    fn materialize_cleanup_binding_root(
        &mut self,
        output: &mut String,
        local: hir::LocalId,
    ) -> Option<(String, Ty)> {
        let value = self.cleanup_binding(local)?.clone();
        let slot = self.fresh_temp();
        let _ = writeln!(output, "  {slot} = alloca {}", value.llvm_ty);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {slot}",
            value.llvm_ty, value.repr
        );
        Some((slot, value.ty))
    }

    fn render_statement(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        statement: &mir::Statement,
    ) {
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                let expected_ty = self.prepared_place_type(place, statement.span);
                let target_is_void = expected_ty.as_ref().is_some_and(is_void_ty);
                if let Some(rendered) = self.render_rvalue(
                    output,
                    block_id,
                    value,
                    expected_ty.as_ref(),
                    statement.span,
                ) && !target_is_void
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
            } => match pattern_kind(self.emitter.input.hir, *pattern) {
                PatternKind::Binding(_)
                | PatternKind::Tuple(_)
                | PatternKind::Array(_)
                | PatternKind::TupleStruct { .. }
                | PatternKind::Struct { .. } => {
                    let rendered = self.render_operand(output, source, statement.span);
                    self.bind_pattern_value(output, *pattern, rendered, statement.span);
                }
                PatternKind::Path(_)
                | PatternKind::Integer(_)
                | PatternKind::String(_)
                | PatternKind::Bool(_)
                | PatternKind::Wildcard => {}
                _ => panic!("prepared patterns should only contain supported bind patterns"),
            },
            StatementKind::Eval { value } => {
                let _ = self.render_rvalue(output, block_id, value, None, statement.span);
            }
            StatementKind::StorageLive { .. } | StatementKind::StorageDead { .. } => {}
            StatementKind::RegisterCleanup { .. } => {}
            StatementKind::RunCleanup { cleanup } => {
                self.render_cleanup_action(output, *cleanup, statement.span);
            }
        }
    }

    fn render_cleanup_action(&mut self, output: &mut String, cleanup: mir::CleanupId, span: Span) {
        self.cleanup_loop_labels.clear();
        self.cleanup_path_open = true;
        match &self.body.cleanup(cleanup).kind {
            mir::CleanupKind::Defer { expr } => self.render_cleanup_expr(output, *expr, span),
        }
    }

    fn render_cleanup_block_prefix(
        &mut self,
        output: &mut String,
        block_id: hir::BlockId,
        span: Span,
    ) -> Option<hir::ExprId> {
        let block = self.emitter.input.hir.block(block_id);
        for statement_id in &block.statements {
            if !self.cleanup_path_open {
                return None;
            }
            self.render_cleanup_statement(output, *statement_id, span);
        }
        if self.cleanup_path_open {
            block.tail
        } else {
            None
        }
    }

    fn render_cleanup_block(&mut self, output: &mut String, block_id: hir::BlockId, span: Span) {
        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        if let Some(tail) = self.render_cleanup_block_prefix(output, block_id, span) {
            self.render_cleanup_expr(output, tail, span);
        }
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
    }

    fn render_cleanup_statement(
        &mut self,
        output: &mut String,
        statement_id: hir::StmtId,
        span: Span,
    ) {
        match &self.emitter.input.hir.stmt(statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                self.render_cleanup_let(output, *pattern, *value, span)
            }
            hir::StmtKind::Expr { expr, .. } => {
                if let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &self.emitter.input.hir.expr(*expr).kind
                    && let Some(binding) = cleanup_capturing_closure_assignment(
                        self.emitter.input.hir,
                        self.emitter.input.resolution,
                        self.body,
                        &self.prepared.direct_local_capturing_closures,
                        &self.cleanup_capturing_closure_bindings,
                        *left,
                        *right,
                    )
                {
                    self.cleanup_capturing_closure_bindings.push(
                        CleanupCapturingClosureBinding::direct(binding.local, binding.closure_id),
                    );
                    return;
                }
                self.render_cleanup_expr(output, *expr, span)
            }
            hir::StmtKind::While { condition, body } => {
                self.render_cleanup_while(output, *condition, *body, span)
            }
            hir::StmtKind::Loop { body } => self.render_cleanup_loop(output, *body, span),
            hir::StmtKind::For {
                is_await,
                pattern,
                iterable,
                body,
            } => self.render_cleanup_for(output, *is_await, *pattern, *iterable, *body, span),
            hir::StmtKind::Break => {
                let labels = self.cleanup_loop_labels.last().unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render `break` inside cleanup loops"
                    )
                });
                let _ = writeln!(output, "  br label %{}", labels.break_label);
                self.cleanup_path_open = false;
            }
            hir::StmtKind::Continue => {
                let labels = self.cleanup_loop_labels.last().unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render `continue` inside cleanup loops"
                    )
                });
                let _ = writeln!(output, "  br label %{}", labels.continue_label);
                self.cleanup_path_open = false;
            }
            hir::StmtKind::Return(_) | hir::StmtKind::Defer(_) => {
                panic!(
                    "supported cleanup lowering at {span:?} should only render let, expr, while, loop, for, break, or continue statements inside cleanup blocks"
                )
            }
        }
    }

    fn render_cleanup_let(
        &mut self,
        output: &mut String,
        pattern: hir::PatternId,
        value: hir::ExprId,
        span: Span,
    ) {
        if let PatternKind::Binding(local) = pattern_kind(self.emitter.input.hir, pattern) {
            if let Some(binding_value) = cleanup_bound_capturing_closure_value_for_expr(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                &self.cleanup_capturing_closure_bindings,
                value,
            ) {
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                return;
            }
            if let Some(callee) =
                self.supported_direct_cleanup_capturing_closure_callee_for_expr(value)
            {
                if let CleanupDirectCapturingClosureExpr::Assignment(binding) = callee {
                    self.cleanup_capturing_closure_bindings.push(
                        CleanupCapturingClosureBinding::direct(binding.local, binding.closure_id),
                    );
                }
                self.cleanup_capturing_closure_bindings.push(
                    CleanupCapturingClosureBinding::direct(*local, callee.closure_id()),
                );
                return;
            }
            if let Some(binding) = cleanup_supported_shared_local_if_capturing_closure_binding(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                self.body,
                &self.prepared.direct_local_capturing_closures,
                &self.cleanup_capturing_closure_bindings,
                value,
            ) {
                let rendered_binding = self
                    .render_cleanup_shared_local_if_capturing_closure_binding(
                        output, binding, span,
                    );
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding {
                        local: binding.target_local,
                        value: rendered_binding.clone(),
                    });
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: rendered_binding,
                    });
                return;
            }
            if let Some(binding) = cleanup_supported_shared_local_match_capturing_closure_binding(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                self.body,
                &self.prepared.direct_local_capturing_closures,
                &self.cleanup_capturing_closure_bindings,
                value,
            ) {
                let target_local = match &binding {
                    SupportedCleanupSharedLocalMatchBinding::Tagged { target_local, .. }
                    | SupportedCleanupSharedLocalMatchBinding::Bool { target_local, .. }
                    | SupportedCleanupSharedLocalMatchBinding::Integer { target_local, .. } => {
                        *target_local
                    }
                };
                let rendered_binding = self
                    .render_cleanup_shared_local_match_capturing_closure_binding(
                        output, &binding, span,
                    );
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding {
                        local: target_local,
                        value: rendered_binding.clone(),
                    });
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: rendered_binding,
                    });
                return;
            }
            if let Some(closure_id) = cleanup_supported_capturing_closure_callee_closure(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                self.body,
                &self.prepared.direct_local_capturing_closures,
                &self.cleanup_capturing_closure_bindings,
                value,
            ) {
                if cleanup_capturing_closure_binding_requires_runtime_eval(
                    self.emitter.input.hir,
                    value,
                ) {
                    self.render_cleanup_capturing_closure_binding_expr(output, value, span);
                }
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding::direct(*local, closure_id));
                return;
            }
            if let Some(closure_id) = self.supported_direct_local_capturing_closure_for_expr(value)
            {
                self.cleanup_capturing_closure_bindings
                    .push(CleanupCapturingClosureBinding::direct(*local, closure_id));
                return;
            }
        }
        let expected_ty = self
            .emitter
            .cleanup_let_expected_ty(pattern, value)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render binding, wildcard, tuple, or struct cleanup patterns"
                )
            });
        let rendered = self.render_cleanup_value_expr(output, value, &expected_ty, span);
        self.bind_cleanup_pattern(output, pattern, rendered, span);
    }

    fn render_cleanup_shared_local_if_capturing_closure_binding(
        &mut self,
        output: &mut String,
        binding: SupportedCleanupSharedLocalIfBinding,
        span: Span,
    ) -> CleanupCapturingClosureBindingValue {
        let condition = self.render_bool_guard_expr(output, binding.condition_expr, span, None);
        let then_label = self.fresh_label("cleanup_then");
        let else_label = self.fresh_label("cleanup_else");
        let end_label = self.fresh_label("cleanup_end");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        self.cleanup_path_open = true;
        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        if let Some(tail) = self.render_cleanup_block_prefix(output, binding.then_branch, span) {
            self.render_cleanup_capturing_closure_binding_expr(output, tail, span);
        }
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        self.cleanup_path_open = true;
        self.render_cleanup_capturing_closure_binding_expr(output, binding.else_expr, span);
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;

        CleanupCapturingClosureBindingValue::IfBranch {
            condition: Some(condition),
            then_closure: binding.then_closure,
            else_closure: binding.else_closure,
        }
    }

    fn render_cleanup_shared_local_match_capturing_closure_binding(
        &mut self,
        output: &mut String,
        binding: &SupportedCleanupSharedLocalMatchBinding,
        span: Span,
    ) -> CleanupCapturingClosureBindingValue {
        match binding {
            SupportedCleanupSharedLocalMatchBinding::Tagged {
                expr_id, closures, ..
            } => {
                let tag_ty = Ty::Builtin(BuiltinType::Int);
                let tag_llvm_ty = self
                    .emitter
                    .lower_llvm_type(&tag_ty, span, "cleanup match tag")
                    .expect("supported cleanup lowering should lower cleanup match tags");
                let tag_slot = self.fresh_temp();
                let _ = writeln!(output, "  {tag_slot} = alloca {tag_llvm_ty}");
                let hir::ExprKind::Match { value, arms } =
                    &self.emitter.input.hir.expr(*expr_id).kind
                else {
                    panic!(
                        "supported cleanup lowering at {span:?} should preserve tagged match binding shape"
                    )
                };
                self.render_cleanup_capturing_closure_binding_match_expr(
                    output,
                    *value,
                    arms,
                    span,
                    Some(&tag_slot),
                );
                let tag = self.render_loaded_pointer_value(output, tag_slot, tag_ty, span);
                CleanupCapturingClosureBindingValue::TaggedMatch {
                    tag: Some(tag),
                    closures: closures.clone(),
                }
            }
            SupportedCleanupSharedLocalMatchBinding::Bool {
                expr_id,
                true_closure,
                false_closure,
                ..
            } => {
                let hir::ExprKind::Match { value, arms } =
                    &self.emitter.input.hir.expr(*expr_id).kind
                else {
                    panic!(
                        "supported cleanup lowering at {span:?} should preserve bool match binding shape"
                    )
                };
                let scrutinee = self.render_cleanup_value_expr(
                    output,
                    *value,
                    &Ty::Builtin(BuiltinType::Bool),
                    span,
                );
                self.render_cleanup_capturing_closure_binding_bool_match_expr(
                    output,
                    scrutinee.clone(),
                    arms,
                    span,
                    None,
                );
                CleanupCapturingClosureBindingValue::BoolMatch {
                    scrutinee: Some(scrutinee),
                    true_closure: *true_closure,
                    false_closure: *false_closure,
                }
            }
            SupportedCleanupSharedLocalMatchBinding::Integer {
                expr_id,
                arms,
                fallback_closure,
                ..
            } => {
                let hir::ExprKind::Match {
                    value,
                    arms: match_arms,
                } = &self.emitter.input.hir.expr(*expr_id).kind
                else {
                    panic!(
                        "supported cleanup lowering at {span:?} should preserve integer match binding shape"
                    )
                };
                let scrutinee = self.render_cleanup_value_expr(
                    output,
                    *value,
                    &Ty::Builtin(BuiltinType::Int),
                    span,
                );
                self.render_cleanup_capturing_closure_binding_integer_match_expr(
                    output,
                    scrutinee.clone(),
                    match_arms,
                    span,
                    None,
                );
                CleanupCapturingClosureBindingValue::IntegerMatch {
                    scrutinee: Some(scrutinee),
                    arms: arms.clone(),
                    fallback_closure: *fallback_closure,
                }
            }
        }
    }

    fn render_cleanup_capturing_closure_binding_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
    ) {
        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Question(inner) => {
                self.render_cleanup_capturing_closure_binding_expr(output, *inner, span);
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                let binding_depth = self.cleanup_bindings.len();
                let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
                if let Some(tail) = self.render_cleanup_block_prefix(output, *block_id, span) {
                    self.render_cleanup_capturing_closure_binding_expr(output, tail, span);
                }
                self.cleanup_bindings.truncate(binding_depth);
                self.cleanup_capturing_closure_bindings
                    .truncate(closure_binding_depth);
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch: Some(other),
            } => {
                let condition = self.render_bool_guard_expr(output, *condition, span, None);
                let then_label = self.fresh_label("cleanup_then");
                let else_label = self.fresh_label("cleanup_else");
                let end_label = self.fresh_label("cleanup_end");
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{then_label}, label %{else_label}",
                    condition.repr
                );

                let _ = writeln!(output, "{then_label}:");
                self.cleanup_path_open = true;
                let binding_depth = self.cleanup_bindings.len();
                let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
                if let Some(tail) = self.render_cleanup_block_prefix(output, *then_branch, span) {
                    self.render_cleanup_capturing_closure_binding_expr(output, tail, span);
                }
                self.cleanup_bindings.truncate(binding_depth);
                self.cleanup_capturing_closure_bindings
                    .truncate(closure_binding_depth);
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{else_label}:");
                self.cleanup_path_open = true;
                self.render_cleanup_capturing_closure_binding_expr(output, *other, span);
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{end_label}:");
                self.cleanup_path_open = true;
            }
            hir::ExprKind::Match { value, arms } => {
                self.render_cleanup_capturing_closure_binding_match_expr(
                    output, *value, arms, span, None,
                );
            }
            _ => {}
        }
    }

    fn render_cleanup_capturing_closure_binding_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        span: Span,
        match_tag_slot: Option<&str>,
    ) {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!(
                "supported cleanup lowering at {span:?} should type-check cleanup binding matches"
            )
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Bool),
                span,
            );
            self.render_cleanup_capturing_closure_binding_bool_match_expr(
                output,
                scrutinee,
                arms,
                span,
                match_tag_slot,
            );
            return;
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Int),
                span,
            );
            self.render_cleanup_capturing_closure_binding_integer_match_expr(
                output,
                scrutinee,
                arms,
                span,
                match_tag_slot,
            );
            return;
        }

        let scrutinee = self.render_cleanup_value_expr(output, value_expr, scrutinee_ty, span);
        self.render_cleanup_capturing_closure_binding_guard_only_match_expr(
            output,
            scrutinee,
            arms,
            span,
            match_tag_slot,
        );
    }

    fn render_cleanup_capturing_closure_binding_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
        match_tag_slot: Option<&str>,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render supported bool cleanup-match patterns"
                )
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            if let Some(tag_slot) = match_tag_slot {
                let _ = writeln!(output, "  store i64 {index}, ptr {tag_slot}");
            }
            self.render_cleanup_capturing_closure_binding_expr(output, arm.body, span);
            let _ = writeln!(output, "  br label %{end_label}");

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_capturing_closure_binding_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
        match_tag_slot: Option<&str>,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render supported integer cleanup-match patterns"
                )
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            if let Some(tag_slot) = match_tag_slot {
                let _ = writeln!(output, "  store i64 {index}, ptr {tag_slot}");
            }
            self.render_cleanup_capturing_closure_binding_expr(output, arm.body, span);
            let _ = writeln!(output, "  br label %{end_label}");

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_capturing_closure_binding_guard_only_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
        match_tag_slot: Option<&str>,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_depth = self.cleanup_bindings.len();
            let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
            self.bind_cleanup_pattern(output, arm.pattern, scrutinee.clone(), span);

            if let Some(guard) = arm.guard {
                let guard = self.render_bool_guard_expr(output, guard, span, None);
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{body_label}, label %{next_label}",
                    guard.repr
                );
            } else {
                let _ = writeln!(output, "  br label %{body_label}");
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            if let Some(tag_slot) = match_tag_slot {
                let _ = writeln!(output, "  store i64 {index}, ptr {tag_slot}");
            }
            self.render_cleanup_capturing_closure_binding_expr(output, arm.body, span);
            let _ = writeln!(output, "  br label %{end_label}");

            self.cleanup_bindings.truncate(binding_depth);
            self.cleanup_capturing_closure_bindings
                .truncate(closure_binding_depth);

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn bind_cleanup_pattern(
        &mut self,
        output: &mut String,
        pattern: hir::PatternId,
        value: LoweredValue,
        span: Span,
    ) {
        match pattern_kind(self.emitter.input.hir, pattern) {
            PatternKind::Binding(local) => {
                self.cleanup_bindings.push(GuardBindingValue {
                    local: *local,
                    value,
                });
            }
            PatternKind::Tuple(items) => {
                let Ty::Tuple(item_tys) = &value.ty else {
                    panic!(
                        "supported cleanup lowering at {span:?} should only destructure tuple cleanup values with tuple patterns"
                    );
                };
                assert_eq!(
                    items.len(),
                    item_tys.len(),
                    "supported cleanup lowering at {span:?} should only destructure tuple cleanup values with matching arity"
                );
                for (index, (item, item_ty)) in items.iter().zip(item_tys.iter()).enumerate() {
                    let item_llvm_ty = self
                        .emitter
                        .lower_llvm_type(item_ty, span, "cleanup tuple pattern item")
                        .expect(
                            "supported cleanup lowering should lower cleanup tuple pattern items",
                        );
                    let item_value = self.extract_aggregate_value(
                        output,
                        &value,
                        index,
                        item_ty.clone(),
                        item_llvm_ty,
                    );
                    self.bind_cleanup_pattern(output, *item, item_value, span);
                }
            }
            PatternKind::Array(items) => {
                let Ty::Array { element, len } = &value.ty else {
                    panic!(
                        "supported cleanup lowering at {span:?} should only destructure fixed-array cleanup values with fixed-array patterns"
                    );
                };
                let len = known_array_len(len).expect(
                    "supported cleanup lowering should only destructure concrete fixed-array values",
                );
                assert_eq!(
                    items.len(),
                    len,
                    "supported cleanup lowering at {span:?} should only destructure fixed-array cleanup values with matching arity"
                );
                let item_llvm_ty = self
                    .emitter
                    .lower_llvm_type(element, span, "cleanup fixed-array pattern item")
                    .expect(
                        "supported cleanup lowering should lower cleanup fixed-array pattern items",
                    );
                for (index, item) in items.iter().enumerate() {
                    let item_value = self.extract_aggregate_value(
                        output,
                        &value,
                        index,
                        element.as_ref().clone(),
                        item_llvm_ty.clone(),
                    );
                    self.bind_cleanup_pattern(output, *item, item_value, span);
                }
            }
            PatternKind::Struct { fields, .. } => {
                let field_layouts = self
                    .emitter
                    .struct_field_lowerings(&value.ty, span, "cleanup struct pattern")
                    .unwrap_or_else(|_| {
                        panic!(
                            "supported cleanup lowering at {span:?} should only destructure struct cleanup values with struct patterns"
                        )
                    });
                for field in fields {
                    let (index, field_layout) = field_layouts
                        .iter()
                        .enumerate()
                        .find(|(_, candidate)| candidate.name == field.name)
                        .unwrap_or_else(|| {
                            panic!(
                                "supported cleanup lowering at {span:?} should only destructure cleanup struct patterns with known fields"
                            )
                        });
                    let field_value = self.extract_aggregate_value(
                        output,
                        &value,
                        index,
                        field_layout.ty.clone(),
                        field_layout.llvm_ty.clone(),
                    );
                    self.bind_cleanup_pattern(output, field.pattern, field_value, span);
                }
            }
            PatternKind::Wildcard => {}
            _ => panic!(
                "supported cleanup lowering at {span:?} should only destructure binding, wildcard, tuple, fixed-array, or struct cleanup patterns"
            ),
        }
    }

    fn bind_pattern_value(
        &mut self,
        output: &mut String,
        pattern: hir::PatternId,
        value: LoweredValue,
        span: Span,
    ) {
        match pattern_kind(self.emitter.input.hir, pattern) {
            PatternKind::Binding(local) => {
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
                    value.llvm_ty,
                    value.repr,
                    llvm_slot_name(self.body, binding_local)
                );
            }
            PatternKind::Tuple(items) => {
                let Ty::Tuple(item_tys) = &value.ty else {
                    panic!(
                        "supported bind-pattern lowering at {span:?} should only destructure tuple values with tuple patterns"
                    );
                };
                assert_eq!(
                    items.len(),
                    item_tys.len(),
                    "supported bind-pattern lowering at {span:?} should only destructure tuple values with matching arity"
                );
                for (index, (item, item_ty)) in items.iter().zip(item_tys.iter()).enumerate() {
                    let item_llvm_ty = self
                        .emitter
                        .lower_llvm_type(item_ty, span, "tuple pattern item")
                        .expect("supported bind-pattern lowering should lower tuple pattern items");
                    let item_value = self.extract_aggregate_value(
                        output,
                        &value,
                        index,
                        item_ty.clone(),
                        item_llvm_ty,
                    );
                    self.bind_pattern_value(output, *item, item_value, span);
                }
            }
            PatternKind::Array(items) => {
                let Ty::Array { element, len } = &value.ty else {
                    panic!(
                        "supported bind-pattern lowering at {span:?} should only destructure fixed-array values with fixed-array patterns"
                    );
                };
                let len = known_array_len(len).expect(
                    "supported bind-pattern lowering should only destructure concrete fixed-array values",
                );
                assert_eq!(
                    items.len(),
                    len,
                    "supported bind-pattern lowering at {span:?} should only destructure fixed-array values with matching arity"
                );
                let item_llvm_ty = self
                    .emitter
                    .lower_llvm_type(element, span, "fixed-array pattern item")
                    .expect(
                        "supported bind-pattern lowering should lower fixed-array pattern items",
                    );
                for (index, item) in items.iter().enumerate() {
                    let item_value = self.extract_aggregate_value(
                        output,
                        &value,
                        index,
                        element.as_ref().clone(),
                        item_llvm_ty.clone(),
                    );
                    self.bind_pattern_value(output, *item, item_value, span);
                }
            }
            PatternKind::TupleStruct { path, items } => {
                let enum_lowering = self
                    .emitter
                    .enum_lowering(&value.ty, span, "tuple-struct pattern")
                    .unwrap_or_else(|_| {
                        panic!(
                            "supported bind-pattern lowering at {span:?} should only destructure enum values with tuple-struct patterns"
                        )
                    });
                let (_, variant) = self
                    .emitter
                    .enum_variant_lowering_for_path(
                        &enum_lowering,
                        path,
                        &value.ty,
                        span,
                        "tuple-struct pattern",
                    )
                    .unwrap_or_else(|_| {
                        panic!(
                            "supported bind-pattern lowering at {span:?} should only destructure known enum variants"
                        )
                    });
                assert_eq!(
                    items.len(),
                    variant.fields.len(),
                    "supported bind-pattern lowering at {span:?} should only destructure tuple-struct patterns with matching arity"
                );
                let enum_slot = self.materialize_loadable_value_slot(output, &value);
                let payload_ptr =
                    self.enum_variant_payload_ptr(output, &enum_slot, &enum_lowering, variant);
                for (index, (item, field)) in items.iter().zip(variant.fields.iter()).enumerate() {
                    let field_ptr = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {field_ptr} = getelementptr inbounds {}, ptr {payload_ptr}, i32 0, i32 {index}",
                        variant
                            .payload_llvm_ty
                            .as_ref()
                            .expect("tuple enum patterns should have payloads")
                    );
                    let field_value =
                        self.render_loaded_pointer_value(output, field_ptr, field.ty.clone(), span);
                    self.bind_pattern_value(output, *item, field_value, span);
                }
            }
            PatternKind::Struct { fields, .. } => {
                if matches!(
                    &value.ty,
                    Ty::Item { item_id, .. }
                        if matches!(&self.emitter.input.hir.item(*item_id).kind, ItemKind::Enum(_))
                ) {
                    let enum_lowering = self
                        .emitter
                        .enum_lowering(&value.ty, span, "struct pattern")
                        .unwrap_or_else(|_| {
                            panic!(
                                "supported bind-pattern lowering at {span:?} should only destructure enum values with struct patterns"
                            )
                        });
                    let path = match pattern_kind(self.emitter.input.hir, pattern) {
                        PatternKind::Struct { path, .. } => path,
                        _ => unreachable!("checked struct pattern"),
                    };
                    let (_, variant) = self
                        .emitter
                        .enum_variant_lowering_for_path(
                            &enum_lowering,
                            path,
                            &value.ty,
                            span,
                            "struct pattern",
                        )
                        .unwrap_or_else(|_| {
                            panic!(
                                "supported bind-pattern lowering at {span:?} should only destructure known enum variants"
                            )
                        });
                    let enum_slot = self.materialize_loadable_value_slot(output, &value);
                    let payload_ptr =
                        self.enum_variant_payload_ptr(output, &enum_slot, &enum_lowering, variant);
                    for field in fields {
                        let (index, field_layout) = variant
                            .fields
                            .iter()
                            .enumerate()
                            .find(|(_, candidate)| candidate.name.as_deref() == Some(field.name.as_str()))
                            .unwrap_or_else(|| {
                                panic!(
                                    "supported bind-pattern lowering at {span:?} should only destructure struct patterns with known fields"
                                )
                            });
                        let field_ptr = self.fresh_temp();
                        let _ = writeln!(
                            output,
                            "  {field_ptr} = getelementptr inbounds {}, ptr {payload_ptr}, i32 0, i32 {index}",
                            variant
                                .payload_llvm_ty
                                .as_ref()
                                .expect("struct enum patterns should have payloads")
                        );
                        let field_value = self.render_loaded_pointer_value(
                            output,
                            field_ptr,
                            field_layout.ty.clone(),
                            span,
                        );
                        self.bind_pattern_value(output, field.pattern, field_value, span);
                    }
                } else {
                    let field_layouts = self
                        .emitter
                        .struct_field_lowerings(&value.ty, span, "struct pattern")
                        .unwrap_or_else(|_| {
                            panic!(
                                "supported bind-pattern lowering at {span:?} should only destructure struct values with struct patterns"
                            )
                        });
                    for field in fields {
                        let (index, field_layout) = field_layouts
                            .iter()
                            .enumerate()
                            .find(|(_, candidate)| candidate.name == field.name)
                            .unwrap_or_else(|| {
                                panic!(
                                    "supported bind-pattern lowering at {span:?} should only destructure struct patterns with known fields"
                                )
                            });
                        let field_value = self.extract_aggregate_value(
                            output,
                            &value,
                            index,
                            field_layout.ty.clone(),
                            field_layout.llvm_ty.clone(),
                        );
                        self.bind_pattern_value(output, field.pattern, field_value, span);
                    }
                }
            }
            PatternKind::Path(_)
            | PatternKind::Integer(_)
            | PatternKind::String(_)
            | PatternKind::Bool(_)
            | PatternKind::Wildcard => {}
            _ => panic!(
                "supported bind-pattern lowering at {span:?} should only destructure binding, wildcard, tuple, tuple-struct, fixed-array, or struct patterns"
            ),
        }
    }

    fn materialize_loadable_value_slot(
        &mut self,
        output: &mut String,
        value: &LoweredValue,
    ) -> String {
        let slot = self.fresh_temp();
        let _ = writeln!(output, "  {slot} = alloca {}", value.llvm_ty);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {slot}",
            value.llvm_ty, value.repr
        );
        slot
    }

    fn enum_variant_payload_ptr(
        &mut self,
        output: &mut String,
        enum_slot: &str,
        enum_lowering: &EnumLowering,
        variant: &EnumVariantLowering,
    ) -> String {
        assert!(
            variant.payload_llvm_ty.is_some() && enum_lowering.storage.is_some(),
            "enum payload pointer requests should only happen for payload-bearing variants"
        );
        let payload_storage_ptr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {payload_storage_ptr} = getelementptr inbounds {}, ptr {enum_slot}, i32 0, i32 1",
            enum_lowering.llvm_ty
        );
        payload_storage_ptr
    }

    fn extract_aggregate_value(
        &mut self,
        output: &mut String,
        aggregate: &LoweredValue,
        index: usize,
        ty: Ty,
        llvm_ty: String,
    ) -> LoweredValue {
        let temp = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {temp} = extractvalue {} {}, {index}",
            aggregate.llvm_ty, aggregate.repr
        );
        LoweredValue {
            ty,
            llvm_ty,
            repr: temp,
        }
    }

    fn render_cleanup_while(
        &mut self,
        output: &mut String,
        condition: hir::ExprId,
        body: hir::BlockId,
        span: Span,
    ) {
        let cond_label = self.fresh_label("cleanup_while_cond");
        let body_label = self.fresh_label("cleanup_while_body");
        let end_label = self.fresh_label("cleanup_while_end");
        let _ = writeln!(output, "  br label %{cond_label}");
        let _ = writeln!(output, "{cond_label}:");
        let condition = self.render_bool_guard_expr(output, condition, span, None);
        let _ = writeln!(
            output,
            "  br i1 {}, label %{body_label}, label %{end_label}",
            condition.repr
        );
        let _ = writeln!(output, "{body_label}:");
        self.cleanup_loop_labels.push(CleanupLoopLabels {
            continue_label: cond_label.clone(),
            break_label: end_label.clone(),
        });
        self.cleanup_path_open = true;
        self.render_cleanup_block(output, body, span);
        let body_falls_through = self.cleanup_path_open;
        self.cleanup_loop_labels.pop();
        if body_falls_through {
            let _ = writeln!(output, "  br label %{cond_label}");
        }
        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_loop(&mut self, output: &mut String, body: hir::BlockId, span: Span) {
        let body_label = self.fresh_label("cleanup_loop_body");
        let end_label = self.fresh_label("cleanup_loop_end");
        let _ = writeln!(output, "  br label %{body_label}");
        let _ = writeln!(output, "{body_label}:");
        self.cleanup_loop_labels.push(CleanupLoopLabels {
            continue_label: body_label.clone(),
            break_label: end_label.clone(),
        });
        self.cleanup_path_open = true;
        self.render_cleanup_block(output, body, span);
        let body_falls_through = self.cleanup_path_open;
        self.cleanup_loop_labels.pop();
        if body_falls_through {
            let _ = writeln!(output, "  br label %{body_label}");
        }
        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_for(
        &mut self,
        output: &mut String,
        is_await: bool,
        pattern: hir::PatternId,
        iterable: hir::ExprId,
        body: hir::BlockId,
        span: Span,
    ) {
        let iterable_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(iterable)
            .cloned()
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should type-check cleanup `for` iterables")
            });
        let (iterable_kind, element_ty, iterable_len) =
            cleanup_for_iterable_shape(&iterable_ty).unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render fixed-shape cleanup `for` iterables"
                )
            });
        let (item_ty, auto_await_task_elements) = if is_await {
            match &element_ty {
                Ty::TaskHandle(result_ty) => ((**result_ty).clone(), true),
                _ => (element_ty.clone(), false),
            }
        } else {
            (element_ty.clone(), false)
        };
        let rendered_iterable =
            self.render_cleanup_value_expr(output, iterable, &iterable_ty, span);
        let iterable_slot = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {iterable_slot} = alloca {}",
            rendered_iterable.llvm_ty
        );
        let _ = writeln!(
            output,
            "  store {} {}, ptr {iterable_slot}",
            rendered_iterable.llvm_ty, rendered_iterable.repr
        );

        match iterable_kind {
            SupportedForLoopIterableKind::Array => {
                self.render_cleanup_array_for(
                    output,
                    iterable_slot,
                    &iterable_ty,
                    &element_ty,
                    &item_ty,
                    auto_await_task_elements,
                    iterable_len,
                    pattern,
                    body,
                    span,
                );
            }
            SupportedForLoopIterableKind::Tuple => {
                self.render_cleanup_tuple_for(
                    output,
                    iterable_slot,
                    &iterable_ty,
                    &element_ty,
                    &item_ty,
                    auto_await_task_elements,
                    iterable_len,
                    pattern,
                    body,
                    span,
                );
            }
        }
    }

    fn render_cleanup_array_for(
        &mut self,
        output: &mut String,
        iterable_slot: String,
        iterable_ty: &Ty,
        element_ty: &Ty,
        item_ty: &Ty,
        auto_await_task_elements: bool,
        iterable_len: usize,
        pattern: hir::PatternId,
        body: hir::BlockId,
        span: Span,
    ) {
        let iterable_llvm_ty = self
            .emitter
            .lower_llvm_type(iterable_ty, span, "cleanup for iterable")
            .expect("supported cleanup lowering should lower cleanup `for` array iterables");
        let element_llvm_ty = self
            .emitter
            .lower_llvm_type(element_ty, span, "cleanup for item")
            .expect("supported cleanup lowering should lower cleanup `for` array items");
        let index_slot = self.fresh_temp();
        let cond_label = self.fresh_label("cleanup_for_cond");
        let body_label = self.fresh_label("cleanup_for_body");
        let step_label = self.fresh_label("cleanup_for_step");
        let end_label = self.fresh_label("cleanup_for_end");
        let _ = writeln!(output, "  {index_slot} = alloca i64");
        let _ = writeln!(output, "  store i64 0, ptr {index_slot}");
        let _ = writeln!(output, "  br label %{cond_label}");
        let _ = writeln!(output, "{cond_label}:");
        let index = self.fresh_temp();
        let continue_flag = self.fresh_temp();
        let _ = writeln!(output, "  {index} = load i64, ptr {index_slot}");
        let _ = writeln!(
            output,
            "  {continue_flag} = icmp ult i64 {index}, {iterable_len}"
        );
        let _ = writeln!(
            output,
            "  br i1 {continue_flag}, label %{body_label}, label %{end_label}"
        );
        let _ = writeln!(output, "{body_label}:");
        let item_ptr = self.fresh_temp();
        let item_repr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {item_ptr} = getelementptr inbounds {iterable_llvm_ty}, ptr {iterable_slot}, i64 0, i64 {index}"
        );
        let _ = writeln!(
            output,
            "  {item_repr} = load {element_llvm_ty}, ptr {item_ptr}"
        );
        self.render_cleanup_for_item(
            output,
            pattern,
            LoweredValue {
                ty: element_ty.clone(),
                llvm_ty: element_llvm_ty.clone(),
                repr: item_repr,
            },
            item_ty,
            auto_await_task_elements,
            step_label.clone(),
            end_label.clone(),
            body,
            span,
        );
        let body_falls_through = self.cleanup_path_open;
        if body_falls_through {
            let _ = writeln!(output, "  br label %{step_label}");
        }
        let _ = writeln!(output, "{step_label}:");
        let next_index = self.fresh_temp();
        let _ = writeln!(output, "  {next_index} = add i64 {index}, 1");
        let _ = writeln!(output, "  store i64 {next_index}, ptr {index_slot}");
        let _ = writeln!(output, "  br label %{cond_label}");
        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_tuple_for(
        &mut self,
        output: &mut String,
        iterable_slot: String,
        iterable_ty: &Ty,
        element_ty: &Ty,
        item_ty: &Ty,
        auto_await_task_elements: bool,
        iterable_len: usize,
        pattern: hir::PatternId,
        body: hir::BlockId,
        span: Span,
    ) {
        let iterable_llvm_ty = self
            .emitter
            .lower_llvm_type(iterable_ty, span, "cleanup for iterable")
            .expect("supported cleanup lowering should lower cleanup `for` tuple iterables");
        let element_llvm_ty = self
            .emitter
            .lower_llvm_type(element_ty, span, "cleanup for item")
            .expect("supported cleanup lowering should lower cleanup `for` tuple items");
        let end_label = self.fresh_label("cleanup_for_end");
        let mut item_label = self.fresh_label("cleanup_for_tuple_item");
        let _ = writeln!(output, "  br label %{item_label}");

        for index in 0..iterable_len {
            let _ = writeln!(output, "{item_label}:");
            let item_ptr = self.fresh_temp();
            let item_repr = self.fresh_temp();
            let next_label = if index + 1 < iterable_len {
                self.fresh_label("cleanup_for_tuple_item")
            } else {
                end_label.clone()
            };
            let _ = writeln!(
                output,
                "  {item_ptr} = getelementptr inbounds {iterable_llvm_ty}, ptr {iterable_slot}, i32 0, i32 {index}"
            );
            let _ = writeln!(
                output,
                "  {item_repr} = load {element_llvm_ty}, ptr {item_ptr}"
            );
            self.render_cleanup_for_item(
                output,
                pattern,
                LoweredValue {
                    ty: element_ty.clone(),
                    llvm_ty: element_llvm_ty.clone(),
                    repr: item_repr,
                },
                item_ty,
                auto_await_task_elements,
                next_label.clone(),
                end_label.clone(),
                body,
                span,
            );
            let body_falls_through = self.cleanup_path_open;
            if body_falls_through {
                let _ = writeln!(output, "  br label %{next_label}");
            }
            item_label = next_label;
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_for_item(
        &mut self,
        output: &mut String,
        pattern: hir::PatternId,
        element: LoweredValue,
        item_ty: &Ty,
        auto_await_task_elements: bool,
        continue_label: String,
        break_label: String,
        body: hir::BlockId,
        span: Span,
    ) {
        let item = if auto_await_task_elements {
            let handle_info = self.task_handle_info_for_ty(&element.ty, span);
            self.render_await_handle(output, element, handle_info, span)
                .expect("prepared cleanup `for await` lowering should produce an item value")
        } else {
            element
        };
        assert!(
            item_ty.compatible_with(&item.ty),
            "supported cleanup lowering at {span:?} should only bind compatible cleanup `for` items"
        );
        let binding_depth = self.cleanup_bindings.len();
        self.bind_cleanup_pattern(output, pattern, item, span);
        self.cleanup_loop_labels.push(CleanupLoopLabels {
            continue_label,
            break_label,
        });
        self.cleanup_path_open = true;
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        self.render_cleanup_block(output, body, span);
        self.cleanup_loop_labels.pop();
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
    }

    fn render_cleanup_expr(&mut self, output: &mut String, expr_id: hir::ExprId, span: Span) {
        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                let _ = self.render_cleanup_call(output, *callee, args, span);
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                self.render_cleanup_block(output, *block_id, span);
            }
            hir::ExprKind::Question(inner) => {
                self.render_cleanup_expr(output, *inner, span);
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let _ = self.render_cleanup_await_expr(output, *expr, span);
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Spawn,
                expr,
            } => {
                let _ = self.render_cleanup_spawn_expr(output, *expr, span);
            }
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let _ = self.render_cleanup_assignment_expr(output, *left, *right, span);
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.render_bool_guard_expr(output, *condition, span, None);
                let then_label = self.fresh_label("cleanup_then");
                let else_label = self.fresh_label("cleanup_else");
                let end_label = self.fresh_label("cleanup_end");
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{then_label}, label %{else_label}",
                    condition.repr
                );
                let _ = writeln!(output, "{then_label}:");
                self.cleanup_path_open = true;
                self.render_cleanup_block(output, *then_branch, span);
                if self.cleanup_path_open {
                    let _ = writeln!(output, "  br label %{end_label}");
                }
                let _ = writeln!(output, "{else_label}:");
                self.cleanup_path_open = true;
                if let Some(other) = else_branch {
                    self.render_cleanup_expr(output, *other, span);
                }
                if self.cleanup_path_open {
                    let _ = writeln!(output, "  br label %{end_label}");
                }
                let _ = writeln!(output, "{end_label}:");
                self.cleanup_path_open = true;
            }
            hir::ExprKind::Match { value, arms } => {
                self.render_cleanup_match_expr(output, *value, arms, span);
            }
            _ => panic!("prepared functions should not contain unsupported cleanup expressions"),
        }
    }

    fn render_cleanup_assignment_expr(
        &mut self,
        output: &mut String,
        target_expr: hir::ExprId,
        value_expr: hir::ExprId,
        span: Span,
    ) -> LoweredValue {
        let (place, target_ty) = guard_expr_place_with_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.local_types,
            None,
            target_expr,
        )
        .unwrap_or_else(|| {
            panic!(
                "supported cleanup lowering at {span:?} should only assign through supported cleanup local/projection places"
            )
        });
        let rendered = self.render_cleanup_value_expr(output, value_expr, &target_ty, span);
        assert!(
            target_ty.compatible_with(&rendered.ty),
            "supported cleanup lowering at {span:?} should only store compatible cleanup assignment values"
        );
        let target_ptr = self.render_place_pointer(output, &place, span).0;
        let _ = writeln!(
            output,
            "  store {} {}, ptr {}",
            rendered.llvm_ty, rendered.repr, target_ptr
        );
        rendered
    }

    fn render_cleanup_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        span: Span,
    ) {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!("supported cleanup lowering at {span:?} should type-check cleanup matches")
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Bool),
                span,
            );
            self.render_cleanup_bool_match_expr(output, scrutinee, arms, span);
            return;
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Int),
                span,
            );
            self.render_cleanup_integer_match_expr(output, scrutinee, arms, span);
            return;
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::String)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::String),
                span,
            );
            self.render_cleanup_string_match_expr(output, scrutinee, arms, span);
            return;
        }

        let scrutinee = self.render_cleanup_value_expr(output, value_expr, scrutinee_ty, span);
        self.render_cleanup_guard_only_match_expr(output, scrutinee, arms, span);
    }

    fn render_cleanup_guard_only_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_depth = self.cleanup_bindings.len();
            let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
            self.bind_cleanup_pattern(output, arm.pattern, scrutinee.clone(), span);

            if let Some(guard) = arm.guard {
                let guard = self.render_bool_guard_expr(output, guard, span, None);
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{body_label}, label %{next_label}",
                    guard.repr
                );
            } else {
                let _ = writeln!(output, "  br label %{body_label}");
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            self.render_cleanup_expr(output, arm.body, span);
            if self.cleanup_path_open {
                let _ = writeln!(output, "  br label %{end_label}");
            }

            self.cleanup_bindings.truncate(binding_depth);
            self.cleanup_capturing_closure_bindings
                .truncate(closure_binding_depth);

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_value_guard_only_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "cleanup match value")
            .expect("supported cleanup lowering should lower cleanup match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("cleanup_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_depth = self.cleanup_bindings.len();
            let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
            self.bind_cleanup_pattern(output, arm.pattern, scrutinee.clone(), span);

            if let Some(guard) = arm.guard {
                let guard = self.render_bool_guard_expr(output, guard, span, None);
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{body_label}, label %{next_label}",
                    guard.repr
                );
            } else {
                let _ = writeln!(output, "  br label %{body_label}");
            }

            let _ = writeln!(output, "{body_label}:");
            let arm_value = self.render_cleanup_value_expr(output, arm.body, expected_ty, span);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            self.cleanup_bindings.truncate(binding_depth);
            self.cleanup_capturing_closure_bindings
                .truncate(closure_binding_depth);

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_cleanup_value_if_expr(
        &mut self,
        output: &mut String,
        condition_expr: hir::ExprId,
        then_branch: hir::BlockId,
        else_expr: hir::ExprId,
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let condition = self.render_bool_guard_expr(output, condition_expr, span, None);
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "cleanup if value")
            .expect("supported cleanup lowering should lower cleanup if values");
        let result_slot = self.fresh_temp();
        let then_label = self.fresh_label("cleanup_then");
        let else_label = self.fresh_label("cleanup_else");
        let end_label = self.fresh_label("cleanup_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        self.cleanup_path_open = true;
        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        let then_tail = self
            .render_cleanup_block_prefix(output, then_branch, span)
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render valued cleanup `if` branches with tails"
                )
            });
        let then_value = self.render_cleanup_value_expr(output, then_tail, expected_ty, span);
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            then_value.llvm_ty, then_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        self.cleanup_path_open = true;
        let else_value = self.render_cleanup_value_expr(output, else_expr, expected_ty, span);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            else_value.llvm_ty, else_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_cleanup_value_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!("supported cleanup lowering at {span:?} should type-check cleanup value matches")
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Bool),
                span,
            );
            return self.render_cleanup_value_bool_match_expr(
                output,
                scrutinee,
                arms,
                expected_ty,
                span,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Int),
                span,
            );
            return self.render_cleanup_value_integer_match_expr(
                output,
                scrutinee,
                arms,
                expected_ty,
                span,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::String)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::String),
                span,
            );
            return self.render_cleanup_value_string_match_expr(
                output,
                scrutinee,
                arms,
                expected_ty,
                span,
            );
        }

        let scrutinee = self.render_cleanup_value_expr(output, value_expr, scrutinee_ty, span);
        self.render_cleanup_value_guard_only_match_expr(output, scrutinee, arms, expected_ty, span)
    }

    fn render_cleanup_value_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "cleanup match value")
            .expect("supported cleanup lowering should lower cleanup match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("cleanup_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported bool cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(
                            "bool cleanup-match rendering should have handled catch-all separately"
                        ),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_value_expr(output, arm.body, expected_ty, span);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_cleanup_value_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "cleanup match value")
            .expect("supported cleanup lowering should lower cleanup match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("cleanup_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported integer cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_value_expr(output, arm.body, expected_ty, span);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_cleanup_value_string_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "cleanup match value")
            .expect("supported cleanup lowering should lower cleanup match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("cleanup_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_string_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported string cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedStringMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self
                        .render_string_match_literal_condition(
                            output,
                            scrutinee.clone(),
                            value,
                            span,
                        )
                        .repr;
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedStringMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_value_expr(output, arm.body, expected_ty, span);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedStringMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_cleanup_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported bool cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(
                            "bool cleanup-match rendering should have handled catch-all separately"
                        ),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            self.render_cleanup_expr(output, arm.body, span);
            if self.cleanup_path_open {
                let _ = writeln!(output, "  br label %{end_label}");
            }

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported integer cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            self.render_cleanup_expr(output, arm.body, span);
            if self.cleanup_path_open {
                let _ = writeln!(output, "  br label %{end_label}");
            }

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_string_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        span: Span,
    ) {
        let end_label = self.fresh_label("cleanup_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_string_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported string cleanup-match patterns")
            });
            let body_label = self.fresh_label("cleanup_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedStringMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self
                        .render_string_match_literal_condition(
                            output,
                            scrutinee.clone(),
                            value,
                            span,
                        )
                        .repr;
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedStringMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            self.render_cleanup_expr(output, arm.body, span);
            if self.cleanup_path_open {
                let _ = writeln!(output, "  br label %{end_label}");
            }

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedStringMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
    }

    fn render_cleanup_await_expr(
        &mut self,
        output: &mut String,
        task_expr: hir::ExprId,
        span: Span,
    ) -> LoweredValue {
        let task_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(task_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should type-check awaited cleanup task values"
                )
            });
        let handle_info = self.task_handle_info_for_ty(&task_ty, span);
        let rendered = self.render_cleanup_value_expr(output, task_expr, &task_ty, span);
        self.render_await_handle(output, rendered, handle_info, span)
            .expect("prepared cleanup await lowering should produce a value")
    }

    fn render_cleanup_spawn_expr(
        &mut self,
        output: &mut String,
        task_expr: hir::ExprId,
        span: Span,
    ) -> LoweredValue {
        let task_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(task_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should type-check spawned cleanup task values"
                )
            });
        let handle_info = self.task_handle_info_for_ty(&task_ty, span);
        let rendered = self.render_cleanup_value_expr(output, task_expr, &task_ty, span);
        self.render_spawn_handle(output, rendered, handle_info.result_ty)
            .expect("prepared cleanup spawn lowering should produce a value")
    }

    fn supported_direct_local_capturing_closure_for_expr(
        &self,
        expr_id: hir::ExprId,
    ) -> Option<mir::ClosureId> {
        direct_local_capturing_closure_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.direct_local_capturing_closures,
            &self.cleanup_capturing_closure_bindings,
            expr_id,
        )
    }

    fn supported_direct_cleanup_capturing_closure_callee_for_expr(
        &self,
        expr_id: hir::ExprId,
    ) -> Option<CleanupDirectCapturingClosureExpr> {
        cleanup_direct_capturing_closure_callee_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.direct_local_capturing_closures,
            &self.cleanup_capturing_closure_bindings,
            expr_id,
        )
    }

    fn cleanup_call_callee_return_ty(&self, callee_expr: hir::ExprId, span: Span) -> Ty {
        let callee_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(callee_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should type-check callable cleanup callees"
                )
            });
        let Ty::Callable { ret, .. } = callee_ty else {
            panic!(
                "supported cleanup lowering at {span:?} should only recurse through callable cleanup callees"
            );
        };
        ret.as_ref().clone()
    }

    fn render_cleanup_call_block_expr(
        &mut self,
        output: &mut String,
        block_id: hir::BlockId,
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        if let Some(tail) = callable_elided_block_tail_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            block_id,
        ) {
            return self.render_cleanup_call(output, tail, args, span);
        }

        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        let tail = self
            .render_cleanup_block_prefix(output, block_id, span)
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render callable cleanup blocks with tails"
                )
            });
        let value = self.render_cleanup_call(output, tail, args, span);
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        value
    }

    fn render_cleanup_call_if_expr(
        &mut self,
        output: &mut String,
        condition_expr: hir::ExprId,
        then_branch: hir::BlockId,
        else_expr: hir::ExprId,
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let condition = self.render_bool_guard_expr(output, condition_expr, span, None);
        let return_llvm_ty = (!is_void_ty(return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup-call if results")
        });
        let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let then_label = self.fresh_label("cleanup_call_if_then");
        let else_label = self.fresh_label("cleanup_call_if_else");
        let end_label = self.fresh_label("cleanup_call_if_end");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        self.cleanup_path_open = true;
        let then_value = self.render_cleanup_call_block_expr(output, then_branch, args, span);
        if let Some(result_slot) = result_slot.as_ref() {
            let then_value = then_value.unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render valued cleanup-call `if` branches"
                )
            });
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                then_value.llvm_ty, then_value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        self.cleanup_path_open = true;
        let else_value = self.render_cleanup_call(output, else_expr, args, span);
        if let Some(result_slot) = result_slot.as_ref() {
            let else_value = else_value.unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should only render valued cleanup-call `if` branches"
                )
            });
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                else_value.llvm_ty, else_value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        result_slot
            .map(|slot| self.render_loaded_pointer_value(output, slot, return_ty.clone(), span))
    }

    fn render_cleanup_call_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!("supported cleanup lowering at {span:?} should type-check cleanup call matches")
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Bool),
                span,
            );
            return self.render_cleanup_call_bool_match_expr(
                output, scrutinee, arms, args, return_ty, span,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::Int),
                span,
            );
            return self.render_cleanup_call_integer_match_expr(
                output, scrutinee, arms, args, return_ty, span,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::String)) {
            let scrutinee = self.render_cleanup_value_expr(
                output,
                value_expr,
                &Ty::Builtin(BuiltinType::String),
                span,
            );
            return self.render_cleanup_call_string_match_expr(
                output, scrutinee, arms, args, return_ty, span,
            );
        }

        panic!(
            "supported cleanup lowering at {span:?} should only render supported cleanup call matches"
        );
    }

    fn render_cleanup_call_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let result_llvm_ty = (!is_void_ty(return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup-call match results")
        });
        let result_slot = result_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let end_label = self.fresh_label("cleanup_call_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported bool cleanup-call match patterns")
            });
            let body_label = self.fresh_label("cleanup_call_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_call_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_call_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_call(output, arm.body, args, span);
            if let Some(result_slot) = result_slot.as_ref() {
                let arm_value = arm_value.unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render valued cleanup-call match arms"
                    )
                });
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {result_slot}",
                    arm_value.llvm_ty, arm_value.repr
                );
            }
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        result_slot
            .map(|slot| self.render_loaded_pointer_value(output, slot, return_ty.clone(), span))
    }

    fn render_cleanup_call_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let result_llvm_ty = (!is_void_ty(return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup-call match results")
        });
        let result_slot = result_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let end_label = self.fresh_label("cleanup_call_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported integer cleanup-call match patterns")
            });
            let body_label = self.fresh_label("cleanup_call_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_call_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_call_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_call(output, arm.body, args, span);
            if let Some(result_slot) = result_slot.as_ref() {
                let arm_value = arm_value.unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render valued cleanup-call match arms"
                    )
                });
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {result_slot}",
                    arm_value.llvm_ty, arm_value.repr
                );
            }
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        result_slot
            .map(|slot| self.render_loaded_pointer_value(output, slot, return_ty.clone(), span))
    }

    fn render_cleanup_call_string_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let result_llvm_ty = (!is_void_ty(return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup-call match results")
        });
        let result_slot = result_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let end_label = self.fresh_label("cleanup_call_match_end");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_string_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("supported cleanup lowering at {span:?} should only render supported string cleanup-call match patterns")
            });
            let body_label = self.fresh_label("cleanup_call_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("cleanup_call_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedStringMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("cleanup_call_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self
                        .render_string_match_literal_condition(
                            output,
                            scrutinee.clone(),
                            value,
                            span,
                        )
                        .repr;
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedStringMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, None);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            self.cleanup_path_open = true;
            let arm_value = self.render_cleanup_call(output, arm.body, args, span);
            if let Some(result_slot) = result_slot.as_ref() {
                let arm_value = arm_value.unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render valued cleanup-call match arms"
                    )
                });
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {result_slot}",
                    arm_value.llvm_ty, arm_value.repr
                );
            }
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedStringMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.cleanup_path_open = true;
        result_slot
            .map(|slot| self.render_loaded_pointer_value(output, slot, return_ty.clone(), span))
    }

    fn render_cleanup_direct_capturing_closure_call(
        &mut self,
        output: &mut String,
        closure_id: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        let closure = self.body.closure(closure_id);
        let closure_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(closure.expr)
            .cloned()
            .expect(
                "supported cleanup lowering should preserve direct local capturing closure callable types",
            );
        let Ty::Callable { params, ret } = &closure_ty else {
            panic!(
                "supported cleanup lowering at {span:?} should only treat callable capturing closures as cleanup callees"
            );
        };
        assert!(
            args.len() == params.len()
                && args
                    .iter()
                    .all(|arg| matches!(arg, hir::CallArg::Positional(_))),
            "supported cleanup lowering at {span:?} should only render positional capturing-closure cleanup arguments matching the callable arity"
        );

        let mut rendered_args = Vec::with_capacity(closure.captures.len() + args.len());
        for capture in &closure.captures {
            let rendered =
                self.render_operand(output, &Operand::Place(Place::local(capture.local)), span);
            rendered_args.push(format!("{} {}", rendered.llvm_ty, rendered.repr));
        }
        for (arg, param_ty) in args.iter().zip(params.iter()) {
            let rendered =
                self.render_cleanup_value_expr(output, guard_call_arg_expr(arg), param_ty, span);
            assert!(
                param_ty.compatible_with(&rendered.ty),
                "supported cleanup lowering at {span:?} should only render compatible capturing-closure cleanup arguments"
            );
            rendered_args.push(format!("{} {}", rendered.llvm_ty, rendered.repr));
        }

        let rendered_args = rendered_args.join(", ");
        let return_ty = ret.as_ref().clone();
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "cleanup call result")
            .expect(
                "supported cleanup lowering should only emit lowered capturing-closure cleanup results",
            );
        let callee_name = closure_llvm_name(&self.prepared.signature.llvm_name, closure_id);

        if is_void_ty(&return_ty) {
            let _ = writeln!(
                output,
                "  call {return_llvm_ty} @{callee_name}({rendered_args})"
            );
            None
        } else {
            let temp = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {temp} = call {return_llvm_ty} @{callee_name}({rendered_args})"
            );
            Some(LoweredValue {
                ty: return_ty,
                llvm_ty: return_llvm_ty,
                repr: temp,
            })
        }
    }

    fn render_cleanup_bound_if_capturing_closure_call(
        &mut self,
        output: &mut String,
        condition: LoweredValue,
        then_closure: mir::ClosureId,
        else_closure: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(then_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "supported cleanup lowering should preserve callable return types for shared-local control-flow bindings",
            );
        let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(&return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup call results")
        });
        let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let then_label = self.fresh_label("cleanup_call_if_then");
        let else_label = self.fresh_label("cleanup_call_if_else");
        let end_label = self.fresh_label("cleanup_call_if_end");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        if let Some(value) =
            self.render_cleanup_direct_capturing_closure_call(output, then_closure, args, span)
            && let Some(result_slot) = result_slot.as_ref()
        {
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        if let Some(value) =
            self.render_cleanup_direct_capturing_closure_call(output, else_closure, args, span)
            && let Some(result_slot) = result_slot.as_ref()
        {
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        result_slot.map(|slot| self.render_loaded_pointer_value(output, slot, return_ty, span))
    }

    fn render_cleanup_bound_integer_match_capturing_closure_call(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[CleanupIntegerCapturingClosureMatchArm],
        fallback_closure: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(fallback_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "supported cleanup lowering should preserve callable return types for shared-local match bindings",
            );
        let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(&return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup call results")
        });
        let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let end_label = self.fresh_label("cleanup_call_match_end");
        let fallback_label = self.fresh_label("cleanup_call_match_fallback");
        let dispatch_labels = (0..arms.len())
            .map(|_| self.fresh_label("cleanup_call_match_dispatch"))
            .collect::<Vec<_>>();
        let arm_labels = (0..arms.len())
            .map(|_| self.fresh_label("cleanup_call_match_arm"))
            .collect::<Vec<_>>();
        let opcode = compare_opcode(BinaryOp::EqEq, &scrutinee.ty);

        if arms.is_empty() {
            let _ = writeln!(output, "  br label %{fallback_label}");
        } else {
            for (index, arm) in arms.iter().enumerate() {
                if index > 0 {
                    let _ = writeln!(output, "{}:", dispatch_labels[index]);
                }
                let compare = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {compare} = {opcode} {} {}, {}",
                    scrutinee.llvm_ty, scrutinee.repr, arm.value
                );
                let false_target = if index + 1 == arms.len() {
                    fallback_label.clone()
                } else {
                    dispatch_labels[index + 1].clone()
                };
                let _ = writeln!(
                    output,
                    "  br i1 {compare}, label %{}, label %{false_target}",
                    arm_labels[index]
                );
            }
        }

        for (arm, arm_label) in arms.iter().zip(arm_labels.iter()) {
            let _ = writeln!(output, "{arm_label}:");
            if let Some(value) = self.render_cleanup_direct_capturing_closure_call(
                output,
                arm.closure_id,
                args,
                span,
            ) && let Some(result_slot) = result_slot.as_ref()
            {
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {result_slot}",
                    value.llvm_ty, value.repr
                );
            }
            let _ = writeln!(output, "  br label %{end_label}");
        }

        let _ = writeln!(output, "{fallback_label}:");
        if let Some(value) =
            self.render_cleanup_direct_capturing_closure_call(output, fallback_closure, args, span)
            && let Some(result_slot) = result_slot.as_ref()
        {
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        result_slot.map(|slot| self.render_loaded_pointer_value(output, slot, return_ty, span))
    }

    fn render_cleanup_bound_tagged_match_capturing_closure_call(
        &mut self,
        output: &mut String,
        tag: LoweredValue,
        closures: &[mir::ClosureId],
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        let fallback_closure = *closures
            .last()
            .expect("supported cleanup tagged match bindings should preserve at least one arm");
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(fallback_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "supported cleanup lowering should preserve callable return types for tagged match bindings",
            );
        let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
            self.emitter
                .lower_llvm_type(&return_ty, span, "cleanup call result")
                .expect("supported cleanup lowering should lower cleanup call results")
        });
        let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
            let slot = self.fresh_temp();
            let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
            slot
        });
        let end_label = self.fresh_label("cleanup_call_match_end");
        let fallback_label = self.fresh_label("cleanup_call_match_fallback");
        let dispatch_labels = (0..closures.len())
            .map(|_| self.fresh_label("cleanup_call_match_dispatch"))
            .collect::<Vec<_>>();
        let arm_labels = (0..closures.len())
            .map(|_| self.fresh_label("cleanup_call_match_arm"))
            .collect::<Vec<_>>();
        let opcode = compare_opcode(BinaryOp::EqEq, &tag.ty);

        if closures.is_empty() {
            let _ = writeln!(output, "  br label %{fallback_label}");
        } else {
            for index in 0..closures.len() {
                if index > 0 {
                    let _ = writeln!(output, "{}:", dispatch_labels[index]);
                }
                let compare = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {compare} = {opcode} {} {}, {}",
                    tag.llvm_ty, tag.repr, index
                );
                let false_target = if index + 1 == closures.len() {
                    fallback_label.clone()
                } else {
                    dispatch_labels[index + 1].clone()
                };
                let _ = writeln!(
                    output,
                    "  br i1 {compare}, label %{}, label %{false_target}",
                    arm_labels[index]
                );
            }
        }

        for (closure_id, arm_label) in closures.iter().zip(arm_labels.iter()) {
            let _ = writeln!(output, "{arm_label}:");
            if let Some(value) =
                self.render_cleanup_direct_capturing_closure_call(output, *closure_id, args, span)
                && let Some(result_slot) = result_slot.as_ref()
            {
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {result_slot}",
                    value.llvm_ty, value.repr
                );
            }
            let _ = writeln!(output, "  br label %{end_label}");
        }

        let _ = writeln!(output, "{fallback_label}:");
        if let Some(value) =
            self.render_cleanup_direct_capturing_closure_call(output, fallback_closure, args, span)
            && let Some(result_slot) = result_slot.as_ref()
        {
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
        }
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        result_slot.map(|slot| self.render_loaded_pointer_value(output, slot, return_ty, span))
    }

    fn render_cleanup_call(
        &mut self,
        output: &mut String,
        callee_expr: hir::ExprId,
        args: &[hir::CallArg],
        span: Span,
    ) -> Option<LoweredValue> {
        if let Some(CleanupCapturingClosureBindingValue::TaggedMatch {
            tag: Some(tag),
            closures,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_cleanup_bound_tagged_match_capturing_closure_call(
                output, tag, &closures, args, span,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::BoolMatch {
            scrutinee: Some(scrutinee),
            true_closure,
            false_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_cleanup_bound_if_capturing_closure_call(
                output,
                scrutinee,
                true_closure,
                false_closure,
                args,
                span,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::IntegerMatch {
            scrutinee: Some(scrutinee),
            arms,
            fallback_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_cleanup_bound_integer_match_capturing_closure_call(
                output,
                scrutinee,
                &arms,
                fallback_closure,
                args,
                span,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::IfBranch {
            condition: Some(condition),
            then_closure,
            else_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_cleanup_bound_if_capturing_closure_call(
                output,
                condition,
                then_closure,
                else_closure,
                args,
                span,
            );
        }

        if let hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) =
            &self.emitter.input.hir.expr(callee_expr).kind
        {
            return self.render_cleanup_call_block_expr(output, *block_id, args, span);
        }

        if let hir::ExprKind::If {
            condition,
            then_branch,
            else_branch: Some(other),
        } = &self.emitter.input.hir.expr(callee_expr).kind
        {
            let return_ty = self.cleanup_call_callee_return_ty(callee_expr, span);
            return self.render_cleanup_call_if_expr(
                output,
                *condition,
                *then_branch,
                *other,
                args,
                &return_ty,
                span,
            );
        }

        if let hir::ExprKind::Match { value, arms } = &self.emitter.input.hir.expr(callee_expr).kind
        {
            let return_ty = self.cleanup_call_callee_return_ty(callee_expr, span);
            return self
                .render_cleanup_call_match_expr(output, *value, arms, args, &return_ty, span);
        }

        if let Some(function) = guard_direct_callee_function(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            callee_expr,
        ) {
            let signature = self.emitter.signatures.get(&function).unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should resolve cleanup-call signatures"
                )
            });
            let ordered_args = ordered_guard_call_args(args, signature).unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should preserve direct cleanup-call argument mapping"
                )
            });
            let rendered_args = ordered_args
                .into_iter()
                .zip(signature.params.iter())
                .map(|(arg, param)| {
                    let expr_id = guard_call_arg_expr(arg);
                    let rendered = self.render_cleanup_value_expr(output, expr_id, &param.ty, span);
                    format!("{} {}", rendered.llvm_ty, rendered.repr)
                })
                .collect::<Vec<_>>()
                .join(", ");

            if is_void_ty(&signature.return_ty) {
                let _ = writeln!(
                    output,
                    "  call {} @{}({rendered_args})",
                    signature.return_llvm_ty, signature.llvm_name
                );
                return None;
            }

            let temp = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {temp} = call {} @{}({rendered_args})",
                signature.return_llvm_ty, signature.llvm_name
            );
            return Some(LoweredValue {
                ty: cleanup_call_result_ty(signature),
                llvm_ty: signature.return_llvm_ty.clone(),
                repr: temp,
            });
        }

        if let Some(callee) =
            self.supported_direct_cleanup_capturing_closure_callee_for_expr(callee_expr)
        {
            let closure_id = match callee {
                CleanupDirectCapturingClosureExpr::Direct(closure_id) => closure_id,
                CleanupDirectCapturingClosureExpr::Assignment(binding) => {
                    self.cleanup_capturing_closure_bindings.push(
                        CleanupCapturingClosureBinding::direct(binding.local, binding.closure_id),
                    );
                    binding.closure_id
                }
            };
            return self
                .render_cleanup_direct_capturing_closure_call(output, closure_id, args, span);
        }

        if let Some(closure_id) = supported_direct_local_capturing_closure_callee_closure(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.direct_local_capturing_closures,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self
                .render_cleanup_direct_capturing_closure_call(output, closure_id, args, span);
        }

        let callee_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(callee_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "supported cleanup lowering at {span:?} should type-check callable cleanup callees"
                )
            });
        let callee = self.render_cleanup_value_expr(output, callee_expr, &callee_ty, span);
        let Ty::Callable { params, ret } = &callee.ty else {
            panic!(
                "supported cleanup lowering at {span:?} should only render direct resolved calls or callable cleanup values"
            );
        };
        assert!(
            args.len() == params.len()
                && args
                    .iter()
                    .all(|arg| matches!(arg, hir::CallArg::Positional(_))),
            "supported cleanup lowering at {span:?} should only render positional callable cleanup arguments matching the callable arity"
        );
        let rendered_args = args
            .iter()
            .zip(params.iter())
            .map(|(arg, param_ty)| {
                let rendered =
                    self.render_cleanup_value_expr(output, guard_call_arg_expr(arg), param_ty, span);
                assert!(
                    param_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible callable cleanup arguments"
                );
                format!("{} {}", rendered.llvm_ty, rendered.repr)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let return_ty = ret.as_ref().clone();
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "cleanup call result")
            .expect("supported cleanup lowering should only emit lowered callable cleanup results");

        if is_void_ty(&return_ty) {
            let _ = writeln!(
                output,
                "  call {return_llvm_ty} {}({rendered_args})",
                callee.repr
            );
            None
        } else {
            let temp = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {temp} = call {return_llvm_ty} {}({rendered_args})",
                callee.repr
            );
            Some(LoweredValue {
                ty: return_ty,
                llvm_ty: return_llvm_ty,
                repr: temp,
            })
        }
    }

    fn render_cleanup_tuple_expr(
        &mut self,
        output: &mut String,
        items: &[hir::ExprId],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let tuple_ty = match expected_ty {
            Ty::Tuple(items) => Ty::Tuple(items.clone()),
            other => panic!(
                "supported cleanup lowering at {span:?} should only render tuple literals with tuple expected types, found `{other}`"
            ),
        };
        let Ty::Tuple(expected_items) = &tuple_ty else {
            unreachable!();
        };
        assert_eq!(
            expected_items.len(),
            items.len(),
            "supported cleanup lowering at {span:?} should preserve cleanup tuple literal arity"
        );
        let rendered_items = items
            .iter()
            .zip(expected_items.iter())
            .map(|(item, item_ty)| self.render_cleanup_value_expr(output, *item, item_ty, span))
            .collect::<Vec<_>>();
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&tuple_ty, span, "cleanup tuple value")
            .expect("supported cleanup lowering should lower cleanup tuple values");

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

    fn render_cleanup_array_expr(
        &mut self,
        output: &mut String,
        items: &[hir::ExprId],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let array_ty = match expected_ty {
            Ty::Array { element, len } => {
                let known_len = known_array_len(len).expect(
                    "supported cleanup lowering should only render concrete fixed-array literals",
                );
                assert_eq!(
                    known_len,
                    items.len(),
                    "supported cleanup lowering at {span:?} should preserve cleanup array literal length"
                );
                Ty::Array {
                    element: element.clone(),
                    len: len.clone(),
                }
            }
            other => panic!(
                "supported cleanup lowering at {span:?} should only render array literals with array expected types, found `{other}`"
            ),
        };
        let Ty::Array { element, .. } = &array_ty else {
            unreachable!();
        };
        let rendered_items = items
            .iter()
            .map(|item| self.render_cleanup_value_expr(output, *item, element.as_ref(), span))
            .collect::<Vec<_>>();
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&array_ty, span, "cleanup array value")
            .expect("supported cleanup lowering should lower cleanup array values");

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

    fn render_cleanup_repeat_array_expr(
        &mut self,
        output: &mut String,
        value: hir::ExprId,
        len: &hir::ArrayLen,
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let array_ty = match expected_ty {
            Ty::Array { element, len } => Ty::Array {
                element: element.clone(),
                len: len.clone(),
            },
            other => panic!(
                "supported cleanup lowering at {span:?} should only render repeat-array literals with array expected types, found `{other}`"
            ),
        };
        let Ty::Array {
            element,
            len: expected_len,
        } = &array_ty
        else {
            unreachable!();
        };
        let repeat_len = match len {
            hir::ArrayLen::Known(len) => *len,
            hir::ArrayLen::Generic(_) => known_array_len(expected_len)
                .expect("supported cleanup repeat-array lowering should use concrete lengths"),
        };
        assert!(
            known_array_len(expected_len).is_some_and(|expected| expected == repeat_len),
            "supported cleanup repeat-array lowering at {span:?} should preserve array length"
        );
        let rendered = self.render_cleanup_value_expr(output, value, element.as_ref(), span);
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&array_ty, span, "cleanup repeat-array value")
            .expect("supported cleanup lowering should lower cleanup repeat-array values");

        if repeat_len == 0 {
            return LoweredValue {
                ty: array_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            };
        }

        let mut aggregate = "undef".to_owned();
        for index in 0..repeat_len {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                rendered.llvm_ty, rendered.repr, index
            );
            aggregate = next;
        }

        LoweredValue {
            ty: array_ty,
            llvm_ty,
            repr: aggregate,
        }
    }

    fn render_cleanup_struct_expr(
        &mut self,
        output: &mut String,
        fields: &[hir::StructLiteralField],
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        let struct_ty = expected_ty.clone();
        let field_layouts = self
            .emitter
            .struct_field_lowerings(&struct_ty, span, "cleanup struct value")
            .unwrap_or_else(|_| {
                panic!(
                    "supported cleanup lowering at {span:?} should have a loadable struct literal layout"
                )
            });
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&struct_ty, span, "cleanup struct value")
            .unwrap_or_else(|_| {
                panic!(
                    "supported cleanup lowering at {span:?} should have a lowered struct literal type"
                )
            });
        let mut rendered_fields = HashMap::with_capacity(fields.len());
        for field in fields {
            let field_ty = field_layouts
                .iter()
                .find(|layout| layout.name == field.name)
                .map(|layout| &layout.ty)
                .unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should provide declared struct literal field `{}`",
                        field.name
                    )
                });
            let rendered = self.render_cleanup_value_expr(output, field.value, field_ty, span);
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
                    "supported cleanup lowering at {span:?} should provide every declared struct literal field"
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

    fn render_cleanup_projection_pointer(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
    ) -> Option<(String, Ty)> {
        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Tuple(items) => {
                let tuple_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered = self.render_cleanup_tuple_expr(output, items, &tuple_ty, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Array(items) => {
                let array_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered = self.render_cleanup_array_expr(output, items, &array_ty, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::RepeatArray { value, len, .. } => {
                let array_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered =
                    self.render_cleanup_repeat_array_expr(output, *value, len, &array_ty, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                let struct_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered = self.render_cleanup_struct_expr(output, fields, &struct_ty, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch: Some(other),
            } => {
                let value_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered = self.render_cleanup_value_if_expr(
                    output,
                    *condition,
                    *then_branch,
                    *other,
                    &value_ty,
                    span,
                );
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Match { value, arms } => {
                let value_ty = self.emitter.input.typeck.expr_ty(expr_id)?.clone();
                let rendered =
                    self.render_cleanup_value_match_expr(output, *value, arms, &value_ty, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let rendered = self.render_cleanup_assignment_expr(output, *left, *right, span);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let rendered = self.render_cleanup_await_expr(output, *expr, span);
                if is_void_ty(&rendered.ty) {
                    return None;
                }
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Member { object, field, .. } => {
                let (current_ptr, current_ty) =
                    self.render_cleanup_projection_pointer(output, *object, span)?;
                let aggregate_llvm_ty = self
                    .emitter
                    .lower_llvm_type(&current_ty, span, "projection base type")
                    .ok()?;
                let step = self
                    .emitter
                    .resolve_projection_step(
                        &current_ty,
                        &mir::ProjectionElem::Field(field.clone()),
                        span,
                    )
                    .ok()?;
                let ResolvedProjectionStep::Field { index, ty } = step else {
                    return None;
                };
                let next = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i32 0, i32 {index}"
                );
                Some((next, ty))
            }
            hir::ExprKind::Bracket { target, items } => {
                let (mut current_ptr, mut current_ty) =
                    self.render_cleanup_projection_pointer(output, *target, span)?;
                for item in items {
                    let aggregate_llvm_ty = self
                        .emitter
                        .lower_llvm_type(&current_ty, span, "projection base type")
                        .ok()?;
                    match &current_ty {
                        Ty::Array { element, .. } => {
                            let rendered_index =
                                self.render_guard_scalar_expr(output, *item, span, None);
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
                            current_ty = element.as_ref().clone();
                        }
                        Ty::Tuple(_) => {
                            let index = guard_literal_int(
                                self.emitter.input.hir,
                                self.emitter.input.resolution,
                                *item,
                            )?;
                            if index < 0 {
                                return None;
                            }
                            let step = self
                                .emitter
                                .resolve_projection_step(
                                    &current_ty,
                                    &mir::ProjectionElem::Index(Box::new(Operand::Constant(
                                        Constant::Integer(index.to_string()),
                                    ))),
                                    span,
                                )
                                .ok()?;
                            let ResolvedProjectionStep::TupleIndex { index, ty } = step else {
                                return None;
                            };
                            let next = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i32 0, i32 {index}"
                            );
                            current_ptr = next;
                            current_ty = ty;
                        }
                        _ => return None,
                    }
                }
                Some((current_ptr, current_ty))
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                let binding_depth = self.cleanup_bindings.len();
                let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
                let projected = self
                    .render_cleanup_block_prefix(output, *block_id, span)
                    .and_then(|tail| self.render_cleanup_projection_pointer(output, tail, span));
                self.cleanup_bindings.truncate(binding_depth);
                self.cleanup_capturing_closure_bindings
                    .truncate(closure_binding_depth);
                projected
            }
            hir::ExprKind::Question(inner) => {
                self.render_cleanup_projection_pointer(output, *inner, span)
            }
            _ => self.render_guard_projection_pointer(output, expr_id, span, None),
        }
    }

    fn render_cleanup_value_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        expected_ty: &Ty,
        span: Span,
    ) -> LoweredValue {
        if let Some(source_expr) = self.emitter.literal_source_expr(expr_id) {
            return self.render_cleanup_value_expr(output, source_expr, expected_ty, span);
        }

        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                let rendered = self
                    .render_cleanup_call(output, *callee, args, span)
                    .expect("cleanup call arguments should produce a value");
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup-call values"
                );
                rendered
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                let binding_depth = self.cleanup_bindings.len();
                let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
                let tail = self
                    .render_cleanup_block_prefix(output, *block_id, span)
                    .unwrap_or_else(|| {
                    panic!(
                        "supported cleanup lowering at {span:?} should only render valued cleanup blocks with tails"
                    )
                });
                let rendered = self.render_cleanup_value_expr(output, tail, expected_ty, span);
                self.cleanup_bindings.truncate(binding_depth);
                self.cleanup_capturing_closure_bindings
                    .truncate(closure_binding_depth);
                rendered
            }
            hir::ExprKind::Question(inner) => {
                self.render_cleanup_value_expr(output, *inner, expected_ty, span)
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let rendered = self.render_cleanup_await_expr(output, *expr, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup await values"
                );
                rendered
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Spawn,
                expr,
            } => {
                let rendered = self.render_cleanup_spawn_expr(output, *expr, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup spawn values"
                );
                rendered
            }
            hir::ExprKind::If {
                condition,
                then_branch,
                else_branch: Some(other),
            } => self.render_cleanup_value_if_expr(
                output,
                *condition,
                *then_branch,
                *other,
                expected_ty,
                span,
            ),
            hir::ExprKind::Match { value, arms } => {
                self.render_cleanup_value_match_expr(output, *value, arms, expected_ty, span)
            }
            hir::ExprKind::Tuple(items) => {
                let rendered = self.render_cleanup_tuple_expr(output, items, expected_ty, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup tuple values"
                );
                rendered
            }
            hir::ExprKind::Array(items) => {
                let rendered = self.render_cleanup_array_expr(output, items, expected_ty, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup array values"
                );
                rendered
            }
            hir::ExprKind::RepeatArray { value, len, .. } => {
                let rendered =
                    self.render_cleanup_repeat_array_expr(output, *value, len, expected_ty, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup repeat-array values"
                );
                rendered
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                let rendered = self.render_cleanup_struct_expr(output, fields, expected_ty, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup struct values"
                );
                rendered
            }
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let rendered = self.render_cleanup_assignment_expr(output, *left, *right, span);
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup assignment values"
                );
                rendered
            }
            _ => {
                let rendered = if expected_ty.is_bool() {
                    self.render_bool_guard_expr(output, expr_id, span, None)
                } else if expected_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
                    self.render_guard_scalar_expr(output, expr_id, span, None)
                } else {
                    match self.render_cleanup_projection_pointer(output, expr_id, span) {
                        Some((ptr, ty)) => self.render_loaded_pointer_value(output, ptr, ty, span),
                        None => self.render_guard_loadable_expr(output, expr_id, span, None),
                    }
                };
                assert!(
                    expected_ty.compatible_with(&rendered.ty),
                    "supported cleanup lowering at {span:?} should only render compatible cleanup values"
                );
                rendered
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
            TerminatorKind::Match { scrutinee, .. } => {
                let Some(match_lowering) = self.prepared.supported_matches.get(&block_id) else {
                    panic!(
                        "prepared `match` at block {:?} should have lowering metadata",
                        block_id
                    )
                };
                let rendered = self.render_operand(output, scrutinee, terminator.span);
                match match_lowering {
                    SupportedMatchLowering::Bool {
                        true_target,
                        false_target,
                    } => {
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %bb{}, label %bb{}",
                            rendered.repr,
                            true_target.index(),
                            false_target.index()
                        );
                    }
                    SupportedMatchLowering::GuardOnly {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }

                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };

                            match arm.guard {
                                SupportedBoolGuard::Always => {
                                    let _ =
                                        writeln!(output, "  br label %bb{}", arm.target.index());
                                }
                                SupportedBoolGuard::Dynamic(expr_id) => {
                                    let guard_target =
                                        integer_match_guard_block_name(block_id, index);
                                    let _ = writeln!(output, "  br label %{guard_target}");
                                    let _ = writeln!(output, "{guard_target}:");
                                    self.bind_pattern_value(
                                        output,
                                        arm.pattern,
                                        rendered.clone(),
                                        terminator.span,
                                    );
                                    let condition = self.render_bool_guard_expr(
                                        output,
                                        expr_id,
                                        terminator.span,
                                        None,
                                    );
                                    let _ = writeln!(
                                        output,
                                        "  br i1 {}, label %bb{}, label %{false_target}",
                                        condition.repr,
                                        arm.target.index()
                                    );
                                }
                            }
                        }
                    }
                    SupportedMatchLowering::BoolGuarded {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    bool_match_dispatch_block_name(block_id, index)
                                );
                            }

                            let next_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                bool_match_dispatch_block_name(block_id, index + 1)
                            };
                            let guard_target = match arm.guard {
                                SupportedBoolGuard::Always => format!("bb{}", arm.target.index()),
                                SupportedBoolGuard::Dynamic(_) => {
                                    bool_match_guard_block_name(block_id, index)
                                }
                            };

                            match arm.pattern {
                                SupportedBoolMatchPattern::True => {
                                    let _ = writeln!(
                                        output,
                                        "  br i1 {}, label %{guard_target}, label %{next_target}",
                                        rendered.repr
                                    );
                                }
                                SupportedBoolMatchPattern::False => {
                                    let _ = writeln!(
                                        output,
                                        "  br i1 {}, label %{next_target}, label %{guard_target}",
                                        rendered.repr
                                    );
                                }
                                SupportedBoolMatchPattern::CatchAll => match arm.guard {
                                    SupportedBoolGuard::Always => {
                                        let _ = writeln!(
                                            output,
                                            "  br label %bb{}",
                                            arm.target.index()
                                        );
                                    }
                                    SupportedBoolGuard::Dynamic(_) => {
                                        let _ = writeln!(output, "  br label %{guard_target}");
                                    }
                                },
                            }

                            if let SupportedBoolGuard::Dynamic(expr_id) = arm.guard {
                                let _ = writeln!(output, "{guard_target}:");
                                let guard_binding =
                                    arm.binding_local.map(|local| GuardBindingValue {
                                        local,
                                        value: rendered.clone(),
                                    });
                                if let Some(binding) = guard_binding.as_ref()
                                    && let Some(local) =
                                        mir_local_for_hir_local(self.body, binding.local)
                                {
                                    let _ = writeln!(
                                        output,
                                        "  store {} {}, ptr {}",
                                        binding.value.llvm_ty,
                                        binding.value.repr,
                                        llvm_slot_name(self.body, local)
                                    );
                                }
                                let condition = self.render_bool_guard_expr(
                                    output,
                                    expr_id,
                                    terminator.span,
                                    guard_binding.as_ref(),
                                );
                                let _ = writeln!(
                                    output,
                                    "  br i1 {}, label %bb{}, label %{next_target}",
                                    condition.repr,
                                    arm.target.index()
                                );
                            }
                        }
                    }
                    SupportedMatchLowering::Integer {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        let opcode = compare_opcode(BinaryOp::EqEq, &rendered.ty);
                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }
                            let compare = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {compare} = {opcode} {} {}, {}",
                                rendered.llvm_ty, rendered.repr, arm.value
                            );
                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };
                            let _ = writeln!(
                                output,
                                "  br i1 {compare}, label %bb{}, label %{false_target}",
                                arm.target.index()
                            );
                        }
                    }
                    SupportedMatchLowering::IntegerGuarded {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        let opcode = compare_opcode(BinaryOp::EqEq, &rendered.ty);
                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }
                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };
                            let guard_target = match arm.guard {
                                SupportedBoolGuard::Always => format!("bb{}", arm.target.index()),
                                SupportedBoolGuard::Dynamic(_) => {
                                    integer_match_guard_block_name(block_id, index)
                                }
                            };

                            match &arm.pattern {
                                SupportedIntegerMatchPattern::Literal(value) => {
                                    let compare = self.fresh_temp();
                                    let _ = writeln!(
                                        output,
                                        "  {compare} = {opcode} {} {}, {}",
                                        rendered.llvm_ty, rendered.repr, value
                                    );
                                    match arm.guard {
                                        SupportedBoolGuard::Always => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %bb{}, label %{false_target}",
                                                arm.target.index()
                                            );
                                        }
                                        SupportedBoolGuard::Dynamic(_) => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %{guard_target}, label %{false_target}"
                                            );
                                        }
                                    }
                                }
                                SupportedIntegerMatchPattern::CatchAll => match arm.guard {
                                    SupportedBoolGuard::Always => {
                                        let _ = writeln!(
                                            output,
                                            "  br label %bb{}",
                                            arm.target.index()
                                        );
                                    }
                                    SupportedBoolGuard::Dynamic(_) => {
                                        let _ = writeln!(output, "  br label %{guard_target}");
                                    }
                                },
                            }

                            if let SupportedBoolGuard::Dynamic(expr_id) = arm.guard {
                                let _ = writeln!(output, "{guard_target}:");
                                let guard_binding =
                                    arm.binding_local.map(|local| GuardBindingValue {
                                        local,
                                        value: rendered.clone(),
                                    });
                                if let Some(binding) = guard_binding.as_ref()
                                    && let Some(local) =
                                        mir_local_for_hir_local(self.body, binding.local)
                                {
                                    let _ = writeln!(
                                        output,
                                        "  store {} {}, ptr {}",
                                        binding.value.llvm_ty,
                                        binding.value.repr,
                                        llvm_slot_name(self.body, local)
                                    );
                                }
                                let condition = self.render_bool_guard_expr(
                                    output,
                                    expr_id,
                                    terminator.span,
                                    guard_binding.as_ref(),
                                );
                                let _ = writeln!(
                                    output,
                                    "  br i1 {}, label %bb{}, label %{false_target}",
                                    condition.repr,
                                    arm.target.index()
                                );
                            }
                        }
                    }
                    SupportedMatchLowering::String {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }
                            let compare = self
                                .render_string_match_literal_condition(
                                    output,
                                    rendered.clone(),
                                    &arm.value,
                                    terminator.span,
                                )
                                .repr;
                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };
                            let _ = writeln!(
                                output,
                                "  br i1 {compare}, label %bb{}, label %{false_target}",
                                arm.target.index()
                            );
                        }
                    }
                    SupportedMatchLowering::StringGuarded {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }
                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };
                            let guard_target = match arm.guard {
                                SupportedBoolGuard::Always => format!("bb{}", arm.target.index()),
                                SupportedBoolGuard::Dynamic(_) => {
                                    integer_match_guard_block_name(block_id, index)
                                }
                            };

                            match &arm.pattern {
                                SupportedStringMatchPattern::Literal(value) => {
                                    let compare = self
                                        .render_string_match_literal_condition(
                                            output,
                                            rendered.clone(),
                                            value,
                                            terminator.span,
                                        )
                                        .repr;
                                    match arm.guard {
                                        SupportedBoolGuard::Always => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %bb{}, label %{false_target}",
                                                arm.target.index()
                                            );
                                        }
                                        SupportedBoolGuard::Dynamic(_) => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %{guard_target}, label %{false_target}"
                                            );
                                        }
                                    }
                                }
                                SupportedStringMatchPattern::CatchAll => match arm.guard {
                                    SupportedBoolGuard::Always => {
                                        let _ = writeln!(
                                            output,
                                            "  br label %bb{}",
                                            arm.target.index()
                                        );
                                    }
                                    SupportedBoolGuard::Dynamic(_) => {
                                        let _ = writeln!(output, "  br label %{guard_target}");
                                    }
                                },
                            }

                            if let SupportedBoolGuard::Dynamic(expr_id) = arm.guard {
                                let _ = writeln!(output, "{guard_target}:");
                                let guard_binding =
                                    arm.binding_local.map(|local| GuardBindingValue {
                                        local,
                                        value: rendered.clone(),
                                    });
                                if let Some(binding) = guard_binding.as_ref()
                                    && let Some(local) =
                                        mir_local_for_hir_local(self.body, binding.local)
                                {
                                    let _ = writeln!(
                                        output,
                                        "  store {} {}, ptr {}",
                                        binding.value.llvm_ty,
                                        binding.value.repr,
                                        llvm_slot_name(self.body, local)
                                    );
                                }
                                let condition = self.render_bool_guard_expr(
                                    output,
                                    expr_id,
                                    terminator.span,
                                    guard_binding.as_ref(),
                                );
                                let _ = writeln!(
                                    output,
                                    "  br i1 {}, label %bb{}, label %{false_target}",
                                    condition.repr,
                                    arm.target.index()
                                );
                            }
                        }
                    }
                    SupportedMatchLowering::Enum {
                        arms,
                        fallback_target,
                    } => {
                        if arms.is_empty() {
                            let _ = writeln!(output, "  br label %bb{}", fallback_target.index());
                            return;
                        }

                        let tag = self.fresh_temp();
                        let _ = writeln!(
                            output,
                            "  {tag} = extractvalue {} {}, 0",
                            rendered.llvm_ty, rendered.repr
                        );

                        for (index, arm) in arms.iter().enumerate() {
                            if index > 0 {
                                let _ = writeln!(
                                    output,
                                    "{}:",
                                    integer_match_dispatch_block_name(block_id, index)
                                );
                            }
                            let false_target = if index + 1 == arms.len() {
                                format!("bb{}", fallback_target.index())
                            } else {
                                integer_match_dispatch_block_name(block_id, index + 1)
                            };
                            let guard_target = match arm.guard {
                                SupportedBoolGuard::Always => format!("bb{}", arm.target.index()),
                                SupportedBoolGuard::Dynamic(_) => {
                                    integer_match_guard_block_name(block_id, index)
                                }
                            };

                            match arm.variant_index {
                                Some(variant_index) => {
                                    let compare = self.fresh_temp();
                                    let _ = writeln!(
                                        output,
                                        "  {compare} = icmp eq i64 {tag}, {variant_index}"
                                    );
                                    match arm.guard {
                                        SupportedBoolGuard::Always => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %bb{}, label %{false_target}",
                                                arm.target.index()
                                            );
                                        }
                                        SupportedBoolGuard::Dynamic(_) => {
                                            let _ = writeln!(
                                                output,
                                                "  br i1 {compare}, label %{guard_target}, label %{false_target}"
                                            );
                                        }
                                    }
                                }
                                None => match arm.guard {
                                    SupportedBoolGuard::Always => {
                                        let _ = writeln!(
                                            output,
                                            "  br label %bb{}",
                                            arm.target.index()
                                        );
                                    }
                                    SupportedBoolGuard::Dynamic(_) => {
                                        let _ = writeln!(output, "  br label %{guard_target}");
                                    }
                                },
                            }

                            if let SupportedBoolGuard::Dynamic(expr_id) = arm.guard {
                                let _ = writeln!(output, "{guard_target}:");
                                self.bind_pattern_value(
                                    output,
                                    arm.pattern,
                                    rendered.clone(),
                                    terminator.span,
                                );
                                let condition = self.render_bool_guard_expr(
                                    output,
                                    expr_id,
                                    terminator.span,
                                    None,
                                );
                                let _ = writeln!(
                                    output,
                                    "  br i1 {}, label %bb{}, label %{false_target}",
                                    condition.repr,
                                    arm.target.index()
                                );
                            }
                        }
                    }
                }
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
                    loop_lowering.iterable_len
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

    fn render_direct_local_capturing_closure_call(
        &mut self,
        output: &mut String,
        closure_id: mir::ClosureId,
        args: &[mir::CallArgument],
        span: Span,
    ) -> Option<LoweredValue> {
        let closure = self.body.closure(closure_id);
        let closure_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(closure.expr)
            .cloned()
            .expect("prepared direct local capturing-closure calls should preserve callable types");
        let Ty::Callable { params, ret } = closure_ty else {
            panic!(
                "prepared direct local capturing-closure calls at {span:?} should only target callable values"
            );
        };
        assert!(
            args.iter().all(|arg| arg.name.is_none()) && args.len() == params.len(),
            "prepared direct local capturing-closure calls at {span:?} should only contain positional arguments matching the callable arity"
        );

        let mut rendered_args = Vec::with_capacity(closure.captures.len() + args.len());
        for capture in &closure.captures {
            let value =
                self.render_operand(output, &Operand::Place(Place::local(capture.local)), span);
            rendered_args.push(format!("{} {}", value.llvm_ty, value.repr));
        }
        for (arg, param_ty) in args.iter().zip(params.iter()) {
            let value = self.render_operand(output, &arg.value, span);
            assert!(
                param_ty.compatible_with(&value.ty),
                "prepared direct local capturing-closure calls at {span:?} should preserve callable argument types"
            );
            rendered_args.push(format!("{} {}", value.llvm_ty, value.repr));
        }

        let rendered_args = rendered_args.join(", ");
        let return_ty = ret.as_ref().clone();
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "call result")
            .expect(
                "prepared direct local capturing-closure calls should only produce lowered callable result types",
            );
        let callee_name = closure_llvm_name(&self.prepared.signature.llvm_name, closure_id);

        if is_void_ty(&return_ty) {
            let _ = writeln!(
                output,
                "  call {return_llvm_ty} @{callee_name}({rendered_args})"
            );
            None
        } else {
            let temp = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {temp} = call {return_llvm_ty} @{callee_name}({rendered_args})"
            );
            Some(LoweredValue {
                ty: return_ty,
                llvm_ty: return_llvm_ty,
                repr: temp,
            })
        }
    }

    fn ordinary_control_flow_capturing_closure_call(
        &self,
        block_id: mir::BasicBlockId,
        callee: &Operand,
    ) -> Option<SupportedOrdinaryCapturingClosureCall> {
        let Operand::Place(place) = callee else {
            return None;
        };
        if !place.projections.is_empty() {
            return None;
        }
        self.prepared
            .ordinary_control_flow_capturing_closure_calls
            .get(&(block_id, place.base))
            .cloned()
    }

    fn direct_local_capturing_closure_call_return_ty(
        &self,
        closure_id: mir::ClosureId,
        span: Span,
    ) -> Ty {
        let closure = self.body.closure(closure_id);
        let closure_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(closure.expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "prepared direct local capturing-closure calls at {span:?} should preserve callable types"
                )
            });
        let Ty::Callable { ret, .. } = closure_ty else {
            panic!(
                "prepared direct local capturing-closure calls at {span:?} should only target callable values"
            );
        };
        ret.as_ref().clone()
    }

    fn render_ordinary_control_flow_capturing_closure_call(
        &mut self,
        output: &mut String,
        lowering: &SupportedOrdinaryCapturingClosureCall,
        args: &[mir::CallArgument],
        span: Span,
    ) -> Option<LoweredValue> {
        match lowering {
            SupportedOrdinaryCapturingClosureCall::Branch {
                condition,
                then_closure,
                else_closure,
            } => {
                let condition = self.render_operand(output, condition, span);
                assert!(
                    condition.ty.is_bool(),
                    "prepared ordinary capturing-closure control-flow calls at {span:?} should branch on `Bool` conditions"
                );
                let return_ty =
                    self.direct_local_capturing_closure_call_return_ty(*then_closure, span);
                let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
                    self.emitter
                        .lower_llvm_type(&return_ty, span, "call result")
                        .expect(
                            "prepared ordinary capturing-closure control-flow calls should lower callable result types",
                        )
                });
                let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
                    let slot = self.fresh_temp();
                    let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
                    slot
                });
                let then_label = self.fresh_label("ordinary_call_if_then");
                let else_label = self.fresh_label("ordinary_call_if_else");
                let end_label = self.fresh_label("ordinary_call_if_end");
                let _ = writeln!(
                    output,
                    "  br i1 {}, label %{then_label}, label %{else_label}",
                    condition.repr
                );

                let _ = writeln!(output, "{then_label}:");
                if let Some(value) = self.render_direct_local_capturing_closure_call(
                    output,
                    *then_closure,
                    args,
                    span,
                ) && let Some(result_slot) = result_slot.as_ref()
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {result_slot}",
                        value.llvm_ty, value.repr
                    );
                }
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{else_label}:");
                if let Some(value) = self.render_direct_local_capturing_closure_call(
                    output,
                    *else_closure,
                    args,
                    span,
                ) && let Some(result_slot) = result_slot.as_ref()
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {result_slot}",
                        value.llvm_ty, value.repr
                    );
                }
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{end_label}:");
                result_slot.map(|slot| {
                    self.render_loaded_pointer_value(output, slot, return_ty.clone(), span)
                })
            }
            SupportedOrdinaryCapturingClosureCall::BoolMatch {
                scrutinee,
                true_closure,
                false_closure,
            } => self.render_ordinary_control_flow_capturing_closure_call(
                output,
                &SupportedOrdinaryCapturingClosureCall::Branch {
                    condition: scrutinee.clone(),
                    then_closure: *true_closure,
                    else_closure: *false_closure,
                },
                args,
                span,
            ),
            SupportedOrdinaryCapturingClosureCall::IntegerMatch {
                scrutinee,
                arms,
                fallback_closure,
            } => {
                let scrutinee = self.render_operand(output, scrutinee, span);
                let return_ty =
                    self.direct_local_capturing_closure_call_return_ty(*fallback_closure, span);
                let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
                    self.emitter
                        .lower_llvm_type(&return_ty, span, "call result")
                        .expect(
                            "prepared ordinary capturing-closure control-flow calls should lower callable result types",
                        )
                });
                let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
                    let slot = self.fresh_temp();
                    let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
                    slot
                });
                let end_label = self.fresh_label("ordinary_call_match_end");
                let fallback_label = self.fresh_label("ordinary_call_match_fallback");
                let dispatch_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_dispatch"))
                    .collect::<Vec<_>>();
                let arm_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_arm"))
                    .collect::<Vec<_>>();
                let opcode = compare_opcode(BinaryOp::EqEq, &scrutinee.ty);

                if arms.is_empty() {
                    let _ = writeln!(output, "  br label %{fallback_label}");
                } else {
                    for (index, arm) in arms.iter().enumerate() {
                        if index > 0 {
                            let _ = writeln!(output, "{}:", dispatch_labels[index]);
                        }
                        let compare = self.fresh_temp();
                        let _ = writeln!(
                            output,
                            "  {compare} = {opcode} {} {}, {}",
                            scrutinee.llvm_ty, scrutinee.repr, arm.value
                        );
                        let false_target = if index + 1 == arms.len() {
                            fallback_label.clone()
                        } else {
                            dispatch_labels[index + 1].clone()
                        };
                        let _ = writeln!(
                            output,
                            "  br i1 {compare}, label %{}, label %{false_target}",
                            arm_labels[index]
                        );
                    }
                }

                for (arm, arm_label) in arms.iter().zip(arm_labels.iter()) {
                    let _ = writeln!(output, "{arm_label}:");
                    if let Some(value) = self.render_direct_local_capturing_closure_call(
                        output,
                        arm.closure_id,
                        args,
                        span,
                    ) && let Some(result_slot) = result_slot.as_ref()
                    {
                        let _ = writeln!(
                            output,
                            "  store {} {}, ptr {result_slot}",
                            value.llvm_ty, value.repr
                        );
                    }
                    let _ = writeln!(output, "  br label %{end_label}");
                }

                let _ = writeln!(output, "{fallback_label}:");
                if let Some(value) = self.render_direct_local_capturing_closure_call(
                    output,
                    *fallback_closure,
                    args,
                    span,
                ) && let Some(result_slot) = result_slot.as_ref()
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {result_slot}",
                        value.llvm_ty, value.repr
                    );
                }
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{end_label}:");
                result_slot.map(|slot| {
                    self.render_loaded_pointer_value(output, slot, return_ty.clone(), span)
                })
            }
            SupportedOrdinaryCapturingClosureCall::StringMatch {
                scrutinee,
                arms,
                fallback_closure,
            } => {
                let scrutinee = self.render_operand(output, scrutinee, span);
                let return_ty =
                    self.direct_local_capturing_closure_call_return_ty(*fallback_closure, span);
                let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
                    self.emitter
                        .lower_llvm_type(&return_ty, span, "call result")
                        .expect(
                            "prepared ordinary capturing-closure control-flow calls should lower callable result types",
                        )
                });
                let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
                    let slot = self.fresh_temp();
                    let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
                    slot
                });
                let end_label = self.fresh_label("ordinary_call_match_end");
                let fallback_label = self.fresh_label("ordinary_call_match_fallback");
                let dispatch_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_dispatch"))
                    .collect::<Vec<_>>();
                let arm_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_arm"))
                    .collect::<Vec<_>>();

                if arms.is_empty() {
                    let _ = writeln!(output, "  br label %{fallback_label}");
                } else {
                    for (index, arm) in arms.iter().enumerate() {
                        if index > 0 {
                            let _ = writeln!(output, "{}:", dispatch_labels[index]);
                        }
                        let compare = self
                            .render_string_match_literal_condition(
                                output,
                                scrutinee.clone(),
                                &arm.value,
                                span,
                            )
                            .repr;
                        let false_target = if index + 1 == arms.len() {
                            fallback_label.clone()
                        } else {
                            dispatch_labels[index + 1].clone()
                        };
                        let _ = writeln!(
                            output,
                            "  br i1 {compare}, label %{}, label %{false_target}",
                            arm_labels[index]
                        );
                    }
                }

                for (arm, arm_label) in arms.iter().zip(arm_labels.iter()) {
                    let _ = writeln!(output, "{arm_label}:");
                    if let Some(value) = self.render_direct_local_capturing_closure_call(
                        output,
                        arm.closure_id,
                        args,
                        span,
                    ) && let Some(result_slot) = result_slot.as_ref()
                    {
                        let _ = writeln!(
                            output,
                            "  store {} {}, ptr {result_slot}",
                            value.llvm_ty, value.repr
                        );
                    }
                    let _ = writeln!(output, "  br label %{end_label}");
                }

                let _ = writeln!(output, "{fallback_label}:");
                if let Some(value) = self.render_direct_local_capturing_closure_call(
                    output,
                    *fallback_closure,
                    args,
                    span,
                ) && let Some(result_slot) = result_slot.as_ref()
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {result_slot}",
                        value.llvm_ty, value.repr
                    );
                }
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{end_label}:");
                result_slot.map(|slot| {
                    self.render_loaded_pointer_value(output, slot, return_ty.clone(), span)
                })
            }
            SupportedOrdinaryCapturingClosureCall::StringGuardedMatch {
                scrutinee,
                arms,
                fallback_closure,
            } => {
                let scrutinee = self.render_operand(output, scrutinee, span);
                let return_ty =
                    self.direct_local_capturing_closure_call_return_ty(*fallback_closure, span);
                let return_llvm_ty = (!is_void_ty(&return_ty)).then(|| {
                    self.emitter
                        .lower_llvm_type(&return_ty, span, "call result")
                        .expect(
                            "prepared ordinary capturing-closure control-flow calls should lower callable result types",
                        )
                });
                let result_slot = return_llvm_ty.as_ref().map(|llvm_ty| {
                    let slot = self.fresh_temp();
                    let _ = writeln!(output, "  {slot} = alloca {llvm_ty}");
                    slot
                });
                let end_label = self.fresh_label("ordinary_call_match_end");
                let fallback_label = self.fresh_label("ordinary_call_match_fallback");
                let dispatch_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_dispatch"))
                    .collect::<Vec<_>>();
                let guard_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_guard"))
                    .collect::<Vec<_>>();
                let arm_labels = (0..arms.len())
                    .map(|_| self.fresh_label("ordinary_call_match_arm"))
                    .collect::<Vec<_>>();

                if arms.is_empty() {
                    let _ = writeln!(output, "  br label %{fallback_label}");
                } else {
                    for (index, arm) in arms.iter().enumerate() {
                        if index > 0 {
                            let _ = writeln!(output, "{}:", dispatch_labels[index]);
                        }
                        let false_target = if index + 1 == arms.len() {
                            fallback_label.clone()
                        } else {
                            dispatch_labels[index + 1].clone()
                        };
                        let guard_target = match arm.guard {
                            SupportedBoolGuard::Always => arm_labels[index].clone(),
                            SupportedBoolGuard::Dynamic(_) => guard_labels[index].clone(),
                        };

                        match &arm.pattern {
                            SupportedStringMatchPattern::Literal(value) => {
                                let compare = self
                                    .render_string_match_literal_condition(
                                        output,
                                        scrutinee.clone(),
                                        value,
                                        span,
                                    )
                                    .repr;
                                let _ = writeln!(
                                    output,
                                    "  br i1 {compare}, label %{guard_target}, label %{false_target}"
                                );
                            }
                            SupportedStringMatchPattern::CatchAll => {
                                let _ = writeln!(output, "  br label %{guard_target}");
                            }
                        }

                        if let SupportedBoolGuard::Dynamic(expr_id) = arm.guard {
                            let _ = writeln!(output, "{}:", guard_labels[index]);
                            let guard_binding = arm.binding_local.map(|local| GuardBindingValue {
                                local,
                                value: scrutinee.clone(),
                            });
                            if let Some(binding) = guard_binding.as_ref()
                                && let Some(local) =
                                    mir_local_for_hir_local(self.body, binding.local)
                            {
                                let _ = writeln!(
                                    output,
                                    "  store {} {}, ptr {}",
                                    binding.value.llvm_ty,
                                    binding.value.repr,
                                    llvm_slot_name(self.body, local)
                                );
                            }
                            let condition = self.render_bool_guard_expr(
                                output,
                                expr_id,
                                span,
                                guard_binding.as_ref(),
                            );
                            let _ = writeln!(
                                output,
                                "  br i1 {}, label %{}, label %{false_target}",
                                condition.repr, arm_labels[index]
                            );
                        }
                    }
                }

                for (arm, arm_label) in arms.iter().zip(arm_labels.iter()) {
                    let _ = writeln!(output, "{arm_label}:");
                    if let Some(value) = self.render_direct_local_capturing_closure_call(
                        output,
                        arm.closure_id,
                        args,
                        span,
                    ) && let Some(result_slot) = result_slot.as_ref()
                    {
                        let _ = writeln!(
                            output,
                            "  store {} {}, ptr {result_slot}",
                            value.llvm_ty, value.repr
                        );
                    }
                    let _ = writeln!(output, "  br label %{end_label}");
                }

                let _ = writeln!(output, "{fallback_label}:");
                if let Some(value) = self.render_direct_local_capturing_closure_call(
                    output,
                    *fallback_closure,
                    args,
                    span,
                ) && let Some(result_slot) = result_slot.as_ref()
                {
                    let _ = writeln!(
                        output,
                        "  store {} {}, ptr {result_slot}",
                        value.llvm_ty, value.repr
                    );
                }
                let _ = writeln!(output, "  br label %{end_label}");

                let _ = writeln!(output, "{end_label}:");
                result_slot.map(|slot| {
                    self.render_loaded_pointer_value(output, slot, return_ty.clone(), span)
                })
            }
        }
    }

    fn render_rvalue(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        value: &Rvalue,
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        match value {
            Rvalue::Use(operand) => Some(self.render_operand(output, operand, span)),
            Rvalue::Question(operand) => Some(self.render_operand(output, operand, span)),
            Rvalue::Call { callee, args } => {
                if let Operand::Place(place) = callee
                    && place.projections.is_empty()
                    && let Some(closure_id) = self
                        .prepared
                        .direct_local_capturing_closures
                        .get(&place.base)
                        .copied()
                {
                    self.render_direct_local_capturing_closure_call(output, closure_id, args, span)
                } else if let Some(lowering) =
                    self.ordinary_control_flow_capturing_closure_call(block_id, callee)
                {
                    self.render_ordinary_control_flow_capturing_closure_call(
                        output, &lowering, args, span,
                    )
                } else if let Some(function) = self.emitter.resolve_direct_callee_function(callee) {
                    let signature = self
                        .emitter
                        .signatures
                        .get(&function)
                        .expect("callee signatures should exist");
                    let rendered_args = self
                        .emitter
                        .ordered_call_args(args, signature)
                        .expect("prepared direct calls should preserve callee argument mapping")
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
                } else {
                    let callee = self.render_operand(output, callee, span);
                    let Ty::Callable { params, ret } = &callee.ty else {
                        panic!(
                            "prepared calls should only contain direct resolved callees or callable operands"
                        );
                    };
                    assert!(
                        args.iter().all(|arg| arg.name.is_none()) && args.len() == params.len(),
                        "prepared indirect calls at {span:?} should only contain positional arguments matching the callable arity"
                    );
                    let rendered_args = args
                        .iter()
                        .zip(params.iter())
                        .map(|(arg, param_ty)| {
                            let value = self.render_operand(output, &arg.value, span);
                            assert!(
                                backend_value_compatible(
                                    self.emitter.input.hir,
                                    self.emitter.input.resolution,
                                    param_ty,
                                    &value.ty,
                                ),
                                "prepared indirect calls at {span:?} should preserve callable argument types"
                            );
                            format!("{} {}", value.llvm_ty, value.repr)
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    let return_ty = ret.as_ref().clone();
                    let return_llvm_ty = self
                        .emitter
                        .lower_llvm_type(&return_ty, span, "call result")
                        .expect(
                            "prepared indirect calls should only produce lowered callable result types",
                        );

                    if is_void_ty(&return_ty) {
                        let _ = writeln!(
                            output,
                            "  call {return_llvm_ty} {}({rendered_args})",
                            callee.repr
                        );
                        None
                    } else {
                        let temp = self.fresh_temp();
                        let _ = writeln!(
                            output,
                            "  {temp} = call {return_llvm_ty} {}({rendered_args})",
                            callee.repr
                        );
                        Some(LoweredValue {
                            ty: return_ty,
                            llvm_ty: return_llvm_ty,
                            repr: temp,
                        })
                    }
                }
            }
            Rvalue::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let Operand::Place(place) = left else {
                    panic!(
                        "prepared assignment expressions at {span:?} should only target mutable places"
                    );
                };
                let target_ty = self.prepared_place_type(place, span).unwrap_or_else(|| {
                    panic!(
                        "prepared assignment expressions at {span:?} should have a resolved target type"
                    )
                });
                let rendered = self.render_operand(output, right, span);
                assert!(
                    backend_value_compatible(
                        self.emitter.input.hir,
                        self.emitter.input.resolution,
                        &target_ty,
                        &rendered.ty,
                    ),
                    "prepared assignment expressions at {span:?} should preserve value compatibility with the target place"
                );
                let target_ptr = if place.projections.is_empty() {
                    llvm_slot_name(self.body, place.base)
                } else {
                    self.render_place_pointer(output, place, span).0
                };
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {}",
                    rendered.llvm_ty, rendered.repr, target_ptr
                );
                Some(rendered)
            }
            Rvalue::Binary { left, op, right } => {
                let left = self.render_operand(output, left, span);
                let right = self.render_operand(output, right, span);
                self.render_binary(output, *op, left, right)
            }
            Rvalue::Unary { op, operand } => match op {
                UnaryOp::Not => {
                    let operand = self.render_operand(output, operand, span);
                    let temp = self.fresh_temp();
                    let _ = writeln!(output, "  {temp} = xor i1 {}, true", operand.repr);
                    Some(LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Bool),
                        llvm_ty: "i1".to_owned(),
                        repr: temp,
                    })
                }
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
            Rvalue::RepeatArray { value, len } => {
                self.render_repeat_array_rvalue(output, value, len, expected_ty, span)
            }
            Rvalue::AggregateTupleStruct { path, items } => {
                self.render_tuple_struct_rvalue(output, path, items, expected_ty, span)
            }
            Rvalue::AggregateStruct { path, fields } => {
                self.render_struct_rvalue(output, path, fields, expected_ty, span)
            }
            Rvalue::Closure { closure } => {
                let closure_id = *closure;
                let closure = self.body.closure(closure_id);
                assert!(
                    closure.lowered_body.is_some(),
                    "prepared functions should only contain supported closure values"
                );
                let ty = self
                    .emitter
                    .input
                    .typeck
                    .expr_ty(closure.expr)
                    .cloned()
                    .expect("prepared closure values should preserve callable types");
                Some(LoweredValue {
                    ty,
                    llvm_ty: "ptr".to_owned(),
                    repr: if closure.capture_binding_locals.is_empty() {
                        format!(
                            "@{}",
                            closure_llvm_name(&self.prepared.signature.llvm_name, closure_id)
                        )
                    } else {
                        "null".to_owned()
                    },
                })
            }
            Rvalue::OpaqueExpr(_) => {
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
                len: TyArrayLen::Known(rendered_items.len()),
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

    fn render_repeat_array_rvalue(
        &mut self,
        output: &mut String,
        value: &Operand,
        len: &hir::ArrayLen,
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let repeat_len = match len {
            hir::ArrayLen::Known(len) => *len,
            hir::ArrayLen::Generic(_) => known_array_len(match expected_ty {
                Some(Ty::Array { len, .. }) => len,
                _ => return None,
            })
            .expect("prepared repeat-array values should have concrete expected lengths"),
        };
        let rendered = self.render_operand(output, value, span);
        let element_ty = match expected_ty {
            Some(Ty::Array { element, .. }) => element.as_ref().clone(),
            _ => rendered.ty.clone(),
        };
        assert!(
            backend_value_compatible(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                &element_ty,
                &rendered.ty,
            ),
            "prepared repeat-array at {span:?} should preserve compatible element type"
        );

        let ty = Ty::Array {
            element: Box::new(element_ty),
            len: TyArrayLen::Known(repeat_len),
        };
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&ty, span, "repeat-array value")
            .expect("prepared repeat-array values should already have supported LLVM types");

        if repeat_len == 0 {
            return Some(LoweredValue {
                ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            });
        }

        let mut aggregate = "undef".to_owned();
        for index in 0..repeat_len {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                rendered.llvm_ty, rendered.repr, index
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
        path: &ql_ast::Path,
        fields: &[mir::AggregateField],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let struct_ty = expected_ty.cloned().or_else(|| {
            panic!("prepared struct aggregate at {span:?} should have an expected lowered type")
        })?;
        if matches!(
            &struct_ty,
            Ty::Item { item_id, .. } if matches!(&self.emitter.input.hir.item(*item_id).kind, ItemKind::Enum(_))
        ) {
            return self.render_enum_struct_rvalue(output, path, fields, struct_ty, span);
        }
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

    fn render_tuple_struct_rvalue(
        &mut self,
        output: &mut String,
        path: &ql_ast::Path,
        items: &[Operand],
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> Option<LoweredValue> {
        let enum_ty = expected_ty.cloned().or_else(|| {
            panic!("prepared enum aggregate at {span:?} should have an expected lowered type")
        })?;
        self.render_enum_tuple_rvalue(output, path, items, enum_ty, span)
    }

    fn render_enum_struct_rvalue(
        &mut self,
        output: &mut String,
        path: &ql_ast::Path,
        fields: &[mir::AggregateField],
        enum_ty: Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let enum_lowering = self
            .emitter
            .enum_lowering(&enum_ty, span, "enum value")
            .unwrap_or_else(|_| {
                panic!(
                    "prepared enum aggregate at {span:?} should have a lowered declaration layout"
                )
            });
        let (variant_index, variant) = self
            .emitter
            .enum_variant_lowering_for_path(&enum_lowering, path, &enum_ty, span, "enum value")
            .unwrap_or_else(|_| {
                panic!("prepared enum aggregate at {span:?} should reference a known variant")
            });
        assert!(
            variant.fields.iter().all(|field| field.name.is_some()),
            "prepared enum aggregate at {span:?} should only use struct-style variants"
        );

        let slot = self.fresh_temp();
        let _ = writeln!(output, "  {slot} = alloca {}", enum_lowering.llvm_ty);
        let _ = writeln!(
            output,
            "  store {} zeroinitializer, ptr {slot}",
            enum_lowering.llvm_ty
        );

        let tag_ptr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {tag_ptr} = getelementptr inbounds {}, ptr {slot}, i32 0, i32 0",
            enum_lowering.llvm_ty
        );
        let _ = writeln!(output, "  store i64 {variant_index}, ptr {tag_ptr}");

        let mut rendered_fields = HashMap::with_capacity(fields.len());
        for field in fields {
            let rendered = self.render_operand(output, &field.value, span);
            rendered_fields.insert(field.name.clone(), rendered);
        }

        if !variant.fields.is_empty() {
            let payload_storage_ptr = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {payload_storage_ptr} = getelementptr inbounds {}, ptr {slot}, i32 0, i32 1",
                enum_lowering.llvm_ty
            );
            let payload_ptr = payload_storage_ptr;

            for (index, field) in variant.fields.iter().enumerate() {
                let rendered = rendered_fields
                    .remove(field.name.as_ref().expect("checked named enum field"))
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared enum aggregate at {span:?} should provide every declared field"
                        )
                    });
                let field_ptr = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {field_ptr} = getelementptr inbounds {}, ptr {payload_ptr}, i32 0, i32 {index}",
                    variant
                        .payload_llvm_ty
                        .as_ref()
                        .expect("checked enum payload type")
                );
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {field_ptr}",
                    rendered.llvm_ty, rendered.repr
                );
            }
        }

        Some(self.render_loaded_pointer_value(output, slot, enum_ty, span))
    }

    fn render_enum_tuple_rvalue(
        &mut self,
        output: &mut String,
        path: &ql_ast::Path,
        items: &[Operand],
        enum_ty: Ty,
        span: Span,
    ) -> Option<LoweredValue> {
        let enum_lowering = self
            .emitter
            .enum_lowering(&enum_ty, span, "enum value")
            .unwrap_or_else(|_| {
                panic!(
                    "prepared enum aggregate at {span:?} should have a lowered declaration layout"
                )
            });
        let (variant_index, variant) = self
            .emitter
            .enum_variant_lowering_for_path(&enum_lowering, path, &enum_ty, span, "enum value")
            .unwrap_or_else(|_| {
                panic!("prepared enum aggregate at {span:?} should reference a known variant")
            });
        assert!(
            variant.fields.iter().all(|field| field.name.is_none()),
            "prepared enum aggregate at {span:?} should only use unit/tuple-style variants"
        );
        assert_eq!(
            items.len(),
            variant.fields.len(),
            "prepared enum aggregate at {span:?} should provide every declared tuple field"
        );

        let slot = self.fresh_temp();
        let _ = writeln!(output, "  {slot} = alloca {}", enum_lowering.llvm_ty);
        let _ = writeln!(
            output,
            "  store {} zeroinitializer, ptr {slot}",
            enum_lowering.llvm_ty
        );

        let tag_ptr = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {tag_ptr} = getelementptr inbounds {}, ptr {slot}, i32 0, i32 0",
            enum_lowering.llvm_ty
        );
        let _ = writeln!(output, "  store i64 {variant_index}, ptr {tag_ptr}");

        if !items.is_empty() {
            let payload_storage_ptr = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {payload_storage_ptr} = getelementptr inbounds {}, ptr {slot}, i32 0, i32 1",
                enum_lowering.llvm_ty
            );
            let payload_ptr = payload_storage_ptr;

            for (index, item) in items.iter().enumerate() {
                let rendered = self.render_operand(output, item, span);
                let field_ptr = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {field_ptr} = getelementptr inbounds {}, ptr {payload_ptr}, i32 0, i32 {index}",
                    variant
                        .payload_llvm_ty
                        .as_ref()
                        .expect("checked enum payload type")
                );
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {field_ptr}",
                    rendered.llvm_ty, rendered.repr
                );
            }
        }

        Some(self.render_loaded_pointer_value(output, slot, enum_ty, span))
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

    fn task_handle_info_for_ty(&self, ty: &Ty, span: Span) -> AsyncTaskHandleInfo {
        match ty {
            Ty::TaskHandle(result_ty) => AsyncTaskHandleInfo {
                result_ty: result_ty.as_ref().clone(),
                result_layout: self
                    .emitter
                    .build_async_task_result_layout(result_ty, span)
                    .unwrap_or_else(|_| {
                        panic!(
                            "prepared task-handle value at {span:?} should have a loadable async result layout"
                        )
                    }),
            },
            other => panic!(
                "prepared task-handle value at {span:?} should resolve to a task handle, found `{other}`"
            ),
        }
    }

    fn render_await_handle(
        &mut self,
        output: &mut String,
        handle: LoweredValue,
        handle_info: AsyncTaskHandleInfo,
        _span: Span,
    ) -> Option<LoweredValue> {
        let await_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::TaskAwait)
            .expect("prepared await lowering should require the task-await runtime hook");
        let release_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::TaskResultRelease)
            .expect("prepared await lowering should require the task-result-release runtime hook");
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

    fn render_spawn_handle(
        &mut self,
        output: &mut String,
        task: LoweredValue,
        result_ty: Ty,
    ) -> Option<LoweredValue> {
        let spawn_hook = self
            .emitter
            .runtime_hook_signature(RuntimeHook::ExecutorSpawn)
            .expect("prepared spawn lowering should require the executor-spawn runtime hook");
        let submitted = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {submitted} = call {} @{}(ptr null, {} {})",
            spawn_hook.return_type.llvm_ir(),
            spawn_hook.hook.symbol_name(),
            task.llvm_ty,
            task.repr
        );
        Some(LoweredValue {
            ty: Ty::TaskHandle(Box::new(result_ty)),
            llvm_ty: "ptr".to_owned(),
            repr: submitted,
        })
    }

    fn render_await(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> Option<LoweredValue> {
        let Operand::Place(place) = operand else {
            panic!("prepared await operands should lower through task-handle places");
        };
        let handle_info = self.task_handle_info_for_place(place, span);
        let handle = self.render_operand(output, operand, span);
        self.render_await_handle(output, handle, handle_info, span)
    }

    fn render_spawn(
        &mut self,
        output: &mut String,
        operand: &Operand,
        span: Span,
    ) -> Option<LoweredValue> {
        let Operand::Place(place) = operand else {
            panic!("prepared spawn operands should lower through task-handle places");
        };
        let handle_info = self.task_handle_info_for_place(place, span);
        let task = self.render_operand(output, operand, span);
        self.render_spawn_handle(output, task, handle_info.result_ty)
    }

    fn render_binary(
        &mut self,
        output: &mut String,
        op: BinaryOp,
        mut left: LoweredValue,
        mut right: LoweredValue,
    ) -> Option<LoweredValue> {
        left.ty = transparent_backend_value_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &left.ty,
        );
        right.ty = transparent_backend_value_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &right.ty,
        );
        match op {
            BinaryOp::OrOr | BinaryOp::AndAnd => {
                panic!("short-circuit boolean operators should lower structurally in MIR")
            }
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                let temp = self.fresh_temp();
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
                if matches!(left.ty, Ty::Builtin(BuiltinType::String)) {
                    return self.render_string_binary_comparison(output, op, left, right);
                }

                let temp = self.fresh_temp();
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

    fn render_string_binary_comparison(
        &mut self,
        output: &mut String,
        op: BinaryOp,
        left: LoweredValue,
        right: LoweredValue,
    ) -> Option<LoweredValue> {
        debug_assert!(matches!(left.ty, Ty::Builtin(BuiltinType::String)));
        debug_assert!(matches!(
            op,
            BinaryOp::EqEq
                | BinaryOp::BangEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::Lt
                | BinaryOp::LtEq
        ));

        let left_ptr = self.fresh_temp();
        let left_len = self.fresh_temp();
        let right_ptr = self.fresh_temp();
        let right_len = self.fresh_temp();

        let _ = writeln!(
            output,
            "  {left_ptr} = extractvalue {} {}, 0",
            left.llvm_ty, left.repr
        );
        let _ = writeln!(
            output,
            "  {left_len} = extractvalue {} {}, 1",
            left.llvm_ty, left.repr
        );
        let _ = writeln!(
            output,
            "  {right_ptr} = extractvalue {} {}, 0",
            right.llvm_ty, right.repr
        );
        let _ = writeln!(
            output,
            "  {right_len} = extractvalue {} {}, 1",
            right.llvm_ty, right.repr
        );

        let temp = match op {
            BinaryOp::EqEq | BinaryOp::BangEq => {
                let len_eq = self.fresh_temp();
                let memcmp_label = self.fresh_label("string_cmp_memcmp");
                let mismatch_label = self.fresh_label("string_cmp_mismatch");
                let end_label = self.fresh_label("string_cmp_end");
                let memcmp_result = self.fresh_temp();
                let bytes_compare = self.fresh_temp();
                let temp = self.fresh_temp();
                let mismatch_value = match op {
                    BinaryOp::EqEq => "false",
                    BinaryOp::BangEq => "true",
                    _ => unreachable!("validated string comparisons should only use == or !="),
                };
                let memcmp_opcode = match op {
                    BinaryOp::EqEq => "icmp eq",
                    BinaryOp::BangEq => "icmp ne",
                    _ => unreachable!("validated string comparisons should only use == or !="),
                };

                let _ = writeln!(output, "  {len_eq} = icmp eq i64 {left_len}, {right_len}");
                let _ = writeln!(
                    output,
                    "  br i1 {len_eq}, label %{memcmp_label}, label %{mismatch_label}"
                );
                let _ = writeln!(output, "{memcmp_label}:");
                let _ = writeln!(
                    output,
                    "  {memcmp_result} = call i32 @memcmp(ptr {left_ptr}, ptr {right_ptr}, i64 {left_len})"
                );
                let _ = writeln!(
                    output,
                    "  {bytes_compare} = {memcmp_opcode} i32 {memcmp_result}, 0"
                );
                let _ = writeln!(output, "  br label %{end_label}");
                let _ = writeln!(output, "{mismatch_label}:");
                let _ = writeln!(output, "  br label %{end_label}");
                let _ = writeln!(output, "{end_label}:");
                let _ = writeln!(
                    output,
                    "  {temp} = phi i1 [ {mismatch_value}, %{mismatch_label} ], [ {bytes_compare}, %{memcmp_label} ]"
                );

                temp
            }
            BinaryOp::Gt | BinaryOp::GtEq | BinaryOp::Lt | BinaryOp::LtEq => {
                let left_shorter = self.fresh_temp();
                let min_len = self.fresh_temp();
                let memcmp_result = self.fresh_temp();
                let memcmp_eq = self.fresh_temp();
                let bytes_label = self.fresh_label("string_ord_bytes");
                let len_label = self.fresh_label("string_ord_len");
                let end_label = self.fresh_label("string_ord_end");
                let bytes_compare = self.fresh_temp();
                let len_compare = self.fresh_temp();
                let temp = self.fresh_temp();
                let bytes_opcode = match op {
                    BinaryOp::Gt | BinaryOp::GtEq => "icmp sgt",
                    BinaryOp::Lt | BinaryOp::LtEq => "icmp slt",
                    _ => unreachable!(
                        "validated string comparisons should only use ordering operators"
                    ),
                };
                let len_opcode = match op {
                    BinaryOp::Gt => "icmp ugt",
                    BinaryOp::GtEq => "icmp uge",
                    BinaryOp::Lt => "icmp ult",
                    BinaryOp::LtEq => "icmp ule",
                    _ => unreachable!(
                        "validated string comparisons should only use ordering operators"
                    ),
                };

                let _ = writeln!(
                    output,
                    "  {left_shorter} = icmp ult i64 {left_len}, {right_len}"
                );
                let _ = writeln!(
                    output,
                    "  {min_len} = select i1 {left_shorter}, i64 {left_len}, i64 {right_len}"
                );
                let _ = writeln!(
                    output,
                    "  {memcmp_result} = call i32 @memcmp(ptr {left_ptr}, ptr {right_ptr}, i64 {min_len})"
                );
                let _ = writeln!(output, "  {memcmp_eq} = icmp eq i32 {memcmp_result}, 0");
                let _ = writeln!(
                    output,
                    "  br i1 {memcmp_eq}, label %{len_label}, label %{bytes_label}"
                );
                let _ = writeln!(output, "{bytes_label}:");
                let _ = writeln!(
                    output,
                    "  {bytes_compare} = {bytes_opcode} i32 {memcmp_result}, 0"
                );
                let _ = writeln!(output, "  br label %{end_label}");
                let _ = writeln!(output, "{len_label}:");
                let _ = writeln!(
                    output,
                    "  {len_compare} = {len_opcode} i64 {left_len}, {right_len}"
                );
                let _ = writeln!(output, "  br label %{end_label}");
                let _ = writeln!(output, "{end_label}:");
                let _ = writeln!(
                    output,
                    "  {temp} = phi i1 [ {bytes_compare}, %{bytes_label} ], [ {len_compare}, %{len_label} ]"
                );

                temp
            }
            _ => unreachable!("validated string comparisons should only use comparison operators"),
        };

        Some(LoweredValue {
            ty: Ty::Builtin(BuiltinType::Bool),
            llvm_ty: "i1".to_owned(),
            repr: temp,
        })
    }

    fn render_string_match_literal_condition(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        value: &str,
        span: Span,
    ) -> LoweredValue {
        let literal = self.render_string_literal_value(output, value, false, span);
        self.render_string_binary_comparison(output, BinaryOp::EqEq, scrutinee, literal)
            .expect("string match literal comparison should lower to Bool")
    }

    fn render_item_constant(
        &mut self,
        output: &mut String,
        item_id: ItemId,
        span: Span,
    ) -> LoweredValue {
        let (ItemKind::Const(global) | ItemKind::Static(global)) =
            &self.emitter.input.hir.item(item_id).kind
        else {
            panic!(
                "prepared const item lowering at {span:?} should only materialize const/static items"
            );
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
            hir::ExprKind::String { value, is_format } => {
                self.render_string_literal_value(output, value, *is_format, span)
            }
            hir::ExprKind::Bool(value) => LoweredValue {
                ty: Ty::Builtin(BuiltinType::Bool),
                llvm_ty: "i1".to_owned(),
                repr: if *value { "true" } else { "false" }.to_owned(),
            },
            hir::ExprKind::Unary { op, expr } => match op {
                UnaryOp::Not => {
                    let operand = self.render_const_expr(output, *expr, expected_ty, span);
                    let temp = self.fresh_temp();
                    let _ = writeln!(output, "  {temp} = xor i1 {}, true", operand.repr);
                    LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Bool),
                        llvm_ty: "i1".to_owned(),
                        repr: temp,
                    }
                }
                UnaryOp::Neg => {
                    let operand = self.render_const_expr(output, *expr, expected_ty, span);
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
                    LoweredValue {
                        ty: operand.ty,
                        llvm_ty: operand.llvm_ty,
                        repr: temp,
                    }
                }
                UnaryOp::Await | UnaryOp::Spawn => panic!(
                    "prepared const item lowering at {span:?} should not contain async unary expressions"
                ),
            },
            hir::ExprKind::Binary { left, op, right } => {
                let left = self.render_const_expr(output, *left, None, span);
                let right = self.render_const_expr(output, *right, None, span);
                self.render_binary(output, *op, left, right).unwrap_or_else(|| {
                    panic!(
                        "prepared const item lowering at {span:?} should only use supported binary const expressions"
                    )
                })
            }
            hir::ExprKind::Member { .. }
            | hir::ExprKind::Bracket { .. }
            | hir::ExprKind::If { .. }
            | hir::ExprKind::Match { .. } => {
                let mut visited = HashSet::new();
                let source = guard_literal_source_expr(
                    self.emitter.input.hir,
                    self.emitter.input.resolution,
                    expr_id,
                    &mut visited,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "prepared const item lowering at {span:?} should only use foldable projected or branch-selected const expressions"
                    )
                });
                if source == expr_id {
                    panic!(
                        "prepared const item lowering at {span:?} should resolve projected or branch-selected const expressions to their folded source"
                    );
                }
                self.render_const_expr(output, source, expected_ty, span)
            }
            hir::ExprKind::Name(_) => {
                match self.emitter.input.resolution.expr_resolution(expr_id) {
                    Some(ValueResolution::Function(function)) => {
                        self.render_function_constant(*function, span)
                    }
                    Some(ValueResolution::Item(item_id)) => {
                        match &self.emitter.input.hir.item(*item_id).kind {
                            ItemKind::Function(_) => {
                                self.render_function_constant(FunctionRef::Item(*item_id), span)
                            }
                            ItemKind::Const(_) | ItemKind::Static(_) => {
                                self.render_item_constant(output, *item_id, span)
                            }
                            _ => panic!(
                                "prepared const item lowering at {span:?} should only reference resolved local function/const/static items"
                            ),
                        }
                    }
                    Some(ValueResolution::Import(import_binding)) => {
                        let item_id =
                            local_item_for_import_binding(self.emitter.input.hir, import_binding)
                                .unwrap_or_else(|| {
                                    panic!(
                                        "prepared const item lowering at {span:?} should only reference resolved local function/const/static imports"
                                    )
                                });
                        match &self.emitter.input.hir.item(item_id).kind {
                            ItemKind::Function(_) => {
                                self.render_function_constant(FunctionRef::Item(item_id), span)
                            }
                            ItemKind::Const(_) | ItemKind::Static(_) => {
                                self.render_item_constant(output, item_id, span)
                            }
                            _ => panic!(
                                "prepared const item lowering at {span:?} should only reference resolved local function/const/static imports"
                            ),
                        }
                    }
                    _ => panic!(
                        "prepared const item lowering at {span:?} should only reference resolved local function/const/static items"
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
            hir::ExprKind::Call { callee, args } => {
                let rendered = self.render_cleanup_call(output, *callee, args, span).unwrap_or_else(
                    || {
                        panic!(
                            "prepared const item lowering at {span:?} should only use non-void call expressions"
                        )
                    },
                );
                if let Some(expected_ty) = expected_ty {
                    assert!(
                        expected_ty.compatible_with(&rendered.ty),
                        "prepared const item lowering at {span:?} should only render compatible call expressions"
                    );
                }
                rendered
            }
            hir::ExprKind::Closure { .. } => {
                let ty = self
                    .emitter
                    .input
                    .typeck
                    .expr_ty(expr_id)
                    .cloned()
                    .or_else(|| expected_ty.cloned())
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared const item lowering at {span:?} should preserve callable types for closure-backed const/static values"
                        )
                    });
                let llvm_name = self
                    .emitter
                    .const_closure_llvm_names
                    .get(&expr_id)
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared const item lowering at {span:?} should prepare closure-backed const/static values before rendering"
                        )
                    });
                LoweredValue {
                    ty,
                    llvm_ty: "ptr".to_owned(),
                    repr: format!("@{llvm_name}"),
                }
            }
            hir::ExprKind::Tuple(items) => {
                self.render_const_tuple_expr(output, items, expected_ty, expr.span)
            }
            hir::ExprKind::Array(items) => {
                self.render_const_array_expr(output, items, expected_ty, expr.span)
            }
            hir::ExprKind::RepeatArray { value, len, .. } => {
                self.render_const_repeat_array_expr(output, *value, len, expected_ty, expr.span)
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
                let known_len = known_array_len(len).expect(
                    "prepared const array lowering should only render concrete fixed-array literals",
                );
                assert_eq!(
                    known_len,
                    items.len(),
                    "prepared const array lowering at {span:?} should preserve array length"
                );
                Ty::Array {
                    element: element.clone(),
                    len: len.clone(),
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

    fn render_const_repeat_array_expr(
        &mut self,
        output: &mut String,
        value: hir::ExprId,
        len: &hir::ArrayLen,
        expected_ty: Option<&Ty>,
        span: Span,
    ) -> LoweredValue {
        let array_ty = match expected_ty {
            Some(Ty::Array { element, len }) => Ty::Array {
                element: element.clone(),
                len: len.clone(),
            },
            Some(other) => panic!(
                "prepared const repeat-array lowering at {span:?} should have array expected type, found `{other}`"
            ),
            None => {
                panic!(
                    "prepared const repeat-array lowering at {span:?} should have an expected type"
                )
            }
        };
        let Ty::Array {
            element,
            len: expected_len,
        } = &array_ty
        else {
            unreachable!();
        };
        let repeat_len = match len {
            hir::ArrayLen::Known(len) => *len,
            hir::ArrayLen::Generic(_) => known_array_len(expected_len)
                .expect("prepared const repeat-array lowering should use concrete lengths"),
        };
        assert!(
            known_array_len(expected_len).is_some_and(|expected| expected == repeat_len),
            "prepared const repeat-array lowering at {span:?} should preserve array length"
        );
        let rendered = self.render_const_expr(output, value, Some(element.as_ref()), span);
        let llvm_ty = self
            .emitter
            .lower_llvm_type(&array_ty, span, "repeat-array value")
            .expect("prepared const repeat-array values should already have supported LLVM types");

        if repeat_len == 0 {
            return LoweredValue {
                ty: array_ty,
                llvm_ty,
                repr: "zeroinitializer".to_owned(),
            };
        }

        let mut aggregate = "undef".to_owned();
        for index in 0..repeat_len {
            let next = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {next} = insertvalue {llvm_ty} {aggregate}, {} {}, {}",
                rendered.llvm_ty, rendered.repr, index
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
                Constant::String { value, is_format } => {
                    self.render_string_literal_value(output, value, *is_format, span)
                }
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
                Constant::Function { function, .. } => {
                    self.render_function_constant(*function, span)
                }
                Constant::Item { item, .. } => {
                    if let Some((value_expr, ty)) = runtime_task_backed_item_value(
                        self.emitter.input.hir,
                        self.emitter.input.resolution,
                        *item,
                    ) {
                        self.render_cleanup_value_expr(output, value_expr, &ty, span)
                    } else {
                        self.render_item_constant(output, *item, span)
                    }
                }
                Constant::None | Constant::UnresolvedName(_) => {
                    panic!("prepared operands should not contain unsupported constants")
                }
                Constant::Import(path) => {
                    let item_id = local_item_for_import_path(self.emitter.input.hir, path)
                        .unwrap_or_else(|| {
                            panic!(
                                "prepared operands should only contain local function/const/static imports"
                            )
                        });
                    match &self.emitter.input.hir.item(item_id).kind {
                        ItemKind::Function(_) => {
                            self.render_function_constant(FunctionRef::Item(item_id), span)
                        }
                        ItemKind::Const(_) | ItemKind::Static(_) => {
                            if let Some((value_expr, ty)) = runtime_task_backed_item_value(
                                self.emitter.input.hir,
                                self.emitter.input.resolution,
                                item_id,
                            ) {
                                self.render_cleanup_value_expr(output, value_expr, &ty, span)
                            } else {
                                self.render_item_constant(output, item_id, span)
                            }
                        }
                        _ => panic!(
                            "prepared operands should only contain local function/const/static imports"
                        ),
                    }
                }
            },
        }
    }

    fn render_string_literal_value(
        &mut self,
        output: &mut String,
        value: &str,
        is_format: bool,
        span: Span,
    ) -> LoweredValue {
        let key = StringLiteralKey {
            value: value.to_owned(),
            is_format,
        };
        let llvm_name = self
            .emitter
            .string_literal_llvm_names
            .get(&key)
            .unwrap_or_else(|| {
                panic!(
                    "prepared string literal lowering at {span:?} should predeclare the module string global"
                )
            });
        let bytes_len = value.as_bytes().len();
        let llvm_ty = llvm_string_aggregate_ty().to_owned();
        let ptr = self.fresh_temp();
        let with_ptr = self.fresh_temp();
        let with_len = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {ptr} = getelementptr inbounds [{} x i8], ptr @{llvm_name}, i32 0, i32 0",
            bytes_len + 1
        );
        let _ = writeln!(
            output,
            "  {with_ptr} = insertvalue {llvm_ty} undef, ptr {ptr}, 0"
        );
        let _ = writeln!(
            output,
            "  {with_len} = insertvalue {llvm_ty} {with_ptr}, i64 {bytes_len}, 1"
        );
        LoweredValue {
            ty: Ty::Builtin(BuiltinType::String),
            llvm_ty,
            repr: with_len,
        }
    }

    fn render_function_constant(&mut self, function: FunctionRef, span: Span) -> LoweredValue {
        let signature = self.emitter.signatures.get(&function).unwrap_or_else(|| {
            panic!(
                "prepared function-value lowering at {span:?} should resolve the function signature"
            )
        });
        LoweredValue {
            ty: callable_ty_from_signature(signature),
            llvm_ty: "ptr".to_owned(),
            repr: format!("@{}", signature.llvm_name),
        }
    }

    fn render_guard_await_expr(
        &mut self,
        output: &mut String,
        task_expr: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let task_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(task_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should type-check awaited guard task values"
                )
            });
        let handle_info = self.task_handle_info_for_ty(&task_ty, span);
        let rendered =
            self.render_guard_expr_as_type(output, task_expr, &task_ty, span, guard_binding);
        self.render_await_handle(output, rendered, handle_info, span)
            .expect("prepared bool guard await lowering should produce a value")
    }

    fn render_bool_guard_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if let Some(source_expr) = self.emitter.literal_source_expr(expr_id) {
            return self.render_bool_guard_expr(output, source_expr, span, guard_binding);
        }

        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                let rendered =
                    self.render_guard_assignment_expr(output, *left, *right, span, guard_binding);
                assert!(
                    rendered.ty.is_bool(),
                    "prepared bool guard lowering at {span:?} should only render bool-valued guard assignments"
                );
                rendered
            }
            hir::ExprKind::Binary { left, op, right } => match op {
                BinaryOp::AndAnd | BinaryOp::OrOr => {
                    let left = self.render_bool_guard_expr(output, *left, span, guard_binding);
                    let right = self.render_bool_guard_expr(output, *right, span, guard_binding);
                    assert!(
                        left.ty.is_bool() && right.ty.is_bool(),
                        "prepared bool guard lowering at {span:?} should only combine bool guard expressions with &&/||"
                    );
                    let temp = self.fresh_temp();
                    let opcode = match op {
                        BinaryOp::AndAnd => "and",
                        BinaryOp::OrOr => "or",
                        _ => unreachable!("logical guard rendering should only handle &&/||"),
                    };
                    let _ = writeln!(
                        output,
                        "  {temp} = {opcode} i1 {}, {}",
                        left.repr, right.repr
                    );
                    LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Bool),
                        llvm_ty: "i1".to_owned(),
                        repr: temp,
                    }
                }
                _ => {
                    let left = self.render_guard_scalar_expr(output, *left, span, guard_binding);
                    let right = self.render_guard_scalar_expr(output, *right, span, guard_binding);
                    assert_eq!(
                        left.ty, right.ty,
                        "prepared bool guard lowering at {span:?} should only compare same-typed supported guard operands"
                    );
                    let compare = self.fresh_temp();
                    let opcode = match &left.ty {
                        Ty::Builtin(BuiltinType::Bool) => match op {
                            BinaryOp::EqEq | BinaryOp::BangEq => compare_opcode(*op, &left.ty),
                            _ => panic!(
                                "prepared bool guard lowering at {span:?} should only render supported bool guard comparisons"
                            ),
                        },
                        Ty::Builtin(BuiltinType::Int) => match op {
                            BinaryOp::EqEq
                            | BinaryOp::BangEq
                            | BinaryOp::Gt
                            | BinaryOp::GtEq
                            | BinaryOp::Lt
                            | BinaryOp::LtEq => compare_opcode(*op, &left.ty),
                            _ => panic!(
                                "prepared bool guard lowering at {span:?} should only render supported integer guard comparisons"
                            ),
                        },
                        _ => panic!(
                            "prepared bool guard lowering at {span:?} should only render supported scalar guard comparisons"
                        ),
                    };
                    let _ = writeln!(
                        output,
                        "  {compare} = {opcode} {} {}, {}",
                        left.llvm_ty, left.repr, right.repr
                    );
                    LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Bool),
                        llvm_ty: "i1".to_owned(),
                        repr: compare,
                    }
                }
            },
            _ => {
                let rendered = self.render_guard_scalar_expr(output, expr_id, span, guard_binding);
                assert!(
                    rendered.ty.is_bool(),
                    "prepared bool guard lowering at {span:?} should only render bool-valued guard expressions"
                );
                rendered
            }
        }
    }

    fn render_guard_assignment_expr(
        &mut self,
        output: &mut String,
        target_expr: hir::ExprId,
        value_expr: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let (place, target_ty) = guard_expr_place_with_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.local_types,
            None,
            target_expr,
        )
        .unwrap_or_else(|| {
            panic!(
                "prepared bool guard lowering at {span:?} should only assign through supported guard places"
            )
        });
        let rendered =
            self.render_guard_expr_as_type(output, value_expr, &target_ty, span, guard_binding);
        assert!(
            target_ty.compatible_with(&rendered.ty),
            "prepared bool guard lowering at {span:?} should only store compatible guard assignment values"
        );
        let target_ptr = self.render_place_pointer(output, &place, span).0;
        let _ = writeln!(
            output,
            "  store {} {}, ptr {}",
            rendered.llvm_ty, rendered.repr, target_ptr
        );
        rendered
    }

    fn render_guard_scalar_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if let Some(source_expr) = self.emitter.literal_source_expr(expr_id) {
            return self.render_guard_scalar_expr(output, source_expr, span, guard_binding);
        }

        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                return self.render_guard_assignment_expr(
                    output,
                    *left,
                    *right,
                    span,
                    guard_binding,
                );
            }
            hir::ExprKind::Binary { left, op, right }
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) =>
            {
                let left = self.render_guard_scalar_expr(output, *left, span, guard_binding);
                let right = self.render_guard_scalar_expr(output, *right, span, guard_binding);
                assert!(
                    left.ty.compatible_with(&Ty::Builtin(BuiltinType::Int))
                        && right.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                    "prepared bool guard lowering at {span:?} should only render Int arithmetic guard expressions"
                );
                self.render_binary(output, *op, left, right).unwrap_or_else(|| {
                    panic!(
                        "prepared bool guard lowering at {span:?} should only render supported arithmetic guard expressions"
                    )
                })
            }
            hir::ExprKind::Binary {
                op:
                    BinaryOp::AndAnd
                    | BinaryOp::OrOr
                    | BinaryOp::EqEq
                    | BinaryOp::BangEq
                    | BinaryOp::Gt
                    | BinaryOp::GtEq
                    | BinaryOp::Lt
                    | BinaryOp::LtEq,
                ..
            } => {
                let rendered = self.render_bool_guard_expr(output, expr_id, span, guard_binding);
                assert!(
                    rendered.ty.is_bool(),
                    "prepared bool guard lowering at {span:?} should only render bool-valued binary guard expressions"
                );
                rendered
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Not,
                expr,
            } => {
                let inner = self.render_bool_guard_expr(output, *expr, span, guard_binding);
                assert!(
                    inner.ty.is_bool(),
                    "prepared bool guard lowering at {span:?} should only negate bool guard expressions"
                );
                let temp = self.fresh_temp();
                let _ = writeln!(output, "  {temp} = xor i1 {}, true", inner.repr);
                LoweredValue {
                    ty: Ty::Builtin(BuiltinType::Bool),
                    llvm_ty: "i1".to_owned(),
                    repr: temp,
                }
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Neg,
                expr,
            } => {
                let inner = self.render_guard_scalar_expr(output, *expr, span, guard_binding);
                assert!(
                    inner.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                    "prepared bool guard lowering at {span:?} should only negate Int scalar guard expressions"
                );
                let temp = self.fresh_temp();
                let _ = writeln!(output, "  {temp} = sub i64 0, {}", inner.repr);
                LoweredValue {
                    ty: Ty::Builtin(BuiltinType::Int),
                    llvm_ty: "i64".to_owned(),
                    repr: temp,
                }
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let rendered = self.render_guard_await_expr(output, *expr, span, guard_binding);
                assert!(
                    rendered.ty.is_bool()
                        || rendered.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                    "prepared bool guard lowering at {span:?} should only render scalar awaited guard expressions"
                );
                rendered
            }
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
            hir::ExprKind::Call { callee, args } => {
                let rendered =
                    self.render_guard_call_value(output, *callee, args, span, guard_binding);
                assert!(
                    rendered.ty.is_bool()
                        || rendered.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                    "prepared bool guard lowering at {span:?} should only render scalar guard-call results"
                );
                rendered
            }
            hir::ExprKind::Name(_) => {
                match self.emitter.input.resolution.expr_resolution(expr_id) {
                    Some(ValueResolution::Local(local))
                        if guard_binding.is_some_and(|binding| binding.local == *local) =>
                    {
                        guard_binding.expect("checked above").value.clone()
                    }
                    Some(ValueResolution::Local(local))
                        if self.cleanup_binding(*local).is_some() =>
                    {
                        self.cleanup_binding(*local).expect("checked above").clone()
                    }
                    Some(ValueResolution::Item(_)) | Some(ValueResolution::Import(_)) => {
                        if let Some(value) = guard_literal_bool(
                            self.emitter.input.hir,
                            self.emitter.input.resolution,
                            expr_id,
                        ) {
                            LoweredValue {
                                ty: Ty::Builtin(BuiltinType::Bool),
                                llvm_ty: "i1".to_owned(),
                                repr: if value { "true" } else { "false" }.to_owned(),
                            }
                        } else if let Some(value) = guard_literal_int(
                            self.emitter.input.hir,
                            self.emitter.input.resolution,
                            expr_id,
                        ) {
                            LoweredValue {
                                ty: Ty::Builtin(BuiltinType::Int),
                                llvm_ty: "i64".to_owned(),
                                repr: value.to_string(),
                            }
                        } else {
                            panic!(
                                "prepared bool guard lowering at {span:?} should only render supported item-backed guards"
                            )
                        }
                    }
                    Some(ValueResolution::Local(_))
                    | Some(ValueResolution::Param(_))
                    | Some(ValueResolution::ArrayLengthGeneric(_))
                    | Some(ValueResolution::SelfValue)
                    | Some(ValueResolution::Function(_))
                    | None => {
                        let rendered = if let Some((place, ty)) = guard_expr_place_with_ty(
                            self.emitter.input.hir,
                            self.emitter.input.resolution,
                            self.body,
                            &self.prepared.local_types,
                            None,
                            expr_id,
                        ) {
                            let rendered =
                                self.render_operand(output, &Operand::Place(place), span);
                            assert!(
                                ty.is_bool() || ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                                "prepared bool guard lowering at {span:?} should only render bool or Int guard places"
                            );
                            rendered
                        } else {
                            let (ptr, ty) = self
                                .render_guard_projection_pointer(
                                    output,
                                    expr_id,
                                    span,
                                    guard_binding,
                                )
                                .unwrap_or_else(|| {
                                    panic!(
                                        "prepared bool guard lowering at {span:?} should only render resolved scalar guard places"
                                    )
                                });
                            assert!(
                                ty.is_bool() || ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                                "prepared bool guard lowering at {span:?} should only render bool or Int guard places"
                            );
                            self.render_loaded_pointer_value(output, ptr, ty, span)
                        };
                        assert!(
                            rendered.ty.is_bool()
                                || rendered.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                            "prepared bool guard lowering at {span:?} should only render bool or Int guard places"
                        );
                        rendered
                    }
                }
            }
            hir::ExprKind::Member { .. } | hir::ExprKind::Bracket { .. } => {
                if let Some(value) = guard_literal_bool(
                    self.emitter.input.hir,
                    self.emitter.input.resolution,
                    expr_id,
                ) {
                    LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Bool),
                        llvm_ty: "i1".to_owned(),
                        repr: if value { "true" } else { "false" }.to_owned(),
                    }
                } else if let Some(value) = guard_literal_int(
                    self.emitter.input.hir,
                    self.emitter.input.resolution,
                    expr_id,
                ) {
                    LoweredValue {
                        ty: Ty::Builtin(BuiltinType::Int),
                        llvm_ty: "i64".to_owned(),
                        repr: value.to_string(),
                    }
                } else {
                    let rendered = if let Some((place, ty)) = guard_expr_place_with_ty(
                        self.emitter.input.hir,
                        self.emitter.input.resolution,
                        self.body,
                        &self.prepared.local_types,
                        None,
                        expr_id,
                    ) {
                        let rendered = self.render_operand(output, &Operand::Place(place), span);
                        assert!(
                            ty.is_bool() || ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                            "prepared bool guard lowering at {span:?} should only render bool or Int projection guards"
                        );
                        rendered
                    } else {
                        let (ptr, ty) = self
                            .render_guard_projection_pointer(
                                output,
                                expr_id,
                                span,
                                guard_binding,
                            )
                            .unwrap_or_else(|| {
                                panic!(
                                    "prepared bool guard lowering at {span:?} should only render supported scalar projection guards"
                                )
                            });
                        assert!(
                            ty.is_bool() || ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                            "prepared bool guard lowering at {span:?} should only render bool or Int projection guards"
                        );
                        self.render_loaded_pointer_value(output, ptr, ty, span)
                    };
                    assert!(
                        rendered.ty.is_bool()
                            || rendered.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                        "prepared bool guard lowering at {span:?} should only render bool or Int projection guards"
                    );
                    rendered
                }
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                let tail = self
                    .render_cleanup_block_prefix(output, *block_id, span)
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should only render block guard operands with tails"
                        )
                    });
                self.render_guard_scalar_expr(output, tail, span, guard_binding)
            }
            hir::ExprKind::Question(inner) => {
                self.render_guard_scalar_expr(output, *inner, span, guard_binding)
            }
            _ => panic!(
                "prepared bool guard lowering at {span:?} should only render supported scalar guard expressions"
            ),
        }
    }

    fn render_loaded_pointer_value(
        &mut self,
        output: &mut String,
        ptr: String,
        ty: Ty,
        span: Span,
    ) -> LoweredValue {
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

    fn render_guard_loadable_expr(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if let Some(source_expr) = self.emitter.literal_source_expr(expr_id) {
            return self.render_guard_loadable_expr(output, source_expr, span, guard_binding);
        }

        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Binary {
                left,
                op: BinaryOp::Assign,
                right,
            } => {
                return self.render_guard_assignment_expr(
                    output,
                    *left,
                    *right,
                    span,
                    guard_binding,
                );
            }
            hir::ExprKind::Tuple(items) => {
                let tuple_ty = self
                    .emitter
                    .input
                    .typeck
                    .expr_ty(expr_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should have a resolved tuple literal type"
                        )
                    });
                let Ty::Tuple(expected_items) = &tuple_ty else {
                    panic!(
                        "prepared bool guard lowering at {span:?} should preserve tuple literal types"
                    );
                };
                assert_eq!(
                    expected_items.len(),
                    items.len(),
                    "prepared bool guard lowering at {span:?} should preserve tuple literal arity"
                );
                let rendered_items = items
                    .iter()
                    .zip(expected_items.iter())
                    .map(|(item, item_ty)| {
                        self.render_guard_expr_as_type(output, *item, item_ty, span, guard_binding)
                    })
                    .collect::<Vec<_>>();
                let llvm_ty = self
                    .emitter
                    .lower_llvm_type(&tuple_ty, span, "guard tuple value")
                    .expect("prepared guard tuple values should already have supported LLVM types");
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
                return LoweredValue {
                    ty: tuple_ty,
                    llvm_ty,
                    repr: aggregate,
                };
            }
            hir::ExprKind::Array(items) => {
                let array_ty = self
                    .emitter
                    .input
                    .typeck
                    .expr_ty(expr_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should have a resolved array literal type"
                        )
                    });
                let Ty::Array { element, len } = &array_ty else {
                    panic!(
                        "prepared bool guard lowering at {span:?} should preserve array literal types"
                    );
                };
                let known_len = known_array_len(len).expect(
                    "prepared bool guard lowering should only render concrete fixed-array literals",
                );
                assert_eq!(
                    known_len,
                    items.len(),
                    "prepared bool guard lowering at {span:?} should preserve array literal length"
                );
                let rendered_items = items
                    .iter()
                    .map(|item| {
                        self.render_guard_expr_as_type(
                            output,
                            *item,
                            element.as_ref(),
                            span,
                            guard_binding,
                        )
                    })
                    .collect::<Vec<_>>();
                let llvm_ty = self
                    .emitter
                    .lower_llvm_type(&array_ty, span, "guard array value")
                    .expect("prepared guard array values should already have supported LLVM types");
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
                return LoweredValue {
                    ty: array_ty,
                    llvm_ty,
                    repr: aggregate,
                };
            }
            hir::ExprKind::StructLiteral { fields, .. } => {
                let struct_ty = self
                    .emitter
                    .input
                    .typeck
                    .expr_ty(expr_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should have a resolved struct literal type"
                        )
                    });
                let field_layouts = self
                    .emitter
                    .struct_field_lowerings(&struct_ty, span, "guard struct value")
                    .unwrap_or_else(|_| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should have a loadable struct literal layout"
                        )
                    });
                let llvm_ty = self
                    .emitter
                    .lower_llvm_type(&struct_ty, span, "guard struct value")
                    .unwrap_or_else(|_| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should have a lowered struct literal type"
                        )
                    });
                let mut rendered_fields = HashMap::with_capacity(fields.len());
                for field in fields {
                    let field_ty = field_layouts
                        .iter()
                        .find(|layout| layout.name == field.name)
                        .map(|layout| &layout.ty)
                        .unwrap_or_else(|| {
                            panic!(
                                "prepared bool guard lowering at {span:?} should provide declared struct literal field `{}`",
                                field.name
                            )
                        });
                    let rendered = self.render_guard_expr_as_type(
                        output,
                        field.value,
                        field_ty,
                        span,
                        guard_binding,
                    );
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
                            "prepared bool guard lowering at {span:?} should provide every declared struct literal field"
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
                return LoweredValue {
                    ty: struct_ty,
                    llvm_ty,
                    repr: aggregate,
                };
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                let tail = self
                    .emitter
                    .input
                    .hir
                    .block(*block_id)
                    .tail
                    .unwrap_or_else(|| {
                        panic!(
                            "prepared bool guard lowering at {span:?} should only render block loadable guard expressions with tails"
                        )
                    });
                return self.render_guard_loadable_expr(output, tail, span, guard_binding);
            }
            hir::ExprKind::Question(inner) => {
                return self.render_guard_loadable_expr(output, *inner, span, guard_binding);
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                return self.render_guard_await_expr(output, *expr, span, guard_binding);
            }
            _ => {}
        }

        if let Some((place, ty)) = guard_expr_place_with_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.local_types,
            None,
            expr_id,
        ) {
            assert!(
                !is_void_ty(&ty),
                "prepared bool guard lowering at {span:?} should only render valued loadable guard expressions"
            );
            return self.render_operand(output, &Operand::Place(place), span);
        }

        let (ptr, ty) = self
            .render_guard_projection_pointer(output, expr_id, span, guard_binding)
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should only render supported loadable guard expressions"
                )
            });
        assert!(
            !is_void_ty(&ty),
            "prepared bool guard lowering at {span:?} should only render valued loadable guard expressions"
        );
        self.render_loaded_pointer_value(output, ptr, ty, span)
    }

    fn render_guard_callable_block_expr_as_type(
        &mut self,
        output: &mut String,
        block_id: hir::BlockId,
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        let tail = self
            .render_cleanup_block_prefix(output, block_id, span)
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should only render callable block guard expressions with tails"
                )
            });
        let value = self.render_guard_expr_as_type(output, tail, expected_ty, span, guard_binding);
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        value
    }

    fn render_guard_value_if_expr(
        &mut self,
        output: &mut String,
        condition_expr: hir::ExprId,
        then_branch: hir::BlockId,
        else_expr: hir::ExprId,
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let condition = self.render_bool_guard_expr(output, condition_expr, span, guard_binding);
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "guard if value")
            .expect("prepared bool guard lowering should lower guard if values");
        let result_slot = self.fresh_temp();
        let then_label = self.fresh_label("guard_if_then");
        let else_label = self.fresh_label("guard_if_else");
        let end_label = self.fresh_label("guard_if_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        let then_tail = self
            .render_cleanup_block_prefix(output, then_branch, span)
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should only render valued guard `if` branches with tails"
                )
            });
        let then_value =
            self.render_guard_expr_as_type(output, then_tail, expected_ty, span, guard_binding);
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            then_value.llvm_ty, then_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        let else_value =
            self.render_guard_expr_as_type(output, else_expr, expected_ty, span, guard_binding);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            else_value.llvm_ty, else_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_guard_value_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!("prepared bool guard lowering at {span:?} should type-check guard value matches")
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_bool_guard_expr(output, value_expr, span, guard_binding);
            return self.render_guard_value_bool_match_expr(
                output,
                scrutinee,
                arms,
                expected_ty,
                span,
                guard_binding,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_guard_scalar_expr(output, value_expr, span, guard_binding);
            return self.render_guard_value_integer_match_expr(
                output,
                scrutinee,
                arms,
                expected_ty,
                span,
                guard_binding,
            );
        }

        panic!("prepared functions should not contain unsupported guard value matches");
    }

    fn render_guard_value_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "guard match value")
            .expect("prepared bool guard lowering should lower guard match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("guard_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("prepared bool guard lowering at {span:?} should only render supported bool guard-match patterns")
            });
            let body_label = self.fresh_label("guard_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("guard_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("guard_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(
                            "guard match-value lowering should handle bool catch-all separately"
                        ),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            let arm_value =
                self.render_guard_expr_as_type(output, arm.body, expected_ty, span, guard_binding);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_guard_value_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let result_llvm_ty = self
            .emitter
            .lower_llvm_type(expected_ty, span, "guard match value")
            .expect("prepared bool guard lowering should lower guard match values");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("guard_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {result_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("prepared bool guard lowering at {span:?} should only render supported integer guard-match patterns")
            });
            let body_label = self.fresh_label("guard_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("guard_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("guard_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            let arm_value =
                self.render_guard_expr_as_type(output, arm.body, expected_ty, span, guard_binding);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, expected_ty.clone(), span)
    }

    fn render_guard_expr_as_type(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        expected_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if matches!(expected_ty, Ty::Callable { .. }) {
            if let hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) =
                &self.emitter.input.hir.expr(expr_id).kind
            {
                return self.render_guard_callable_block_expr_as_type(
                    output,
                    *block_id,
                    expected_ty,
                    span,
                    guard_binding,
                );
            }
        }

        if let hir::ExprKind::If {
            condition,
            then_branch,
            else_branch: Some(other),
        } = &self.emitter.input.hir.expr(expr_id).kind
        {
            return self.render_guard_value_if_expr(
                output,
                *condition,
                *then_branch,
                *other,
                expected_ty,
                span,
                guard_binding,
            );
        }

        if let hir::ExprKind::Match { value, arms } = &self.emitter.input.hir.expr(expr_id).kind {
            return self.render_guard_value_match_expr(
                output,
                *value,
                arms,
                expected_ty,
                span,
                guard_binding,
            );
        }

        if expected_ty.is_bool() {
            let rendered = self.render_bool_guard_expr(output, expr_id, span, guard_binding);
            assert!(
                expected_ty.compatible_with(&rendered.ty),
                "prepared bool guard lowering at {span:?} should only render compatible bool guard literal elements"
            );
            rendered
        } else if expected_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let rendered = self.render_guard_scalar_expr(output, expr_id, span, guard_binding);
            assert!(
                expected_ty.compatible_with(&rendered.ty),
                "prepared bool guard lowering at {span:?} should only render compatible Int guard literal elements"
            );
            rendered
        } else {
            let rendered = self.render_guard_loadable_expr(output, expr_id, span, guard_binding);
            assert!(
                expected_ty.compatible_with(&rendered.ty),
                "prepared bool guard lowering at {span:?} should only render compatible loadable guard literal elements"
            );
            rendered
        }
    }

    fn render_guard_call_arg(
        &mut self,
        output: &mut String,
        arg: &hir::CallArg,
        param_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> String {
        let expr_id = guard_call_arg_expr(arg);
        let rendered = match guard_scalar_kind_for_ty(param_ty) {
            Some(GuardScalarKind::Bool) => {
                let rendered = self.render_bool_guard_expr(output, expr_id, span, guard_binding);
                assert!(
                    rendered.ty.is_bool(),
                    "prepared bool guard lowering at {span:?} should only render compatible bool guard-call arguments"
                );
                rendered
            }
            Some(GuardScalarKind::Int) => {
                let rendered = self.render_guard_scalar_expr(output, expr_id, span, guard_binding);
                assert!(
                    rendered.ty.compatible_with(&Ty::Builtin(BuiltinType::Int)),
                    "prepared bool guard lowering at {span:?} should only render compatible Int guard-call arguments"
                );
                rendered
            }
            None => self.render_guard_expr_as_type(output, expr_id, param_ty, span, guard_binding),
        };
        assert!(
            param_ty.compatible_with(&rendered.ty),
            "prepared bool guard lowering at {span:?} should only render compatible guard-call arguments"
        );
        format!("{} {}", rendered.llvm_ty, rendered.repr)
    }

    fn render_guard_direct_capturing_closure_call(
        &mut self,
        output: &mut String,
        closure_id: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let closure = self.body.closure(closure_id);
        let closure_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(closure.expr)
            .cloned()
            .expect(
                "prepared bool guard lowering should preserve direct local capturing closure callable types",
            );
        let Ty::Callable { params, ret } = &closure_ty else {
            panic!(
                "prepared bool guard lowering at {span:?} should only treat callable capturing closures as guard callees"
            );
        };
        assert!(
            params.len() == args.len()
                && args
                    .iter()
                    .all(|arg| matches!(arg, hir::CallArg::Positional(_))),
            "prepared bool guard lowering at {span:?} should only render positional capturing-closure guard-call arguments matching the callable arity"
        );
        let return_ty = ret.as_ref().clone();
        assert!(
            !is_void_ty(&return_ty),
            "prepared bool guard lowering at {span:?} should only render valued capturing-closure guard calls"
        );

        let mut rendered_args = Vec::with_capacity(closure.captures.len() + args.len());
        for capture in &closure.captures {
            let rendered =
                self.render_operand(output, &Operand::Place(Place::local(capture.local)), span);
            rendered_args.push(format!("{} {}", rendered.llvm_ty, rendered.repr));
        }
        for (arg, param_ty) in args.iter().zip(params.iter()) {
            rendered_args.push(self.render_guard_call_arg(
                output,
                arg,
                param_ty,
                span,
                guard_binding,
            ));
        }

        let rendered_args = rendered_args.join(", ");
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "guard call result")
            .expect(
                "prepared bool guard lowering should only emit lowered capturing-closure guard results",
            );
        let callee_name = closure_llvm_name(&self.prepared.signature.llvm_name, closure_id);
        let temp = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {temp} = call {return_llvm_ty} @{callee_name}({rendered_args})"
        );
        LoweredValue {
            ty: return_ty,
            llvm_ty: return_llvm_ty,
            repr: temp,
        }
    }

    fn render_guard_bound_if_capturing_closure_call(
        &mut self,
        output: &mut String,
        condition: LoweredValue,
        then_closure: mir::ClosureId,
        else_closure: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(then_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "prepared bool guard lowering should preserve callable return types for shared-local control-flow bindings",
            );
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should only emit lowered guard call results");
        let result_slot = self.fresh_temp();
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");
        let then_label = self.fresh_label("guard_call_if_then");
        let else_label = self.fresh_label("guard_call_if_else");
        let end_label = self.fresh_label("guard_call_if_end");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        let then_value = self.render_guard_direct_capturing_closure_call(
            output,
            then_closure,
            args,
            span,
            guard_binding,
        );
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            then_value.llvm_ty, then_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        let else_value = self.render_guard_direct_capturing_closure_call(
            output,
            else_closure,
            args,
            span,
            guard_binding,
        );
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            else_value.llvm_ty, else_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty, span)
    }

    fn render_guard_bound_integer_match_capturing_closure_call(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[CleanupIntegerCapturingClosureMatchArm],
        fallback_closure: mir::ClosureId,
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(fallback_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "prepared bool guard lowering should preserve callable return types for shared-local match bindings",
            );
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should only emit lowered guard call results");
        let result_slot = self.fresh_temp();
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");
        let end_label = self.fresh_label("guard_call_match_end");
        let fallback_label = self.fresh_label("guard_call_match_fallback");
        let dispatch_labels = (0..arms.len())
            .map(|_| self.fresh_label("guard_call_match_dispatch"))
            .collect::<Vec<_>>();
        let arm_labels = (0..arms.len())
            .map(|_| self.fresh_label("guard_call_match_arm"))
            .collect::<Vec<_>>();
        let opcode = compare_opcode(BinaryOp::EqEq, &scrutinee.ty);

        if arms.is_empty() {
            let _ = writeln!(output, "  br label %{fallback_label}");
        } else {
            for (index, arm) in arms.iter().enumerate() {
                if index > 0 {
                    let _ = writeln!(output, "{}:", dispatch_labels[index]);
                }
                let compare = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {compare} = {opcode} {} {}, {}",
                    scrutinee.llvm_ty, scrutinee.repr, arm.value
                );
                let false_target = if index + 1 == arms.len() {
                    fallback_label.clone()
                } else {
                    dispatch_labels[index + 1].clone()
                };
                let _ = writeln!(
                    output,
                    "  br i1 {compare}, label %{}, label %{false_target}",
                    arm_labels[index]
                );
            }
        }

        for (arm, arm_label) in arms.iter().zip(arm_labels.iter()) {
            let _ = writeln!(output, "{arm_label}:");
            let value = self.render_guard_direct_capturing_closure_call(
                output,
                arm.closure_id,
                args,
                span,
                guard_binding,
            );
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");
        }

        let _ = writeln!(output, "{fallback_label}:");
        let value = self.render_guard_direct_capturing_closure_call(
            output,
            fallback_closure,
            args,
            span,
            guard_binding,
        );
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            value.llvm_ty, value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty, span)
    }

    fn render_guard_bound_tagged_match_capturing_closure_call(
        &mut self,
        output: &mut String,
        tag: LoweredValue,
        closures: &[mir::ClosureId],
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let fallback_closure = *closures
            .last()
            .expect("supported cleanup tagged match bindings should preserve at least one arm");
        let return_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(self.body.closure(fallback_closure).expr)
            .and_then(|ty| match ty {
                Ty::Callable { ret, .. } => Some(ret.as_ref().clone()),
                _ => None,
            })
            .expect(
                "prepared bool guard lowering should preserve callable return types for tagged match bindings",
            );
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should only emit lowered guard call results");
        let result_slot = self.fresh_temp();
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");
        let end_label = self.fresh_label("guard_call_match_end");
        let fallback_label = self.fresh_label("guard_call_match_fallback");
        let dispatch_labels = (0..closures.len())
            .map(|_| self.fresh_label("guard_call_match_dispatch"))
            .collect::<Vec<_>>();
        let arm_labels = (0..closures.len())
            .map(|_| self.fresh_label("guard_call_match_arm"))
            .collect::<Vec<_>>();
        let opcode = compare_opcode(BinaryOp::EqEq, &tag.ty);

        if closures.is_empty() {
            let _ = writeln!(output, "  br label %{fallback_label}");
        } else {
            for index in 0..closures.len() {
                if index > 0 {
                    let _ = writeln!(output, "{}:", dispatch_labels[index]);
                }
                let compare = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {compare} = {opcode} {} {}, {}",
                    tag.llvm_ty, tag.repr, index
                );
                let false_target = if index + 1 == closures.len() {
                    fallback_label.clone()
                } else {
                    dispatch_labels[index + 1].clone()
                };
                let _ = writeln!(
                    output,
                    "  br i1 {compare}, label %{}, label %{false_target}",
                    arm_labels[index]
                );
            }
        }

        for (closure_id, arm_label) in closures.iter().zip(arm_labels.iter()) {
            let _ = writeln!(output, "{arm_label}:");
            let value = self.render_guard_direct_capturing_closure_call(
                output,
                *closure_id,
                args,
                span,
                guard_binding,
            );
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                value.llvm_ty, value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");
        }

        let _ = writeln!(output, "{fallback_label}:");
        let value = self.render_guard_direct_capturing_closure_call(
            output,
            fallback_closure,
            args,
            span,
            guard_binding,
        );
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            value.llvm_ty, value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty, span)
    }

    fn render_guard_call_value(
        &mut self,
        output: &mut String,
        callee_expr: hir::ExprId,
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if let Some(CleanupCapturingClosureBindingValue::TaggedMatch {
            tag: Some(tag),
            closures,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_guard_bound_tagged_match_capturing_closure_call(
                output,
                tag,
                &closures,
                args,
                span,
                guard_binding,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::BoolMatch {
            scrutinee: Some(scrutinee),
            true_closure,
            false_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_guard_bound_if_capturing_closure_call(
                output,
                scrutinee,
                true_closure,
                false_closure,
                args,
                span,
                guard_binding,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::IntegerMatch {
            scrutinee: Some(scrutinee),
            arms,
            fallback_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_guard_bound_integer_match_capturing_closure_call(
                output,
                scrutinee,
                &arms,
                fallback_closure,
                args,
                span,
                guard_binding,
            );
        }

        if let Some(CleanupCapturingClosureBindingValue::IfBranch {
            condition: Some(condition),
            then_closure,
            else_closure,
        }) = cleanup_bound_capturing_closure_value_for_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_guard_bound_if_capturing_closure_call(
                output,
                condition,
                then_closure,
                else_closure,
                args,
                span,
                guard_binding,
            );
        }

        if let hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) =
            &self.emitter.input.hir.expr(callee_expr).kind
        {
            return self.render_guard_call_block_expr(output, *block_id, args, span, guard_binding);
        }

        if let hir::ExprKind::If {
            condition,
            then_branch,
            else_branch: Some(other),
        } = &self.emitter.input.hir.expr(callee_expr).kind
        {
            let return_ty = self
                .guard_call_callee_return_ty(callee_expr, span)
                .unwrap_or_else(|| {
                    panic!(
                        "prepared bool guard lowering at {span:?} should only recurse through callable guard-if callees"
                    )
                });
            return self.render_guard_call_if_expr(
                output,
                *condition,
                *then_branch,
                *other,
                args,
                &return_ty,
                span,
                guard_binding,
            );
        }

        if let hir::ExprKind::Match { value, arms } = &self.emitter.input.hir.expr(callee_expr).kind
        {
            let return_ty = self
                .guard_call_callee_return_ty(callee_expr, span)
                .unwrap_or_else(|| {
                    panic!(
                        "prepared bool guard lowering at {span:?} should only recurse through callable guard-match callees"
                    )
                });
            return self.render_guard_call_match_expr(
                output,
                *value,
                arms,
                args,
                &return_ty,
                span,
                guard_binding,
            );
        }

        if let Some(function) = guard_direct_callee_function(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            callee_expr,
        ) {
            let signature = self.emitter.signatures.get(&function).unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should resolve guard-call signatures"
                )
            });
            assert!(
                !is_void_ty(&signature.return_ty),
                "prepared bool guard lowering at {span:?} should only render valued guard calls"
            );
            let ordered_args = ordered_guard_call_args(args, signature).unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should preserve direct guard-call argument mapping"
                )
            });
            let rendered_args = ordered_args
                .into_iter()
                .zip(signature.params.iter())
                .map(|(arg, param)| {
                    self.render_guard_call_arg(output, arg, &param.ty, span, guard_binding)
                })
                .collect::<Vec<_>>()
                .join(", ");
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
            return LoweredValue {
                ty,
                llvm_ty: signature.return_llvm_ty.clone(),
                repr: temp,
            };
        }

        if let Some(closure_id) =
            self.supported_direct_local_capturing_closure_for_expr(callee_expr)
        {
            return self.render_guard_direct_capturing_closure_call(
                output,
                closure_id,
                args,
                span,
                guard_binding,
            );
        }

        if let Some(closure_id) = supported_direct_local_capturing_closure_callee_closure(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.direct_local_capturing_closures,
            &self.cleanup_capturing_closure_bindings,
            callee_expr,
        ) {
            return self.render_guard_direct_capturing_closure_call(
                output,
                closure_id,
                args,
                span,
                guard_binding,
            );
        }

        let callee_ty = self
            .emitter
            .input
            .typeck
            .expr_ty(callee_expr)
            .cloned()
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should type-check callable guard callees"
                )
            });
        let callee =
            self.render_guard_expr_as_type(output, callee_expr, &callee_ty, span, guard_binding);
        let Ty::Callable { params, ret } = &callee.ty else {
            panic!(
                "prepared bool guard lowering at {span:?} should only render direct resolved guard calls or callable guard values"
            );
        };
        assert!(
            params.len() == args.len()
                && args
                    .iter()
                    .all(|arg| matches!(arg, hir::CallArg::Positional(_))),
            "prepared bool guard lowering at {span:?} should only render positional callable guard-call arguments matching the callable arity"
        );
        let return_ty = ret.as_ref().clone();
        assert!(
            !is_void_ty(&return_ty),
            "prepared bool guard lowering at {span:?} should only render valued callable guard calls"
        );
        let rendered_args = args
            .iter()
            .zip(params.iter())
            .map(|(arg, param_ty)| {
                self.render_guard_call_arg(output, arg, param_ty, span, guard_binding)
            })
            .collect::<Vec<_>>()
            .join(", ");
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(&return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should only emit lowered callable guard results");
        let temp = self.fresh_temp();
        let _ = writeln!(
            output,
            "  {temp} = call {return_llvm_ty} {}({rendered_args})",
            callee.repr
        );
        LoweredValue {
            ty: return_ty,
            llvm_ty: return_llvm_ty,
            repr: temp,
        }
    }

    fn guard_call_callee_return_ty(&self, callee_expr: hir::ExprId, span: Span) -> Option<Ty> {
        let callee_ty = self.emitter.input.typeck.expr_ty(callee_expr)?.clone();
        let Ty::Callable { ret, .. } = callee_ty else {
            panic!(
                "prepared bool guard lowering at {span:?} should only recurse through callable guard callees"
            );
        };
        let return_ty = ret.as_ref().clone();
        assert!(
            !is_void_ty(&return_ty),
            "prepared bool guard lowering at {span:?} should only recurse through valued guard callees"
        );
        Some(return_ty)
    }

    fn render_guard_call_block_expr(
        &mut self,
        output: &mut String,
        block_id: hir::BlockId,
        args: &[hir::CallArg],
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        if let Some(tail) = callable_elided_block_tail_expr(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            block_id,
        ) {
            return self.render_guard_call_value(output, tail, args, span, guard_binding);
        }

        let binding_depth = self.cleanup_bindings.len();
        let closure_binding_depth = self.cleanup_capturing_closure_bindings.len();
        let tail = self
            .render_cleanup_block_prefix(output, block_id, span)
            .unwrap_or_else(|| {
                panic!(
                    "prepared bool guard lowering at {span:?} should only render callable guard blocks with tails"
                )
            });
        let value = self.render_guard_call_value(output, tail, args, span, guard_binding);
        self.cleanup_bindings.truncate(binding_depth);
        self.cleanup_capturing_closure_bindings
            .truncate(closure_binding_depth);
        value
    }

    fn render_guard_call_if_expr(
        &mut self,
        output: &mut String,
        condition_expr: hir::ExprId,
        then_branch: hir::BlockId,
        else_expr: hir::ExprId,
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let condition = self.render_bool_guard_expr(output, condition_expr, span, guard_binding);
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should lower guard-call if results");
        let result_slot = self.fresh_temp();
        let then_label = self.fresh_label("guard_call_if_then");
        let else_label = self.fresh_label("guard_call_if_else");
        let end_label = self.fresh_label("guard_call_if_end");
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");
        let _ = writeln!(
            output,
            "  br i1 {}, label %{then_label}, label %{else_label}",
            condition.repr
        );

        let _ = writeln!(output, "{then_label}:");
        let then_value =
            self.render_guard_call_block_expr(output, then_branch, args, span, guard_binding);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            then_value.llvm_ty, then_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{else_label}:");
        let else_value = self.render_guard_call_value(output, else_expr, args, span, guard_binding);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {result_slot}",
            else_value.llvm_ty, else_value.repr
        );
        let _ = writeln!(output, "  br label %{end_label}");

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty.clone(), span)
    }

    fn render_guard_call_match_expr(
        &mut self,
        output: &mut String,
        value_expr: hir::ExprId,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let Some(scrutinee_ty) = self.emitter.input.typeck.expr_ty(value_expr) else {
            panic!("prepared bool guard lowering at {span:?} should type-check guard call matches")
        };

        if scrutinee_ty.is_bool() {
            let scrutinee = self.render_bool_guard_expr(output, value_expr, span, guard_binding);
            return self.render_guard_call_bool_match_expr(
                output,
                scrutinee,
                arms,
                args,
                return_ty,
                span,
                guard_binding,
            );
        }

        if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
            let scrutinee = self.render_guard_scalar_expr(output, value_expr, span, guard_binding);
            return self.render_guard_call_integer_match_expr(
                output,
                scrutinee,
                arms,
                args,
                return_ty,
                span,
                guard_binding,
            );
        }

        panic!("prepared functions should not contain unsupported guard call matches");
    }

    fn render_guard_call_bool_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should lower guard-call match results");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("guard_call_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_bool_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("prepared bool guard lowering at {span:?} should only render supported bool guard-call match patterns")
            });
            let body_label = self.fresh_label("guard_call_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("guard_call_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match pattern {
                SupportedBoolMatchPattern::True | SupportedBoolMatchPattern::False => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("guard_call_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = match pattern {
                        SupportedBoolMatchPattern::True => scrutinee.repr.clone(),
                        SupportedBoolMatchPattern::False => {
                            let temp = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {temp} = icmp eq {} {}, false",
                                scrutinee.llvm_ty, scrutinee.repr
                            );
                            temp
                        }
                        SupportedBoolMatchPattern::CatchAll => unreachable!(),
                    };
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedBoolMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            let arm_value =
                self.render_guard_call_value(output, arm.body, args, span, guard_binding);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedBoolMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty.clone(), span)
    }

    fn render_guard_call_integer_match_expr(
        &mut self,
        output: &mut String,
        scrutinee: LoweredValue,
        arms: &[hir::MatchArm],
        args: &[hir::CallArg],
        return_ty: &Ty,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> LoweredValue {
        let return_llvm_ty = self
            .emitter
            .lower_llvm_type(return_ty, span, "guard call result")
            .expect("prepared bool guard lowering should lower guard-call match results");
        let result_slot = self.fresh_temp();
        let end_label = self.fresh_label("guard_call_match_end");
        let _ = writeln!(output, "  {result_slot} = alloca {return_llvm_ty}");

        for (index, arm) in arms.iter().enumerate() {
            let pattern = supported_cleanup_integer_match_pattern(
                self.emitter.input.hir,
                self.emitter.input.resolution,
                arm.pattern,
            )
            .unwrap_or_else(|| {
                panic!("prepared bool guard lowering at {span:?} should only render supported integer guard-call match patterns")
            });
            let body_label = self.fresh_label("guard_call_match_arm");
            let next_label = if index + 1 == arms.len() {
                end_label.clone()
            } else {
                self.fresh_label("guard_call_match_next")
            };
            let binding_local = match pattern_kind(self.emitter.input.hir, arm.pattern) {
                PatternKind::Binding(local) => Some(*local),
                _ => None,
            };

            if let Some(local) = binding_local {
                self.cleanup_bindings.push(GuardBindingValue {
                    local,
                    value: scrutinee.clone(),
                });
            }

            match &pattern {
                SupportedIntegerMatchPattern::Literal(value) => {
                    let matched_label = if arm.guard.is_some() {
                        self.fresh_label("guard_call_match_guard")
                    } else {
                        body_label.clone()
                    };
                    let condition = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {condition} = icmp eq {} {}, {}",
                        scrutinee.llvm_ty, scrutinee.repr, value
                    );
                    let _ = writeln!(
                        output,
                        "  br i1 {condition}, label %{matched_label}, label %{next_label}"
                    );
                    if let Some(guard) = arm.guard {
                        let _ = writeln!(output, "{matched_label}:");
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    }
                }
                SupportedIntegerMatchPattern::CatchAll => {
                    if let Some(guard) = arm.guard {
                        let guard = self.render_bool_guard_expr(output, guard, span, guard_binding);
                        let _ = writeln!(
                            output,
                            "  br i1 {}, label %{body_label}, label %{next_label}",
                            guard.repr
                        );
                    } else {
                        let _ = writeln!(output, "  br label %{body_label}");
                    }
                }
            }

            let _ = writeln!(output, "{body_label}:");
            let arm_value =
                self.render_guard_call_value(output, arm.body, args, span, guard_binding);
            let _ = writeln!(
                output,
                "  store {} {}, ptr {result_slot}",
                arm_value.llvm_ty, arm_value.repr
            );
            let _ = writeln!(output, "  br label %{end_label}");

            if binding_local.is_some() {
                self.cleanup_bindings.pop();
            }

            if next_label != end_label {
                let _ = writeln!(output, "{next_label}:");
            }

            if matches!(pattern, SupportedIntegerMatchPattern::CatchAll) && arm.guard.is_none() {
                break;
            }
        }

        let _ = writeln!(output, "{end_label}:");
        self.render_loaded_pointer_value(output, result_slot, return_ty.clone(), span)
    }

    fn materialize_guard_item_root(
        &mut self,
        output: &mut String,
        item_id: ItemId,
        span: Span,
    ) -> Option<(String, Ty)> {
        let rendered = match &self.emitter.input.hir.item(item_id).kind {
            ItemKind::Function(_) => {
                self.render_function_constant(FunctionRef::Item(item_id), span)
            }
            ItemKind::Const(_) | ItemKind::Static(_) => {
                self.render_item_constant(output, item_id, span)
            }
            _ => return None,
        };
        let slot = self.fresh_temp();
        let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {slot}",
            rendered.llvm_ty, rendered.repr
        );
        Some((slot, rendered.ty))
    }

    fn render_guard_projection_pointer(
        &mut self,
        output: &mut String,
        expr_id: hir::ExprId,
        span: Span,
        guard_binding: Option<&GuardBindingValue>,
    ) -> Option<(String, Ty)> {
        if let Some((place, _)) = guard_expr_place_with_ty(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            self.body,
            &self.prepared.local_types,
            None,
            expr_id,
        ) {
            return Some(self.render_place_pointer(output, &place, span));
        }

        match &self.emitter.input.hir.expr(expr_id).kind {
            hir::ExprKind::Call { callee, args } => {
                let rendered =
                    self.render_guard_call_value(output, *callee, args, span, guard_binding);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Unary {
                op: UnaryOp::Await,
                expr,
            } => {
                let rendered = self.render_guard_await_expr(output, *expr, span, guard_binding);
                if is_void_ty(&rendered.ty) {
                    return None;
                }
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Tuple(_)
            | hir::ExprKind::Array(_)
            | hir::ExprKind::StructLiteral { .. } => {
                let rendered =
                    self.render_guard_loadable_expr(output, expr_id, span, guard_binding);
                let slot = self.fresh_temp();
                let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                let _ = writeln!(
                    output,
                    "  store {} {}, ptr {slot}",
                    rendered.llvm_ty, rendered.repr
                );
                Some((slot, rendered.ty))
            }
            hir::ExprKind::Name(_) => {
                match self.emitter.input.resolution.expr_resolution(expr_id) {
                    Some(ValueResolution::Function(function)) => {
                        let rendered = self.render_function_constant(*function, span);
                        let slot = self.fresh_temp();
                        let _ = writeln!(output, "  {slot} = alloca {}", rendered.llvm_ty);
                        let _ = writeln!(
                            output,
                            "  store {} {}, ptr {slot}",
                            rendered.llvm_ty, rendered.repr
                        );
                        Some((slot, rendered.ty))
                    }
                    Some(ValueResolution::Local(local))
                        if guard_binding.is_some_and(|binding| binding.local == *local) =>
                    {
                        let binding_local = mir_local_for_hir_local(self.body, *local)?;
                        let ty = self.prepared.local_types.get(&binding_local)?.clone();
                        Some((llvm_slot_name(self.body, binding_local), ty))
                    }
                    Some(ValueResolution::Local(local))
                        if self.cleanup_binding(*local).is_some() =>
                    {
                        self.materialize_cleanup_binding_root(output, *local)
                    }
                    Some(ValueResolution::Item(item_id)) => {
                        self.materialize_guard_item_root(output, *item_id, span)
                    }
                    Some(ValueResolution::Import(import_binding)) => {
                        let item_id =
                            local_item_for_import_binding(self.emitter.input.hir, import_binding)?;
                        self.materialize_guard_item_root(output, item_id, span)
                    }
                    _ => None,
                }
            }
            hir::ExprKind::Member { object, field, .. } => {
                let (current_ptr, current_ty) =
                    self.render_guard_projection_pointer(output, *object, span, guard_binding)?;
                let aggregate_llvm_ty = self
                    .emitter
                    .lower_llvm_type(&current_ty, span, "projection base type")
                    .ok()?;
                let step = self
                    .emitter
                    .resolve_projection_step(
                        &current_ty,
                        &mir::ProjectionElem::Field(field.clone()),
                        span,
                    )
                    .ok()?;
                let ResolvedProjectionStep::Field { index, ty } = step else {
                    return None;
                };
                let next = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i32 0, i32 {index}"
                );
                Some((next, ty))
            }
            hir::ExprKind::Bracket { target, items } => {
                let (mut current_ptr, mut current_ty) =
                    self.render_guard_projection_pointer(output, *target, span, guard_binding)?;
                for item in items {
                    let aggregate_llvm_ty = self
                        .emitter
                        .lower_llvm_type(&current_ty, span, "projection base type")
                        .ok()?;
                    match &current_ty {
                        Ty::Array { element, .. } => {
                            let rendered_index =
                                self.render_guard_scalar_expr(output, *item, span, guard_binding);
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
                            current_ty = element.as_ref().clone();
                        }
                        Ty::Tuple(_) => {
                            let index = guard_literal_int(
                                self.emitter.input.hir,
                                self.emitter.input.resolution,
                                *item,
                            )?;
                            if index < 0 {
                                return None;
                            }
                            let step = self
                                .emitter
                                .resolve_projection_step(
                                    &current_ty,
                                    &mir::ProjectionElem::Index(Box::new(Operand::Constant(
                                        Constant::Integer(index.to_string()),
                                    ))),
                                    span,
                                )
                                .ok()?;
                            let ResolvedProjectionStep::TupleIndex { index, ty } = step else {
                                return None;
                            };
                            let next = self.fresh_temp();
                            let _ = writeln!(
                                output,
                                "  {next} = getelementptr inbounds {aggregate_llvm_ty}, ptr {current_ptr}, i32 0, i32 {index}"
                            );
                            current_ptr = next;
                            current_ty = ty;
                        }
                        _ => return None,
                    }
                }
                Some((current_ptr, current_ty))
            }
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => self
                .emitter
                .input
                .hir
                .block(*block_id)
                .tail
                .and_then(|tail| {
                    self.render_guard_projection_pointer(output, tail, span, guard_binding)
                }),
            hir::ExprKind::Question(inner) => {
                self.render_guard_projection_pointer(output, *inner, span, guard_binding)
            }
            _ => None,
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

        self.render_loaded_pointer_value(output, ptr, ty, span)
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

    fn fresh_label(&mut self, prefix: &str) -> String {
        let index = self.next_temp;
        self.next_temp += 1;
        format!("{prefix}_{index}")
    }

    fn render_for_loop_iterable_initialization(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        loop_lowering: &SupportedForLoopLowering,
    ) {
        let SupportedForLoopIterableRoot::Item(item_id) = &loop_lowering.iterable_root else {
            return;
        };
        let span = self.body.block(block_id).terminator.span;
        let rendered = self.render_for_loop_item_root_value(output, *item_id, span);
        let _ = writeln!(
            output,
            "  store {} {}, ptr {}",
            rendered.llvm_ty,
            rendered.repr,
            for_iterable_slot_name(block_id)
        );
    }

    fn render_for_loop_item_root_value(
        &mut self,
        output: &mut String,
        item_id: ItemId,
        span: Span,
    ) -> LoweredValue {
        let Some((value_expr, ty)) = runtime_task_iterable_item_value(
            self.emitter.input.hir,
            self.emitter.input.resolution,
            item_id,
        ) else {
            return self.render_item_constant(output, item_id, span);
        };
        self.render_cleanup_value_expr(output, value_expr, &ty, span)
    }

    fn render_for_loop_iterable_pointer(
        &mut self,
        output: &mut String,
        block_id: mir::BasicBlockId,
        loop_lowering: &SupportedForLoopLowering,
        span: Span,
    ) -> (String, Ty) {
        match &loop_lowering.iterable_root {
            SupportedForLoopIterableRoot::Place(place) => {
                self.render_place_pointer(output, place, span)
            }
            SupportedForLoopIterableRoot::Item(item_id) => (
                for_iterable_slot_name(block_id),
                const_or_static_item_type(
                    self.emitter.input.hir,
                    self.emitter.input.resolution,
                    *item_id,
                )
                .unwrap_or_else(|| {
                    panic!(
                        "prepared `for` lowering at {span:?} should only materialize const/static iterable roots"
                    )
                }),
            ),
        }
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
            self.render_for_loop_iterable_pointer(output, block_id, loop_lowering, span);
        let iterable_llvm_ty = self
            .emitter
            .lower_llvm_type(&iterable_ty, span, "for-await iterable type")
            .expect("prepared for-await iterable should have a lowered LLVM type");
        let element_llvm_ty = self
            .emitter
            .lower_llvm_type(&loop_lowering.element_ty, span, "for-await item type")
            .expect("prepared for-await item should have a lowered LLVM type");

        match loop_lowering.iterable_kind {
            SupportedForLoopIterableKind::Array => {
                let element_ptr = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {element_ptr} = getelementptr inbounds {iterable_llvm_ty}, ptr {iterable_ptr}, i64 0, i64 {index}"
                );
                let element = self.fresh_temp();
                let _ = writeln!(
                    output,
                    "  {element} = load {element_llvm_ty}, ptr {element_ptr}"
                );
                self.render_loaded_for_loop_item(
                    output,
                    loop_lowering,
                    &element_llvm_ty,
                    &element,
                    span,
                );
            }
            SupportedForLoopIterableKind::Tuple => {
                if loop_lowering.iterable_len == 1 {
                    let _ = writeln!(
                        output,
                        "  br label %{}",
                        for_await_tuple_item_block_name(block_id, 0)
                    );
                } else {
                    let compare = self.fresh_temp();
                    let _ = writeln!(output, "  {compare} = icmp eq i64 {index}, 0");
                    let _ = writeln!(
                        output,
                        "  br i1 {compare}, label %{}, label %{}",
                        for_await_tuple_item_block_name(block_id, 0),
                        for_await_tuple_check_block_name(block_id, 1)
                    );
                }

                for item_index in 1..loop_lowering.iterable_len {
                    let _ = writeln!(
                        output,
                        "{}:",
                        for_await_tuple_check_block_name(block_id, item_index)
                    );
                    if item_index + 1 < loop_lowering.iterable_len {
                        let compare = self.fresh_temp();
                        let _ = writeln!(output, "  {compare} = icmp eq i64 {index}, {item_index}");
                        let _ = writeln!(
                            output,
                            "  br i1 {compare}, label %{}, label %{}",
                            for_await_tuple_item_block_name(block_id, item_index),
                            for_await_tuple_check_block_name(block_id, item_index + 1)
                        );
                    } else {
                        let _ = writeln!(
                            output,
                            "  br label %{}",
                            for_await_tuple_item_block_name(block_id, item_index)
                        );
                    }
                }

                for item_index in 0..loop_lowering.iterable_len {
                    let _ = writeln!(
                        output,
                        "{}:",
                        for_await_tuple_item_block_name(block_id, item_index)
                    );
                    let element_ptr = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {element_ptr} = getelementptr inbounds {iterable_llvm_ty}, ptr {iterable_ptr}, i32 0, i32 {item_index}"
                    );
                    let element = self.fresh_temp();
                    let _ = writeln!(
                        output,
                        "  {element} = load {element_llvm_ty}, ptr {element_ptr}"
                    );
                    self.render_loaded_for_loop_item(
                        output,
                        loop_lowering,
                        &element_llvm_ty,
                        &element,
                        span,
                    );
                }
            }
        }
    }

    fn render_loaded_for_loop_item(
        &mut self,
        output: &mut String,
        loop_lowering: &SupportedForLoopLowering,
        element_llvm_ty: &str,
        element: &str,
        span: Span,
    ) {
        if loop_lowering.auto_await_task_elements {
            let await_hook = self
                .emitter
                .runtime_hook_signature(RuntimeHook::TaskAwait)
                .expect("prepared for-await task element lowering should require the task-await runtime hook");
            let release_hook = self
                .emitter
                .runtime_hook_signature(RuntimeHook::TaskResultRelease)
                .expect("prepared for-await task element lowering should require the task-result-release runtime hook");
            let result_layout = self
                .emitter
                .build_async_task_result_layout(&loop_lowering.item_ty, span)
                .expect(
                    "prepared for-await task element should have a supported async result layout",
                );
            let result_ptr = self.fresh_temp();
            let _ = writeln!(
                output,
                "  {result_ptr} = call {} @{}({element_llvm_ty} {element})",
                await_hook.return_type.llvm_ir(),
                await_hook.hook.symbol_name(),
            );
            match result_layout {
                AsyncTaskResultLayout::Void => {
                    let _ = writeln!(
                        output,
                        "  call {} @{}(ptr {result_ptr})",
                        release_hook.return_type.llvm_ir(),
                        release_hook.hook.symbol_name()
                    );
                }
                AsyncTaskResultLayout::Loadable { llvm_ty, .. } => {
                    let awaited = self.fresh_temp();
                    let _ = writeln!(output, "  {awaited} = load {llvm_ty}, ptr {result_ptr}");
                    let _ = writeln!(
                        output,
                        "  call {} @{}(ptr {result_ptr})",
                        release_hook.return_type.llvm_ir(),
                        release_hook.hook.symbol_name()
                    );
                    let _ = writeln!(
                        output,
                        "  store {llvm_ty} {awaited}, ptr {}",
                        llvm_slot_name(self.body, loop_lowering.item_local)
                    );
                }
            }
        } else if !is_void_ty(&loop_lowering.item_ty) {
            let _ = writeln!(
                output,
                "  store {element_llvm_ty} {element}, ptr {}",
                llvm_slot_name(self.body, loop_lowering.item_local)
            );
        }
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

fn guard_literal_bool(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
) -> Option<bool> {
    let mut visited = HashSet::new();
    guard_literal_bool_expr(module, resolution, expr_id, &mut visited)
}

fn guard_literal_int(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
) -> Option<i64> {
    let mut visited = HashSet::new();
    guard_literal_int_expr(module, resolution, expr_id, &mut visited)
}

fn pattern_literal_bool(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<bool> {
    let source = pattern_literal_source_expr(module, resolution, pattern)?;
    let mut visited = HashSet::new();
    guard_literal_bool_expr(module, resolution, source, &mut visited)
}

fn pattern_literal_int(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<i64> {
    let source = pattern_literal_source_expr(module, resolution, pattern)?;
    let mut visited = HashSet::new();
    guard_literal_int_expr(module, resolution, source, &mut visited)
}

fn pattern_literal_string(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<String> {
    let source = pattern_literal_source_expr(module, resolution, pattern)?;
    let mut visited = HashSet::new();
    guard_literal_string_expr(module, resolution, source, &mut visited)
}

fn pattern_literal_source_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<hir::ExprId> {
    let item_id = local_item_for_value_resolution(module, resolution.pattern_resolution(pattern)?)?;
    let mut visited = HashSet::new();
    guard_literal_source_item(module, resolution, item_id, &mut visited)
}

enum SupportedBoolGuardAnalysis {
    Always,
    Skip,
    Dynamic(hir::ExprId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoolGuardScrutineeRelation {
    Same,
    Negated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuardScalarKind {
    Bool,
    Int,
}

fn supported_bool_guard(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    immutable_place_aliases: &HashMap<mir::LocalId, mir::Place>,
    scrutinee: &Operand,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
) -> Option<SupportedBoolGuardAnalysis> {
    match guard_literal_bool(module, resolution, expr_id) {
        Some(true) => Some(SupportedBoolGuardAnalysis::Always),
        Some(false) => Some(SupportedBoolGuardAnalysis::Skip),
        None => fold_bool_guard_against_scrutinee(
            module,
            resolution,
            signatures,
            body,
            local_types,
            immutable_place_aliases,
            scrutinee,
            arm_pattern,
            expr_id,
        )
        .or_else(|| {
            runtime_bool_guard_supported(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
            )
            .then_some(SupportedBoolGuardAnalysis::Dynamic(expr_id))
        }),
    }
}

fn fold_bool_guard_against_scrutinee(
    module: &hir::Module,
    resolution: &ResolutionMap,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    immutable_place_aliases: &HashMap<mir::LocalId, mir::Place>,
    scrutinee: &Operand,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
) -> Option<SupportedBoolGuardAnalysis> {
    let pattern = supported_bool_match_pattern(module, resolution, arm_pattern)?;
    let relation = bool_guard_relation_to_scrutinee(
        module,
        resolution,
        signatures,
        body,
        local_types,
        immutable_place_aliases,
        scrutinee,
        arm_pattern,
        expr_id,
    )?;
    match (pattern, relation) {
        (SupportedBoolMatchPattern::True, BoolGuardScrutineeRelation::Same)
        | (SupportedBoolMatchPattern::False, BoolGuardScrutineeRelation::Negated) => {
            Some(SupportedBoolGuardAnalysis::Always)
        }
        (SupportedBoolMatchPattern::True, BoolGuardScrutineeRelation::Negated)
        | (SupportedBoolMatchPattern::False, BoolGuardScrutineeRelation::Same) => {
            Some(SupportedBoolGuardAnalysis::Skip)
        }
        (SupportedBoolMatchPattern::CatchAll, _) => None,
    }
}

fn invert_bool_guard_scrutinee_relation(
    relation: BoolGuardScrutineeRelation,
) -> BoolGuardScrutineeRelation {
    match relation {
        BoolGuardScrutineeRelation::Same => BoolGuardScrutineeRelation::Negated,
        BoolGuardScrutineeRelation::Negated => BoolGuardScrutineeRelation::Same,
    }
}

fn bool_guard_relation_from_scrutinee_bool_compare(
    relation: BoolGuardScrutineeRelation,
    op: BinaryOp,
    literal: bool,
) -> Option<BoolGuardScrutineeRelation> {
    match (op, literal) {
        (BinaryOp::EqEq, true) | (BinaryOp::BangEq, false) => Some(relation),
        (BinaryOp::EqEq, false) | (BinaryOp::BangEq, true) => {
            Some(invert_bool_guard_scrutinee_relation(relation))
        }
        _ => None,
    }
}

fn bool_guard_relation_to_scrutinee(
    module: &hir::Module,
    resolution: &ResolutionMap,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    immutable_place_aliases: &HashMap<mir::LocalId, mir::Place>,
    scrutinee: &Operand,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
) -> Option<BoolGuardScrutineeRelation> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Binary {
            left,
            op: op @ (BinaryOp::EqEq | BinaryOp::BangEq),
            right,
        } => {
            if let Some(relation) = bool_guard_relation_to_scrutinee(
                module,
                resolution,
                signatures,
                body,
                local_types,
                immutable_place_aliases,
                scrutinee,
                arm_pattern,
                *left,
            ) {
                let literal = guard_literal_bool(module, resolution, *right)?;
                return bool_guard_relation_from_scrutinee_bool_compare(relation, *op, literal);
            }

            if let Some(relation) = bool_guard_relation_to_scrutinee(
                module,
                resolution,
                signatures,
                body,
                local_types,
                immutable_place_aliases,
                scrutinee,
                arm_pattern,
                *right,
            ) {
                let literal = guard_literal_bool(module, resolution, *left)?;
                return bool_guard_relation_from_scrutinee_bool_compare(relation, *op, literal);
            }

            None
        }
        hir::ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => match bool_guard_relation_to_scrutinee(
            module,
            resolution,
            signatures,
            body,
            local_types,
            immutable_place_aliases,
            scrutinee,
            arm_pattern,
            *expr,
        )? {
            BoolGuardScrutineeRelation::Same => Some(BoolGuardScrutineeRelation::Negated),
            BoolGuardScrutineeRelation::Negated => Some(BoolGuardScrutineeRelation::Same),
        },
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            module.block(*block_id).tail.and_then(|tail| {
                bool_guard_relation_to_scrutinee(
                    module,
                    resolution,
                    signatures,
                    body,
                    local_types,
                    immutable_place_aliases,
                    scrutinee,
                    arm_pattern,
                    tail,
                )
            })
        }
        hir::ExprKind::Question(inner) => bool_guard_relation_to_scrutinee(
            module,
            resolution,
            signatures,
            body,
            local_types,
            immutable_place_aliases,
            scrutinee,
            arm_pattern,
            *inner,
        ),
        _ => {
            let Operand::Place(scrutinee_place) = scrutinee else {
                return None;
            };
            let (guard_place, guard_ty) = guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                Some(arm_pattern),
                expr_id,
            )?;
            let canonical_scrutinee =
                canonicalize_immutable_place_alias(scrutinee_place, immutable_place_aliases);
            let canonical_guard =
                canonicalize_immutable_place_alias(&guard_place, immutable_place_aliases);
            (guard_ty.is_bool() && canonical_guard == canonical_scrutinee)
                .then_some(BoolGuardScrutineeRelation::Same)
        }
    }
}

fn runtime_bool_guard_supported(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
) -> bool {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => runtime_bool_guard_supported(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            *expr,
        ),
        hir::ExprKind::Binary { left, op, right }
            if matches!(op, BinaryOp::AndAnd | BinaryOp::OrOr) =>
        {
            runtime_bool_guard_supported(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *left,
            ) && runtime_bool_guard_supported(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *right,
            )
        }
        hir::ExprKind::Binary { left, op, right } => {
            let Some(left_kind) = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *left,
            ) else {
                return false;
            };
            let Some(right_kind) = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *right,
            ) else {
                return false;
            };

            left_kind == right_kind
                && match left_kind {
                    GuardScalarKind::Bool => matches!(op, BinaryOp::EqEq | BinaryOp::BangEq),
                    GuardScalarKind::Int => matches!(
                        op,
                        BinaryOp::EqEq
                            | BinaryOp::BangEq
                            | BinaryOp::Gt
                            | BinaryOp::GtEq
                            | BinaryOp::Lt
                            | BinaryOp::LtEq
                    ),
                }
        }
        _ => {
            supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
            ) == Some(GuardScalarKind::Bool)
        }
    }
}

fn supported_guard_scalar_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
) -> Option<GuardScalarKind> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Bool(_) => Some(GuardScalarKind::Bool),
        hir::ExprKind::Integer(_) => Some(GuardScalarKind::Int),
        hir::ExprKind::Binary {
            left,
            op: BinaryOp::Assign,
            right,
        } => {
            let (_, target_ty) = guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                Some(arm_pattern),
                *left,
            )?;
            let target_kind = guard_scalar_kind_for_ty(&target_ty)?;
            let value_kind = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *right,
            )?;
            (target_kind == value_kind).then_some(target_kind)
        }
        hir::ExprKind::Binary { left, op, right }
            if matches!(
                op,
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
            ) =>
        {
            let left_kind = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *left,
            )?;
            let right_kind = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *right,
            )?;
            (left_kind == GuardScalarKind::Int && right_kind == GuardScalarKind::Int)
                .then_some(GuardScalarKind::Int)
        }
        hir::ExprKind::Binary {
            op:
                BinaryOp::AndAnd
                | BinaryOp::OrOr
                | BinaryOp::EqEq
                | BinaryOp::BangEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::Lt
                | BinaryOp::LtEq,
            ..
        } => runtime_bool_guard_supported(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            expr_id,
        )
        .then_some(GuardScalarKind::Bool),
        hir::ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => {
            if guard_literal_bool(module, resolution, *expr).is_some()
                || runtime_bool_guard_supported(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    *expr,
                )
            {
                Some(GuardScalarKind::Bool)
            } else {
                None
            }
        }
        hir::ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => supported_guard_scalar_expr(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            *expr,
        )
        .filter(|kind| *kind == GuardScalarKind::Int),
        hir::ExprKind::Unary {
            op: UnaryOp::Await, ..
        } => typeck.expr_ty(expr_id).and_then(guard_scalar_kind_for_ty),
        hir::ExprKind::Call { callee, args } => supported_guard_call_expr(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            *callee,
            args,
        ),
        hir::ExprKind::Name(_) => match resolution.expr_resolution(expr_id) {
            Some(ValueResolution::Local(local_id))
                if pattern_binds_local(module, arm_pattern, *local_id) =>
            {
                let local = mir_local_for_hir_local(body, *local_id)?;
                let ty = local_types.get(&local)?;
                guard_scalar_kind_for_ty(ty)
            }
            Some(ValueResolution::Item(_)) | Some(ValueResolution::Import(_)) => {
                if guard_literal_bool(module, resolution, expr_id).is_some() {
                    Some(GuardScalarKind::Bool)
                } else if guard_literal_int(module, resolution, expr_id).is_some() {
                    Some(GuardScalarKind::Int)
                } else {
                    None
                }
            }
            Some(ValueResolution::Local(_))
            | Some(ValueResolution::Param(_))
            | Some(ValueResolution::ArrayLengthGeneric(_))
            | Some(ValueResolution::SelfValue)
            | Some(ValueResolution::Function(_))
            | None => guard_expr_ty(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                Some(arm_pattern),
                expr_id,
            )
            .and_then(|ty| guard_scalar_kind_for_ty(&ty)),
        },
        hir::ExprKind::Member { .. } | hir::ExprKind::Bracket { .. } => {
            if guard_literal_bool(module, resolution, expr_id).is_some() {
                Some(GuardScalarKind::Bool)
            } else if guard_literal_int(module, resolution, expr_id).is_some() {
                Some(GuardScalarKind::Int)
            } else {
                guard_expr_ty(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    Some(arm_pattern),
                    expr_id,
                )
                .and_then(|ty| guard_scalar_kind_for_ty(&ty))
            }
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            module.block(*block_id).tail.and_then(|tail| {
                supported_guard_scalar_expr(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    tail,
                )
            })
        }
        hir::ExprKind::Question(inner) => supported_guard_scalar_expr(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            *inner,
        ),
        _ => None,
    }
}

fn supported_guard_call_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    callee_expr: hir::ExprId,
    args: &[hir::CallArg],
) -> Option<GuardScalarKind> {
    let (_, return_ty) = supported_guard_call(
        module,
        resolution,
        typeck,
        signatures,
        body,
        local_types,
        arm_pattern,
        callee_expr,
        args,
    )?;
    let return_kind = guard_scalar_kind_for_ty(&return_ty)?;
    Some(return_kind)
}

fn guard_scalar_kind_for_ty(ty: &Ty) -> Option<GuardScalarKind> {
    if ty.is_bool() {
        Some(GuardScalarKind::Bool)
    } else if ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
        Some(GuardScalarKind::Int)
    } else {
        None
    }
}

fn supported_bool_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<SupportedBoolMatchPattern> {
    match pattern_kind(module, pattern) {
        PatternKind::Bool(true) => Some(SupportedBoolMatchPattern::True),
        PatternKind::Bool(false) => Some(SupportedBoolMatchPattern::False),
        PatternKind::Path(_) => pattern_literal_bool(module, resolution, pattern).map(|value| {
            if value {
                SupportedBoolMatchPattern::True
            } else {
                SupportedBoolMatchPattern::False
            }
        }),
        PatternKind::Binding(_) | PatternKind::Wildcard => {
            Some(SupportedBoolMatchPattern::CatchAll)
        }
        _ => None,
    }
}

fn supported_integer_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<String> {
    match pattern_kind(module, pattern) {
        PatternKind::Integer(value) => Some(value.clone()),
        PatternKind::Path(_) => {
            pattern_literal_int(module, resolution, pattern).map(|value| value.to_string())
        }
        _ => None,
    }
}

fn supported_string_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<String> {
    match pattern_kind(module, pattern) {
        PatternKind::String(value) => Some(value.clone()),
        PatternKind::Path(_) => pattern_literal_string(module, resolution, pattern),
        _ => None,
    }
}

fn resolved_enum_variant_index_for_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<usize> {
    let path = match pattern_kind(module, pattern) {
        PatternKind::Path(path)
        | PatternKind::TupleStruct { path, .. }
        | PatternKind::Struct { path, .. } => path,
        _ => return None,
    };
    let item_id = local_item_for_value_resolution(module, resolution.pattern_resolution(pattern)?)?;
    let ItemKind::Enum(enum_decl) = &module.item(item_id).kind else {
        return None;
    };
    let variant_name = path.segments.last()?;
    enum_decl
        .variants
        .iter()
        .position(|variant| variant.name == *variant_name)
}

fn enum_variant_index_for_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    scrutinee_ty: &Ty,
    pattern: hir::PatternId,
) -> Option<usize> {
    let Ty::Item { item_id, .. } = scrutinee_ty else {
        return None;
    };
    let resolved_item_id =
        local_item_for_value_resolution(module, resolution.pattern_resolution(pattern)?)?;
    (resolved_item_id == *item_id)
        .then(|| resolved_enum_variant_index_for_pattern(module, resolution, pattern))
        .flatten()
}

fn supported_cleanup_bool_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<SupportedBoolMatchPattern> {
    match pattern_kind(module, pattern) {
        PatternKind::Bool(true) => Some(SupportedBoolMatchPattern::True),
        PatternKind::Bool(false) => Some(SupportedBoolMatchPattern::False),
        PatternKind::Path(_) => pattern_literal_bool(module, resolution, pattern).map(|value| {
            if value {
                SupportedBoolMatchPattern::True
            } else {
                SupportedBoolMatchPattern::False
            }
        }),
        PatternKind::Binding(_) => Some(SupportedBoolMatchPattern::CatchAll),
        PatternKind::Wildcard => Some(SupportedBoolMatchPattern::CatchAll),
        _ => None,
    }
}

fn supported_cleanup_integer_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<SupportedIntegerMatchPattern> {
    match pattern_kind(module, pattern) {
        PatternKind::Integer(value) => Some(SupportedIntegerMatchPattern::Literal(value.clone())),
        PatternKind::Path(_) => pattern_literal_int(module, resolution, pattern)
            .map(|value| SupportedIntegerMatchPattern::Literal(value.to_string())),
        PatternKind::Binding(_) => Some(SupportedIntegerMatchPattern::CatchAll),
        PatternKind::Wildcard => Some(SupportedIntegerMatchPattern::CatchAll),
        _ => None,
    }
}

fn supported_cleanup_string_match_pattern(
    module: &hir::Module,
    resolution: &ResolutionMap,
    pattern: hir::PatternId,
) -> Option<SupportedStringMatchPattern> {
    match pattern_kind(module, pattern) {
        PatternKind::String(value) => Some(SupportedStringMatchPattern::Literal(value.clone())),
        PatternKind::Path(_) => pattern_literal_string(module, resolution, pattern)
            .map(SupportedStringMatchPattern::Literal),
        PatternKind::Binding(_) => Some(SupportedStringMatchPattern::CatchAll),
        PatternKind::Wildcard => Some(SupportedStringMatchPattern::CatchAll),
        _ => None,
    }
}

fn pattern_binds_local(
    module: &hir::Module,
    pattern: hir::PatternId,
    local_id: hir::LocalId,
) -> bool {
    match pattern_kind(module, pattern) {
        PatternKind::Binding(binding_local) => *binding_local == local_id,
        PatternKind::Tuple(items)
        | PatternKind::Array(items)
        | PatternKind::TupleStruct { items, .. } => items
            .iter()
            .any(|item| pattern_binds_local(module, *item, local_id)),
        PatternKind::Struct { fields, .. } => fields
            .iter()
            .any(|field| pattern_binds_local(module, field.pattern, local_id)),
        PatternKind::Path(_)
        | PatternKind::Integer(_)
        | PatternKind::String(_)
        | PatternKind::Bool(_)
        | PatternKind::NoneLiteral
        | PatternKind::Wildcard => false,
    }
}

fn mir_local_for_hir_local(body: &mir::MirBody, hir_local: hir::LocalId) -> Option<mir::LocalId> {
    body.local_ids()
        .filter(|candidate| {
            matches!(
                body.local(*candidate).origin,
                LocalOrigin::Binding(candidate_local) if candidate_local == hir_local
            )
        })
        .last()
}

fn mir_param_local(body: &mir::MirBody, index: usize) -> Option<mir::LocalId> {
    body.local_ids().find(|candidate| {
        matches!(body.local(*candidate).origin, LocalOrigin::Param { index: candidate_index } if candidate_index == index)
    })
}

fn mir_receiver_local(body: &mir::MirBody) -> Option<mir::LocalId> {
    body.local_ids()
        .find(|candidate| matches!(body.local(*candidate).origin, LocalOrigin::Receiver))
}

fn collect_immutable_place_aliases(
    module: &hir::Module,
    body: &mir::MirBody,
) -> HashMap<mir::LocalId, mir::Place> {
    let mut aliases = HashMap::new();

    for block in body.blocks() {
        for statement_id in &block.statements {
            let statement = body.statement(*statement_id);
            match &statement.kind {
                StatementKind::BindPattern {
                    pattern,
                    source: Operand::Place(source_place),
                    mutable,
                } => {
                    if *mutable {
                        continue;
                    }
                    let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
                        continue;
                    };
                    let Some(binding_local) = mir_local_for_hir_local(body, *local) else {
                        continue;
                    };
                    aliases.insert(binding_local, source_place.clone());
                }
                StatementKind::Assign {
                    place,
                    value: Rvalue::Use(Operand::Place(source_place)),
                } if place.projections.is_empty()
                    && matches!(body.local(place.base).origin, LocalOrigin::Temp { .. }) =>
                {
                    aliases.insert(place.base, source_place.clone());
                }
                _ => {}
            }
        }
    }

    aliases
}

fn canonicalize_immutable_place_alias(
    place: &mir::Place,
    immutable_place_aliases: &HashMap<mir::LocalId, mir::Place>,
) -> mir::Place {
    let mut canonical = place.clone();
    while let Some(source_place) = immutable_place_aliases.get(&canonical.base) {
        let mut projections = source_place.projections.clone();
        projections.extend(canonical.projections);
        canonical = mir::Place {
            base: source_place.base,
            projections,
        };
    }
    canonical
}

fn guard_expr_place_with_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: Option<hir::PatternId>,
    expr_id: hir::ExprId,
) -> Option<(Place, Ty)> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => {
            let (local, ty) =
                guard_expr_place_root(module, resolution, body, local_types, arm_pattern, expr_id)?;
            Some((Place::local(local), ty))
        }
        hir::ExprKind::Member { object, field, .. } => {
            let (mut place, current_ty) = guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                arm_pattern,
                *object,
            )?;
            let output_ty = guard_field_projection_ty(module, resolution, &current_ty, field)?;
            place
                .projections
                .push(mir::ProjectionElem::Field(field.clone()));
            Some((place, output_ty))
        }
        hir::ExprKind::Bracket { target, items } => {
            let (mut place, mut current_ty) = guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                arm_pattern,
                *target,
            )?;
            for item in items {
                let (index_operand, index_ty) = guard_expr_index_operand_with_ty(
                    module,
                    resolution,
                    body,
                    local_types,
                    arm_pattern,
                    *item,
                )?;
                if !index_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
                    return None;
                }
                current_ty = guard_index_projection_ty(&current_ty, &index_operand)?;
                place
                    .projections
                    .push(mir::ProjectionElem::Index(Box::new(index_operand)));
            }
            Some((place, current_ty))
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            module.block(*block_id).tail.and_then(|tail| {
                guard_expr_place_with_ty(module, resolution, body, local_types, arm_pattern, tail)
            })
        }
        hir::ExprKind::Question(inner) => {
            guard_expr_place_with_ty(module, resolution, body, local_types, arm_pattern, *inner)
        }
        _ => None,
    }
}

fn guard_expr_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: Option<hir::PatternId>,
    expr_id: hir::ExprId,
) -> Option<Ty> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => {
            guard_expr_place_root(module, resolution, body, local_types, arm_pattern, expr_id)
                .map(|(_, ty)| ty)
                .or_else(|| guard_expr_item_root_ty(module, resolution, signatures, expr_id))
        }
        hir::ExprKind::Binary {
            left,
            op: BinaryOp::Assign,
            right,
        } => {
            let (_, target_ty) = guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                arm_pattern,
                *left,
            )?;
            let value_ty = guard_expr_ty(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *right,
            )?;
            target_ty.compatible_with(&value_ty).then_some(target_ty)
        }
        hir::ExprKind::Member { object, field, .. } => {
            let current_ty = guard_expr_ty(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *object,
            )?;
            guard_field_projection_ty(module, resolution, &current_ty, field)
        }
        hir::ExprKind::Bracket { target, items } => {
            let mut current_ty = guard_expr_ty(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *target,
            )?;
            for item in items {
                current_ty = guard_index_expr_projection_ty(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    &current_ty,
                    *item,
                )?;
            }
            Some(current_ty)
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            module.block(*block_id).tail.and_then(|tail| {
                guard_expr_ty(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    tail,
                )
            })
        }
        hir::ExprKind::Question(inner) => guard_expr_ty(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            *inner,
        ),
        hir::ExprKind::Unary {
            op: UnaryOp::Await, ..
        } => typeck.expr_ty(expr_id).cloned(),
        hir::ExprKind::Tuple(_) | hir::ExprKind::Array(_) | hir::ExprKind::StructLiteral { .. } => {
            typeck.expr_ty(expr_id).cloned()
        }
        hir::ExprKind::Call { callee, args } => supported_guard_call(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern?,
            *callee,
            args,
        )
        .map(|(_, return_ty)| return_ty),
        _ => None,
    }
}

fn guard_expr_item_root_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    expr_id: hir::ExprId,
) -> Option<Ty> {
    match resolution.expr_resolution(expr_id)? {
        ValueResolution::Function(function) => {
            signatures.get(function).map(callable_ty_from_signature)
        }
        ValueResolution::Item(item_id) => match &module.item(*item_id).kind {
            ItemKind::Function(_) => signatures
                .get(&FunctionRef::Item(*item_id))
                .map(callable_ty_from_signature),
            _ => const_or_static_item_type(module, resolution, *item_id),
        },
        ValueResolution::Import(import_binding) => {
            let item_id = local_item_for_import_binding(module, import_binding)?;
            match &module.item(item_id).kind {
                ItemKind::Function(_) => signatures
                    .get(&FunctionRef::Item(item_id))
                    .map(callable_ty_from_signature),
                _ => const_or_static_item_type(module, resolution, item_id),
            }
        }
        ValueResolution::Local(_)
        | ValueResolution::Param(_)
        | ValueResolution::ArrayLengthGeneric(_)
        | ValueResolution::SelfValue => None,
    }
}

fn guard_expr_place_root(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: Option<hir::PatternId>,
    expr_id: hir::ExprId,
) -> Option<(mir::LocalId, Ty)> {
    match resolution.expr_resolution(expr_id)? {
        ValueResolution::Local(local_id) => {
            if arm_pattern.is_some_and(|pattern| pattern_binds_local(module, pattern, *local_id)) {
                let local = mir_local_for_hir_local(body, *local_id)?;
                let ty = local_types.get(&local)?.clone();
                return Some((local, ty));
            }
            let local = mir_local_for_hir_local(body, *local_id)?;
            let ty = local_types.get(&local)?.clone();
            Some((local, ty))
        }
        ValueResolution::Param(binding) => {
            let local = mir_param_local(body, binding.index)?;
            let ty = local_types.get(&local)?.clone();
            Some((local, ty))
        }
        ValueResolution::SelfValue => {
            let local = mir_receiver_local(body)?;
            let ty = local_types.get(&local)?.clone();
            Some((local, ty))
        }
        ValueResolution::Item(_)
        | ValueResolution::Import(_)
        | ValueResolution::Function(_)
        | ValueResolution::ArrayLengthGeneric(_) => None,
    }
}

fn guard_index_expr_projection_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: Option<hir::PatternId>,
    current_ty: &Ty,
    expr_id: hir::ExprId,
) -> Option<Ty> {
    match current_ty {
        Ty::Array { element, .. } => {
            let arm_pattern = arm_pattern?;
            (supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
            ) == Some(GuardScalarKind::Int))
            .then_some(element.as_ref().clone())
        }
        Ty::Tuple(items) => {
            let index = guard_literal_int(module, resolution, expr_id)?;
            if index < 0 {
                return None;
            }
            items.get(index as usize).cloned()
        }
        _ => None,
    }
}

fn guard_expr_index_operand_with_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: Option<hir::PatternId>,
    expr_id: hir::ExprId,
) -> Option<(Operand, Ty)> {
    if let Some(value) = guard_literal_int(module, resolution, expr_id) {
        if value < 0 {
            return None;
        }
        return Some((
            Operand::Constant(Constant::Integer(value.to_string())),
            Ty::Builtin(BuiltinType::Int),
        ));
    }

    match &module.expr(expr_id).kind {
        hir::ExprKind::Integer(value) => Some((
            Operand::Constant(Constant::Integer(value.clone())),
            Ty::Builtin(BuiltinType::Int),
        )),
        hir::ExprKind::Name(_) => match resolution.expr_resolution(expr_id) {
            Some(ValueResolution::Item(_)) | Some(ValueResolution::Import(_)) => {
                let value = guard_literal_int(module, resolution, expr_id)?;
                Some((
                    Operand::Constant(Constant::Integer(value.to_string())),
                    Ty::Builtin(BuiltinType::Int),
                ))
            }
            Some(ValueResolution::Local(_))
            | Some(ValueResolution::Param(_))
            | Some(ValueResolution::ArrayLengthGeneric(_))
            | Some(ValueResolution::SelfValue)
            | Some(ValueResolution::Function(_))
            | None => guard_expr_place_with_ty(
                module,
                resolution,
                body,
                local_types,
                arm_pattern,
                expr_id,
            )
            .map(|(place, ty)| (Operand::Place(place), ty)),
        },
        hir::ExprKind::Member { .. }
        | hir::ExprKind::Bracket { .. }
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Unsafe(_)
        | hir::ExprKind::Question(_) => {
            guard_expr_place_with_ty(module, resolution, body, local_types, arm_pattern, expr_id)
                .map(|(place, ty)| (Operand::Place(place), ty))
        }
        _ => None,
    }
}

fn guard_field_projection_ty(
    module: &hir::Module,
    resolution: &ResolutionMap,
    current_ty: &Ty,
    field: &str,
) -> Option<Ty> {
    let Ty::Item { item_id, args, .. } = current_ty else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }
    let item = module.item(*item_id);
    let ItemKind::Struct(struct_decl) = &item.kind else {
        return None;
    };
    if !struct_decl.generics.is_empty() {
        return None;
    }
    struct_decl
        .fields
        .iter()
        .find(|candidate| candidate.name == field)
        .map(|field| lower_type(module, resolution, field.ty))
}

fn guard_index_projection_ty(current_ty: &Ty, index: &Operand) -> Option<Ty> {
    match current_ty {
        Ty::Array { element, .. } => Some(element.as_ref().clone()),
        Ty::Tuple(items) => {
            let Operand::Constant(Constant::Integer(raw)) = index else {
                return None;
            };
            let index = ql_ast::parse_usize_literal(raw)?;
            items.get(index).cloned()
        }
        _ => None,
    }
}

fn guard_literal_bool_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<bool> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Bool(value) => Some(*value),
        hir::ExprKind::Unary {
            op: UnaryOp::Not,
            expr,
        } => guard_literal_bool_expr(module, resolution, *expr, visited).map(|value| !value),
        hir::ExprKind::Binary { left, op, right } => {
            if let (Some(left), Some(right)) = (
                guard_literal_bool_expr(module, resolution, *left, visited),
                guard_literal_bool_expr(module, resolution, *right, visited),
            ) {
                match op {
                    BinaryOp::OrOr => Some(left || right),
                    BinaryOp::AndAnd => Some(left && right),
                    BinaryOp::EqEq => Some(left == right),
                    BinaryOp::BangEq => Some(left != right),
                    _ => None,
                }
            } else {
                let left = guard_literal_int_expr(module, resolution, *left, visited)?;
                let right = guard_literal_int_expr(module, resolution, *right, visited)?;
                match op {
                    BinaryOp::EqEq => Some(left == right),
                    BinaryOp::BangEq => Some(left != right),
                    BinaryOp::Gt => Some(left > right),
                    BinaryOp::GtEq => Some(left >= right),
                    BinaryOp::Lt => Some(left < right),
                    BinaryOp::LtEq => Some(left <= right),
                    _ => None,
                }
            }
        }
        hir::ExprKind::Name(_)
        | hir::ExprKind::Member { .. }
        | hir::ExprKind::Bracket { .. }
        | hir::ExprKind::If { .. }
        | hir::ExprKind::Match { .. }
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Unsafe(_)
        | hir::ExprKind::Question(_) => {
            let source = guard_literal_source_expr(module, resolution, expr_id, visited)?;
            if source == expr_id {
                None
            } else {
                guard_literal_bool_expr(module, resolution, source, visited)
            }
        }
        _ => None,
    }
}

fn guard_literal_int_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<i64> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Integer(value) => ql_ast::parse_i64_literal(value),
        hir::ExprKind::Unary {
            op: UnaryOp::Neg,
            expr,
        } => guard_literal_int_expr(module, resolution, *expr, visited)
            .and_then(|value| value.checked_neg()),
        hir::ExprKind::Binary { left, op, right } => {
            let left = guard_literal_int_expr(module, resolution, *left, visited)?;
            let right = guard_literal_int_expr(module, resolution, *right, visited)?;
            match op {
                BinaryOp::Add => left.checked_add(right),
                BinaryOp::Sub => left.checked_sub(right),
                BinaryOp::Mul => left.checked_mul(right),
                BinaryOp::Div => left.checked_div(right),
                BinaryOp::Rem => left.checked_rem(right),
                _ => None,
            }
        }
        hir::ExprKind::Name(_)
        | hir::ExprKind::Member { .. }
        | hir::ExprKind::Bracket { .. }
        | hir::ExprKind::If { .. }
        | hir::ExprKind::Match { .. }
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Unsafe(_)
        | hir::ExprKind::Question(_) => {
            let source = guard_literal_source_expr(module, resolution, expr_id, visited)?;
            if source == expr_id {
                None
            } else {
                guard_literal_int_expr(module, resolution, source, visited)
            }
        }
        _ => None,
    }
}

fn guard_literal_string_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<String> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::String { value, .. } => Some(value.clone()),
        hir::ExprKind::Name(_)
        | hir::ExprKind::Member { .. }
        | hir::ExprKind::Bracket { .. }
        | hir::ExprKind::If { .. }
        | hir::ExprKind::Match { .. }
        | hir::ExprKind::Block(_)
        | hir::ExprKind::Unsafe(_)
        | hir::ExprKind::Question(_) => {
            let source = guard_literal_source_expr(module, resolution, expr_id, visited)?;
            if source == expr_id {
                None
            } else {
                guard_literal_string_expr(module, resolution, source, visited)
            }
        }
        _ => None,
    }
}

fn guard_literal_source_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<hir::ExprId> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Bool(_)
        | hir::ExprKind::Integer(_)
        | hir::ExprKind::String { .. }
        | hir::ExprKind::Unary {
            op: UnaryOp::Not, ..
        }
        | hir::ExprKind::Unary {
            op: UnaryOp::Neg, ..
        }
        | hir::ExprKind::Binary {
            op:
                BinaryOp::AndAnd
                | BinaryOp::OrOr
                | BinaryOp::EqEq
                | BinaryOp::BangEq
                | BinaryOp::Gt
                | BinaryOp::GtEq
                | BinaryOp::Lt
                | BinaryOp::LtEq,
            ..
        }
        | hir::ExprKind::Binary {
            op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem,
            ..
        }
        | hir::ExprKind::Tuple(_)
        | hir::ExprKind::Array(_)
        | hir::ExprKind::StructLiteral { .. } => Some(expr_id),
        hir::ExprKind::Name(_) => resolution
            .expr_resolution(expr_id)
            .and_then(|resolution| local_item_for_value_resolution(module, resolution))
            .and_then(|item_id| guard_literal_source_item(module, resolution, item_id, visited)),
        hir::ExprKind::Member { object, field, .. } => {
            let object = guard_literal_source_expr(module, resolution, *object, visited)?;
            let hir::ExprKind::StructLiteral { fields, .. } = &module.expr(object).kind else {
                return None;
            };
            let value = fields
                .iter()
                .find(|candidate| candidate.name == *field)?
                .value;
            guard_literal_source_expr(module, resolution, value, visited).or(Some(value))
        }
        hir::ExprKind::Bracket { target, items } if items.len() == 1 => {
            let index = guard_literal_int_expr(module, resolution, items[0], visited)?;
            if index < 0 {
                return None;
            }
            let index = index as usize;
            let target = guard_literal_source_expr(module, resolution, *target, visited)?;
            let value = match &module.expr(target).kind {
                hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => {
                    items.get(index).copied()
                }
                _ => None,
            }?;
            guard_literal_source_expr(module, resolution, value, visited).or(Some(value))
        }
        hir::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => guard_literal_if_source_expr(
            module,
            resolution,
            *condition,
            *then_branch,
            *else_branch,
            visited,
        ),
        hir::ExprKind::Match { value, arms } => {
            guard_literal_match_source_expr(module, resolution, *value, arms, visited)
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => module
            .block(*block_id)
            .tail
            .and_then(|tail| guard_literal_source_expr(module, resolution, tail, visited)),
        hir::ExprKind::Question(inner) => {
            guard_literal_source_expr(module, resolution, *inner, visited)
        }
        _ => None,
    }
}

fn guard_literal_if_source_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    condition: hir::ExprId,
    then_branch: hir::BlockId,
    else_branch: Option<hir::ExprId>,
    visited: &mut HashSet<ItemId>,
) -> Option<hir::ExprId> {
    if guard_literal_bool_expr(module, resolution, condition, visited)? {
        let tail = module.block(then_branch).tail?;
        guard_literal_source_expr(module, resolution, tail, visited).or(Some(tail))
    } else {
        let other = else_branch?;
        guard_literal_source_expr(module, resolution, other, visited).or(Some(other))
    }
}

fn guard_literal_match_source_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    value: hir::ExprId,
    arms: &[hir::MatchArm],
    visited: &mut HashSet<ItemId>,
) -> Option<hir::ExprId> {
    if let Some(scrutinee) = guard_literal_bool_expr(module, resolution, value, visited) {
        for arm in arms {
            let matches = match pattern_kind(module, arm.pattern) {
                PatternKind::Bool(pattern) => *pattern == scrutinee,
                PatternKind::Path(_) => {
                    pattern_literal_bool(module, resolution, arm.pattern)? == scrutinee
                }
                PatternKind::Binding(_) | PatternKind::Wildcard => true,
                _ => return None,
            };
            if !matches {
                continue;
            }
            let guard = arm
                .guard
                .map(|guard| guard_literal_bool_expr(module, resolution, guard, visited))
                .unwrap_or(Some(true))?;
            if guard {
                return guard_literal_source_expr(module, resolution, arm.body, visited)
                    .or(Some(arm.body));
            }
        }
        return None;
    }

    if let Some(scrutinee) = guard_literal_int_expr(module, resolution, value, visited) {
        for arm in arms {
            let matches = match pattern_kind(module, arm.pattern) {
                PatternKind::Integer(pattern) => ql_ast::parse_i64_literal(pattern)? == scrutinee,
                PatternKind::Path(_) => {
                    pattern_literal_int(module, resolution, arm.pattern)? == scrutinee
                }
                PatternKind::Binding(_) | PatternKind::Wildcard => true,
                _ => return None,
            };
            if !matches {
                continue;
            }
            let guard = arm
                .guard
                .map(|guard| guard_literal_bool_expr(module, resolution, guard, visited))
                .unwrap_or(Some(true))?;
            if guard {
                return guard_literal_source_expr(module, resolution, arm.body, visited)
                    .or(Some(arm.body));
            }
        }
        return None;
    }

    if let Some(scrutinee) = guard_literal_string_expr(module, resolution, value, visited) {
        for arm in arms {
            let matches = match pattern_kind(module, arm.pattern) {
                PatternKind::String(pattern) => pattern == &scrutinee,
                PatternKind::Path(_) => pattern_literal_string(module, resolution, arm.pattern)
                    .as_ref()
                    .is_some_and(|value| value == &scrutinee),
                PatternKind::Binding(_) | PatternKind::Wildcard => true,
                _ => return None,
            };
            if !matches {
                continue;
            }
            let guard = arm
                .guard
                .map(|guard| guard_literal_bool_expr(module, resolution, guard, visited))
                .unwrap_or(Some(true))?;
            if guard {
                return guard_literal_source_expr(module, resolution, arm.body, visited)
                    .or(Some(arm.body));
            }
        }
        return None;
    }

    for arm in arms {
        match pattern_kind(module, arm.pattern) {
            PatternKind::Binding(_) | PatternKind::Wildcard => {
                let guard = arm
                    .guard
                    .map(|guard| guard_literal_bool_expr(module, resolution, guard, visited))
                    .unwrap_or(Some(true))?;
                if guard {
                    return guard_literal_source_expr(module, resolution, arm.body, visited)
                        .or(Some(arm.body));
                }
            }
            _ => return None,
        }
    }

    None
}

fn guard_literal_source_item(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
    visited: &mut HashSet<ItemId>,
) -> Option<hir::ExprId> {
    if !visited.insert(item_id) {
        return None;
    }

    let result = match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => {
            guard_literal_source_expr(module, resolution, global.value, visited)
        }
        _ => None,
    };

    visited.remove(&item_id);
    result
}

fn runtime_task_backed_item_value(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
) -> Option<(hir::ExprId, Ty)> {
    let ty = const_or_static_item_type(module, resolution, item_id)?;
    if !ty_contains_task_handles_in_runtime_item(module, resolution, &ty) {
        return None;
    }
    let value = match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => global.value,
        _ => return None,
    };
    Some((value, ty))
}

fn runtime_task_iterable_item_value(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
) -> Option<(hir::ExprId, Ty)> {
    let (value, ty) = runtime_task_backed_item_value(module, resolution, item_id)?;
    cleanup_for_iterable_shape(&ty)?;
    Some((value, ty))
}

fn task_backed_item_root_expr(
    module: &hir::Module,
    resolution_map: &ResolutionMap,
    expr_id: hir::ExprId,
) -> Option<ItemId> {
    let hir::ExprKind::Name(_) = &module.expr(expr_id).kind else {
        return None;
    };
    let resolution = resolution_map.expr_resolution(expr_id)?;
    let item_id = local_item_for_value_resolution(module, resolution)?;
    runtime_task_backed_item_value(module, resolution_map, item_id).map(|_| item_id)
}

fn task_iterable_item_root_expr(
    module: &hir::Module,
    resolution_map: &ResolutionMap,
    expr_id: hir::ExprId,
) -> Option<ItemId> {
    let item_id = task_backed_item_root_expr(module, resolution_map, expr_id)?;
    runtime_task_iterable_item_value(module, resolution_map, item_id).map(|_| item_id)
}

fn ty_contains_task_handles_in_runtime_item(
    module: &hir::Module,
    resolution: &ResolutionMap,
    ty: &Ty,
) -> bool {
    match ty {
        Ty::TaskHandle(_) => true,
        Ty::Array { element, .. } | Ty::Pointer { inner: element, .. } => {
            ty_contains_task_handles_in_runtime_item(module, resolution, element)
        }
        Ty::Tuple(items) => items
            .iter()
            .any(|item| ty_contains_task_handles_in_runtime_item(module, resolution, item)),
        Ty::Item { item_id, args, .. } => {
            if !args.is_empty() {
                return false;
            }
            let item = module.item(*item_id);
            let ItemKind::Struct(struct_decl) = &item.kind else {
                return false;
            };
            if !struct_decl.generics.is_empty() {
                return false;
            }
            struct_decl.fields.iter().any(|field| {
                let field_ty = lower_type(module, resolution, field.ty);
                ty_contains_task_handles_in_runtime_item(module, resolution, &field_ty)
            })
        }
        _ => false,
    }
}

fn local_item_for_value_resolution(
    module: &hir::Module,
    resolution: &ValueResolution,
) -> Option<ItemId> {
    match resolution {
        ValueResolution::Item(item_id) => Some(*item_id),
        ValueResolution::Import(import_binding) => {
            local_item_for_import_binding(module, import_binding)
        }
        _ => None,
    }
}

fn local_item_for_import_path(module: &hir::Module, path: &ql_ast::Path) -> Option<ItemId> {
    let Some(name) = path.segments.last() else {
        return None;
    };

    module
        .items
        .iter()
        .copied()
        .find(|item_id| match &module.item(*item_id).kind {
            ItemKind::Function(function) => function.name == *name,
            ItemKind::Const(global) | ItemKind::Static(global) => global.name == *name,
            ItemKind::Struct(struct_decl) => struct_decl.name == *name,
            ItemKind::Enum(enum_decl) => enum_decl.name == *name,
            ItemKind::Trait(trait_decl) => trait_decl.name == *name,
            ItemKind::TypeAlias(alias) => alias.name == *name,
            ItemKind::Impl(_) | ItemKind::Extend(_) | ItemKind::ExternBlock(_) => false,
        })
}

fn local_item_for_import_binding(
    module: &hir::Module,
    import_binding: &ImportBinding,
) -> Option<ItemId> {
    let Some(name) = import_binding.path.segments.last() else {
        return None;
    };

    module
        .items
        .iter()
        .copied()
        .find(|item_id| match &module.item(*item_id).kind {
            ItemKind::Function(function) => function.name == *name,
            ItemKind::Const(global) | ItemKind::Static(global) => global.name == *name,
            ItemKind::Struct(struct_decl) => struct_decl.name == *name,
            ItemKind::Enum(enum_decl) => enum_decl.name == *name,
            ItemKind::Trait(trait_decl) => trait_decl.name == *name,
            ItemKind::TypeAlias(alias) => alias.name == *name,
            ItemKind::Impl(_) | ItemKind::Extend(_) | ItemKind::ExternBlock(_) => false,
        })
}

fn const_or_static_callable_function_ref(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
    visited: &mut HashSet<ItemId>,
) -> Option<FunctionRef> {
    if !visited.insert(item_id) {
        return None;
    }

    let result = match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => {
            const_expr_sync_function_ref(module, resolution, global.value, visited)
        }
        _ => None,
    };

    visited.remove(&item_id);
    result
}

fn const_or_static_callable_closure_target(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
    visited: &mut HashSet<ItemId>,
) -> Option<(ItemId, hir::ExprId)> {
    if !visited.insert(item_id) {
        return None;
    }

    let result = match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => {
            const_expr_sync_closure_target(module, resolution, item_id, global.value, visited)
        }
        _ => None,
    };

    visited.remove(&item_id);
    result
}

fn const_expr_sync_function_ref(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<FunctionRef> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => match resolution.expr_resolution(expr_id)? {
            ValueResolution::Function(function_ref) => Some(*function_ref),
            ValueResolution::Local(_) | ValueResolution::Param(_) | ValueResolution::SelfValue => {
                None
            }
            value_resolution => {
                let item_id = local_item_for_value_resolution(module, value_resolution)?;
                match &module.item(item_id).kind {
                    ItemKind::Function(_) => Some(FunctionRef::Item(item_id)),
                    ItemKind::Const(_) | ItemKind::Static(_) => {
                        const_or_static_callable_function_ref(module, resolution, item_id, visited)
                    }
                    _ => None,
                }
            }
        },
        hir::ExprKind::Block(block) | hir::ExprKind::Unsafe(block) => {
            let tail = module.block(*block).tail?;
            const_expr_sync_function_ref(module, resolution, tail, visited)
        }
        hir::ExprKind::Question(inner) => {
            const_expr_sync_function_ref(module, resolution, *inner, visited)
        }
        _ => None,
    }
}

fn const_expr_sync_closure_target(
    module: &hir::Module,
    resolution: &ResolutionMap,
    owner_item_id: ItemId,
    expr_id: hir::ExprId,
    visited: &mut HashSet<ItemId>,
) -> Option<(ItemId, hir::ExprId)> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Closure { .. } => Some((owner_item_id, expr_id)),
        hir::ExprKind::Name(_) => match resolution.expr_resolution(expr_id)? {
            ValueResolution::Function(_)
            | ValueResolution::Local(_)
            | ValueResolution::Param(_)
            | ValueResolution::SelfValue => None,
            value_resolution => {
                let item_id = local_item_for_value_resolution(module, value_resolution)?;
                match &module.item(item_id).kind {
                    ItemKind::Const(_) | ItemKind::Static(_) => {
                        const_or_static_callable_closure_target(
                            module, resolution, item_id, visited,
                        )
                    }
                    _ => None,
                }
            }
        },
        hir::ExprKind::Block(block) | hir::ExprKind::Unsafe(block) => {
            let tail = module.block(*block).tail?;
            const_expr_sync_closure_target(module, resolution, owner_item_id, tail, visited)
        }
        hir::ExprKind::Question(inner) => {
            const_expr_sync_closure_target(module, resolution, owner_item_id, *inner, visited)
        }
        _ => None,
    }
}

fn cleanup_binding_value_for_local(
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    local: hir::LocalId,
) -> Option<CleanupCapturingClosureBindingValue> {
    cleanup_aliases
        .iter()
        .rev()
        .find(|binding| binding.local == local)
        .map(|binding| binding.value.clone())
}

fn cleanup_bound_capturing_closure_value_for_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<CleanupCapturingClosureBindingValue> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => {
            let ValueResolution::Local(local_id) = resolution.expr_resolution(expr_id)? else {
                return None;
            };
            cleanup_binding_value_for_local(cleanup_aliases, *local_id)
        }
        hir::ExprKind::Question(inner) => cleanup_bound_capturing_closure_value_for_expr(
            module,
            resolution,
            cleanup_aliases,
            *inner,
        ),
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            let block = module.block(*block_id);
            if !block.statements.is_empty() {
                return None;
            }
            cleanup_bound_capturing_closure_value_for_expr(
                module,
                resolution,
                cleanup_aliases,
                block.tail?,
            )
        }
        _ => None,
    }
}

fn direct_local_capturing_closure_for_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<mir::ClosureId> {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => {
            let ValueResolution::Local(local_id) = resolution.expr_resolution(expr_id)? else {
                return None;
            };
            if let Some(closure_id) = cleanup_binding_value_for_local(cleanup_aliases, *local_id)
                .and_then(|value| value.direct_closure_id())
            {
                return Some(closure_id);
            }
            let local = mir_local_for_hir_local(body, *local_id)?;
            supported.get(&local).copied()
        }
        hir::ExprKind::Question(inner) => direct_local_capturing_closure_for_expr(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            *inner,
        ),
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            let block = module.block(*block_id);
            if !block.statements.is_empty() {
                return None;
            }
            direct_local_capturing_closure_for_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                block.tail?,
            )
        }
        hir::ExprKind::If {
            then_branch,
            else_branch: Some(other),
            ..
        } => {
            let block = module.block(*then_branch);
            if !block.statements.is_empty() {
                return None;
            }
            let then_closure = direct_local_capturing_closure_for_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                block.tail?,
            )?;
            let other_closure = direct_local_capturing_closure_for_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *other,
            )?;
            (then_closure == other_closure).then_some(then_closure)
        }
        hir::ExprKind::Match { arms, .. } => {
            let mut closure_id = None;
            for arm in arms {
                let current = direct_local_capturing_closure_for_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    cleanup_aliases,
                    arm.body,
                )?;
                match closure_id {
                    Some(existing) if existing != current => return None,
                    Some(_) => {}
                    None => closure_id = Some(current),
                }
            }
            closure_id
        }
        _ => None,
    }
}

fn supported_direct_local_capturing_closure_locals_for_guard(
    module: &hir::Module,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
) -> HashMap<mir::LocalId, mir::ClosureId> {
    let mut staged = HashMap::new();
    let mut supported = HashMap::new();
    let mut closure_spans = HashMap::new();

    for statement in body.statements() {
        let StatementKind::Assign { place, value } = &statement.kind else {
            continue;
        };
        if !place.projections.is_empty() {
            continue;
        }
        let Rvalue::Closure { closure } = value else {
            continue;
        };
        let closure_decl = body.closure(*closure);
        if closure_decl.capture_binding_locals.is_empty() {
            continue;
        }

        let closure_supported = closure_decl.captures.iter().all(|capture| {
            local_types
                .get(&capture.local)
                .is_some_and(is_supported_capture_ty)
        });
        if !closure_supported || staged.contains_key(&place.base) {
            continue;
        }
        staged.insert(place.base, *closure);
        closure_spans.insert(place.base, closure_decl.span);
    }

    for statement in body.statements() {
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                if !place.projections.is_empty()
                    || !matches!(body.local(place.base).origin, LocalOrigin::Temp { .. })
                {
                    continue;
                }
                let Some((closure_id, closure_span)) =
                    direct_local_capturing_closure_assignment_source(
                        value,
                        &staged,
                        &supported,
                        &closure_spans,
                    )
                else {
                    continue;
                };
                match supported.get(&place.base).copied() {
                    Some(current) if current != closure_id => {}
                    Some(_) => {}
                    None => {
                        supported.insert(place.base, closure_id);
                        closure_spans.insert(place.base, closure_span);
                    }
                }
            }
            StatementKind::BindPattern {
                pattern, source, ..
            } => {
                let Operand::Place(place) = source else {
                    continue;
                };
                if !place.projections.is_empty() {
                    continue;
                }
                let Some(closure_id) = staged
                    .get(&place.base)
                    .copied()
                    .or_else(|| supported.get(&place.base).copied())
                else {
                    continue;
                };
                let Some(closure_span) = closure_spans.get(&place.base).copied() else {
                    continue;
                };
                let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
                    continue;
                };
                let Some(binding_local) = mir_local_for_hir_local(body, *local) else {
                    continue;
                };
                supported.entry(binding_local).or_insert(closure_id);
                closure_spans.entry(binding_local).or_insert(closure_span);
            }
            StatementKind::Eval { .. }
            | StatementKind::RegisterCleanup { .. }
            | StatementKind::RunCleanup { .. }
            | StatementKind::StorageLive { .. }
            | StatementKind::StorageDead { .. } => {}
        }
    }

    supported
}

fn supported_direct_local_capturing_closure_callee_closure(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<mir::ClosureId> {
    if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    ) {
        return Some(expr.closure_id());
    }

    if let Some(closure_id) = direct_local_capturing_closure_for_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    ) {
        return Some(closure_id);
    }

    match &module.expr(expr_id).kind {
        hir::ExprKind::Question(inner) => supported_direct_local_capturing_closure_callee_closure(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            *inner,
        ),
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            supported_direct_local_capturing_closure_callee_block_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *block_id,
            )
        }
        hir::ExprKind::If {
            then_branch,
            else_branch: Some(other),
            ..
        } => {
            let then_closure = supported_direct_local_capturing_closure_callee_block_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *then_branch,
            )?;
            let other_closure = supported_direct_local_capturing_closure_callee_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *other,
            )?;
            (then_closure == other_closure).then_some(then_closure)
        }
        hir::ExprKind::Match { arms, .. } => {
            let mut closure_id = None;
            for arm in arms {
                let current = supported_direct_local_capturing_closure_callee_closure(
                    module,
                    resolution,
                    body,
                    supported,
                    cleanup_aliases,
                    arm.body,
                )?;
                match closure_id {
                    Some(existing) if existing != current => return None,
                    Some(_) => {}
                    None => closure_id = Some(current),
                }
            }
            closure_id
        }
        _ => None,
    }
}

fn supported_direct_local_capturing_closure_callee_block_closure(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    block_id: hir::BlockId,
) -> Option<mir::ClosureId> {
    let block = module.block(block_id);
    let mut scoped_aliases = cleanup_aliases.to_vec();
    let mut block_locals = HashSet::new();
    for statement_id in &block.statements {
        match &module.stmt(*statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
                    return None;
                };
                if let Some(binding_value) = cleanup_bound_capturing_closure_value_for_expr(
                    module,
                    resolution,
                    &scoped_aliases,
                    *value,
                ) {
                    if binding_value.direct_closure_id().is_none() {
                        return None;
                    }
                    block_locals.insert(*local);
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                    continue;
                }
                let mut assignment_binding = None;
                let closure_id = if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    if let CleanupDirectCapturingClosureExpr::Assignment(binding) = expr {
                        assignment_binding = Some(binding);
                    }
                    expr.closure_id()
                } else if let Some(closure_id) =
                    supported_direct_local_capturing_closure_callee_closure(
                        module,
                        resolution,
                        body,
                        supported,
                        &scoped_aliases,
                        *value,
                    )
                {
                    closure_id
                } else if let Some(closure_id) = direct_local_capturing_closure_for_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    closure_id
                } else {
                    return None;
                };
                if let Some(binding) = assignment_binding {
                    if !block_locals.contains(&binding.local) {
                        return None;
                    }
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: binding.local,
                        value: CleanupCapturingClosureBindingValue::Direct(binding.closure_id),
                    });
                }
                block_locals.insert(*local);
                scoped_aliases.push(CleanupCapturingClosureBinding {
                    local: *local,
                    value: CleanupCapturingClosureBindingValue::Direct(closure_id),
                });
            }
            hir::StmtKind::Expr { expr, .. } => {
                let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &module.expr(*expr).kind
                else {
                    return None;
                };
                let binding = cleanup_capturing_closure_assignment(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *left,
                    *right,
                )?;
                if !block_locals.contains(&binding.local) {
                    return None;
                }
                scoped_aliases.push(CleanupCapturingClosureBinding {
                    local: binding.local,
                    value: CleanupCapturingClosureBindingValue::Direct(binding.closure_id),
                });
            }
            _ => return None,
        }
    }

    supported_direct_local_capturing_closure_callee_closure(
        module,
        resolution,
        body,
        supported,
        &scoped_aliases,
        block.tail?,
    )
}

fn direct_local_capturing_closure_assignment_source(
    value: &Rvalue,
    staged: &HashMap<mir::LocalId, mir::ClosureId>,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    closure_spans: &HashMap<mir::LocalId, Span>,
) -> Option<(mir::ClosureId, Span)> {
    let Rvalue::Use(Operand::Place(source)) = value else {
        return None;
    };
    if !source.projections.is_empty() {
        return None;
    }
    let closure_id = staged
        .get(&source.base)
        .copied()
        .or_else(|| supported.get(&source.base).copied())?;
    let closure_span = *closure_spans
        .get(&source.base)
        .expect("capturing closure local should preserve its source span");
    Some((closure_id, closure_span))
}

fn control_flow_capturing_closure_block_assignment(
    body: &mir::MirBody,
    block_id: mir::BasicBlockId,
    staged: &HashMap<mir::LocalId, mir::ClosureId>,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    closure_spans: &HashMap<mir::LocalId, Span>,
) -> Option<(mir::BasicBlockId, mir::LocalId, mir::ClosureId)> {
    let block = body.block(block_id);
    let TerminatorKind::Goto { target } = &block.terminator.kind else {
        return None;
    };

    let mut assignment = None;
    for statement_id in &block.statements {
        let statement = body.statement(*statement_id);
        match &statement.kind {
            StatementKind::Assign { place, value } => {
                if !place.projections.is_empty() {
                    return None;
                }
                let (closure_id, _) = direct_local_capturing_closure_assignment_source(
                    value,
                    staged,
                    supported,
                    closure_spans,
                )?;
                assignment = Some((place.base, closure_id));
            }
            StatementKind::BindPattern { .. }
            | StatementKind::StorageLive { .. }
            | StatementKind::StorageDead { .. } => {}
            StatementKind::Eval { .. }
            | StatementKind::RegisterCleanup { .. }
            | StatementKind::RunCleanup { .. } => return None,
        }
    }

    let (local, closure_id) = assignment?;
    Some((*target, local, closure_id))
}

fn ordinary_control_flow_capturing_closure_predecessors(
    body: &mir::MirBody,
) -> HashMap<mir::BasicBlockId, Vec<mir::BasicBlockId>> {
    let mut predecessors = body
        .block_ids()
        .map(|block_id| (block_id, Vec::new()))
        .collect::<HashMap<_, _>>();

    for block_id in body.block_ids() {
        for successor in ordinary_control_flow_capturing_closure_successors(body, block_id) {
            predecessors.entry(successor).or_default().push(block_id);
        }
    }

    predecessors
}

fn ordinary_control_flow_capturing_closure_successors(
    body: &mir::MirBody,
    block_id: mir::BasicBlockId,
) -> Vec<mir::BasicBlockId> {
    match &body.block(block_id).terminator.kind {
        TerminatorKind::Goto { target } => vec![*target],
        TerminatorKind::Branch {
            then_target,
            else_target,
            ..
        } => vec![*then_target, *else_target],
        TerminatorKind::Match {
            arms, else_target, ..
        } => {
            let mut successors = arms.iter().map(|arm| arm.target).collect::<Vec<_>>();
            successors.push(*else_target);
            successors
        }
        TerminatorKind::ForLoop {
            body_target,
            exit_target,
            ..
        } => vec![*body_target, *exit_target],
        TerminatorKind::Return | TerminatorKind::Terminate => Vec::new(),
    }
}

fn ordinary_control_flow_capturing_closure_block_reassigns_local(
    emitter: &ModuleEmitter<'_>,
    body: &mir::MirBody,
    block_id: mir::BasicBlockId,
    local: mir::LocalId,
) -> bool {
    body.block(block_id).statements.iter().any(|statement_id| {
        let statement = body.statement(*statement_id);
        match &statement.kind {
            StatementKind::Assign { place, .. } => {
                place.projections.is_empty() && place.base == local
            }
            StatementKind::BindPattern { pattern, .. } => emitter
                .binding_local_for_pattern(body, *pattern)
                .is_some_and(|binding_local| binding_local == local),
            StatementKind::Eval { .. }
            | StatementKind::RegisterCleanup { .. }
            | StatementKind::RunCleanup { .. }
            | StatementKind::StorageLive { .. }
            | StatementKind::StorageDead { .. } => false,
        }
    })
}

fn cleanup_capturing_closure_assignment(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    target_expr: hir::ExprId,
    value_expr: hir::ExprId,
) -> Option<CleanupCapturingClosureAssignment> {
    let hir::ExprKind::Name(_) = &module.expr(target_expr).kind else {
        return None;
    };
    let ValueResolution::Local(local) = resolution.expr_resolution(target_expr)? else {
        return None;
    };
    let value_closure = direct_local_capturing_closure_for_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        value_expr,
    )?;
    Some(CleanupCapturingClosureAssignment {
        local: *local,
        closure_id: value_closure,
    })
}

fn cleanup_if_branch_info_for_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<CleanupDirectCapturingClosureExpr> {
    if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    ) {
        return Some(expr);
    }

    cleanup_supported_capturing_closure_callee_closure(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    )
    .map(CleanupDirectCapturingClosureExpr::Direct)
}

fn cleanup_if_branch_info_for_block(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    block_id: hir::BlockId,
) -> Option<CleanupDirectCapturingClosureExpr> {
    let block = module.block(block_id);
    if !block.statements.is_empty() {
        return None;
    }
    cleanup_if_branch_info_for_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        block.tail?,
    )
}

fn cleanup_supported_shared_local_if_capturing_closure_binding(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<SupportedCleanupSharedLocalIfBinding> {
    let hir::ExprKind::If {
        condition,
        then_branch,
        else_branch: Some(other),
    } = &module.expr(expr_id).kind
    else {
        return None;
    };

    let then_info = cleanup_if_branch_info_for_block(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        *then_branch,
    );
    let else_info = cleanup_if_branch_info_for_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        *other,
    );
    let then_info = then_info?;
    let else_info = else_info?;

    let target_local = match (then_info, else_info) {
        (
            CleanupDirectCapturingClosureExpr::Assignment(binding),
            CleanupDirectCapturingClosureExpr::Assignment(other_binding),
        ) if binding.local == other_binding.local => binding.local,
        (CleanupDirectCapturingClosureExpr::Assignment(binding), _)
        | (_, CleanupDirectCapturingClosureExpr::Assignment(binding)) => binding.local,
        _ => return None,
    };
    let current_closure = cleanup_binding_value_for_local(cleanup_aliases, target_local)
        .and_then(|value| value.direct_closure_id())
        .or_else(|| {
            let local = mir_local_for_hir_local(body, target_local)?;
            supported.get(&local).copied()
        })?;
    let normalize = |expr: CleanupDirectCapturingClosureExpr| match expr {
        CleanupDirectCapturingClosureExpr::Direct(closure_id) if closure_id == current_closure => {
            Some(closure_id)
        }
        CleanupDirectCapturingClosureExpr::Assignment(binding) if binding.local == target_local => {
            Some(binding.closure_id)
        }
        _ => None,
    };
    let then_closure = normalize(then_info)?;
    let else_closure = normalize(else_info)?;

    Some(SupportedCleanupSharedLocalIfBinding {
        target_local,
        condition_expr: *condition,
        then_branch: *then_branch,
        else_expr: *other,
        then_closure,
        else_closure,
    })
}

fn cleanup_shared_local_match_binding_target_local(
    binding: &SupportedCleanupSharedLocalMatchBinding,
) -> hir::LocalId {
    match binding {
        SupportedCleanupSharedLocalMatchBinding::Tagged { target_local, .. }
        | SupportedCleanupSharedLocalMatchBinding::Bool { target_local, .. }
        | SupportedCleanupSharedLocalMatchBinding::Integer { target_local, .. } => *target_local,
    }
}

fn cleanup_shared_local_match_binding_value(
    binding: &SupportedCleanupSharedLocalMatchBinding,
) -> CleanupCapturingClosureBindingValue {
    match binding {
        SupportedCleanupSharedLocalMatchBinding::Tagged { closures, .. } => {
            CleanupCapturingClosureBindingValue::TaggedMatch {
                tag: None,
                closures: closures.clone(),
            }
        }
        SupportedCleanupSharedLocalMatchBinding::Bool {
            true_closure,
            false_closure,
            ..
        } => CleanupCapturingClosureBindingValue::BoolMatch {
            scrutinee: None,
            true_closure: *true_closure,
            false_closure: *false_closure,
        },
        SupportedCleanupSharedLocalMatchBinding::Integer {
            arms,
            fallback_closure,
            ..
        } => CleanupCapturingClosureBindingValue::IntegerMatch {
            scrutinee: None,
            arms: arms.clone(),
            fallback_closure: *fallback_closure,
        },
    }
}

fn cleanup_supported_shared_local_match_capturing_closure_binding(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<SupportedCleanupSharedLocalMatchBinding> {
    let hir::ExprKind::Match { value: _, arms } = &module.expr(expr_id).kind else {
        return None;
    };

    let mut normalized_arms = Vec::with_capacity(arms.len());
    let mut target_local = None;
    for arm in arms {
        let info = cleanup_if_branch_info_for_expr(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            arm.body,
        )?;
        if let CleanupDirectCapturingClosureExpr::Assignment(binding) = info {
            match target_local {
                Some(existing) if existing != binding.local => return None,
                Some(_) => {}
                None => target_local = Some(binding.local),
            }
        }
        normalized_arms.push(info);
    }
    let target_local = target_local?;
    let current_closure = cleanup_binding_value_for_local(cleanup_aliases, target_local)
        .and_then(|value| value.direct_closure_id())
        .or_else(|| {
            let local = mir_local_for_hir_local(body, target_local)?;
            supported.get(&local).copied()
        })?;
    let normalize = |expr: CleanupDirectCapturingClosureExpr| match expr {
        CleanupDirectCapturingClosureExpr::Direct(closure_id) if closure_id == current_closure => {
            Some(closure_id)
        }
        CleanupDirectCapturingClosureExpr::Assignment(binding) if binding.local == target_local => {
            Some(binding.closure_id)
        }
        _ => None,
    };
    let arm_closures = normalized_arms
        .into_iter()
        .map(normalize)
        .collect::<Option<Vec<_>>>()?;

    if arms.iter().any(|arm| arm.guard.is_some()) {
        return Some(SupportedCleanupSharedLocalMatchBinding::Tagged {
            target_local,
            expr_id,
            closures: arm_closures,
        });
    }

    let bool_patterns = arms
        .iter()
        .map(|arm| supported_cleanup_bool_match_pattern(module, resolution, arm.pattern))
        .collect::<Option<Vec<_>>>();
    if let Some(patterns) = bool_patterns {
        let mut saw_explicit = false;
        let mut true_closure = None;
        let mut false_closure = None;
        for (pattern, closure_id) in patterns.into_iter().zip(arm_closures.iter().copied()) {
            match pattern {
                SupportedBoolMatchPattern::True => {
                    saw_explicit = true;
                    true_closure.get_or_insert(closure_id);
                }
                SupportedBoolMatchPattern::False => {
                    saw_explicit = true;
                    false_closure.get_or_insert(closure_id);
                }
                SupportedBoolMatchPattern::CatchAll => {
                    true_closure.get_or_insert(closure_id);
                    false_closure.get_or_insert(closure_id);
                }
            }
            if true_closure.is_some() && false_closure.is_some() {
                break;
            }
        }
        if saw_explicit {
            return Some(SupportedCleanupSharedLocalMatchBinding::Bool {
                target_local,
                expr_id,
                true_closure: true_closure?,
                false_closure: false_closure?,
            });
        }
    }

    let int_patterns = arms
        .iter()
        .map(|arm| supported_cleanup_integer_match_pattern(module, resolution, arm.pattern))
        .collect::<Option<Vec<_>>>()?;
    let mut saw_literal = false;
    let mut seen_literals = HashSet::new();
    let mut ordered_arms = Vec::new();
    let mut fallback_closure = None;
    for (pattern, closure_id) in int_patterns.into_iter().zip(arm_closures.into_iter()) {
        match pattern {
            SupportedIntegerMatchPattern::Literal(value) => {
                saw_literal = true;
                if fallback_closure.is_none() && seen_literals.insert(value.clone()) {
                    ordered_arms.push(CleanupIntegerCapturingClosureMatchArm { value, closure_id });
                }
            }
            SupportedIntegerMatchPattern::CatchAll => {
                fallback_closure.get_or_insert(closure_id);
                break;
            }
        }
    }
    if saw_literal {
        return Some(SupportedCleanupSharedLocalMatchBinding::Integer {
            target_local,
            expr_id,
            arms: ordered_arms,
            fallback_closure: fallback_closure?,
        });
    }

    None
}

fn cleanup_direct_capturing_closure_callee_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<CleanupDirectCapturingClosureExpr> {
    if let Some(closure_id) = direct_local_capturing_closure_for_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    ) {
        return Some(CleanupDirectCapturingClosureExpr::Direct(closure_id));
    }

    let hir::ExprKind::Binary {
        left,
        op: BinaryOp::Assign,
        right,
    } = &module.expr(expr_id).kind
    else {
        return None;
    };

    cleanup_capturing_closure_assignment(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        *left,
        *right,
    )
    .map(CleanupDirectCapturingClosureExpr::Assignment)
}

fn cleanup_supported_capturing_closure_callee_closure(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> Option<mir::ClosureId> {
    if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
        module,
        resolution,
        body,
        supported,
        cleanup_aliases,
        expr_id,
    ) {
        return Some(expr.closure_id());
    }

    match &module.expr(expr_id).kind {
        hir::ExprKind::Question(inner) => cleanup_supported_capturing_closure_callee_closure(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            *inner,
        ),
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            cleanup_supported_capturing_closure_callee_block_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *block_id,
            )
        }
        hir::ExprKind::If {
            then_branch,
            else_branch: Some(other),
            ..
        } => {
            let then_closure = cleanup_supported_capturing_closure_callee_block_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *then_branch,
            )?;
            let other_closure = cleanup_supported_capturing_closure_callee_closure(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *other,
            )?;
            (then_closure == other_closure).then_some(then_closure)
        }
        hir::ExprKind::Match { arms, .. } => {
            let mut closure_id = None;
            for arm in arms {
                let current = cleanup_supported_capturing_closure_callee_closure(
                    module,
                    resolution,
                    body,
                    supported,
                    cleanup_aliases,
                    arm.body,
                )?;
                match closure_id {
                    Some(existing) if existing != current => return None,
                    Some(_) => {}
                    None => closure_id = Some(current),
                }
            }
            closure_id
        }
        _ => None,
    }
}

fn cleanup_supported_capturing_closure_callee_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    expr_id: hir::ExprId,
) -> bool {
    if cleanup_bound_capturing_closure_value_for_expr(module, resolution, cleanup_aliases, expr_id)
        .is_some()
        || cleanup_direct_capturing_closure_callee_expr(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            expr_id,
        )
        .is_some()
        || cleanup_supported_capturing_closure_callee_closure(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            expr_id,
        )
        .is_some()
        || direct_local_capturing_closure_for_expr(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            expr_id,
        )
        .is_some()
    {
        return true;
    }

    match &module.expr(expr_id).kind {
        hir::ExprKind::Question(inner) => cleanup_supported_capturing_closure_callee_expr(
            module,
            resolution,
            body,
            supported,
            cleanup_aliases,
            *inner,
        ),
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            if let Some(tail) = callable_elided_block_tail_expr(module, resolution, *block_id) {
                return cleanup_supported_capturing_closure_callee_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    cleanup_aliases,
                    tail,
                );
            }
            cleanup_supported_capturing_closure_callee_block_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *block_id,
            )
        }
        hir::ExprKind::If {
            then_branch,
            else_branch: Some(other),
            ..
        } => {
            cleanup_supported_capturing_closure_callee_block_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *then_branch,
            ) && cleanup_supported_capturing_closure_callee_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                *other,
            )
        }
        hir::ExprKind::Match { arms, .. } => arms.iter().all(|arm| {
            cleanup_supported_capturing_closure_callee_expr(
                module,
                resolution,
                body,
                supported,
                cleanup_aliases,
                arm.body,
            )
        }),
        _ => false,
    }
}

fn cleanup_supported_capturing_closure_callee_block_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    block_id: hir::BlockId,
) -> bool {
    let block = module.block(block_id);
    let mut scoped_aliases = cleanup_aliases.to_vec();
    for statement_id in &block.statements {
        match &module.stmt(*statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
                    return false;
                };
                if let Some(binding_value) = cleanup_bound_capturing_closure_value_for_expr(
                    module,
                    resolution,
                    &scoped_aliases,
                    *value,
                ) {
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                    continue;
                }
                if let Some(binding) = cleanup_supported_shared_local_if_capturing_closure_binding(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    let binding_value = CleanupCapturingClosureBindingValue::IfBranch {
                        condition: None,
                        then_closure: binding.then_closure,
                        else_closure: binding.else_closure,
                    };
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: binding.target_local,
                        value: binding_value.clone(),
                    });
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                    continue;
                }
                if let Some(binding) =
                    cleanup_supported_shared_local_match_capturing_closure_binding(
                        module,
                        resolution,
                        body,
                        supported,
                        &scoped_aliases,
                        *value,
                    )
                {
                    let binding_value = cleanup_shared_local_match_binding_value(&binding);
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: cleanup_shared_local_match_binding_target_local(&binding),
                        value: binding_value.clone(),
                    });
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                    continue;
                }
                let mut assignment_binding = None;
                let closure_id = if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    if let CleanupDirectCapturingClosureExpr::Assignment(binding) = expr {
                        assignment_binding = Some(binding);
                    }
                    expr.closure_id()
                } else if let Some(closure_id) = cleanup_supported_capturing_closure_callee_closure(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    closure_id
                } else if let Some(closure_id) = direct_local_capturing_closure_for_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    closure_id
                } else {
                    return false;
                };
                if let Some(binding) = assignment_binding {
                    scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                        binding.local,
                        binding.closure_id,
                    ));
                }
                scoped_aliases.push(CleanupCapturingClosureBinding::direct(*local, closure_id));
            }
            hir::StmtKind::Expr { expr, .. } => {
                let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &module.expr(*expr).kind
                else {
                    return false;
                };
                let Some(binding) = cleanup_capturing_closure_assignment(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *left,
                    *right,
                ) else {
                    return false;
                };
                scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                    binding.local,
                    binding.closure_id,
                ));
            }
            _ => return false,
        }
    }

    block.tail.is_some_and(|tail| {
        cleanup_supported_capturing_closure_callee_expr(
            module,
            resolution,
            body,
            supported,
            &scoped_aliases,
            tail,
        )
    })
}

fn cleanup_supported_capturing_closure_callee_block_closure(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
    block_id: hir::BlockId,
) -> Option<mir::ClosureId> {
    let block = module.block(block_id);
    let mut scoped_aliases = cleanup_aliases.to_vec();
    for statement_id in &block.statements {
        match &module.stmt(*statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
                    return None;
                };
                if let Some(binding_value) = cleanup_bound_capturing_closure_value_for_expr(
                    module,
                    resolution,
                    &scoped_aliases,
                    *value,
                ) {
                    if binding_value.direct_closure_id().is_none() {
                        return None;
                    }
                    scoped_aliases.push(CleanupCapturingClosureBinding {
                        local: *local,
                        value: binding_value,
                    });
                    continue;
                }
                let mut assignment_binding = None;
                let closure_id = if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    if let CleanupDirectCapturingClosureExpr::Assignment(binding) = expr {
                        assignment_binding = Some(binding);
                    }
                    expr.closure_id()
                } else if let Some(closure_id) = cleanup_supported_capturing_closure_callee_closure(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    closure_id
                } else if let Some(closure_id) = direct_local_capturing_closure_for_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *value,
                ) {
                    closure_id
                } else {
                    return None;
                };
                if let Some(binding) = assignment_binding {
                    scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                        binding.local,
                        binding.closure_id,
                    ));
                }
                scoped_aliases.push(CleanupCapturingClosureBinding::direct(*local, closure_id));
            }
            hir::StmtKind::Expr { expr, .. } => {
                let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &module.expr(*expr).kind
                else {
                    return None;
                };
                let binding = cleanup_capturing_closure_assignment(
                    module,
                    resolution,
                    body,
                    supported,
                    &scoped_aliases,
                    *left,
                    *right,
                )?;
                scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                    binding.local,
                    binding.closure_id,
                ));
            }
            _ => return None,
        }
    }
    cleanup_supported_capturing_closure_callee_closure(
        module,
        resolution,
        body,
        supported,
        &scoped_aliases,
        block.tail?,
    )
}

fn cleanup_capturing_closure_binding_requires_runtime_eval(
    module: &hir::Module,
    expr_id: hir::ExprId,
) -> bool {
    match &module.expr(expr_id).kind {
        hir::ExprKind::If { .. } | hir::ExprKind::Match { .. } => true,
        hir::ExprKind::Question(inner) => {
            cleanup_capturing_closure_binding_requires_runtime_eval(module, *inner)
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            let block = module.block(*block_id);
            block.tail.is_some_and(|tail| {
                cleanup_capturing_closure_binding_requires_runtime_eval(module, tail)
            })
        }
        _ => false,
    }
}

fn cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    expr_id: hir::ExprId,
    binding_locals: &HashSet<hir::LocalId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
) -> bool {
    match &module.expr(expr_id).kind {
        hir::ExprKind::Name(_) => matches!(
            resolution.expr_resolution(expr_id),
            Some(ValueResolution::Local(local_id))
                if binding_locals.contains(local_id)
                    || cleanup_aliases
                        .iter()
                        .rev()
                        .any(|binding| binding.local == *local_id)
        ),
        hir::ExprKind::Integer(_)
        | hir::ExprKind::String { .. }
        | hir::ExprKind::Bool(_)
        | hir::ExprKind::NoneLiteral => false,
        hir::ExprKind::Tuple(items) | hir::ExprKind::Array(items) => items.iter().any(|item| {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *item,
                binding_locals,
                cleanup_aliases,
            )
        }),
        hir::ExprKind::RepeatArray { value, .. } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *value,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
            cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *block_id,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *condition,
                binding_locals,
                cleanup_aliases,
            ) || cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *then_branch,
                binding_locals,
                cleanup_aliases,
            ) || else_branch.is_some_and(|expr_id| {
                cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    expr_id,
                    binding_locals,
                    cleanup_aliases,
                )
            })
        }
        hir::ExprKind::Match { value, arms } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *value,
                binding_locals,
                cleanup_aliases,
            ) || arms.iter().any(|arm| {
                arm.guard.is_some_and(|guard| {
                    cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                        module,
                        resolution,
                        body,
                        supported,
                        guard,
                        binding_locals,
                        cleanup_aliases,
                    )
                }) || cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    arm.body,
                    binding_locals,
                    cleanup_aliases,
                )
            })
        }
        hir::ExprKind::Closure { body: closure_body, .. } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *closure_body,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::Call { callee, args } => {
            let callee_allowed = args.iter().all(|arg| matches!(arg, hir::CallArg::Positional(_)))
                && cleanup_supported_capturing_closure_callee_expr(
                    module,
                    resolution,
                    body,
                    supported,
                    cleanup_aliases,
                    *callee,
                );
            (!callee_allowed
                && cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *callee,
                    binding_locals,
                    cleanup_aliases,
                ))
                || args.iter().any(|arg| {
                    let value = match arg {
                        hir::CallArg::Positional(expr_id) => *expr_id,
                        hir::CallArg::Named { value, .. } => *value,
                    };
                    cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                        module,
                        resolution,
                        body,
                        supported,
                        value,
                        binding_locals,
                        cleanup_aliases,
                    )
                })
        }
        hir::ExprKind::Member { object, .. } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *object,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::Bracket { target, items } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *target,
                binding_locals,
                cleanup_aliases,
            ) || items.iter().any(|item| {
                cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *item,
                    binding_locals,
                    cleanup_aliases,
                )
            })
        }
        hir::ExprKind::StructLiteral { fields, .. } => fields.iter().any(|field| {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                field.value,
                binding_locals,
                cleanup_aliases,
            )
        }),
        hir::ExprKind::Binary { left, right, .. } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *left,
                binding_locals,
                cleanup_aliases,
            ) || cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *right,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::Question(inner) => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *inner,
                binding_locals,
                cleanup_aliases,
            )
        }
        hir::ExprKind::Unary { expr, .. } => {
            cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                module,
                resolution,
                body,
                supported,
                *expr,
                binding_locals,
                cleanup_aliases,
            )
        }
    }
}

fn cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
    module: &hir::Module,
    resolution: &ResolutionMap,
    body: &mir::MirBody,
    supported: &HashMap<mir::LocalId, mir::ClosureId>,
    block_id: hir::BlockId,
    binding_locals: &HashSet<hir::LocalId>,
    cleanup_aliases: &[CleanupCapturingClosureBinding],
) -> bool {
    let block = module.block(block_id);
    let mut scoped_aliases = cleanup_aliases.to_vec();
    for statement_id in &block.statements {
        match &module.stmt(*statement_id).kind {
            hir::StmtKind::Let { pattern, value, .. } => {
                let alias_binding = match pattern_kind(module, *pattern) {
                    PatternKind::Binding(local) => {
                        if let Some(binding_value) = cleanup_bound_capturing_closure_value_for_expr(
                            module,
                            resolution,
                            &scoped_aliases,
                            *value,
                        ) {
                            Some(CleanupCapturingClosureBinding {
                                local: *local,
                                value: binding_value,
                            })
                        } else if let Some(binding) =
                            cleanup_supported_shared_local_if_capturing_closure_binding(
                                module,
                                resolution,
                                body,
                                supported,
                                &scoped_aliases,
                                *value,
                            )
                        {
                            let binding_value = CleanupCapturingClosureBindingValue::IfBranch {
                                condition: None,
                                then_closure: binding.then_closure,
                                else_closure: binding.else_closure,
                            };
                            scoped_aliases.push(CleanupCapturingClosureBinding {
                                local: binding.target_local,
                                value: binding_value.clone(),
                            });
                            Some(CleanupCapturingClosureBinding {
                                local: *local,
                                value: binding_value,
                            })
                        } else if let Some(binding) =
                            cleanup_supported_shared_local_match_capturing_closure_binding(
                                module,
                                resolution,
                                body,
                                supported,
                                &scoped_aliases,
                                *value,
                            )
                        {
                            let binding_value = cleanup_shared_local_match_binding_value(&binding);
                            scoped_aliases.push(CleanupCapturingClosureBinding {
                                local: cleanup_shared_local_match_binding_target_local(&binding),
                                value: binding_value.clone(),
                            });
                            Some(CleanupCapturingClosureBinding {
                                local: *local,
                                value: binding_value,
                            })
                        } else if let Some(expr) = cleanup_direct_capturing_closure_callee_expr(
                            module,
                            resolution,
                            body,
                            supported,
                            &scoped_aliases,
                            *value,
                        ) {
                            if let CleanupDirectCapturingClosureExpr::Assignment(binding) = expr {
                                scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                                    binding.local,
                                    binding.closure_id,
                                ));
                            }
                            Some(CleanupCapturingClosureBinding::direct(
                                *local,
                                expr.closure_id(),
                            ))
                        } else if let Some(closure_id) =
                            cleanup_supported_capturing_closure_callee_closure(
                                module,
                                resolution,
                                body,
                                supported,
                                &scoped_aliases,
                                *value,
                            )
                        {
                            Some(CleanupCapturingClosureBinding::direct(*local, closure_id))
                        } else {
                            direct_local_capturing_closure_for_expr(
                                module,
                                resolution,
                                body,
                                supported,
                                &scoped_aliases,
                                *value,
                            )
                            .map(|closure_id| {
                                CleanupCapturingClosureBinding::direct(*local, closure_id)
                            })
                        }
                    }
                    _ => None,
                };
                if let Some(binding) = alias_binding {
                    scoped_aliases.push(binding);
                    continue;
                }
                if cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *value,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::Return(expr) => {
                if expr.is_some_and(|expr_id| {
                    cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                        module,
                        resolution,
                        body,
                        supported,
                        expr_id,
                        binding_locals,
                        &scoped_aliases,
                    )
                }) {
                    return true;
                }
            }
            hir::StmtKind::Expr { expr, .. } => {
                if let hir::ExprKind::Binary {
                    left,
                    op: BinaryOp::Assign,
                    right,
                } = &module.expr(*expr).kind
                    && let Some(binding) = cleanup_capturing_closure_assignment(
                        module,
                        resolution,
                        body,
                        supported,
                        &scoped_aliases,
                        *left,
                        *right,
                    )
                {
                    scoped_aliases.push(CleanupCapturingClosureBinding::direct(
                        binding.local,
                        binding.closure_id,
                    ));
                    continue;
                }
                if cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *expr,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::Defer(expr) => {
                if cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *expr,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::While {
                condition,
                body: loop_body,
            } => {
                if cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *condition,
                    binding_locals,
                    &scoped_aliases,
                ) || cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *loop_body,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::Loop { body: loop_body } => {
                if cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *loop_body,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::For {
                iterable,
                body: loop_body,
                ..
            } => {
                if cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *iterable,
                    binding_locals,
                    &scoped_aliases,
                ) || cleanup_block_mentions_binding_local_outside_direct_local_capturing_closure_call(
                    module,
                    resolution,
                    body,
                    supported,
                    *loop_body,
                    binding_locals,
                    &scoped_aliases,
                ) {
                    return true;
                }
            }
            hir::StmtKind::Break | hir::StmtKind::Continue => {}
        }
    }
    block.tail.is_some_and(|expr_id| {
        cleanup_expr_mentions_binding_local_outside_direct_local_capturing_closure_call(
            module,
            resolution,
            body,
            supported,
            expr_id,
            binding_locals,
            &scoped_aliases,
        )
    })
}

fn guard_direct_callee_function(
    module: &hir::Module,
    resolution: &ResolutionMap,
    callee_expr: hir::ExprId,
) -> Option<FunctionRef> {
    match resolution.expr_resolution(callee_expr)? {
        ValueResolution::Function(function_ref) => Some(*function_ref),
        ValueResolution::Item(item_id) => {
            matches!(&module.item(*item_id).kind, ItemKind::Function(_))
                .then_some(FunctionRef::Item(*item_id))
        }
        ValueResolution::Import(import_binding) => {
            let item_id = local_item_for_import_binding(module, import_binding)?;
            matches!(&module.item(item_id).kind, ItemKind::Function(_))
                .then_some(FunctionRef::Item(item_id))
        }
        ValueResolution::Local(_)
        | ValueResolution::Param(_)
        | ValueResolution::ArrayLengthGeneric(_)
        | ValueResolution::SelfValue => None,
    }
}

fn ordered_guard_call_args<'a>(
    args: &'a [hir::CallArg],
    signature: &FunctionSignature,
) -> Option<Vec<&'a hir::CallArg>> {
    if args
        .iter()
        .all(|arg| matches!(arg, hir::CallArg::Positional(_)))
    {
        return (args.len() == signature.params.len()).then(|| args.iter().collect());
    }

    let mut ordered = vec![None; signature.params.len()];
    let mut next_positional = 0usize;

    for arg in args {
        let index = if let hir::CallArg::Named { name, .. } = arg {
            signature
                .params
                .iter()
                .position(|param| param.name == *name)?
        } else {
            while next_positional < ordered.len() && ordered[next_positional].is_some() {
                next_positional += 1;
            }
            if next_positional == ordered.len() {
                return None;
            }
            next_positional
        };

        if ordered[index].is_some() {
            return None;
        }
        ordered[index] = Some(arg);
    }

    ordered.into_iter().collect()
}

fn supported_guard_value_expr_as_type(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    expr_id: hir::ExprId,
    expected_ty: &Ty,
) -> bool {
    if is_void_ty(expected_ty) {
        return false;
    }

    if matches!(expected_ty, Ty::Callable { .. }) {
        if let hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) =
            &module.expr(expr_id).kind
        {
            return supported_guard_callable_block_expr_as_type(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *block_id,
                expected_ty,
            );
        }
    }

    match &module.expr(expr_id).kind {
        hir::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            return runtime_bool_guard_supported(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                *condition,
            ) && if matches!(expected_ty, Ty::Callable { .. }) {
                supported_guard_callable_block_expr_as_type(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    *then_branch,
                    expected_ty,
                )
            } else {
                module.block(*then_branch).tail.is_some_and(|tail| {
                    supported_guard_value_expr_as_type(
                        module,
                        resolution,
                        typeck,
                        signatures,
                        body,
                        local_types,
                        arm_pattern,
                        tail,
                        expected_ty,
                    )
                })
            } && else_branch.is_some_and(|other| {
                supported_guard_value_expr_as_type(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    other,
                    expected_ty,
                )
            });
        }
        hir::ExprKind::Match { value, arms } => {
            let Some(scrutinee_ty) = typeck.expr_ty(*value) else {
                return false;
            };

            if scrutinee_ty.is_bool() {
                return runtime_bool_guard_supported(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    *value,
                ) && arms.iter().all(|arm| {
                    supported_bool_match_pattern(module, resolution, arm.pattern).is_some()
                        && arm.guard.is_none_or(|guard| {
                            runtime_bool_guard_supported(
                                module,
                                resolution,
                                typeck,
                                signatures,
                                body,
                                local_types,
                                arm_pattern,
                                guard,
                            )
                        })
                        && supported_guard_value_expr_as_type(
                            module,
                            resolution,
                            typeck,
                            signatures,
                            body,
                            local_types,
                            arm_pattern,
                            arm.body,
                            expected_ty,
                        )
                });
            }

            if scrutinee_ty.compatible_with(&Ty::Builtin(BuiltinType::Int)) {
                return supported_guard_scalar_expr(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    *value,
                ) == Some(GuardScalarKind::Int)
                    && arms.iter().all(|arm| {
                        supported_cleanup_integer_match_pattern(module, resolution, arm.pattern)
                            .is_some()
                            && arm.guard.is_none_or(|guard| {
                                runtime_bool_guard_supported(
                                    module,
                                    resolution,
                                    typeck,
                                    signatures,
                                    body,
                                    local_types,
                                    arm_pattern,
                                    guard,
                                )
                            })
                            && supported_guard_value_expr_as_type(
                                module,
                                resolution,
                                typeck,
                                signatures,
                                body,
                                local_types,
                                arm_pattern,
                                arm.body,
                                expected_ty,
                            )
                    });
            }

            return false;
        }
        _ => {}
    }

    guard_expr_ty(
        module,
        resolution,
        typeck,
        signatures,
        body,
        local_types,
        Some(arm_pattern),
        expr_id,
    )
    .is_some_and(|actual_ty| expected_ty.compatible_with(&actual_ty))
}

fn callable_elided_block_tail_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    block_id: hir::BlockId,
) -> Option<hir::ExprId> {
    let block = module.block(block_id);
    if block.statements.is_empty() {
        return None;
    }

    let mut bindings = HashMap::new();
    for statement_id in &block.statements {
        let hir::StmtKind::Let { pattern, value, .. } = &module.stmt(*statement_id).kind else {
            return None;
        };
        let PatternKind::Binding(local) = pattern_kind(module, *pattern) else {
            return None;
        };
        if !callable_binding_expr_is_elidable(module, resolution, &bindings, *value) {
            return None;
        }
        bindings.insert(*local, *value);
    }

    resolve_callable_binding_expr(module, resolution, &bindings, block.tail?)
}

fn callable_binding_expr_is_elidable(
    module: &hir::Module,
    resolution: &ResolutionMap,
    bindings: &HashMap<hir::LocalId, hir::ExprId>,
    expr_id: hir::ExprId,
) -> bool {
    resolve_callable_binding_expr(module, resolution, bindings, expr_id).is_some()
}

fn resolve_callable_binding_expr(
    module: &hir::Module,
    resolution: &ResolutionMap,
    bindings: &HashMap<hir::LocalId, hir::ExprId>,
    expr_id: hir::ExprId,
) -> Option<hir::ExprId> {
    let mut current = expr_id;
    let mut visited = HashSet::new();

    loop {
        if !visited.insert(current) {
            return None;
        }

        match &module.expr(current).kind {
            hir::ExprKind::If { .. } | hir::ExprKind::Match { .. } => return Some(current),
            hir::ExprKind::Name(_) => {
                let ValueResolution::Local(local) = resolution.expr_resolution(current)? else {
                    return None;
                };
                current = *bindings.get(local)?;
            }
            hir::ExprKind::Question(inner) => current = *inner,
            hir::ExprKind::Block(block_id) | hir::ExprKind::Unsafe(block_id) => {
                current = callable_elided_block_tail_expr(module, resolution, *block_id)?;
            }
            _ => return None,
        }
    }
}

fn supported_guard_callable_block_expr_as_type(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    block_id: hir::BlockId,
    expected_ty: &Ty,
) -> bool {
    let block = module.block(block_id);
    if block.statements.is_empty() {
        return block.tail.is_some_and(|tail| {
            guard_expr_ty(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                Some(arm_pattern),
                tail,
            )
            .is_some_and(|actual_ty| expected_ty.compatible_with(&actual_ty))
        });
    }

    if let Some(tail) = callable_elided_block_tail_expr(module, resolution, block_id) {
        return supported_guard_value_expr_as_type(
            module,
            resolution,
            typeck,
            signatures,
            body,
            local_types,
            arm_pattern,
            tail,
            expected_ty,
        );
    }

    let supported_direct_local_capturing_closures =
        supported_direct_local_capturing_closure_locals_for_guard(module, body, local_types);
    supported_direct_local_capturing_closure_callee_block_closure(
        module,
        resolution,
        body,
        &supported_direct_local_capturing_closures,
        &[],
        block_id,
    )
    .and_then(|closure_id| typeck.expr_ty(body.closure(closure_id).expr).cloned())
    .is_some_and(|actual_ty| expected_ty.compatible_with(&actual_ty))
}

fn supported_guard_call<'a>(
    module: &hir::Module,
    resolution: &ResolutionMap,
    typeck: &TypeckResult,
    signatures: &HashMap<FunctionRef, FunctionSignature>,
    body: &mir::MirBody,
    local_types: &HashMap<mir::LocalId, Ty>,
    arm_pattern: hir::PatternId,
    callee_expr: hir::ExprId,
    args: &'a [hir::CallArg],
) -> Option<(Vec<Ty>, Ty)> {
    let supported_direct_local_capturing_closures =
        supported_direct_local_capturing_closure_locals_for_guard(module, body, local_types);

    if let Some(function) = guard_direct_callee_function(module, resolution, callee_expr) {
        let signature = signatures.get(&function)?;
        let ordered_args = ordered_guard_call_args(args, signature)?;
        for (arg, param) in ordered_args.iter().copied().zip(signature.params.iter()) {
            let expr_id = guard_call_arg_expr(arg);
            if let Some(expected_kind) = guard_scalar_kind_for_ty(&param.ty) {
                let actual_kind = supported_guard_scalar_expr(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    expr_id,
                )?;
                if actual_kind != expected_kind {
                    return None;
                }
            } else {
                if !supported_guard_value_expr_as_type(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    expr_id,
                    &param.ty,
                ) {
                    return None;
                }
            }
        }
        return Some((
            signature
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            signature.return_ty.clone(),
        ));
    }

    if let Some(closure_id) = supported_direct_local_capturing_closure_callee_closure(
        module,
        resolution,
        body,
        &supported_direct_local_capturing_closures,
        &[],
        callee_expr,
    ) {
        let closure = body.closure(closure_id);
        let closure_ty = typeck.expr_ty(closure.expr)?.clone();
        let Ty::Callable { params, ret } = &closure_ty else {
            return None;
        };
        if args.len() != params.len()
            || args
                .iter()
                .any(|arg| matches!(arg, hir::CallArg::Named { .. }))
        {
            return None;
        }
        for (arg, param_ty) in args.iter().zip(params.iter()) {
            let expr_id = guard_call_arg_expr(arg);
            if let Some(expected_kind) = guard_scalar_kind_for_ty(param_ty) {
                let actual_kind = supported_guard_scalar_expr(
                    module,
                    resolution,
                    typeck,
                    signatures,
                    body,
                    local_types,
                    arm_pattern,
                    expr_id,
                )?;
                if actual_kind != expected_kind {
                    return None;
                }
            } else if !supported_guard_value_expr_as_type(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
                param_ty,
            ) {
                return None;
            }
        }
        return Some((params.clone(), ret.as_ref().clone()));
    }

    let callee_ty = typeck.expr_ty(callee_expr).cloned()?;
    if !supported_guard_value_expr_as_type(
        module,
        resolution,
        typeck,
        signatures,
        body,
        local_types,
        arm_pattern,
        callee_expr,
        &callee_ty,
    ) {
        return None;
    }
    let Ty::Callable { params, ret } = callee_ty else {
        return None;
    };
    if args.len() != params.len()
        || args
            .iter()
            .any(|arg| matches!(arg, hir::CallArg::Named { .. }))
    {
        return None;
    }
    for (arg, param_ty) in args.iter().zip(params.iter()) {
        let expr_id = guard_call_arg_expr(arg);
        if let Some(expected_kind) = guard_scalar_kind_for_ty(param_ty) {
            let actual_kind = supported_guard_scalar_expr(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
            )?;
            if actual_kind != expected_kind {
                return None;
            }
        } else {
            if !supported_guard_value_expr_as_type(
                module,
                resolution,
                typeck,
                signatures,
                body,
                local_types,
                arm_pattern,
                expr_id,
                param_ty,
            ) {
                return None;
            }
        }
    }
    Some((params, ret.as_ref().clone()))
}

fn guard_call_arg_expr(arg: &hir::CallArg) -> hir::ExprId {
    match arg {
        hir::CallArg::Positional(expr_id) => *expr_id,
        hir::CallArg::Named { value, .. } => *value,
    }
}

fn llvm_string_aggregate_ty() -> &'static str {
    "{ ptr, i64 }"
}

fn string_loadable_abi_layout() -> LoadableAbiLayout {
    LoadableAbiLayout { size: 16, align: 8 }
}

fn llvm_string_literal_bytes(value: &str) -> String {
    let mut escaped = String::new();
    for byte in value.as_bytes() {
        let _ = write!(escaped, "\\{byte:02X}");
    }
    escaped.push_str("\\00");
    escaped
}

fn lower_llvm_type(ty: &Ty, span: Span, context: &str) -> Result<String, Diagnostic> {
    match ty {
        Ty::Array { element, len } => {
            let element_llvm_ty = lower_llvm_type(element, span, context)?;
            let Some(len) = known_array_len(len) else {
                return Err(unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation requires concrete array length for {context} `{ty}`"
                    ),
                ));
            };
            Ok(format!("[{len} x {element_llvm_ty}]"))
        }
        Ty::Builtin(BuiltinType::String) => Ok(llvm_string_aggregate_ty().to_owned()),
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
        Ty::Callable { .. } => Ok("ptr".to_owned()),
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
            let Some(len) = known_array_len(len) else {
                return Err(unsupported(
                    span,
                    format!(
                        "LLVM IR backend foundation requires concrete array length for {context} `{ty}`"
                    ),
                ));
            };
            Ok(LoadableAbiLayout {
                size: element.size * (len as u64),
                align: element.align,
            })
        }
        Ty::Builtin(BuiltinType::String) => Ok(string_loadable_abi_layout()),
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
        Ty::Builtin(BuiltinType::String) => Ok(ScalarAbiLayout { size: 16, align: 8 }),
        _ => Err(Diagnostic::error(format!(
            "LLVM IR backend foundation does not support {context} `{ty}` yet"
        ))
        .with_label(Label::new(span))),
    }
}

fn is_supported_capture_ty(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::Builtin(BuiltinType::String)
            | Ty::Builtin(BuiltinType::Bool)
            | Ty::Builtin(BuiltinType::Int)
            | Ty::Builtin(BuiltinType::UInt)
            | Ty::Builtin(BuiltinType::I8)
            | Ty::Builtin(BuiltinType::I16)
            | Ty::Builtin(BuiltinType::I32)
            | Ty::Builtin(BuiltinType::I64)
            | Ty::Builtin(BuiltinType::ISize)
            | Ty::Builtin(BuiltinType::U8)
            | Ty::Builtin(BuiltinType::U16)
            | Ty::Builtin(BuiltinType::U32)
            | Ty::Builtin(BuiltinType::U64)
            | Ty::Builtin(BuiltinType::USize)
            | Ty::Builtin(BuiltinType::F32)
            | Ty::Builtin(BuiltinType::F64)
            | Ty::TaskHandle(_)
    )
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

fn const_or_static_item_type(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
) -> Option<Ty> {
    match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => {
            Some(lower_type(module, resolution, global.ty))
        }
        _ => None,
    }
}

fn const_or_static_item_integer_literal(
    module: &hir::Module,
    resolution: &ResolutionMap,
    item_id: ItemId,
) -> Option<i64> {
    let global = match &module.item(item_id).kind {
        ItemKind::Const(global) | ItemKind::Static(global) => global,
        _ => return None,
    };
    let mut visited = HashSet::new();
    guard_literal_int_expr(module, resolution, global.value, &mut visited)
}

fn for_await_index_slot_name(block_id: mir::BasicBlockId) -> String {
    format!("%for_await_index_bb{}", block_id.index())
}

fn for_iterable_slot_name(block_id: mir::BasicBlockId) -> String {
    format!("%for_iterable_bb{}", block_id.index())
}

fn bool_match_dispatch_block_name(block_id: mir::BasicBlockId, arm_index: usize) -> String {
    format!("bb{}_match_dispatch{arm_index}", block_id.index())
}

fn bool_match_guard_block_name(block_id: mir::BasicBlockId, arm_index: usize) -> String {
    format!("bb{}_match_guard{arm_index}", block_id.index())
}

fn integer_match_dispatch_block_name(block_id: mir::BasicBlockId, arm_index: usize) -> String {
    format!("bb{}_match_dispatch{arm_index}", block_id.index())
}

fn integer_match_guard_block_name(block_id: mir::BasicBlockId, arm_index: usize) -> String {
    format!("bb{}_match_guard{arm_index}", block_id.index())
}

fn for_await_setup_block_name(block_id: mir::BasicBlockId) -> String {
    format!("bb{}_for_await_setup", block_id.index())
}

fn for_await_tuple_check_block_name(block_id: mir::BasicBlockId, index: usize) -> String {
    format!("bb{}_for_await_tuple_check_{}", block_id.index(), index)
}

fn for_await_tuple_item_block_name(block_id: mir::BasicBlockId, index: usize) -> String {
    format!("bb{}_for_await_tuple_item_{}", block_id.index(), index)
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

const MAX_TRANSPARENT_BACKEND_ALIAS_DEPTH: usize = 32;

fn backend_value_compatible(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expected: &Ty,
    actual: &Ty,
) -> bool {
    backend_value_compatible_inner(module, resolution, expected, actual, 0)
}

fn backend_value_compatible_inner(
    module: &hir::Module,
    resolution: &ResolutionMap,
    expected: &Ty,
    actual: &Ty,
    depth: usize,
) -> bool {
    if expected.compatible_with(actual) {
        return true;
    }
    if depth >= MAX_TRANSPARENT_BACKEND_ALIAS_DEPTH {
        return false;
    }

    if let Some(expected_target) =
        transparent_backend_value_alias_target(module, resolution, expected)
        && backend_value_compatible_inner(module, resolution, &expected_target, actual, depth + 1)
    {
        return true;
    }
    if let Some(actual_target) = transparent_backend_value_alias_target(module, resolution, actual)
        && backend_value_compatible_inner(module, resolution, expected, &actual_target, depth + 1)
    {
        return true;
    }

    match (expected, actual) {
        (
            Ty::Array {
                element: expected_element,
                len: expected_len,
            },
            Ty::Array {
                element: actual_element,
                len: actual_len,
            },
        ) => {
            expected_len == actual_len
                && backend_value_compatible_inner(
                    module,
                    resolution,
                    expected_element,
                    actual_element,
                    depth + 1,
                )
        }
        (
            Ty::Item {
                item_id: expected_item,
                args: expected_args,
                ..
            },
            Ty::Item {
                item_id: actual_item,
                args: actual_args,
                ..
            },
        ) => {
            expected_item == actual_item
                && expected_args.len() == actual_args.len()
                && expected_args
                    .iter()
                    .zip(actual_args)
                    .all(|(expected, actual)| {
                        backend_value_compatible_inner(
                            module,
                            resolution,
                            expected,
                            actual,
                            depth + 1,
                        )
                    })
        }
        (
            Ty::Import {
                path: expected_path,
                args: expected_args,
            },
            Ty::Import {
                path: actual_path,
                args: actual_args,
            },
        )
        | (
            Ty::Named {
                path: expected_path,
                args: expected_args,
            },
            Ty::Named {
                path: actual_path,
                args: actual_args,
            },
        ) => {
            expected_path == actual_path
                && expected_args.len() == actual_args.len()
                && expected_args
                    .iter()
                    .zip(actual_args)
                    .all(|(expected, actual)| {
                        backend_value_compatible_inner(
                            module,
                            resolution,
                            expected,
                            actual,
                            depth + 1,
                        )
                    })
        }
        (
            Ty::Pointer {
                is_const: expected_const,
                inner: expected_inner,
            },
            Ty::Pointer {
                is_const: actual_const,
                inner: actual_inner,
            },
        ) => {
            expected_const == actual_const
                && backend_value_compatible_inner(
                    module,
                    resolution,
                    expected_inner,
                    actual_inner,
                    depth + 1,
                )
        }
        (Ty::Tuple(expected_items), Ty::Tuple(actual_items)) => {
            expected_items.len() == actual_items.len()
                && expected_items
                    .iter()
                    .zip(actual_items)
                    .all(|(expected, actual)| {
                        backend_value_compatible_inner(
                            module,
                            resolution,
                            expected,
                            actual,
                            depth + 1,
                        )
                    })
        }
        (Ty::TaskHandle(expected), Ty::TaskHandle(actual)) => {
            backend_value_compatible_inner(module, resolution, expected, actual, depth + 1)
        }
        (
            Ty::Callable {
                params: expected_params,
                ret: expected_ret,
            },
            Ty::Callable {
                params: actual_params,
                ret: actual_ret,
            },
        ) => {
            expected_params.len() == actual_params.len()
                && expected_params
                    .iter()
                    .zip(actual_params)
                    .all(|(expected, actual)| {
                        backend_value_compatible_inner(
                            module,
                            resolution,
                            expected,
                            actual,
                            depth + 1,
                        )
                    })
                && backend_value_compatible_inner(
                    module,
                    resolution,
                    expected_ret,
                    actual_ret,
                    depth + 1,
                )
        }
        _ => false,
    }
}

fn transparent_backend_value_alias_target(
    module: &hir::Module,
    resolution: &ResolutionMap,
    ty: &Ty,
) -> Option<Ty> {
    let Ty::Item { item_id, args, .. } = ty else {
        return None;
    };
    if !args.is_empty() {
        return None;
    }

    match &module.item(*item_id).kind {
        ItemKind::TypeAlias(alias) if !alias.is_opaque && alias.generics.is_empty() => {
            Some(lower_type(module, resolution, alias.ty))
        }
        _ => None,
    }
}

fn transparent_backend_value_ty(module: &hir::Module, resolution: &ResolutionMap, ty: &Ty) -> Ty {
    transparent_backend_value_ty_inner(module, resolution, ty, 0)
}

fn transparent_backend_value_ty_inner(
    module: &hir::Module,
    resolution: &ResolutionMap,
    ty: &Ty,
    depth: usize,
) -> Ty {
    if depth >= MAX_TRANSPARENT_BACKEND_ALIAS_DEPTH {
        return ty.clone();
    }

    if let Some(target) = transparent_backend_value_alias_target(module, resolution, ty) {
        transparent_backend_value_ty_inner(module, resolution, &target, depth + 1)
    } else {
        ty.clone()
    }
}

fn backend_value_is_bool(module: &hir::Module, resolution: &ResolutionMap, ty: &Ty) -> bool {
    transparent_backend_value_ty(module, resolution, ty).is_bool()
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

fn library_function_refs(module: &hir::Module, item_id: ItemId) -> Vec<FunctionRef> {
    match &module.item(item_id).kind {
        ItemKind::Function(function) if function.body.is_some() && function.generics.is_empty() => {
            vec![FunctionRef::Item(item_id)]
        }
        ItemKind::Trait(trait_decl) => trait_decl
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.body.is_some() && method.generics.is_empty())
            .map(|(index, _)| FunctionRef::TraitMethod {
                item: item_id,
                index,
            })
            .collect(),
        ItemKind::Impl(impl_block) => impl_block
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.body.is_some() && method.generics.is_empty())
            .map(|(index, _)| FunctionRef::ImplMethod {
                item: item_id,
                index,
            })
            .collect(),
        ItemKind::Extend(extend_block) => extend_block
            .methods
            .iter()
            .enumerate()
            .filter(|(_, method)| method.body.is_some() && method.generics.is_empty())
            .map(|(index, _)| FunctionRef::ExtendMethod {
                item: item_id,
                index,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn function_body_owner(function_ref: FunctionRef) -> BodyOwner {
    match function_ref {
        FunctionRef::Item(item) => BodyOwner::Item(item),
        FunctionRef::TraitMethod { item, index } => BodyOwner::TraitMethod { item, index },
        FunctionRef::ImplMethod { item, index } => BodyOwner::ImplMethod { item, index },
        FunctionRef::ExtendMethod { item, index } => BodyOwner::ExtendMethod { item, index },
        FunctionRef::ExternBlockMember { block, .. } => BodyOwner::Item(block),
    }
}

fn receiver_param_type(
    module: &hir::Module,
    resolution: &ResolutionMap,
    function_ref: FunctionRef,
) -> Option<Ty> {
    match function_ref {
        FunctionRef::ImplMethod { item, .. } => match &module.item(item).kind {
            ItemKind::Impl(impl_block) => Some(lower_type(module, resolution, impl_block.target)),
            _ => None,
        },
        FunctionRef::ExtendMethod { item, .. } => match &module.item(item).kind {
            ItemKind::Extend(extend_block) => {
                Some(lower_type(module, resolution, extend_block.target))
            }
            _ => None,
        },
        _ => None,
    }
}

fn lowered_param_index(local: &mir::LocalDecl) -> Option<usize> {
    match local.origin {
        LocalOrigin::Param { index } => Some(index),
        LocalOrigin::Receiver => Some(0),
        LocalOrigin::ReturnSlot | LocalOrigin::Binding(_) | LocalOrigin::Temp { .. } => None,
    }
}

fn function_sort_key(function_ref: FunctionRef) -> (usize, usize, usize) {
    match function_ref {
        FunctionRef::Item(item_id) => (item_id.index(), 0, 0),
        FunctionRef::TraitMethod { item, index } => (item.index(), 1, index),
        FunctionRef::ImplMethod { item, index } => (item.index(), 2, index),
        FunctionRef::ExtendMethod { item, index } => (item.index(), 3, index),
        FunctionRef::ExternBlockMember { block, index } => (block.index(), 4, index),
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
mod tests;
