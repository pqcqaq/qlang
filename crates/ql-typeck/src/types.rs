use std::fmt;

use ql_hir::{Function, FunctionRef, ItemId, ItemKind, Module, Param, TypeId, TypeKind};
use ql_resolve::{BuiltinType, ResolutionMap, TypeResolution};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ty {
    Unknown,
    Builtin(BuiltinType),
    Generic(String),
    Item {
        item_id: ItemId,
        name: String,
        args: Vec<Ty>,
    },
    Import {
        path: String,
        args: Vec<Ty>,
    },
    Named {
        path: String,
        args: Vec<Ty>,
    },
    Pointer {
        is_const: bool,
        inner: Box<Ty>,
    },
    Tuple(Vec<Ty>),
    Callable {
        params: Vec<Ty>,
        ret: Box<Ty>,
    },
}

impl Ty {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Builtin(BuiltinType::Bool))
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Builtin(
                BuiltinType::Int
                    | BuiltinType::UInt
                    | BuiltinType::I8
                    | BuiltinType::I16
                    | BuiltinType::I32
                    | BuiltinType::I64
                    | BuiltinType::ISize
                    | BuiltinType::U8
                    | BuiltinType::U16
                    | BuiltinType::U32
                    | BuiltinType::U64
                    | BuiltinType::USize
                    | BuiltinType::F32
                    | BuiltinType::F64
            )
        )
    }

    pub fn compatible_with(&self, actual: &Ty) -> bool {
        match (self, actual) {
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            (Ty::Builtin(left), Ty::Builtin(right)) => left == right,
            (Ty::Generic(_), _) | (_, Ty::Generic(_)) => true,
            (
                Ty::Item {
                    item_id: left_item,
                    args: left_args,
                    ..
                },
                Ty::Item {
                    item_id: right_item,
                    args: right_args,
                    ..
                },
            ) => {
                left_item == right_item
                    && left_args.len() == right_args.len()
                    && left_args
                        .iter()
                        .zip(right_args)
                        .all(|(left, right)| left.compatible_with(right))
            }
            (
                Ty::Import {
                    path: left_path,
                    args: left_args,
                },
                Ty::Import {
                    path: right_path,
                    args: right_args,
                },
            )
            | (
                Ty::Named {
                    path: left_path,
                    args: left_args,
                },
                Ty::Named {
                    path: right_path,
                    args: right_args,
                },
            ) => {
                left_path == right_path
                    && left_args.len() == right_args.len()
                    && left_args
                        .iter()
                        .zip(right_args)
                        .all(|(left, right)| left.compatible_with(right))
            }
            (
                Ty::Pointer {
                    is_const: left_const,
                    inner: left_inner,
                },
                Ty::Pointer {
                    is_const: right_const,
                    inner: right_inner,
                },
            ) => left_const == right_const && left_inner.compatible_with(right_inner),
            (Ty::Tuple(left_items), Ty::Tuple(right_items)) => {
                left_items.len() == right_items.len()
                    && left_items
                        .iter()
                        .zip(right_items)
                        .all(|(left, right)| left.compatible_with(right))
            }
            (
                Ty::Callable {
                    params: left_params,
                    ret: left_ret,
                },
                Ty::Callable {
                    params: right_params,
                    ret: right_ret,
                },
            ) => {
                left_params.len() == right_params.len()
                    && left_params
                        .iter()
                        .zip(right_params)
                        .all(|(left, right)| left.compatible_with(right))
                    && left_ret.compatible_with(right_ret)
            }
            _ => false,
        }
    }

    pub fn from_function(module: &Module, resolution: &ResolutionMap, function: &Function) -> Ty {
        let params = function
            .params
            .iter()
            .filter_map(|param| match param {
                Param::Regular(param) => Some(lower_type(module, resolution, param.ty)),
                Param::Receiver(_) => None,
            })
            .collect();
        let ret = Box::new(
            function
                .return_type
                .map(|type_id| lower_type(module, resolution, type_id))
                .unwrap_or_else(void_ty),
        );

        Ty::Callable { params, ret }
    }

    pub fn from_function_ref(
        module: &Module,
        resolution: &ResolutionMap,
        function_ref: FunctionRef,
    ) -> Ty {
        Self::from_function(module, resolution, module.function(function_ref))
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Unknown => f.write_str("<unknown>"),
            Ty::Builtin(builtin) => f.write_str(builtin_name(*builtin)),
            Ty::Generic(name) => f.write_str(name),
            Ty::Item { name, args, .. } => write_named(f, name, args),
            Ty::Import { path, args } | Ty::Named { path, args } => write_named(f, path, args),
            Ty::Pointer { is_const, inner } => {
                if *is_const {
                    write!(f, "*const {inner}")
                } else {
                    write!(f, "*{inner}")
                }
            }
            Ty::Tuple(items) => {
                f.write_str("(")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                if items.len() == 1 {
                    f.write_str(",")?;
                }
                f.write_str(")")
            }
            Ty::Callable { params, ret } => {
                f.write_str("(")?;
                for (index, param) in params.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, ") -> {ret}")
            }
        }
    }
}

