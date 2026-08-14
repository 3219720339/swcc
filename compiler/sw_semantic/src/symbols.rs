use sw_common::Span;

use crate::types::Type;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SymbolId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ModuleId(pub u32);

#[derive(Clone, Debug)]
pub struct FunctionSig {
    pub module: ModuleId,
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<ParamSig>,
    pub ret: Type,
    pub extern_c: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ParamSig {
    pub name: String,
    pub ty: Type,
    pub has_default: bool,
    /// `...rest` 可变参数标记（只允许出现在最后一个参数）。
    pub rest: bool,
}

#[derive(Clone, Debug)]
pub struct FieldInfo {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct MethodInfo {
    pub name: String,
    pub sig: FunctionSig,
    pub virtual_: bool,
    pub override_: bool,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct StructInfo {
    pub module: ModuleId,
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<FieldInfo>,
}

#[derive(Clone, Debug)]
pub struct EnumInfo {
    pub module: ModuleId,
    pub name: String,
    pub members: Vec<(String, i64)>,
}

#[derive(Clone, Debug)]
pub struct ClassInfo {
    pub module: ModuleId,
    pub name: String,
    pub generics: Vec<String>,
    pub base: Option<u32>,
    pub fields: Vec<FieldInfo>,
    pub methods: Vec<MethodInfo>,
    pub final_: bool,
}

#[derive(Clone, Debug)]
pub struct InterfaceInfo {
    pub module: ModuleId,
    pub name: String,
    pub generics: Vec<String>,
    pub methods: Vec<FunctionSig>,
}

/// 类型表：struct/enum/class/interface 的聚合信息。
#[derive(Clone, Debug, Default)]
pub struct TypeTable {
    pub structs: Vec<StructInfo>,
    pub enums: Vec<EnumInfo>,
    pub classes: Vec<ClassInfo>,
    pub interfaces: Vec<InterfaceInfo>,
    /// class id → 其实现的接口 id 列表。
    pub class_interfaces: HashMap<u32, Vec<u32>>,
    /// (泛型 struct id, 类型实参) → 实例化 struct id。
    pub generic_struct_instances: HashMap<(u32, Vec<Type>), u32>,
    /// (泛型 class id, 类型实参) → 实例化 class id。
    pub generic_class_instances: HashMap<(u32, Vec<Type>), u32>,
}

impl TypeTable {
    pub fn struct_name(&self, id: u32) -> &str {
        &self.structs[id as usize].name
    }

    pub fn enum_name(&self, id: u32) -> &str {
        &self.enums[id as usize].name
    }

    pub fn class_name(&self, id: u32) -> &str {
        &self.classes[id as usize].name
    }

    pub fn interface_name(&self, id: u32) -> &str {
        &self.interfaces[id as usize].name
    }

    /// 在类及其基类链中查找字段。
    pub fn find_class_field(&self, class_id: u32, name: &str) -> Option<(u32, usize)> {
        let mut current = Some(class_id);
        while let Some(id) = current {
            let class = &self.classes[id as usize];
            if let Some(index) = class.fields.iter().position(|field| field.name == name) {
                return Some((id, index));
            }
            current = class.base;
        }
        None
    }

    /// 在类及其基类链中查找方法。
    pub fn find_class_method(&self, class_id: u32, name: &str) -> Option<(u32, usize)> {
        let mut current = Some(class_id);
        while let Some(id) = current {
            let class = &self.classes[id as usize];
            if let Some(index) = class.methods.iter().position(|method| method.name == name) {
                return Some((id, index));
            }
            current = class.base;
        }
        None
    }

    pub fn class_base_chain(&self, class_id: u32) -> Vec<u32> {
        let mut chain = Vec::new();
        let mut current = Some(class_id);
        while let Some(id) = current {
            chain.push(id);
            current = self.classes[id as usize].base;
        }
        chain
    }

    pub fn is_class_assignable_to(&self, from: u32, to: u32) -> bool {
        self.class_base_chain(from).contains(&to)
    }
}
