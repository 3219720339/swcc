//! 语义层类型系统。

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Void,
    Bool,
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
    /// `int`，与 `isize` 同宽但保持独立以改善错误信息。
    Int,
    /// `uint`，与 `usize` 同宽。
    UInt,
    F32,
    F64,
    Char,
    Str,
    /// `null` 字面量的类型，只能赋给可空类型或引用类型。
    Null,
    Array(Box<Type>),
    Nullable(Box<Type>),
    Ptr(Box<Type>),
    Struct(u32),
    Enum(u32),
    Class(u32),
    Interface(u32),
    /// 泛型参数。
    TypeParam(String),
    /// 函数/回调类型。
    Function {
        params: Vec<Type>,
        ret: Box<Type>,
    },
    /// 推断尚未完成。
    Unknown,
    /// 类型错误占位，避免级联诊断。
    Error,
}

impl Type {
    pub fn display(&self) -> String {
        match self {
            Type::Void => "void".to_owned(),
            Type::Bool => "bool".to_owned(),
            Type::I8 => "i8".to_owned(),
            Type::I16 => "i16".to_owned(),
            Type::I32 => "i32".to_owned(),
            Type::I64 => "i64".to_owned(),
            Type::Isize => "isize".to_owned(),
            Type::U8 => "u8".to_owned(),
            Type::U16 => "u16".to_owned(),
            Type::U32 => "u32".to_owned(),
            Type::U64 => "u64".to_owned(),
            Type::Usize => "usize".to_owned(),
            Type::Int => "int".to_owned(),
            Type::UInt => "uint".to_owned(),
            Type::F32 => "f32".to_owned(),
            Type::F64 => "f64".to_owned(),
            Type::Char => "char".to_owned(),
            Type::Str => "string".to_owned(),
            Type::Null => "null".to_owned(),
            Type::Array(inner) => format!("{}[]", inner.display()),
            Type::Nullable(inner) => format!("{}?", inner.display()),
            Type::Ptr(inner) => format!("ptr<{}>", inner.display()),
            Type::Struct(id) => format!("struct#{id}"),
            Type::Enum(id) => format!("enum#{id}"),
            Type::Class(id) => format!("class#{id}"),
            Type::Interface(id) => format!("interface#{id}"),
            Type::TypeParam(name) => name.clone(),
            Type::Function { .. } => "function".to_owned(),
            Type::Unknown => "未知类型".to_owned(),
            Type::Error => "错误类型".to_owned(),
        }
    }

    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::Isize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::Usize
                | Type::Int
                | Type::UInt
                | Type::Char
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::Isize | Type::Int
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Type::F32 | Type::F64)
    }

    pub fn is_numeric(&self) -> bool {
        self.is_integer() || self.is_float()
    }

    pub fn is_reference(&self) -> bool {
        matches!(
            self,
            Type::Str | Type::Array(_) | Type::Class(_) | Type::Ptr(_) | Type::Null
        )
    }

    /// 可空性基类型：`T?` 返回 `T`，其余返回自身。
    pub fn without_nullable(&self) -> &Type {
        match self {
            Type::Nullable(inner) => inner,
            other => other,
        }
    }
}