pub fn lower_type(module: &Module, resolution: &ResolutionMap, type_id: TypeId) -> Ty {
    let ty = module.ty(type_id);
    match &ty.kind {
        TypeKind::Pointer { is_const, inner } => Ty::Pointer {
            is_const: *is_const,
            inner: Box::new(lower_type(module, resolution, *inner)),
        },
        TypeKind::Named { path, args } => {
            let args = args
                .iter()
                .map(|&arg| lower_type(module, resolution, arg))
                .collect();
            match resolution.type_resolution(type_id) {
                Some(TypeResolution::Builtin(builtin)) => Ty::Builtin(*builtin),
                Some(TypeResolution::Generic(_)) => Ty::Generic(path.segments.join(".")),
                Some(TypeResolution::Item(item_id)) => Ty::Item {
                    item_id: *item_id,
                    name: item_display_name(module, *item_id),
                    args,
                },
                Some(TypeResolution::Import(import_path)) => Ty::Import {
                    path: import_path.segments.join("."),
                    args,
                },
                None => Ty::Named {
                    path: path.segments.join("."),
                    args,
                },
            }
        }
        TypeKind::Tuple(items) => Ty::Tuple(
            items
                .iter()
                .map(|&item| lower_type(module, resolution, item))
                .collect(),
        ),
        TypeKind::Callable { params, ret } => Ty::Callable {
            params: params
                .iter()
                .map(|&param| lower_type(module, resolution, param))
                .collect(),
            ret: Box::new(lower_type(module, resolution, *ret)),
        },
    }
}

pub fn item_display_name(module: &Module, item_id: ItemId) -> String {
    match &module.item(item_id).kind {
        ItemKind::Function(function) => function.name.clone(),
        ItemKind::Const(global) | ItemKind::Static(global) => global.name.clone(),
        ItemKind::Struct(struct_decl) => struct_decl.name.clone(),
        ItemKind::Enum(enum_decl) => enum_decl.name.clone(),
        ItemKind::Trait(trait_decl) => trait_decl.name.clone(),
        ItemKind::TypeAlias(alias) => alias.name.clone(),
        ItemKind::Impl(_) | ItemKind::Extend(_) | ItemKind::ExternBlock(_) => "<item>".to_owned(),
    }
}

pub fn void_ty() -> Ty {
    Ty::Builtin(BuiltinType::Void)
}

fn builtin_name(builtin: BuiltinType) -> &'static str {
    match builtin {
        BuiltinType::Bool => "Bool",
        BuiltinType::Char => "Char",
        BuiltinType::String => "String",
        BuiltinType::Bytes => "Bytes",
        BuiltinType::Void => "Void",
        BuiltinType::Never => "Never",
        BuiltinType::Int => "Int",
        BuiltinType::UInt => "UInt",
        BuiltinType::I8 => "I8",
        BuiltinType::I16 => "I16",
        BuiltinType::I32 => "I32",
        BuiltinType::I64 => "I64",
        BuiltinType::ISize => "ISize",
        BuiltinType::U8 => "U8",
        BuiltinType::U16 => "U16",
        BuiltinType::U32 => "U32",
        BuiltinType::U64 => "U64",
        BuiltinType::USize => "USize",
        BuiltinType::F32 => "F32",
        BuiltinType::F64 => "F64",
    }
}

fn write_named(f: &mut fmt::Formatter<'_>, name: &str, args: &[Ty]) -> fmt::Result {
    f.write_str(name)?;
    if !args.is_empty() {
        f.write_str("[")?;
        for (index, arg) in args.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{arg}")?;
        }
        f.write_str("]")?;
    }
    Ok(())
}
