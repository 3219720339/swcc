use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use sw_common::{Diagnostics, Source, Span};
use sw_frontend::Parser;
use sw_frontend::ast::*;

use crate::mir::*;
use crate::symbols::*;
use crate::types::Type;

/// 一次完整语义分析的输出。
pub struct AnalysisResult {
    pub diagnostics: Diagnostics,
    pub modules: Vec<MirModule>,
    pub type_table: TypeTable,
    /// 模块路径与源码文本（供诊断渲染行列号）。
    pub module_sources: Vec<(PathBuf, String)>,
}

pub fn analyze(entry: &Path, stdlib_dir: Option<&Path>) -> AnalysisResult {
    let mut analyzer = Analyzer::new(stdlib_dir);
    let entry_path = normalize_path(entry);
    if analyzer.load_module(&entry_path, true).is_err() {
        // 加载失败的诊断已记录。
    }
    // 两阶段：先加载/绑定所有模块（含循环 import），再统一 finalize/检查/降级，
    // 保证循环依赖下被引用模块的符号签名已完整。
    // 逆序 finalize：被依赖模块（import 链上的 std/用户库）后加载，
    // 先完成其类型解析，再 finalize 使用者；循环依赖下 finalize 只解析
    // 本模块声明，不依赖对方的 finalize 状态，因此安全。
    let module_ids: Vec<ModuleId> = analyzer.states.iter().map(|state| state.id).collect();
    for id in module_ids.iter().rev() {
        if analyzer.finalize(*id).is_err() {
            break;
        }
    }
    for id in &module_ids {
        analyzer.check_globals(*id);
        analyzer.check_all_functions(*id);
    }
    for id in module_ids {
        analyzer.lower_mir(id);
    }
    let modules = analyzer
        .states
        .iter()
        .filter_map(|state| state.mir.clone())
        .collect();
    let module_sources = analyzer
        .states
        .iter()
        .map(|state| (state.path.clone(), state.source_text.clone()))
        .collect();
    AnalysisResult {
        diagnostics: analyzer.diagnostics,
        modules,
        type_table: analyzer.types,
        module_sources,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let path = if path.extension().is_none() {
        path.with_extension("sw")
    } else {
        path.to_path_buf()
    };
    fs::canonicalize(&path).unwrap_or(path)
}

#[derive(Clone, Debug)]
enum SymbolKind {
    Function(FunctionSig),
    Type(SymbolType),
    Global { ty: Type, mutable: bool },
    Namespace(ModuleId),
    Local { ty: Type, mutable: bool },
    Param { ty: Type },
}

#[derive(Clone, Debug)]
enum SymbolType {
    Struct(u32),
    Enum(u32),
    Class(u32),
    Interface(u32),
    Alias(Type),
}

#[derive(Clone, Debug)]
struct Symbol {
    kind: SymbolKind,
    exported: bool,
    span: Span,
    /// 声明该符号的模块（跨模块全局变量导入时用于生成链接符号名）。
    module: ModuleId,
}

struct ModuleState {
    id: ModuleId,
    path: PathBuf,
    source_text: String,
    names: HashMap<String, Vec<SymbolId>>,
    span_symbols: HashMap<usize, SymbolId>,
    result: CheckResult,
    mir: Option<MirModule>,
    mir_strings: Vec<String>,
}

#[derive(Clone, Default)]
struct CheckResult {
    expr_types: HashMap<(usize, usize), Type>,
    ident_symbols: HashMap<usize, SymbolId>,
    call_targets: HashMap<(usize, usize), CallTarget>,
    field_targets: HashMap<usize, FieldTarget>,
    new_types: HashMap<usize, Type>,
    object_types: HashMap<usize, Type>,
    /// 表达式起始偏移 → 目标枚举 id（带注解声明传播，供泛型枚举构造推断）。
    enum_targets: HashMap<usize, u32>,
    /// 函数声明起始偏移 → 所属类（方法）。
    method_classes: HashMap<usize, u32>,
    /// 类静态成员访问 `Class.field` / `Class.method`（表达式起始 → 目标）。
    static_member_targets: HashMap<usize, StaticMemberTarget>,
}

#[derive(Clone, Debug)]
enum CallTarget {
    Function(SymbolId),
    Method {
        class: u32,
        index: usize,
    },
    InterfaceMethod {
        interface: u32,
        index: usize,
    },
    /// 字符串/字符串数组内建方法：接收者作为第一个参数，按 extern 符号调用。
    StrMethod {
        runtime_name: String,
        sig: FunctionSig,
    },
    /// ADT 枚举变体构造：`EnumName.Variant(args)`。
    EnumConstruct {
        enum_id: u32,
        variant_index: usize,
    },
    /// 数组迭代器方法（编译器内联循环）：map/filter。
    ArrayMethod {
        method: ArrayMethodKind,
        elem: Type,
        ret: Type,
    },
    /// 类静态方法调用：`ClassName.staticMethod(...)`。
    StaticMethod {
        class: u32,
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArrayMethodKind {
    Map,
    Filter,
    ForEach,
    Some,
    Every,
    Find,
    Push,
    Pop,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum FieldTarget {
    Struct(u32, usize),
    Class(u32, usize),
}

#[derive(Clone, Copy, Debug)]
enum StaticMemberTarget {
    Field(u32, usize),
    Method(u32, usize),
}

struct Analyzer {
    diagnostics: Diagnostics,
    types: TypeTable,
    symbols: Vec<Symbol>,
    modules: Vec<Module>,
    states: Vec<ModuleState>,
    /// "module_id:name" → 已导出的类型。
    registry: HashMap<String, Type>,
    stdlib_dir: Option<PathBuf>,
    loading: HashSet<PathBuf>,
    loaded: HashMap<PathBuf, ModuleId>,
    module_names: HashMap<ModuleId, HashMap<String, Vec<SymbolId>>>,
    current_path: PathBuf,
    next_module_id: u32,
}

impl Analyzer {
    fn new(stdlib_dir: Option<&Path>) -> Self {
        Self {
            diagnostics: Diagnostics::new(),
            types: TypeTable::default(),
            symbols: Vec::new(),
            modules: Vec::new(),
            states: Vec::new(),
            registry: HashMap::new(),
            stdlib_dir: stdlib_dir.map(Path::to_path_buf),
            loading: HashSet::new(),
            loaded: HashMap::new(),
            module_names: HashMap::new(),
            current_path: PathBuf::new(),
            next_module_id: 0,
        }
    }

    fn state(&self, module_id: ModuleId) -> &ModuleState {
        &self.states[module_id.0 as usize]
    }

    fn state_mut(&mut self, module_id: ModuleId) -> &mut ModuleState {
        &mut self.states[module_id.0 as usize]
    }

    fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    fn alloc_symbol(&mut self, module: ModuleId) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            kind: SymbolKind::Global {
                ty: Type::Error,
                mutable: false,
            },
            exported: false,
            span: Span::empty(0),
            module,
        });
        id
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.diagnostics.error(message, Some(span));
    }

    /// 加载并完整分析一个模块（入口或导入）。
    fn load_module(&mut self, path: &Path, is_entry: bool) -> Result<ModuleId, ()> {
        if let Some(id) = self.loaded.get(path) {
            return Ok(*id);
        }
        if self.loading.contains(path) {
            self.error("检测到循环导入", Span::empty(0));
            return Err(());
        }
        self.loading.insert(path.to_path_buf());

        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                self.error(
                    format!("无法读取模块 `{}`：{error}", path.display()),
                    Span::empty(0),
                );
                self.loading.remove(path);
                return Err(());
            }
        };
        self.current_path = path.to_path_buf();
        let source_text = text.clone();
        let source = Source::new(path.to_path_buf(), text);
        let mut diagnostics = Diagnostics::new();
        let mut parser = Parser::new(&source, &mut diagnostics);
        let module = parser.parse_module();
        self.diagnostics.extend(diagnostics);

        let id = ModuleId(self.next_module_id);
        self.next_module_id += 1;
        self.modules.push(module);
        self.states.push(ModuleState {
            id,
            path: path.to_path_buf(),
            source_text,
            names: HashMap::new(),
            span_symbols: HashMap::new(),
            result: CheckResult::default(),
            mir: None,
            mir_strings: Vec::new(),
        });
        self.loaded.insert(path.to_path_buf(), id);

        self.predeclare(id)?;
        self.resolve_imports(id)?;
        self.module_names.insert(id, self.state(id).names.clone());

        self.loading.remove(path);
        let _ = is_entry;
        Ok(id)
    }

    /// 第一遍：为所有顶层名字预留符号（两遍解析以支持前向引用）。
    fn predeclare(&mut self, module_id: ModuleId) -> Result<(), ()> {
        let names: Vec<(String, SymbolKind, Span, bool)> = {
            let module = &self.modules[module_id.0 as usize];
            module
                .items
                .iter()
                .filter_map(|item| {
                    let span = item.span;
                    let exported = item.exported;
                    let kind = match &item.kind {
                        ItemKind::Function(function) => Some((
                            function.name.name.clone(),
                            SymbolKind::Function(placeholder_sig()),
                            span,
                        )),
                        ItemKind::Struct(structure) => {
                            let id = self.types.structs.len() as u32;
                            self.types.structs.push(StructInfo {
                                module: module_id,
                                name: structure.name.name.clone(),
                                generics: Vec::new(),
                                fields: Vec::new(),
                            });
                            Some((
                                structure.name.name.clone(),
                                SymbolKind::Type(SymbolType::Struct(id)),
                                span,
                            ))
                        }
                        ItemKind::Enum(enumeration) => {
                            let id = self.types.enums.len() as u32;
                            self.types.enums.push(EnumInfo {
                                module: module_id,
                                name: enumeration.name.name.clone(),
                                generics: Vec::new(),
                                members: Vec::new(),
                            });
                            Some((
                                enumeration.name.name.clone(),
                                SymbolKind::Type(SymbolType::Enum(id)),
                                span,
                            ))
                        }
                        ItemKind::Class(class) => {
                            let id = self.types.classes.len() as u32;
                            self.types.classes.push(ClassInfo {
                                module: module_id,
                                name: class.name.name.clone(),
                                generics: Vec::new(),
                                base: None,
                                fields: Vec::new(),
                                methods: Vec::new(),
                                static_fields: Vec::new(),
                                static_methods: Vec::new(),
                                final_: class.final_,
                                implements: Vec::new(),
                            });
                            Some((
                                class.name.name.clone(),
                                SymbolKind::Type(SymbolType::Class(id)),
                                span,
                            ))
                        }
                        ItemKind::Interface(interface) => {
                            let id = self.types.interfaces.len() as u32;
                            self.types.interfaces.push(InterfaceInfo {
                                module: module_id,
                                name: interface.name.name.clone(),
                                generics: Vec::new(),
                                methods: Vec::new(),
                            });
                            Some((
                                interface.name.name.clone(),
                                SymbolKind::Type(SymbolType::Interface(id)),
                                span,
                            ))
                        }
                        ItemKind::TypeAlias(alias) => Some((
                            alias.name.name.clone(),
                            SymbolKind::Type(SymbolType::Alias(Type::Error)),
                            span,
                        )),
                        ItemKind::Variable(variable) => Some((
                            variable.name.name.clone(),
                            SymbolKind::Global {
                                ty: Type::Error,
                                mutable: variable.kind == VarKind::Let,
                            },
                            span,
                        )),
                        ItemKind::Import(_) => None,
                    };
                    kind.map(|(name, kind, span)| (name, kind, span, exported))
                })
                .collect()
        };

        for (name, kind, span, exported) in names {
            if let Some(existing_ids) = self.state(module_id).names.get(&name) {
                let existing_is_function = existing_ids
                    .first()
                    .map(|id| matches!(self.symbols[id.0 as usize].kind, SymbolKind::Function(_)))
                    .unwrap_or(false);
                let is_function = matches!(kind, SymbolKind::Function(_));
                if is_function && existing_is_function {
                    // 顶层函数允许重载
                    let id = self.alloc_symbol(module_id);
                    if let Some(existing) = self.symbols.get_mut(id.0 as usize) {
                        existing.kind = kind;
                        existing.exported = exported;
                        existing.span = span;
                    }
                    self.state_mut(module_id)
                        .span_symbols
                        .insert(span.start, id);
                    self.state_mut(module_id)
                        .names
                        .get_mut(&name)
                        .expect("名称存在")
                        .push(id);
                    continue;
                }
                self.error(format!("名称 `{name}` 重复声明"), span);
                continue;
            }
            let id = self.alloc_symbol(module_id);
            if let Some(existing) = self.symbols.get_mut(id.0 as usize) {
                existing.kind = kind;
                existing.exported = exported;
                existing.span = span;
            }
            self.state_mut(module_id)
                .span_symbols
                .insert(span.start, id);
            self.state_mut(module_id).names.insert(name, vec![id]);
        }
        Ok(())
    }

    /// 第二遍：解析导入并绑定名字。
    fn resolve_imports(&mut self, module_id: ModuleId) -> Result<(), ()> {
        let imports: Vec<ImportDecl> = {
            let module = &self.modules[module_id.0 as usize];
            module
                .items
                .iter()
                .filter_map(|item| match &item.kind {
                    ItemKind::Import(import) => Some(import.clone()),
                    _ => None,
                })
                .collect()
        };
        for import in imports {
            let target = match self.resolve_import_path(module_id, &import.path) {
                Some(target) => target,
                None => continue,
            };
            let target_id = match self.load_module(&target, false) {
                Ok(id) => id,
                Err(()) => continue,
            };
            match &import.kind {
                ImportKind::SideEffect => {
                    // 升级语义：`import "path"` 以文件名作为命名空间导入。
                    let stem = import
                        .path
                        .rsplit(['/', '\\'])
                        .next()
                        .unwrap_or(&import.path);
                    let stem = stem.strip_suffix(".sw").unwrap_or(stem).to_string();
                    let id = SymbolId(self.symbols.len() as u32);
                    self.symbols.push(Symbol {
                        kind: SymbolKind::Namespace(target_id),
                        exported: false,
                        span: import.span,
                        module: module_id,
                    });
                    self.bind_import(module_id, stem, vec![id], import.span)?;
                }
                ImportKind::Named(specifiers) => {
                    for specifier in specifiers {
                        let target_names = self.state(target_id).names.clone();
                        let Some(symbol_ids) = target_names.get(&specifier.name.name) else {
                            self.error(
                                format!("模块没有导出名称 `{}`", specifier.name.name),
                                specifier.name.span,
                            );
                            continue;
                        };
                        let symbol_ids = symbol_ids.clone();
                        for symbol_id in &symbol_ids {
                            if !self.symbol(*symbol_id).exported {
                                self.error(
                                    format!("名称 `{}` 未从模块导出", specifier.name.name),
                                    specifier.name.span,
                                );
                            }
                        }
                        let alias = specifier
                            .alias
                            .as_ref()
                            .unwrap_or(&specifier.name)
                            .name
                            .clone();
                        self.bind_import(module_id, alias, symbol_ids, specifier.name.span)?;
                    }
                }
                ImportKind::Namespace(alias) => {
                    let id = SymbolId(self.symbols.len() as u32);
                    self.symbols.push(Symbol {
                        kind: SymbolKind::Namespace(target_id),
                        exported: false,
                        span: alias.span,
                        module: module_id,
                    });
                    self.bind_import(module_id, alias.name.clone(), vec![id], alias.span)?;
                }
                ImportKind::Wildcard => {
                    let target_names = self.state(target_id).names.clone();
                    for (name, symbol_ids) in target_names {
                        let symbol_ids = symbol_ids.clone();
                        if symbol_ids.iter().all(|id| self.symbol(*id).exported) {
                            self.bind_import(module_id, name, symbol_ids, import.span)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn bind_import(
        &mut self,
        module_id: ModuleId,
        alias: String,
        symbol_ids: Vec<SymbolId>,
        span: Span,
    ) -> Result<(), ()> {
        if self.state(module_id).names.contains_key(&alias) {
            self.error(format!("导入名称 `{alias}` 与已有名称冲突"), span);
            return Err(());
        }
        self.state_mut(module_id).names.insert(alias, symbol_ids);
        Ok(())
    }

    fn resolve_import_path(&mut self, module_id: ModuleId, path: &str) -> Option<PathBuf> {
        if let Some(rest) = path.strip_prefix("./") {
            let base = self.state(module_id).path.parent()?;
            let mut candidate = base.join(rest);
            if candidate.extension().is_none() {
                candidate.set_extension("sw");
            }
            Some(normalize_path(&candidate))
        } else if let Some(rest) = path.strip_prefix("std/") {
            let stdlib = self.stdlib_dir.as_ref()?;
            let mut candidate = stdlib.join(rest);
            if candidate.extension().is_none() {
                candidate.set_extension("sw");
            }
            Some(candidate)
        } else {
            self.diagnostics.error(
                format!("不支持的导入路径 `{path}`（v0.1 支持 `./相对路径` 与 `std/...`）"),
                Some(Span::empty(0)),
            );
            None
        }
    }

    /// 第三遍：填充类型细节、函数签名与全局变量类型。
    fn finalize(&mut self, module_id: ModuleId) -> Result<(), ()> {
        let generics = Vec::new();
        let items = self.modules[module_id.0 as usize].items.clone();
        for item in &items {
            match &item.kind {
                ItemKind::Struct(structure) => {
                    let generics = structure
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    let fields = self.finalize_fields(module_id, &structure.fields, &generics);
                    let id = self.type_id_for(module_id, &structure.name.name);
                    if let Some(SymbolKind::Type(SymbolType::Struct(id))) = id {
                        self.types.structs[id as usize].generics = generics;
                        self.types.structs[id as usize].fields = fields;
                        if self.types.structs[id as usize]
                            .fields
                            .iter()
                            .any(|field| matches!(field.ty, Type::Struct(inner) if inner == id))
                        {
                            self.error(
                                "struct 不能包含自身的值类型字段（会造成无限大小）",
                                structure.span,
                            );
                        }
                    }
                }
                ItemKind::Enum(enumeration) => {
                    let id = self.type_id_for(module_id, &enumeration.name.name);
                    let generics = enumeration
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    let mut resolver = TypeResolver::new(
                        &self.symbols,
                        &mut self.types,
                        &self.registry,
                        &self.states[module_id.0 as usize].names,
                    );
                    let field_tys: Vec<Vec<Type>> = enumeration
                        .members
                        .iter()
                        .map(|member| {
                            member
                                .fields
                                .iter()
                                .map(|ty| resolver.lower(ty, &generics))
                                .collect()
                        })
                        .collect();
                    let mut members = Vec::new();
                    let mut next_value = 0i64;
                    for (index, member) in enumeration.members.iter().enumerate() {
                        let value = match &member.value {
                            Some(expr) => match self.const_int(expr) {
                                Some(value) => value,
                                None => {
                                    self.error("枚举成员的值必须是整数常量", member.span);
                                    next_value
                                }
                            },
                            None => next_value,
                        };
                        if member.value.is_some() && !member.fields.is_empty() {
                            self.error("枚举变体不能同时有判别值 `=` 和字段", member.span);
                        }
                        members.push(EnumVariant {
                            name: member.name.name.clone(),
                            discriminant: value,
                            fields: field_tys[index].clone(),
                        });
                        next_value = value + 1;
                    }
                    if let Some(SymbolKind::Type(SymbolType::Enum(id))) = id {
                        self.types.enums[id as usize].generics = generics;
                        self.types.enums[id as usize].members = members;
                    }
                }
                ItemKind::Class(class) => {
                    let generics = class
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    let mut base = None;
                    if let Some(extends) = &class.extends {
                        let mut resolver = TypeResolver::new(
                            &self.symbols,
                            &mut self.types,
                            &self.registry,
                            &self.states[module_id.0 as usize].names,
                        );
                        match resolver.lower(extends, &generics) {
                            Type::Class(id) => base = Some(id),
                            other => {
                                self.error(
                                    format!("`extends` 只能继承 class，实际为 {}", other.display()),
                                    extends.span,
                                );
                            }
                        }
                    }
                    let id = self.type_id_for(module_id, &class.name.name);
                    if let Some(SymbolKind::Type(SymbolType::Class(id))) = id {
                        let info = &mut self.types.classes[id as usize];
                        info.generics = generics.clone();
                        info.base = base;
                    }
                    let class_id = match &id {
                        Some(SymbolKind::Type(SymbolType::Class(id))) => Some(*id),
                        _ => None,
                    };
                    let (fields, methods, static_fields, static_methods) = self
                        .finalize_class_members(module_id, class, &generics, class_id.unwrap_or(0));
                    if let Some(id) = class_id {
                        {
                            let info = &mut self.types.classes[id as usize];
                            info.fields = fields;
                            info.methods = methods;
                            info.static_fields = static_fields;
                            info.static_methods = static_methods;
                        }
                        // 接口实现：解析 implements 并校验方法覆盖。
                        let mut template_implements: Vec<(u32, Vec<Type>)> = Vec::new();
                        let resolved: Vec<(u32, &TypeRef)> = {
                            let mut resolver = TypeResolver::new(
                                &self.symbols,
                                &mut self.types,
                                &self.registry,
                                &self.states[module_id.0 as usize].names,
                            );
                            class
                                .implements
                                .iter()
                                .map(|iface_ref| {
                                    let segment = iface_ref.segments.first();
                                    let args: Vec<Type> = segment
                                        .map(|segment| {
                                            segment
                                                .generics
                                                .iter()
                                                .map(|arg| resolver.lower(arg, &generics))
                                                .collect()
                                        })
                                        .unwrap_or_default();
                                    let template_id = segment.and_then(|segment| {
                                        resolver
                                            .names
                                            .get(&segment.name.name)
                                            .and_then(|ids| ids.first())
                                            .and_then(|id| {
                                                match &resolver.symbols[id.0 as usize].kind {
                                                    SymbolKind::Type(SymbolType::Interface(id)) => {
                                                        Some(*id)
                                                    }
                                                    _ => None,
                                                }
                                            })
                                    });
                                    let Some(template_id) = template_id else {
                                        return (u32::MAX, iface_ref);
                                    };
                                    if !generics.is_empty() {
                                        // 泛型类：暂存接口模板 + 实参（可含 T）。
                                        template_implements.push((template_id, args));
                                        return (u32::MAX, iface_ref);
                                    }
                                    let iface_is_generic = !resolver.types.interfaces
                                        [template_id as usize]
                                        .generics
                                        .is_empty();
                                    if iface_is_generic {
                                        (
                                            resolver.instantiate_interface(template_id, &args),
                                            iface_ref,
                                        )
                                    } else {
                                        (template_id, iface_ref)
                                    }
                                })
                                .collect()
                        };
                        let mut implemented = Vec::new();
                        for (iface_id, iface_ref) in resolved {
                            if iface_id == u32::MAX {
                                if !generics.is_empty() {
                                    continue;
                                }
                                self.error("`implements` 目标必须是接口", iface_ref.span);
                                continue;
                            }
                            let iface_methods: Vec<String> = self.types.interfaces
                                [iface_id as usize]
                                .methods
                                .iter()
                                .map(|method| method.name.clone())
                                .collect();
                            let class_name = self.types.class_name(id).to_string();
                            let iface_name = self.types.interfaces[iface_id as usize].name.clone();
                            for method_name in &iface_methods {
                                if self.types.find_class_method(id, method_name).is_none() {
                                    self.error(
                                        format!(
                                            "类 {} 未实现接口 {} 的方法 `{}`",
                                            class_name, iface_name, method_name
                                        ),
                                        class.span,
                                    );
                                }
                            }
                            implemented.push(iface_id);
                        }
                        if generics.is_empty() {
                            self.types.class_interfaces.insert(id, implemented);
                        } else {
                            let info = &mut self.types.classes[id as usize];
                            info.implements = template_implements;
                        }
                    }
                }
                ItemKind::Interface(interface) => {
                    let generics = interface
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    let mut methods = Vec::new();
                    for method in &interface.methods {
                        methods.push(self.build_function_sig(module_id, method, &generics, None));
                    }
                    let id = self.type_id_for(module_id, &interface.name.name);
                    if let Some(SymbolKind::Type(SymbolType::Interface(id))) = id {
                        let info = &mut self.types.interfaces[id as usize];
                        info.generics = generics;
                        info.methods = methods;
                    }
                }
                ItemKind::TypeAlias(alias) => {
                    let mut resolver = TypeResolver::new(
                        &self.symbols,
                        &mut self.types,
                        &self.registry,
                        &self.states[module_id.0 as usize].names,
                    );
                    let ty = resolver.lower(&alias.ty, &generics);
                    if let Some(SymbolId(id)) = self
                        .state(module_id)
                        .span_symbols
                        .get(&alias.span.start)
                        .copied()
                    {
                        self.symbols[id as usize].kind = SymbolKind::Type(SymbolType::Alias(ty));
                    }
                }
                ItemKind::Function(function) => {
                    let generics = function
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    // 泛型接口约束必须带类型实参；缺实参（where T: Container）会
                    // 静默退化成模板 id，约束校验永不匹配、方法返回类型残留 TypeParam，
                    // 显式报错以避免误导性的"未实现约束接口"。
                    for constraint in &function.where_clause {
                        if let Some(segment) = constraint.bound.segments.first() {
                            if segment.generics.is_empty() {
                                let generic_interface = self
                                    .state(module_id)
                                    .names
                                    .get(&segment.name.name)
                                    .and_then(|ids| ids.first())
                                    .and_then(|id| match &self.symbols[id.0 as usize].kind {
                                        SymbolKind::Type(SymbolType::Interface(iface_id)) => Some(
                                            !self.types.interfaces[*iface_id as usize]
                                                .generics
                                                .is_empty(),
                                        ),
                                        _ => None,
                                    })
                                    .unwrap_or(false);
                                if generic_interface {
                                    self.error(
                                        "泛型接口约束必须带类型实参（如 `where T: Container<int>`）",
                                        constraint.bound.span,
                                    );
                                }
                            }
                        }
                    }
                    let sig = self.build_function_sig(module_id, function, &generics, None);
                    if sig.name == "main" {
                        let params_ok = sig.params.is_empty()
                            || (sig.params.len() == 1
                                && matches!(
                                    sig.params[0].ty,
                                    Type::Array(ref inner) if matches!(**inner, Type::Str)
                                ));
                        if !params_ok {
                            self.error("main 参数只能为 string[]（或省略）", function.span);
                        }
                        if sig.ret != Type::Int {
                            self.error("main 返回类型必须为 int", function.span);
                        }
                    }
                    if let Some(SymbolId(id)) = self
                        .state(module_id)
                        .span_symbols
                        .get(&function.span.start)
                        .copied()
                    {
                        self.symbols[id as usize].kind = SymbolKind::Function(sig);
                    }
                }
                ItemKind::Variable(variable) => {
                    let generics = Vec::new();
                    let ty = if let Some(annotation) = &variable.ty {
                        let mut resolver = TypeResolver::new(
                            &self.symbols,
                            &mut self.types,
                            &self.registry,
                            &self.states[module_id.0 as usize].names,
                        );
                        resolver.lower(annotation, &generics)
                    } else if let Some(init) = &variable.init {
                        self.const_type(init).unwrap_or(Type::Error)
                    } else {
                        Type::Error
                    };
                    if let Some(SymbolId(id)) = self
                        .state(module_id)
                        .span_symbols
                        .get(&variable.span.start)
                        .copied()
                    {
                        self.symbols[id as usize].kind = SymbolKind::Global {
                            ty,
                            mutable: variable.kind == VarKind::Let,
                        };
                    }
                }
                ItemKind::Import(_) => {}
            }
        }
        self.register_exports(module_id);
        Ok(())
    }

    fn finalize_fields(
        &mut self,
        module_id: ModuleId,
        fields: &[FieldDecl],
        generics: &[String],
    ) -> Vec<FieldInfo> {
        let mut resolver = TypeResolver::new(
            &self.symbols,
            &mut self.types,
            &self.registry,
            &self.states[module_id.0 as usize].names,
        );
        let lowered: Vec<(Type, Span)> = fields
            .iter()
            .map(|field| (resolver.lower(&field.ty, generics), field.span))
            .collect();
        let mut result = Vec::new();
        for (field, (ty, span)) in fields.iter().zip(lowered) {
            self.reject_complex_field(&ty, span, true);
            result.push(FieldInfo {
                name: field.name.name.clone(),
                ty,
                mutable: !field.modifiers.contains(&MemberModifier::Final),
                span: field.span,
            });
        }
        result
    }

    /// struct/class 字段允许嵌套 struct 值字段；struct 数组作为字段类型暂不支持。
    fn reject_complex_field(&mut self, ty: &Type, span: Span, allow_struct_value: bool) {
        let bad = (!allow_struct_value && matches!(ty, Type::Struct(_)))
            || matches!(ty, Type::Array(inner) if matches!(**inner, Type::Struct(_)));
        if bad {
            self.error(
                "v0.1 暂不支持该字段类型（struct 数组/class 的 struct 值字段）",
                span,
            );
        }
    }

    fn finalize_class_members(
        &mut self,
        module_id: ModuleId,
        class: &ClassDecl,
        generics: &[String],
        class_id: u32,
    ) -> (
        Vec<FieldInfo>,
        Vec<MethodInfo>,
        Vec<FieldInfo>,
        Vec<MethodInfo>,
    ) {
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut static_fields = Vec::new();
        let mut static_methods = Vec::new();
        for member in &class.members {
            match member {
                ClassMember::Field(field) => {
                    let mut resolver = TypeResolver::new(
                        &self.symbols,
                        &mut self.types,
                        &self.registry,
                        &self.states[module_id.0 as usize].names,
                    );
                    let ty = resolver.lower(&field.ty, generics);
                    self.reject_complex_field(&ty, field.span, true);
                    let info = FieldInfo {
                        name: field.name.name.clone(),
                        ty: ty.clone(),
                        mutable: !field.modifiers.contains(&MemberModifier::Final),
                        span: field.span,
                    };
                    if field
                        .modifiers
                        .iter()
                        .any(|m| matches!(m, MemberModifier::Static))
                    {
                        static_fields.push(info);
                    } else {
                        fields.push(info);
                    }
                }
                ClassMember::Method(function) => {
                    let sig =
                        self.build_function_sig(module_id, function, generics, Some(class_id));
                    let info = MethodInfo {
                        name: function.name.name.clone(),
                        sig: sig.clone(),
                        virtual_: false,
                        override_: false,
                        span: function.span,
                    };
                    if function.static_ {
                        static_methods.push(info);
                    } else {
                        methods.push(info);
                    }
                    self.state_mut(module_id)
                        .result
                        .method_classes
                        .insert(function.span.start, class_id);
                }
                ClassMember::Constructor(constructor) => {
                    let generics_vec = generics.to_vec();
                    let sig =
                        self.build_constructor_sig(module_id, constructor, &generics_vec, class_id);
                    methods.push(MethodInfo {
                        name: "constructor".to_owned(),
                        sig,
                        virtual_: false,
                        override_: false,
                        span: constructor.span,
                    });
                    self.state_mut(module_id)
                        .result
                        .method_classes
                        .insert(constructor.span.start, class_id);
                }
                ClassMember::Destructor(_) | ClassMember::Property(_) => {
                    self.error("v0.1 语义阶段暂不支持析构函数与属性", member_span(member));
                }
            }
        }
        (fields, methods, static_fields, static_methods)
    }

    fn build_function_sig(
        &mut self,
        module_id: ModuleId,
        function: &FunctionDecl,
        generics: &[String],
        this_class: Option<u32>,
    ) -> FunctionSig {
        let mut resolver = TypeResolver::new(
            &self.symbols,
            &mut self.types,
            &self.registry,
            &self.states[module_id.0 as usize].names,
        );
        let mut bounds: HashMap<String, Vec<Type>> = HashMap::new();
        for constraint in &function.where_clause {
            let bound_ty = resolver.lower(&constraint.bound, generics);
            bounds
                .entry(constraint.name.name.clone())
                .or_default()
                .push(bound_ty);
        }
        let params = function
            .params
            .iter()
            .map(|param| ParamSig {
                name: param.name.name.clone(),
                ty: resolver.lower(&param.ty, generics),
                has_default: param.default.is_some(),
                rest: param.rest,
            })
            .collect();
        let ret = match &function.return_type {
            Some(ty) => resolver.lower(ty, generics),
            None => Type::Unknown,
        };
        let _ = this_class;
        FunctionSig {
            module: module_id,
            name: function.name.name.clone(),
            generics: generics.to_vec(),
            bounds,
            params,
            ret,
            extern_c: function.extern_c,
            span: function.span,
        }
    }

    fn build_constructor_sig(
        &mut self,
        module_id: ModuleId,
        constructor: &ConstructorDecl,
        generics: &[String],
        _class_id: u32,
    ) -> FunctionSig {
        let mut resolver = TypeResolver::new(
            &self.symbols,
            &mut self.types,
            &self.registry,
            &self.states[module_id.0 as usize].names,
        );
        let params = constructor
            .params
            .iter()
            .map(|param| ParamSig {
                name: param.name.name.clone(),
                ty: resolver.lower(&param.ty, generics),
                has_default: param.default.is_some(),
                rest: param.rest,
            })
            .collect();
        FunctionSig {
            module: module_id,
            name: "constructor".to_owned(),
            generics: generics.to_vec(),
            bounds: HashMap::new(),
            params,
            ret: Type::Void,
            extern_c: false,
            span: constructor.span,
        }
    }

    fn register_exports(&mut self, module_id: ModuleId) {
        let names = self.state(module_id).names.clone();
        for (name, ids) in names {
            if let Some(SymbolId(id)) = ids.first().copied() {
                if !self.symbols[id as usize].exported {
                    continue;
                }
            }
            let key = format!("{}:{name}", module_id.0);
            if let Some(SymbolId(id)) = ids.first().copied() {
                if let SymbolKind::Type(kind) = &self.symbols[id as usize].kind {
                    let ty = match kind {
                        SymbolType::Struct(id) => Type::Struct(*id),
                        SymbolType::Enum(id) => Type::Enum(*id),
                        SymbolType::Class(id) => Type::Class(*id),
                        SymbolType::Interface(id) => Type::Interface(*id),
                        SymbolType::Alias(ty) => ty.clone(),
                    };
                    self.registry.insert(key, ty);
                }
            }
        }
    }

    fn lookup_name(&self, module_id: ModuleId, name: &str) -> Option<SymbolId> {
        self.state(module_id)
            .names
            .get(name)
            .and_then(|ids| ids.first().copied())
    }

    fn type_id_for(&self, module_id: ModuleId, name: &str) -> Option<SymbolKind> {
        self.lookup_name(module_id, name)
            .map(|id| self.symbol(id).kind.clone())
    }

    fn const_int(&self, expr: &Expr) -> Option<i64> {
        match &expr.kind {
            ExprKind::Integer { text, .. } => parse_int(text),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                expr,
            } => self.const_int(expr).map(|v| -v),
            ExprKind::Unary {
                op: UnaryOp::Pos,
                expr,
            } => self.const_int(expr),
            ExprKind::Binary { op, left, right } => {
                let (l, r) = (self.const_int(left)?, self.const_int(right)?);
                match op {
                    BinaryOp::Add => l.checked_add(r),
                    BinaryOp::Sub => l.checked_sub(r),
                    BinaryOp::Mul => l.checked_mul(r),
                    BinaryOp::Div => l.checked_div(r),
                    BinaryOp::Rem => l.checked_rem(r),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn const_type(&self, expr: &Expr) -> Option<Type> {
        match &expr.kind {
            ExprKind::Integer { .. } => Some(Type::Int),
            ExprKind::Float { .. } => Some(Type::F64),
            ExprKind::Str(_) => Some(Type::Str),
            ExprKind::Bool(_) => Some(Type::Bool),
            ExprKind::Char(_) => Some(Type::Char),
            ExprKind::Binary { op, left, right } => {
                let l = self.const_type(left)?;
                let r = self.const_type(right)?;
                if matches!(
                    op,
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
                ) {
                    if l.is_numeric() && r.is_numeric() {
                        return Some(l);
                    }
                }
                if matches!(
                    op,
                    BinaryOp::Eq
                        | BinaryOp::Ne
                        | BinaryOp::Lt
                        | BinaryOp::Le
                        | BinaryOp::Gt
                        | BinaryOp::Ge
                ) {
                    return Some(Type::Bool);
                }
                None
            }
            ExprKind::Unary { op, expr } => {
                let inner = self.const_type(expr)?;
                let valid = match op {
                    UnaryOp::Neg | UnaryOp::Pos => inner.is_numeric(),
                    UnaryOp::Not => inner == Type::Bool,
                    UnaryOp::BitNot => inner.is_integer(),
                    _ => false,
                };
                if valid { Some(inner) } else { None }
            }
            _ => None,
        }
    }

    fn check_globals(&mut self, module_id: ModuleId) {
        let items = self.modules[module_id.0 as usize].items.clone();
        for item in &items {
            if let ItemKind::Variable(variable) = &item.kind {
                if let Some(init) = &variable.init {
                    let ty = self.const_type(init);
                    if ty.is_none() {
                        self.error("顶层变量初始化式必须是编译期常量表达式", init.span);
                    }
                }
            }
        }
    }

    fn check_all_functions(&mut self, module_id: ModuleId) {
        let items = self.modules[module_id.0 as usize].items.clone();
        for item in &items {
            match &item.kind {
                ItemKind::Function(function) => {
                    if let Some(body) = &function.body {
                        let sig = self.function_sig(module_id, function.span);
                        let generics = function
                            .generics
                            .iter()
                            .map(|g| g.name.clone())
                            .collect::<Vec<_>>();
                        self.check_function_body(
                            module_id,
                            function.span,
                            None,
                            &sig,
                            &generics,
                            sig.bounds.clone(),
                            body,
                        );
                    }
                }
                ItemKind::Class(class) => {
                    let class_id = match self.type_id_for(module_id, &class.name.name) {
                        Some(SymbolKind::Type(SymbolType::Class(id))) => id,
                        _ => continue,
                    };
                    let class_info = self.types.classes[class_id as usize].clone();
                    let generics = class
                        .generics
                        .iter()
                        .map(|g| g.name.clone())
                        .collect::<Vec<_>>();
                    let mut method_index = 0usize;
                    for member in &class.members {
                        let (body, span) = match member {
                            ClassMember::Method(function) => {
                                (function.body.as_ref(), function.span)
                            }
                            ClassMember::Constructor(constructor) => {
                                (Some(&constructor.body), constructor.span)
                            }
                            _ => (None, Span::empty(0)),
                        };
                        if let Some(body) = body {
                            let method_name = match member {
                                ClassMember::Method(function) => function.name.name.clone(),
                                ClassMember::Constructor(_) => "constructor".to_owned(),
                                _ => String::new(),
                            };
                            let is_static = match member {
                                ClassMember::Method(function) => function.static_,
                                _ => false,
                            };
                            // 重载方法按 class members 顺序对应 methods 索引精确定位
                            // （find 按名字只能拿到第一个重载的签名）。
                            let sig = if is_static {
                                class_info
                                    .static_methods
                                    .iter()
                                    .find(|method| method.name == method_name)
                                    .map(|method| method.sig.clone())
                                    .unwrap_or_else(|| FunctionSig {
                                        module: module_id,
                                        name: method_name.clone(),
                                        generics: Vec::new(),
                                        bounds: HashMap::new(),
                                        params: Vec::new(),
                                        ret: Type::Void,
                                        extern_c: false,
                                        span,
                                    })
                            } else {
                                let index = method_index;
                                method_index += 1;
                                class_info
                                    .methods
                                    .get(index)
                                    .map(|method| method.sig.clone())
                                    .unwrap_or_else(|| FunctionSig {
                                        module: module_id,
                                        name: method_name.clone(),
                                        generics: Vec::new(),
                                        bounds: HashMap::new(),
                                        params: Vec::new(),
                                        ret: Type::Void,
                                        extern_c: false,
                                        span,
                                    })
                            };
                            self.check_function_body(
                                module_id,
                                span,
                                if is_static { None } else { Some(class_id) },
                                &sig,
                                &generics,
                                sig.bounds.clone(),
                                body,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn function_sig(&self, module_id: ModuleId, span: Span) -> FunctionSig {
        if let Some(SymbolId(id)) = self.state(module_id).span_symbols.get(&span.start).copied() {
            if let SymbolKind::Function(sig) = &self.symbols[id as usize].kind {
                return sig.clone();
            }
        }
        FunctionSig {
            module: module_id,
            name: String::new(),
            generics: Vec::new(),
            bounds: HashMap::new(),
            params: Vec::new(),
            ret: Type::Unknown,
            extern_c: false,
            span,
        }
    }

    fn check_function_body(
        &mut self,
        module_id: ModuleId,
        span: Span,
        this_class: Option<u32>,
        sig: &FunctionSig,
        generics: &[String],
        bounds: HashMap<String, Vec<Type>>,
        body: &Block,
    ) {
        let symbols = &mut self.symbols;
        let types = &mut self.types;
        let registry = &self.registry;
        let diagnostics = &mut self.diagnostics;
        let state = &mut self.states[module_id.0 as usize];
        let module_names = &self.module_names;
        let mut checker = Checker {
            symbols,
            types,
            registry,
            module_names,
            diagnostics,
            state,
            scopes: vec![HashMap::new()],
            ret: sig.ret.clone(),
            this_class,
            loop_depth: 0,
            switch_depth: 0,
            generics: generics.to_vec(),
            bounds,
            saw_return_value: false,
        };
        for param in &sig.params {
            let id = checker.alloc_local();
            checker.symbols[id.0 as usize].kind = SymbolKind::Param {
                ty: param.ty.clone(),
            };
            checker.scopes[0].insert(param.name.clone(), id);
        }
        checker.check_block(body);
        if checker.ret == Type::Unknown {
            if checker.saw_return_value {
                checker.error("无法推导函数返回类型：return 表达式类型不一致", span);
            }
        } else if checker.ret != Type::Void
            && !checker.saw_return_value
            && !matches!(checker.ret, Type::Class(_))
            && !sig.extern_c
        {
            checker.error(
                format!("函数缺少 return，返回类型为 {}", checker.ret.display()),
                span,
            );
        }
    }

    fn lower_mir(&mut self, module_id: ModuleId) {
        let module = &self.modules[module_id.0 as usize];
        let symbols = &self.symbols;
        let types = &mut self.types;
        let registry = &self.registry;
        let diagnostics = &mut self.diagnostics;
        let mut decl_names = HashMap::new();
        for st in &self.states {
            for (name, ids) in &st.names {
                for id in ids {
                    decl_names.entry(id.0).or_insert_with(|| name.clone());
                }
            }
        }
        let all_results: Vec<CheckResult> = self
            .states
            .iter()
            .map(|state| state.result.clone())
            .collect();
        let module_stems: HashMap<u32, String> = self
            .states
            .iter()
            .map(|state| {
                let stem = state
                    .path
                    .file_stem()
                    .map(|s| {
                        s.to_string_lossy()
                            .replace(|c: char| !c.is_ascii_alphanumeric(), "_")
                    })
                    .unwrap_or_else(|| format!("mod{}", state.id.0));
                (state.id.0, stem)
            })
            .collect();
        let state = &mut self.states[module_id.0 as usize];
        let mir = {
            let mut lowerer = MirLowerer {
                module,
                all_modules: &self.modules,
                all_results,
                module_stems,
                symbols,
                types,
                registry,
                diagnostics,
                state,
                global_index_by_symbol: HashMap::new(),
                decl_names,
                hidden_functions: Vec::new(),
                generic_instances: HashMap::new(),
                generic_counter: 0,
                static_field_globals: HashMap::new(),
            };
            lowerer.lower_module()
        };
        self.states[module_id.0 as usize].mir = Some(mir);
    }
}

fn placeholder_sig() -> FunctionSig {
    FunctionSig {
        module: ModuleId(0),
        name: String::new(),
        generics: Vec::new(),
        bounds: HashMap::new(),
        params: Vec::new(),
        ret: Type::Error,
        extern_c: false,
        span: Span::empty(0),
    }
}

fn parse_int(text: &str) -> Option<i64> {
    let digits = text.replace('_', "");
    let (radix, digits) = if let Some(rest) = digits.strip_prefix("0x") {
        (16, rest)
    } else if let Some(rest) = digits.strip_prefix("0b") {
        (2, rest)
    } else if let Some(rest) = digits.strip_prefix("0o") {
        (8, rest)
    } else {
        (10, digits.as_str())
    };
    i64::from_str_radix(digits, radix).ok()
}

fn member_span(member: &ClassMember) -> Span {
    match member {
        ClassMember::Field(field) => field.span,
        ClassMember::Constructor(constructor) => constructor.span,
        ClassMember::Destructor(destructor) => destructor.span,
        ClassMember::Property(property) => property.span,
        ClassMember::Method(function) => function.span,
    }
}

/// 类型引用解析器：把 AST 类型降级为语义类型。
#[allow(dead_code)]
struct TypeResolver<'a> {
    symbols: &'a [Symbol],
    types: &'a mut TypeTable,
    registry: &'a HashMap<String, Type>,
    names: &'a HashMap<String, Vec<SymbolId>>,
}

impl<'a> TypeResolver<'a> {
    fn new(
        symbols: &'a [Symbol],
        types: &'a mut TypeTable,
        registry: &'a HashMap<String, Type>,
        names: &'a HashMap<String, Vec<SymbolId>>,
    ) -> Self {
        Self {
            symbols,
            types,
            registry,
            names,
        }
    }

    fn lower(&mut self, ty: &TypeRef, generics: &[String]) -> Type {
        let builtin = |name: &str| -> Option<Type> {
            Some(match name {
                "void" => Type::Void,
                "bool" => Type::Bool,
                "i8" => Type::I8,
                "i16" => Type::I16,
                "i32" => Type::I32,
                "i64" => Type::I64,
                "isize" => Type::Isize,
                "u8" => Type::U8,
                "u16" => Type::U16,
                "u32" => Type::U32,
                "u64" => Type::U64,
                "usize" => Type::Usize,
                "int" => Type::Int,
                "uint" => Type::UInt,
                "f32" => Type::F32,
                "f64" => Type::F64,
                "float" => Type::F64,
                "char" => Type::Char,
                "string" => Type::Str,
                "any" => Type::Any,
                _ => return None,
            })
        };

        let suffixes = &ty.suffixes;
        let mut result = self.lower_segment(&ty.segments, generics, &builtin);
        for suffix in suffixes {
            result = match suffix {
                TypeSuffix::Array => Type::Array(Box::new(result)),
                TypeSuffix::Nullable => Type::Nullable(Box::new(result)),
            };
        }
        result
    }

    fn lower_segment(
        &mut self,
        segments: &[TypeSegment],
        generics: &[String],
        builtin: &dyn Fn(&str) -> Option<Type>,
    ) -> Type {
        let first = &segments[0];
        let name = &first.name.name;
        if segments.len() == 1 {
            if name == "ptr" {
                if let Some(inner) = first.generics.first() {
                    let inner = self.lower(inner, generics);
                    return Type::Ptr(Box::new(inner));
                }
                return Type::Error;
            }
            if let Some(builtin) = builtin(name) {
                return builtin;
            }
            if generics.iter().any(|g| g == name) {
                return Type::TypeParam(name.clone());
            }
            if let Some(ids) = self.names.get(name) {
                if let Some(SymbolId(id)) = ids.first() {
                    if let SymbolKind::Type(kind) = &self.symbols[*id as usize].kind {
                        return match kind {
                            SymbolType::Struct(id) => {
                                if !first.generics.is_empty()
                                    || !self.types.structs[*id as usize].generics.is_empty()
                                {
                                    return self.instantiate_struct(*id, &first.generics, generics);
                                }
                                Type::Struct(*id)
                            }
                            SymbolType::Enum(id) => {
                                if !first.generics.is_empty()
                                    || !self.types.enums[*id as usize].generics.is_empty()
                                {
                                    return self.instantiate_enum(*id, &first.generics, generics);
                                }
                                Type::Enum(*id)
                            }
                            SymbolType::Class(id) => {
                                if !first.generics.is_empty()
                                    || !self.types.classes[*id as usize].generics.is_empty()
                                {
                                    return self.instantiate_class(*id, &first.generics, generics);
                                }
                                Type::Class(*id)
                            }
                            SymbolType::Interface(id) => {
                                if !first.generics.is_empty()
                                    || !self.types.interfaces[*id as usize].generics.is_empty()
                                {
                                    let args: Vec<Type> = first
                                        .generics
                                        .iter()
                                        .map(|arg| self.lower(arg, generics))
                                        .collect();
                                    let instance_id = self.instantiate_interface(*id, &args);
                                    return Type::Interface(instance_id);
                                }
                                Type::Interface(*id)
                            }
                            SymbolType::Alias(ty) => ty.clone(),
                        };
                    }
                }
            }
            return Type::Error;
        }
        // 命名空间限定类型：ns.Type
        if let Some(ids) = self.names.get(name) {
            if let Some(SymbolId(id)) = ids.first() {
                if let SymbolKind::Namespace(target) = &self.symbols[*id as usize].kind {
                    let key = format!("{}:{}", target.0, segments[1].name.name);
                    if let Some(ty) = self.registry.get(&key) {
                        return ty.clone();
                    }
                }
            }
        }
        Type::Error
    }

    /// 泛型 struct 实例化：替换字段类型，注册新 struct id。
    fn instantiate_struct(
        &mut self,
        struct_id: u32,
        arg_refs: &[TypeRef],
        generics: &[String],
    ) -> Type {
        let info = self.types.structs[struct_id as usize].clone();
        if info.generics.len() != arg_refs.len() {
            return Type::Error;
        }
        let args: Vec<Type> = arg_refs
            .iter()
            .map(|arg| self.lower(arg, generics))
            .collect();
        let key = (struct_id, args.clone());
        if let Some(&id) = self.types.generic_struct_instances.get(&key) {
            return Type::Struct(id);
        }
        let type_args: HashMap<String, Type> = info
            .generics
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let fields = info
            .fields
            .iter()
            .map(|field| FieldInfo {
                name: field.name.clone(),
                ty: substitute_type(&field.ty, &type_args),
                mutable: field.mutable,
                span: field.span,
            })
            .collect();
        let id = self.types.structs.len() as u32;
        self.types.structs.push(StructInfo {
            module: info.module,
            name: info.name,
            generics: Vec::new(),
            fields,
        });
        self.types.generic_struct_instances.insert(key, id);
        Type::Struct(id)
    }

    fn instantiate_enum(
        &mut self,
        enum_id: u32,
        arg_refs: &[TypeRef],
        generics: &[String],
    ) -> Type {
        let info = self.types.enums[enum_id as usize].clone();
        if info.generics.len() != arg_refs.len() {
            return Type::Error;
        }
        let args: Vec<Type> = arg_refs
            .iter()
            .map(|arg| self.lower(arg, generics))
            .collect();
        let key = (enum_id, args.clone());
        if let Some(&id) = self.types.generic_enum_instances.get(&key) {
            return Type::Enum(id);
        }
        let type_args: HashMap<String, Type> = info
            .generics
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let members = info
            .members
            .iter()
            .map(|member| EnumVariant {
                name: member.name.clone(),
                discriminant: member.discriminant,
                fields: member
                    .fields
                    .iter()
                    .map(|ty| substitute_type(ty, &type_args))
                    .collect(),
            })
            .collect();
        let id = self.types.enums.len() as u32;
        self.types.enums.push(EnumInfo {
            module: info.module,
            name: info.name,
            generics: Vec::new(),
            members,
        });
        self.types.generic_enum_instances.insert(key, id);
        Type::Enum(id)
    }

    /// 泛型 interface 实例化：替换方法签名类型实参，注册新 interface id。
    fn instantiate_interface(&mut self, interface_id: u32, args: &[Type]) -> u32 {
        let info = self.types.interfaces[interface_id as usize].clone();
        if info.generics.is_empty() {
            return interface_id;
        }
        if info.generics.len() != args.len() {
            return interface_id;
        }
        let key = (interface_id, args.to_vec());
        if let Some(&id) = self.types.generic_interface_instances.get(&key) {
            return id;
        }
        let type_args: HashMap<String, Type> = info
            .generics
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let methods = info
            .methods
            .iter()
            .map(|method| FunctionSig {
                module: method.module,
                name: method.name.clone(),
                generics: Vec::new(),
                bounds: method.bounds.clone(),
                params: method
                    .params
                    .iter()
                    .map(|param| ParamSig {
                        name: param.name.clone(),
                        ty: substitute_type(&param.ty, &type_args),
                        has_default: param.has_default,
                        rest: param.rest,
                    })
                    .collect(),
                ret: substitute_type(&method.ret, &type_args),
                extern_c: method.extern_c,
                span: method.span,
            })
            .collect();
        let id = self.types.interfaces.len() as u32;
        self.types.interfaces.push(InterfaceInfo {
            module: info.module,
            name: info.name,
            generics: Vec::new(),
            methods,
        });
        self.types.generic_interface_instances.insert(key, id);
        id
    }

    /// 泛型 class 实例化：替换字段与方法签名，注册新 class id（方法体在降级阶段生成）。
    fn instantiate_class(
        &mut self,
        class_id: u32,
        arg_refs: &[TypeRef],
        generics: &[String],
    ) -> Type {
        let info = self.types.classes[class_id as usize].clone();
        if info.generics.len() != arg_refs.len() {
            return Type::Error;
        }
        let args: Vec<Type> = arg_refs
            .iter()
            .map(|arg| self.lower(arg, generics))
            .collect();
        let key = (class_id, args.clone());
        if let Some(&id) = self.types.generic_class_instances.get(&key) {
            return Type::Class(id);
        }
        let type_args: HashMap<String, Type> = info
            .generics
            .iter()
            .cloned()
            .zip(args.iter().cloned())
            .collect();
        let fields = info
            .fields
            .iter()
            .map(|field| FieldInfo {
                name: field.name.clone(),
                ty: substitute_type(&field.ty, &type_args),
                mutable: field.mutable,
                span: field.span,
            })
            .collect();
        let methods = info
            .methods
            .iter()
            .map(|method| MethodInfo {
                name: method.name.clone(),
                sig: FunctionSig {
                    module: method.sig.module,
                    name: method.sig.name.clone(),
                    generics: Vec::new(),
                    bounds: HashMap::new(),
                    params: method
                        .sig
                        .params
                        .iter()
                        .map(|param| ParamSig {
                            name: param.name.clone(),
                            ty: substitute_type(&param.ty, &type_args),
                            has_default: param.has_default,
                            rest: param.rest,
                        })
                        .collect(),
                    ret: substitute_type(&method.sig.ret, &type_args),
                    extern_c: false,
                    span: method.sig.span,
                },
                virtual_: method.virtual_,
                override_: method.override_,
                span: method.span,
            })
            .collect();
        let id = self.types.classes.len() as u32;
        let template_interfaces = self.types.class_interfaces.get(&class_id).cloned();
        self.types.classes.push(ClassInfo {
            module: info.module,
            name: info.name,
            generics: Vec::new(),
            base: info.base,
            fields,
            methods,
            static_fields: info.static_fields.clone(),
            static_methods: info.static_methods.clone(),
            final_: info.final_,
            implements: info.implements.clone(),
        });
        // 泛型接口 implements（如 Box<T> implements Container<T>）：
        // 替换类型实参后实例化接口并注册到实例类的 vtable 表。
        let mut iface_ids = template_interfaces.unwrap_or_default();
        for (template_id, args) in &info.implements {
            let resolved_args: Vec<Type> = args
                .iter()
                .map(|ty| substitute_type(ty, &type_args))
                .collect();
            let instance_id = self.instantiate_interface(*template_id, &resolved_args);
            iface_ids.push(instance_id);
        }
        if !iface_ids.is_empty() {
            self.types.class_interfaces.insert(id, iface_ids);
        }
        self.types.generic_class_instances.insert(key, id);
        Type::Class(id)
    }
}

// ---------------------------------------------------------------------------
// 类型检查器
// ---------------------------------------------------------------------------

struct Checker<'s> {
    symbols: &'s mut Vec<Symbol>,
    types: &'s mut TypeTable,
    registry: &'s HashMap<String, Type>,
    module_names: &'s HashMap<ModuleId, HashMap<String, Vec<SymbolId>>>,
    diagnostics: &'s mut Diagnostics,
    state: &'s mut ModuleState,
    scopes: Vec<HashMap<String, SymbolId>>,
    ret: Type,
    this_class: Option<u32>,
    loop_depth: usize,
    switch_depth: usize,
    generics: Vec<String>,
    /// 泛型参数名 → 约束接口（`where T: Shape`），用于约束内方法调用解析。
    bounds: HashMap<String, Vec<Type>>,
    saw_return_value: bool,
}

impl<'s> Checker<'s> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        let file = self.state.path.clone();
        self.diagnostics.error_at(message, Some(span), Some(file));
    }

    fn record_call_result(&mut self, span: Span, ty: Type) -> Type {
        self.state
            .result
            .expr_types
            .insert((span.start, span.end), ty.clone());
        ty
    }

    /// 泛型枚举实例化（类型实参已推断）：生成/复用实例枚举 id。
    fn instantiate_enum_types(&mut self, enum_id: u32, type_args: &HashMap<String, Type>) -> u32 {
        let info = self.types.enums[enum_id as usize].clone();
        if info.generics.is_empty() {
            return enum_id;
        }
        let args: Vec<Type> = info
            .generics
            .iter()
            .map(|name| type_args.get(name).cloned().unwrap_or(Type::Error))
            .collect();
        let key = (enum_id, args.clone());
        if let Some(&id) = self.types.generic_enum_instances.get(&key) {
            return id;
        }
        let members = info
            .members
            .iter()
            .map(|member| EnumVariant {
                name: member.name.clone(),
                discriminant: member.discriminant,
                fields: member
                    .fields
                    .iter()
                    .map(|ty| substitute_type(ty, type_args))
                    .collect(),
            })
            .collect();
        let id = self.types.enums.len() as u32;
        self.types.enums.push(EnumInfo {
            module: info.module,
            name: info.name,
            generics: Vec::new(),
            members,
        });
        self.types.generic_enum_instances.insert(key, id);
        id
    }

    fn alloc_local(&mut self) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            kind: SymbolKind::Param { ty: Type::Error },
            exported: false,
            span: Span::empty(0),
            module: ModuleId(0),
        });
        id
    }

    fn lookup(&self, name: &str) -> Option<SymbolId> {
        for scope in self.scopes.iter().rev() {
            if let Some(id) = scope.get(name) {
                return Some(*id);
            }
        }
        self.state
            .names
            .get(name)
            .and_then(|ids| ids.first().copied())
    }

    fn symbol_type(&self, id: SymbolId) -> Type {
        match &self.symbols[id.0 as usize].kind {
            SymbolKind::Function(sig) => Type::Function {
                params: sig.params.iter().map(|param| param.ty.clone()).collect(),
                ret: Box::new(sig.ret.clone()),
            },
            SymbolKind::Global { ty, .. } => ty.clone(),
            SymbolKind::Local { ty, .. } => ty.clone(),
            SymbolKind::Param { ty } => ty.clone(),
            SymbolKind::Type(_) => Type::Error,
            SymbolKind::Namespace(_) => Type::Error,
        }
    }

    fn lower_type(&mut self, ty: &TypeRef) -> Type {
        let mut resolver = TypeResolver::new(
            self.symbols,
            &mut *self.types,
            self.registry,
            &self.state.names,
        );
        resolver.lower(ty, &self.generics)
    }

    /// Result 枚举识别：返回 (Ok 变体索引, Ok payload 类型, Err payload 类型)。
    fn result_variants(&self, enum_id: u32) -> Option<(usize, Type, Type)> {
        let info = self.types.enums.get(enum_id as usize)?;
        let ok = info
            .members
            .iter()
            .position(|m| m.name == "Ok")
            .filter(|&i| info.members[i].fields.len() == 1)?;
        let err = info
            .members
            .iter()
            .position(|m| m.name == "Err")
            .filter(|&i| info.members[i].fields.len() == 1)?;
        Some((
            ok,
            info.members[ok].fields[0].clone(),
            info.members[err].fields[0].clone(),
        ))
    }

    fn is_assignable(&self, from: &Type, to: &Type) -> bool {
        if matches!(from, Type::Error | Type::Unknown) {
            return true;
        }
        if from == to {
            return true;
        }
        if matches!(to, Type::Any) {
            return true;
        }
        match (from, to) {
            (Type::Int, Type::Isize) | (Type::Isize, Type::Int) => true,
            (Type::UInt, Type::Usize) | (Type::Usize, Type::UInt) => true,
            (Type::Null, Type::Nullable(_)) => true,
            (Type::Null, target) if target.is_reference() => true,
            (Type::Class(from_id), Type::Class(to_id)) => {
                self.types.is_class_assignable_to(*from_id, *to_id)
            }
            (Type::Class(from_id), Type::Interface(to_id)) => {
                let mut current = Some(*from_id);
                while let Some(class_id) = current {
                    if self
                        .types
                        .class_interfaces
                        .get(&class_id)
                        .map(|list| list.contains(to_id))
                        .unwrap_or(false)
                    {
                        return true;
                    }
                    current = self.types.classes[class_id as usize].base;
                }
                false
            }
            (Type::Array(from_inner), Type::Array(to_inner)) => {
                self.is_assignable(from_inner, to_inner)
            }
            (_, Type::Nullable(to_inner)) => self.is_assignable(from, to_inner),
            (Type::Nullable(from_inner), Type::Nullable(to_inner)) => {
                self.is_assignable(from_inner, to_inner)
            }
            _ => false,
        }
    }

    fn check_block(&mut self, block: &Block) {
        self.scopes.push(HashMap::new());
        for statement in &block.statements {
            self.check_stmt(statement);
        }
        self.scopes.pop();
    }

    fn check_stmt(&mut self, statement: &Stmt) {
        match &statement.kind {
            StmtKind::Block(block) => self.check_block(block),
            StmtKind::Variable(variable) => self.check_var_decl(variable),
            StmtKind::If { cond, then, else_ } => {
                let ty = self.check_expr(cond);
                if ty != Type::Bool {
                    self.error(
                        format!("if 条件必须是 bool，实际为 {}", ty.display()),
                        cond.span,
                    );
                }
                self.check_stmt(then);
                if let Some(else_) = else_ {
                    self.check_stmt(else_);
                }
            }
            StmtKind::While { cond, body } => {
                let ty = self.check_expr(cond);
                if ty != Type::Bool {
                    self.error(
                        format!("while 条件必须是 bool，实际为 {}", ty.display()),
                        cond.span,
                    );
                }
                self.loop_depth += 1;
                self.check_stmt(body);
                self.loop_depth -= 1;
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    match init {
                        ForInit::Variable(variable) => self.check_var_decl(variable),
                        ForInit::Expr(expr) => {
                            self.check_expr(expr);
                        }
                    }
                }
                if let Some(cond) = cond {
                    let ty = self.check_expr(cond);
                    if ty != Type::Bool {
                        self.error(
                            format!("for 条件必须是 bool，实际为 {}", ty.display()),
                            cond.span,
                        );
                    }
                }
                if let Some(update) = update {
                    self.check_expr(update);
                }
                self.loop_depth += 1;
                self.check_stmt(body);
                self.loop_depth -= 1;
            }
            StmtKind::ForEach {
                kind,
                name,
                iterable,
                body,
            } => {
                let iterable_ty = self.check_expr(iterable);
                let element = match iterable_ty {
                    Type::Array(inner) => *inner,
                    Type::Str => Type::Char,
                    Type::Nullable(inner) => {
                        self.error("for-of 迭代值可能为空，请先判空", iterable.span);
                        *inner
                    }
                    other => {
                        self.error(
                            format!("for-of 只能迭代 T[] 或 string，实际为 {}", other.display()),
                            iterable.span,
                        );
                        Type::Error
                    }
                };
                let id = self.alloc_local();
                self.symbols[id.0 as usize].kind = SymbolKind::Local {
                    ty: element.clone(),
                    mutable: *kind == VarKind::Let,
                };
                self.scopes
                    .last_mut()
                    .expect("作用域存在")
                    .insert(name.name.clone(), id);
                self.loop_depth += 1;
                self.check_stmt(body);
                self.loop_depth -= 1;
            }
            StmtKind::Switch {
                value,
                cases,
                default,
            } => {
                let value_ty = self.check_expr(value);
                self.switch_depth += 1;
                for case in cases {
                    let case_ty = self.check_expr(&case.value);
                    if !self.is_assignable(&case_ty, &value_ty)
                        && !self.is_assignable(&value_ty, &case_ty)
                    {
                        self.error(
                            format!(
                                "case 类型 {} 与 switch 值类型 {} 不匹配",
                                case_ty.display(),
                                value_ty.display()
                            ),
                            case.value.span,
                        );
                    }
                    for statement in &case.body {
                        self.check_stmt(statement);
                    }
                }
                if let Some(statements) = default {
                    for statement in statements {
                        self.check_stmt(statement);
                    }
                }
                self.switch_depth -= 1;
            }
            StmtKind::Match { value, arms } => {
                let value_ty = self.check_expr(value);
                let Type::Enum(enum_id) = value_ty.without_nullable() else {
                    self.error("match 目标必须是枚举类型", value.span);
                    return;
                };
                let info = self.types.enums[*enum_id as usize].clone();
                let mut covered = vec![false; info.members.len()];
                let mut wildcard = false;
                for arm in arms {
                    match &arm.pattern {
                        Pattern::Wildcard(_) => {
                            wildcard = true;
                            self.check_block(&arm.body);
                        }
                        Pattern::Variant {
                            name,
                            bindings,
                            span,
                        } => {
                            let Some(index) = info
                                .members
                                .iter()
                                .position(|member| member.name == name.name)
                            else {
                                self.error(
                                    format!("枚举 {} 没有变体 `{}`", info.name, name.name),
                                    name.span,
                                );
                                continue;
                            };
                            let variant = &info.members[index];
                            if variant.fields.len() != bindings.len() {
                                self.error(
                                    format!(
                                        "变体 `{}` 需要 {} 个绑定变量，实际 {}",
                                        variant.name,
                                        variant.fields.len(),
                                        bindings.len()
                                    ),
                                    *span,
                                );
                            }
                            covered[index] = true;
                            self.scopes.push(HashMap::new());
                            for (binding, ty) in bindings.iter().zip(variant.fields.iter()) {
                                let id = self.alloc_local();
                                self.symbols[id.0 as usize].kind = SymbolKind::Local {
                                    ty: ty.clone(),
                                    mutable: false,
                                };
                                self.scopes
                                    .last_mut()
                                    .expect("作用域存在")
                                    .insert(binding.name.clone(), id);
                            }
                            self.check_block(&arm.body);
                            self.scopes.pop();
                        }
                    }
                }
                if !wildcard {
                    let uncovered: Vec<&str> = info
                        .members
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !covered[*index])
                        .map(|(_, member)| member.name.as_str())
                        .collect();
                    if !uncovered.is_empty() {
                        self.error(
                            format!("match 未穷尽：缺少变体 {}", uncovered.join(", ")),
                            value.span,
                        );
                    }
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally,
            } => {
                self.check_block(body);
                for catch in catches {
                    let catch_ty = catch
                        .ty
                        .as_ref()
                        .map(|ty| self.lower_type(ty))
                        .unwrap_or(Type::Unknown);
                    let id = self.alloc_local();
                    self.symbols[id.0 as usize].kind = SymbolKind::Local {
                        ty: catch_ty,
                        mutable: false,
                    };
                    self.scopes.push(HashMap::new());
                    self.scopes
                        .last_mut()
                        .expect("作用域存在")
                        .insert(catch.name.name.clone(), id);
                    self.check_block(&catch.body);
                    self.scopes.pop();
                }
                if let Some(finally) = finally {
                    self.check_block(finally);
                }
            }
            StmtKind::Throw(expr) => {
                let ty = self.check_expr(expr);
                if ty != Type::Str && !matches!(ty, Type::Class(_)) {
                    self.error(
                        format!(
                            "throw 的值必须是 string 或 class 实例，实际为 {}",
                            ty.display()
                        ),
                        expr.span,
                    );
                }
            }
            StmtKind::Defer(expr) => {
                self.check_expr(expr);
            }
            StmtKind::Break => {
                if self.loop_depth == 0 && self.switch_depth == 0 {
                    self.error("break 只能在循环或 switch 内使用", statement.span);
                }
            }
            StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.error("continue 只能在循环内使用", statement.span);
                }
            }
            StmtKind::Return(expr) => match expr {
                Some(expr) => {
                    // return 的目标类型传播：Result 枚举构造按函数返回类型实例化。
                    if let Type::Enum(ret_enum) = self.ret.without_nullable() {
                        let is_enum_ctor = match &expr.kind {
                            ExprKind::Call { callee, .. } => {
                                matches!(callee.kind, ExprKind::Member { .. })
                            }
                            ExprKind::Member { .. } => true,
                            _ => false,
                        };
                        if is_enum_ctor {
                            self.state
                                .result
                                .enum_targets
                                .insert(expr.span.start, *ret_enum);
                        }
                    }
                    let ty = self.check_expr(expr);
                    self.saw_return_value = true;
                    if self.ret == Type::Unknown {
                        self.ret = ty.clone();
                    } else if !self.is_assignable(&ty, &self.ret) {
                        self.error(
                            format!(
                                "return 类型 {} 与函数返回类型 {} 不匹配",
                                ty.display(),
                                self.ret.display()
                            ),
                            expr.span,
                        );
                    }
                }
                None => {
                    if self.ret == Type::Unknown {
                        self.ret = Type::Void;
                    } else if self.ret != Type::Void {
                        self.error(
                            format!("函数声明返回 {}，但 return 缺少表达式", self.ret.display()),
                            statement.span,
                        );
                    }
                }
            },
            StmtKind::Expr(expr) => {
                self.check_expr(expr);
            }
            StmtKind::Empty => {}
        }
    }

    fn check_var_decl(&mut self, variable: &VariableDecl) {
        if let Some(pattern) = &variable.pattern {
            let Some(init) = &variable.init else {
                self.error("解构声明需要初始化表达式", variable.span);
                return;
            };
            let init_ty = self.check_expr(init);
            match pattern {
                VariablePattern::Array(bindings) => {
                    let Type::Array(elem) = init_ty.without_nullable() else {
                        self.error(
                            format!("数组解构目标必须是数组，实际为 {}", init_ty.display()),
                            init.span,
                        );
                        return;
                    };
                    for binding in bindings {
                        let id = self.alloc_local();
                        self.symbols[id.0 as usize].kind = SymbolKind::Local {
                            ty: (**elem).clone(),
                            mutable: variable.kind == VarKind::Let,
                        };
                        self.scopes
                            .last_mut()
                            .expect("作用域存在")
                            .insert(binding.clone(), id);
                    }
                }
                VariablePattern::Object(bindings) => {
                    let Type::Struct(struct_id) = init_ty.without_nullable() else {
                        self.error(
                            format!("对象解构目标必须是 struct，实际为 {}", init_ty.display()),
                            init.span,
                        );
                        return;
                    };
                    let info = self.types.structs[*struct_id as usize].clone();
                    for (field_name, binding) in bindings {
                        let Some(field) = info.fields.iter().find(|f| f.name == *field_name) else {
                            self.error(
                                format!("struct {} 没有字段 `{}`", info.name, field_name),
                                variable.span,
                            );
                            continue;
                        };
                        let id = self.alloc_local();
                        self.symbols[id.0 as usize].kind = SymbolKind::Local {
                            ty: field.ty.clone(),
                            mutable: variable.kind == VarKind::Let,
                        };
                        self.scopes
                            .last_mut()
                            .expect("作用域存在")
                            .insert(binding.clone(), id);
                    }
                }
            }
            return;
        }
        let ty = if let Some(annotation) = &variable.ty {
            let ty = self.lower_type(annotation);
            if let Some(init) = &variable.init {
                if matches!(init.kind, ExprKind::Object(_)) {
                    self.state
                        .result
                        .object_types
                        .insert(init.span.start, ty.clone());
                }
                if let Type::Enum(enum_id) = ty.without_nullable() {
                    let is_enum_ctor = match &init.kind {
                        ExprKind::Call { callee, .. } => {
                            matches!(callee.kind, ExprKind::Member { .. })
                        }
                        ExprKind::Member { .. } => true,
                        _ => false,
                    };
                    if is_enum_ctor {
                        self.state
                            .result
                            .enum_targets
                            .insert(init.span.start, *enum_id);
                    }
                }
                let init_ty = self.check_expr(init);
                if !self.is_assignable(&init_ty, &ty) && !self.literal_target_ok(init, &ty) {
                    self.error(
                        format!(
                            "初始化式类型 {} 不能赋给 {}",
                            init_ty.display(),
                            ty.display()
                        ),
                        init.span,
                    );
                }
            }
            ty
        } else if let Some(init) = &variable.init {
            self.check_expr(init)
        } else {
            self.error("变量声明缺少类型标注或初始化表达式", variable.span);
            Type::Error
        };

        if self
            .scopes
            .last()
            .expect("作用域存在")
            .contains_key(&variable.name.name)
        {
            self.error(
                format!("变量 `{}` 重复声明", variable.name.name),
                variable.name.span,
            );
            return;
        }
        let id = self.alloc_local();
        self.symbols[id.0 as usize].kind = SymbolKind::Local {
            ty,
            mutable: variable.kind == VarKind::Let,
        };
        self.scopes
            .last_mut()
            .expect("作用域存在")
            .insert(variable.name.name.clone(), id);
    }

    fn literal_target_ok(&self, expr: &Expr, target: &Type) -> bool {
        match &expr.kind {
            ExprKind::Integer { .. } => target.is_integer(),
            ExprKind::Float { .. } => target.is_float(),
            _ => false,
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Type {
        let ty = self.check_expr_inner(expr);
        self.state
            .result
            .expr_types
            .insert((expr.span.start, expr.span.end), ty.clone());
        ty
    }

    fn check_expr_inner(&mut self, expr: &Expr) -> Type {
        match &expr.kind {
            ExprKind::Integer { suffix, .. } => match suffix {
                Some(sw_frontend::IntegerSuffix::I8) => Type::I8,
                Some(sw_frontend::IntegerSuffix::I16) => Type::I16,
                Some(sw_frontend::IntegerSuffix::I32) => Type::I32,
                Some(sw_frontend::IntegerSuffix::I64) => Type::I64,
                Some(sw_frontend::IntegerSuffix::Isize) => Type::Isize,
                Some(sw_frontend::IntegerSuffix::U8) => Type::U8,
                Some(sw_frontend::IntegerSuffix::U16) => Type::U16,
                Some(sw_frontend::IntegerSuffix::U32) => Type::U32,
                Some(sw_frontend::IntegerSuffix::U64) => Type::U64,
                Some(sw_frontend::IntegerSuffix::Usize) => Type::Usize,
                None => Type::Int,
            },
            ExprKind::Float { suffix, .. } => match suffix {
                Some(sw_frontend::FloatSuffix::F32) => Type::F32,
                Some(sw_frontend::FloatSuffix::F64) | None => Type::F64,
            },
            ExprKind::Str(_) => Type::Str,
            ExprKind::Char(_) => Type::Char,
            ExprKind::Bool(_) => Type::Bool,
            ExprKind::Null => Type::Null,
            ExprKind::Ident(ident) => match self.lookup(&ident.name) {
                Some(id) => {
                    self.state.result.ident_symbols.insert(expr.span.start, id);
                    self.symbol_type(id)
                }
                None => {
                    self.error(format!("未定义的名称 `{}`", ident.name), ident.span);
                    Type::Error
                }
            },
            ExprKind::This => match self.this_class {
                Some(class) => Type::Class(class),
                None => {
                    self.error("`this` 只能在类方法中使用", expr.span);
                    Type::Error
                }
            },
            ExprKind::Super => match self
                .this_class
                .and_then(|id| self.types.classes[id as usize].base)
            {
                Some(base) => Type::Class(base),
                None => {
                    self.error("`super` 只能在有基类的类中使用", expr.span);
                    Type::Error
                }
            },
            ExprKind::Group(inner) => self.check_expr(inner),
            ExprKind::Cast { expr: inner, ty } => {
                let from = self.check_expr(inner);
                let target = self.lower_type(ty);
                let convertible = match (&from, &target) {
                    (Type::Error, _) | (_, Type::Error) => true,
                    (from, to) => {
                        (from.is_numeric() && to.is_numeric())
                            || (*from == Type::Char && to.is_integer())
                            || (from.is_integer() && *to == Type::Char)
                            || (matches!(from, Type::Ptr(_)) && to.is_integer())
                            || (from.is_integer() && matches!(to, Type::Ptr(_)))
                            || (matches!(from, Type::Ptr(_)) && matches!(to, Type::Ptr(_)))
                    }
                };
                if !convertible {
                    self.error(
                        format!("不能把 {} 转换为 {}", from.display(), target.display()),
                        expr.span,
                    );
                }
                target
            }
            ExprKind::Unary { op, expr: inner } => {
                let ty = self.check_expr(inner);
                match op {
                    UnaryOp::Not => {
                        if ty != Type::Bool {
                            self.error(
                                format!("`!` 需要 bool，实际为 {}", ty.display()),
                                inner.span,
                            );
                        }
                        Type::Bool
                    }
                    UnaryOp::Neg | UnaryOp::Pos => {
                        if !ty.is_numeric() {
                            self.error(
                                format!("一元符号需要数值，实际为 {}", ty.display()),
                                inner.span,
                            );
                        }
                        ty
                    }
                    UnaryOp::BitNot => {
                        if !ty.is_integer() {
                            self.error(
                                format!("`~` 需要整数，实际为 {}", ty.display()),
                                inner.span,
                            );
                        }
                        ty
                    }
                    UnaryOp::Inc | UnaryOp::Dec => {
                        if !ty.is_numeric() || !self.is_lvalue(inner) {
                            self.error("`++`/`--` 需要可赋值的数值目标", inner.span);
                        }
                        ty
                    }
                    UnaryOp::Await => {
                        self.error("v0.1 语义阶段暂不支持 await", inner.span);
                        ty
                    }
                }
            }
            ExprKind::Binary { op, left, right } => self.check_binary(op, left, right, expr.span),
            ExprKind::Assign { op, target, value } => {
                if !self.is_lvalue(target) {
                    self.error("赋值目标不可写", target.span);
                }
                let target_ty = self.check_expr(target);
                let value_ty = self.check_expr(value);
                if *op == AssignOp::Assign {
                    if !self.is_assignable(&value_ty, &target_ty)
                        && !self.literal_target_ok(value, &target_ty)
                    {
                        self.error(
                            format!("不能把 {} 赋给 {}", value_ty.display(), target_ty.display()),
                            value.span,
                        );
                    }
                } else if matches!(op, AssignOp::LogicalAnd | AssignOp::LogicalOr) {
                    if target_ty != Type::Bool || !self.is_assignable(&value_ty, &target_ty) {
                        self.error(
                            format!("`&&=`/`||=` 需要 bool 目标，实际为 {}", target_ty.display()),
                            target.span,
                        );
                    }
                } else if *op == AssignOp::Add && target_ty == Type::Str {
                    // `string += x`：x 必须是字符串或可拼接标量。
                    if value_ty != Type::Str
                        && !Checker::concatable_with_string(&value_ty)
                        && !self.is_assignable(&value_ty, &Type::Str)
                    {
                        self.error(
                            format!("不能把 {} 拼接到 string", value_ty.display()),
                            value.span,
                        );
                    }
                } else if !target_ty.is_numeric() {
                    self.error("复合赋值需要数值目标", target.span);
                }
                target_ty
            }
            ExprKind::Conditional { cond, then, else_ } => {
                let cond_ty = self.check_expr(cond);
                if cond_ty != Type::Bool {
                    self.error(
                        format!("三元条件必须是 bool，实际为 {}", cond_ty.display()),
                        cond.span,
                    );
                }
                let then_ty = self.check_expr(then);
                let else_ty = self.check_expr(else_);
                if self.is_assignable(&then_ty, &else_ty) {
                    else_ty
                } else if self.is_assignable(&else_ty, &then_ty) {
                    then_ty
                } else {
                    self.error(
                        format!(
                            "三元表达式分支类型不一致：{} 与 {}",
                            then_ty.display(),
                            else_ty.display()
                        ),
                        expr.span,
                    );
                    Type::Error
                }
            }
            ExprKind::Call { callee, args } => self.check_call(callee, args, expr.span),
            ExprKind::Member {
                object,
                name,
                optional,
            } => self.check_member(object, name, *optional, expr.span),
            ExprKind::Index {
                object,
                index,
                optional,
            } => {
                let object_ty = self.check_expr(object);
                let index_ty = self.check_expr(index);
                if !index_ty.is_integer() {
                    self.error(
                        format!("索引必须是整数，实际为 {}", index_ty.display()),
                        index.span,
                    );
                }
                let base = object_ty.without_nullable().clone();
                let element = match &base {
                    Type::Array(inner) => (**inner).clone(),
                    Type::Str => Type::Char,
                    Type::Error => Type::Error,
                    other => {
                        self.error(format!("类型 {} 不能索引", other.display()), object.span);
                        Type::Error
                    }
                };
                if *optional || matches!(object_ty, Type::Nullable(_)) {
                    Type::Nullable(Box::new(element))
                } else {
                    element
                }
            }
            ExprKind::Slice { object, start, end } => {
                let object_ty = self.check_expr(object);
                for bound in [start.as_deref(), end.as_deref()].into_iter().flatten() {
                    let bound_ty = self.check_expr(bound);
                    if !bound_ty.is_integer() {
                        self.error(
                            format!("切片边界必须是整数，实际为 {}", bound_ty.display()),
                            bound.span,
                        );
                    }
                }
                match object_ty.without_nullable() {
                    Type::Array(inner) => Type::Array(inner.clone()),
                    other => {
                        self.error(
                            format!("切片只能用于数组，实际为 {}", other.display()),
                            object.span,
                        );
                        Type::Error
                    }
                }
            }
            ExprKind::Postfix { expr: inner, .. } => {
                let ty = self.check_expr(inner);
                if !ty.is_numeric() || !self.is_lvalue(inner) {
                    self.error("`++`/`--` 需要可赋值的数值目标", inner.span);
                }
                ty
            }
            ExprKind::TryOp(inner) => {
                let inner_ty = self.check_expr(inner);
                let Type::Enum(enum_id) = inner_ty.without_nullable() else {
                    self.error("`?` 只能用于 Result 枚举（含 Ok/Err 变体）", expr.span);
                    return Type::Error;
                };
                let Some((_, ok_ty, err_ty)) = self.result_variants(*enum_id) else {
                    self.error("`?` 目标必须是含 Ok/Err 变体的 Result 枚举", expr.span);
                    return Type::Error;
                };
                match self.ret.without_nullable() {
                    Type::Enum(ret_id) => {
                        let Some((_, _, ret_err_ty)) = self.result_variants(*ret_id) else {
                            self.error("`?` 所在函数必须返回 Result 类型", expr.span);
                            return Type::Error;
                        };
                        if !self.is_assignable(&err_ty, &ret_err_ty) {
                            self.error(
                                format!(
                                    "`?` 的错误类型 {} 与函数返回的 Err 类型 {} 不兼容",
                                    err_ty.display(),
                                    ret_err_ty.display()
                                ),
                                expr.span,
                            );
                        }
                    }
                    _ => {
                        self.error("`?` 所在函数必须返回 Result 类型", expr.span);
                        return Type::Error;
                    }
                }
                ok_ty
            }
            ExprKind::MatchExpr { value, arms } => {
                let value_ty = self.check_expr(value);
                let Type::Enum(enum_id) = value_ty.without_nullable() else {
                    self.error("match 目标必须是枚举类型", value.span);
                    return Type::Error;
                };
                let info = self.types.enums[*enum_id as usize].clone();
                let mut covered = vec![false; info.members.len()];
                let mut wildcard = false;
                let mut merged: Option<Type> = None;
                for arm in arms {
                    let body_ty = match &arm.pattern {
                        Pattern::Wildcard(_) => {
                            wildcard = true;
                            self.check_expr(&arm.body)
                        }
                        Pattern::Variant {
                            name,
                            bindings,
                            span,
                        } => {
                            let Some(index) = info
                                .members
                                .iter()
                                .position(|member| member.name == name.name)
                            else {
                                self.error(
                                    format!("枚举 {} 没有变体 `{}`", info.name, name.name),
                                    name.span,
                                );
                                continue;
                            };
                            let variant = &info.members[index];
                            if variant.fields.len() != bindings.len() {
                                self.error(
                                    format!(
                                        "变体 `{}` 需要 {} 个绑定变量，实际 {}",
                                        variant.name,
                                        variant.fields.len(),
                                        bindings.len()
                                    ),
                                    *span,
                                );
                            }
                            covered[index] = true;
                            self.scopes.push(HashMap::new());
                            for (binding, ty) in bindings.iter().zip(variant.fields.iter()) {
                                let id = self.alloc_local();
                                self.symbols[id.0 as usize].kind = SymbolKind::Local {
                                    ty: ty.clone(),
                                    mutable: false,
                                };
                                self.scopes
                                    .last_mut()
                                    .expect("作用域存在")
                                    .insert(binding.name.clone(), id);
                            }
                            let ty = self.check_expr(&arm.body);
                            self.scopes.pop();
                            ty
                        }
                    };
                    merged = Some(match merged {
                        None => body_ty,
                        Some(prev) => {
                            if self.is_assignable(&body_ty, &prev) {
                                prev
                            } else if self.is_assignable(&prev, &body_ty) {
                                body_ty
                            } else {
                                self.error(
                                    format!(
                                        "match 分支类型不一致：{} 与 {}",
                                        prev.display(),
                                        body_ty.display()
                                    ),
                                    arm.span,
                                );
                                prev
                            }
                        }
                    });
                }
                if !wildcard {
                    let uncovered: Vec<&str> = info
                        .members
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !covered[*index])
                        .map(|(_, member)| member.name.as_str())
                        .collect();
                    if !uncovered.is_empty() {
                        self.error(
                            format!("match 未穷尽：缺少变体 {}", uncovered.join(", ")),
                            value.span,
                        );
                    }
                }
                merged.unwrap_or(Type::Error)
            }
            ExprKind::Spread(inner) => {
                self.error("`...` 展开只能用于数组/对象字面量或调用参数", expr.span);
                self.check_expr(inner)
            }
            ExprKind::Array(items) => {
                let mut element = None;
                for item in items {
                    let ty = match &item.kind {
                        ExprKind::Spread(inner) => {
                            let ty = self.check_expr(inner);
                            match ty.without_nullable() {
                                Type::Array(inner_ty) => (**inner_ty).clone(),
                                other => {
                                    self.error(
                                        format!("展开目标必须是数组，实际为 {}", other.display()),
                                        inner.span,
                                    );
                                    Type::Error
                                }
                            }
                        }
                        _ => self.check_expr(item),
                    };
                    match &element {
                        None => element = Some(ty),
                        Some(expected) => {
                            if expected != &ty {
                                self.error(
                                    format!(
                                        "数组元素类型不一致：{} 与 {}",
                                        expected.display(),
                                        ty.display()
                                    ),
                                    item.span,
                                );
                            }
                        }
                    }
                }
                match element {
                    Some(element) => Type::Array(Box::new(element)),
                    None => Type::Array(Box::new(Type::Error)),
                }
            }
            ExprKind::Object(fields) => {
                let Some(target) = self
                    .state
                    .result
                    .object_types
                    .get(&expr.span.start)
                    .cloned()
                else {
                    self.error("对象字面量需要目标类型（结构体或类初始化）", expr.span);
                    return Type::Error;
                };
                match target {
                    Type::Struct(id) => {
                        let struct_name = self.types.struct_name(id).to_string();
                        let field_types: Vec<(String, Type)> = self.types.structs[id as usize]
                            .fields
                            .iter()
                            .map(|field| (field.name.clone(), field.ty.clone()))
                            .collect();
                        for field in fields {
                            if let Some(spread_expr) = &field.spread {
                                let spread_ty = self.check_expr(spread_expr);
                                if !matches!(
                                    spread_ty.without_nullable(),
                                    Type::Struct(spread_id) if *spread_id == id
                                ) {
                                    self.error(
                                        format!(
                                            "`{{...}}` 展开目标必须是同类型 struct（{}）",
                                            struct_name
                                        ),
                                        spread_expr.span,
                                    );
                                }
                                continue;
                            }
                            let field_name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            let field_ty = field_types
                                .iter()
                                .find(|(name, _)| *name == field_name)
                                .map(|(_, ty)| ty.clone());
                            if field_ty.is_none() {
                                self.error(
                                    format!("结构体 {struct_name} 没有字段 `{field_name}`"),
                                    expr.span,
                                );
                            }
                            // 嵌套结构体字面量：按字段类型传播目标类型。
                            if let (Some(Type::Struct(_)), ExprKind::Object(_)) =
                                (&field_ty, &field.value.kind)
                            {
                                self.state
                                    .result
                                    .object_types
                                    .insert(field.value.span.start, field_ty.clone().unwrap());
                            }
                            self.check_expr(&field.value);
                        }
                        Type::Struct(id)
                    }
                    Type::Class(id) => {
                        let class_name = self.types.class_name(id).to_string();
                        for field in fields {
                            let field_name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            if self.types.find_class_field(id, &field_name).is_none() {
                                self.error(
                                    format!("类 {class_name} 没有字段 `{field_name}`"),
                                    expr.span,
                                );
                            }
                            self.check_expr(&field.value);
                        }
                        Type::Class(id)
                    }
                    other => {
                        self.error(
                            format!(
                                "对象字面量目标类型必须是 struct/class，实际为 {}",
                                other.display()
                            ),
                            expr.span,
                        );
                        Type::Error
                    }
                }
            }
            ExprKind::New { ty, args } => {
                let class = self.lower_type(ty);
                match class {
                    Type::Class(id) => {
                        self.state
                            .result
                            .new_types
                            .insert(expr.span.start, Type::Class(id));
                        let args_ty = self.call_args_ty(args);
                        let info = self.types.classes[id as usize].clone();
                        let Some(constructor) = info
                            .methods
                            .iter()
                            .find(|method| method.name == "constructor")
                        else {
                            if !args_ty.is_empty() {
                                self.error(format!("类 {} 没有构造函数", info.name), expr.span);
                            }
                            return Type::Class(id);
                        };
                        self.match_call_args(&constructor.sig, &args_ty, expr.span, true);
                        Type::Class(id)
                    }
                    other => {
                        self.error(
                            format!("`new` 只能创建 class，实际为 {}", other.display()),
                            ty.span,
                        );
                        Type::Error
                    }
                }
            }
            ExprKind::Template(parts) => {
                for part in parts {
                    match part {
                        TemplatePart::Text(_) => {}
                        TemplatePart::Expr(expr) => {
                            let ty = self.check_expr(expr);
                            if !matches!(ty, Type::Str | Type::Char | Type::Bool | Type::Int)
                                && !ty.is_numeric()
                            {
                                self.error(
                                    format!("模板插值不支持类型 {}", ty.display()),
                                    expr.span,
                                );
                            }
                        }
                    }
                }
                Type::Str
            }
            ExprKind::Lambda { params, body } => {
                let mut param_types = Vec::new();
                self.scopes.push(HashMap::new());
                for param in params {
                    let ty = match &param.ty {
                        Some(ty) => self.lower_type(ty),
                        None => {
                            self.error("v0.1 lambda 参数必须标注类型", param.name.span);
                            Type::Error
                        }
                    };
                    let id = self.alloc_local();
                    self.symbols[id.0 as usize].kind = SymbolKind::Param { ty: ty.clone() };
                    self.scopes
                        .last_mut()
                        .expect("作用域存在")
                        .insert(param.name.name.clone(), id);
                    param_types.push(ty);
                }
                let ret = match body {
                    LambdaBody::Expr(expr) => self.check_expr(expr),
                    LambdaBody::Block(block) => {
                        self.check_block(block);
                        Type::Void
                    }
                };
                self.scopes.pop();
                Type::Function {
                    params: param_types,
                    ret: Box::new(ret),
                }
            }
        }
    }

    /// 可与字符串 `+` 拼接的类型（自动转字符串）。
    fn concatable_with_string(ty: &Type) -> bool {
        matches!(
            ty,
            Type::Str | Type::Int | Type::F32 | Type::F64 | Type::Bool | Type::Char
        )
    }

    fn check_binary(&mut self, op: &BinaryOp, left: &Expr, right: &Expr, span: Span) -> Type {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        let (left_ty, right_ty) = self.adapt_literal_operands(op, left, right, left_ty, right_ty);
        match op {
            BinaryOp::Add => {
                if left_ty == Type::Str && right_ty == Type::Str {
                    return Type::Str;
                }
                // "a" + 42 / "a" + true / 42 + "a"：标量自动转字符串拼接（JS 风格）。
                if (left_ty == Type::Str && Self::concatable_with_string(&right_ty))
                    || (right_ty == Type::Str && Self::concatable_with_string(&left_ty))
                {
                    return Type::Str;
                }
                if left_ty.is_numeric() && left_ty == right_ty {
                    return left_ty;
                }
                self.binary_type_error(op, &left_ty, &right_ty, span);
                Type::Error
            }
            BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem | BinaryOp::Pow => {
                if left_ty.is_numeric() && left_ty == right_ty {
                    return left_ty;
                }
                self.binary_type_error(op, &left_ty, &right_ty, span);
                Type::Error
            }
            BinaryOp::Eq | BinaryOp::Ne => {
                if left_ty == right_ty
                    || (left_ty == Type::Null
                        && (right_ty.is_reference() || matches!(right_ty, Type::Nullable(_))))
                    || (right_ty == Type::Null
                        && (left_ty.is_reference() || matches!(left_ty, Type::Nullable(_))))
                {
                    Type::Bool
                } else {
                    self.binary_type_error(op, &left_ty, &right_ty, span);
                    Type::Bool
                }
            }
            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                if left_ty.is_numeric() && left_ty == right_ty {
                    Type::Bool
                } else {
                    self.binary_type_error(op, &left_ty, &right_ty, span);
                    Type::Bool
                }
            }
            BinaryOp::And | BinaryOp::Or => {
                if left_ty == Type::Bool && right_ty == Type::Bool {
                    Type::Bool
                } else {
                    self.binary_type_error(op, &left_ty, &right_ty, span);
                    Type::Bool
                }
            }
            BinaryOp::BitAnd
            | BinaryOp::BitOr
            | BinaryOp::BitXor
            | BinaryOp::Shl
            | BinaryOp::Shr => {
                if left_ty.is_integer() && left_ty == right_ty {
                    left_ty
                } else {
                    self.binary_type_error(op, &left_ty, &right_ty, span);
                    Type::Error
                }
            }
            BinaryOp::Coalesce => {
                if matches!(left_ty, Type::Nullable(_))
                    && self.is_assignable(&right_ty, left_ty.without_nullable())
                {
                    left_ty.without_nullable().clone()
                } else {
                    self.error(
                        format!("`??` 左侧必须是可空类型，实际为 {}", left_ty.display()),
                        span,
                    );
                    Type::Error
                }
            }
        }
    }

    fn adapt_literal_operands(
        &self,
        _op: &BinaryOp,
        left: &Expr,
        right: &Expr,
        left_ty: Type,
        right_ty: Type,
    ) -> (Type, Type) {
        let left_literal = matches!(left.kind, ExprKind::Integer { .. } | ExprKind::Float { .. });
        let right_literal = matches!(
            right.kind,
            ExprKind::Integer { .. } | ExprKind::Float { .. }
        );
        if left_literal && right_ty.is_numeric() {
            (right_ty.clone(), right_ty)
        } else if right_literal && left_ty.is_numeric() {
            (left_ty.clone(), left_ty)
        } else {
            (left_ty, right_ty)
        }
    }

    fn binary_type_error(&mut self, op: &BinaryOp, left: &Type, right: &Type, span: Span) {
        self.error(
            format!(
                "二元运算 `{}` 的操作数类型不匹配：{} 与 {}",
                binary_op_name(op),
                left.display(),
                right.display()
            ),
            span,
        );
    }

    fn is_lvalue(&mut self, expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ident(ident) => match self.lookup(&ident.name) {
                Some(id) => match &self.symbols[id.0 as usize].kind {
                    SymbolKind::Local { mutable, .. } => *mutable,
                    SymbolKind::Global { mutable, .. } => *mutable,
                    SymbolKind::Param { .. } => false,
                    _ => false,
                },
                None => false,
            },
            ExprKind::Member { object, name, .. } => {
                let ty = self.check_expr(object);
                match ty {
                    Type::Struct(id) => self
                        .types
                        .structs
                        .get(id as usize)
                        .and_then(|info| info.fields.iter().find(|f| f.name == name.name))
                        .map(|field| field.mutable)
                        .unwrap_or(false),
                    Type::Class(id) => self
                        .types
                        .find_class_field(id, &name.name)
                        .map(|(_, index)| {
                            let (class, _) = self.types.find_class_field(id, &name.name).unwrap();
                            self.types.classes[class as usize].fields[index].mutable
                        })
                        .unwrap_or(false),
                    _ => false,
                }
            }
            ExprKind::Index { .. } => true,
            _ => false,
        }
    }

    fn check_member(&mut self, object: &Expr, name: &Ident, optional: bool, span: Span) -> Type {
        // 类静态字段/方法访问：ClassName.member（类型名作为值解析为 Error）。
        if let ExprKind::Ident(type_ident) = &object.kind {
            if let Some(SymbolId(id)) = self.lookup(&type_ident.name) {
                if let SymbolKind::Type(SymbolType::Class(class_id)) =
                    &self.symbols[id as usize].kind
                {
                    let class_id = *class_id;
                    let info = self.types.classes[class_id as usize].clone();
                    if let Some(index) = info.static_fields.iter().position(|f| f.name == name.name)
                    {
                        self.state
                            .result
                            .static_member_targets
                            .insert(span.start, StaticMemberTarget::Field(class_id, index));
                        return info.static_fields[index].ty.clone();
                    }
                    if let Some(index) =
                        info.static_methods.iter().position(|m| m.name == name.name)
                    {
                        let sig = &info.static_methods[index].sig;
                        self.state
                            .result
                            .static_member_targets
                            .insert(span.start, StaticMemberTarget::Method(class_id, index));
                        return Type::Function {
                            params: sig.params.iter().map(|p| p.ty.clone()).collect(),
                            ret: Box::new(sig.ret.clone()),
                        };
                    }
                    self.error(
                        format!("类 {} 没有静态成员 `{}`", info.name, name.name),
                        name.span,
                    );
                    return Type::Error;
                }
            }
        }
        // ADT 枚举无参变体构造：EnumName.Variant（类型名作为值解析为 Error，
        // 这里直接识别枚举类型名）。
        if let ExprKind::Ident(type_ident) = &object.kind {
            if let Some(SymbolId(id)) = self.lookup(&type_ident.name) {
                if let SymbolKind::Type(SymbolType::Enum(enum_id)) = &self.symbols[id as usize].kind
                {
                    let enum_id = *enum_id;
                    let info = self.types.enums[enum_id as usize].clone();
                    if !info.members.is_empty() {
                        let Some(index) = info.members.iter().position(|m| m.name == name.name)
                        else {
                            self.error(
                                format!("枚举 {} 没有变体 `{}`", info.name, name.name),
                                name.span,
                            );
                            return Type::Error;
                        };
                        let resolved = self
                            .state
                            .result
                            .enum_targets
                            .get(&span.start)
                            .copied()
                            .unwrap_or(enum_id);
                        self.state.result.call_targets.insert(
                            (span.start, span.end),
                            CallTarget::EnumConstruct {
                                enum_id: resolved,
                                variant_index: index,
                            },
                        );
                        return Type::Enum(resolved);
                    }
                }
            }
        }
        let object_ty = self.check_expr(object);
        let base = object_ty.without_nullable().clone();
        let result = match &base {
            Type::Class(id) => {
                if let Some((class, index)) = self.types.find_class_field(*id, &name.name) {
                    self.state
                        .result
                        .field_targets
                        .insert(span.start, FieldTarget::Class(class, index));
                    self.types.classes[class as usize].fields[index].ty.clone()
                } else if name.name == "length" {
                    Type::Int
                } else if let Some((_, index)) = self.types.find_class_method(*id, &name.name) {
                    let (class, _) = self.types.find_class_method(*id, &name.name).unwrap();
                    let method = &self.types.classes[class as usize].methods[index];
                    let sig = &method.sig;
                    Type::Function {
                        params: sig.params.iter().map(|param| param.ty.clone()).collect(),
                        ret: Box::new(sig.ret.clone()),
                    }
                } else {
                    self.error(
                        format!("类 {} 没有成员 `{}`", self.types.class_name(*id), name.name),
                        name.span,
                    );
                    Type::Error
                }
            }
            Type::Struct(id) => {
                if let Some(field) = self
                    .types
                    .structs
                    .get(*id as usize)
                    .and_then(|info| info.fields.iter().position(|f| f.name == name.name))
                {
                    self.state
                        .result
                        .field_targets
                        .insert(span.start, FieldTarget::Struct(*id, field));
                    self.types.structs[*id as usize].fields[field].ty.clone()
                } else {
                    self.error(
                        format!(
                            "结构体 {} 没有字段 `{}`",
                            self.types.struct_name(*id),
                            name.name
                        ),
                        name.span,
                    );
                    Type::Error
                }
            }
            Type::Interface(id) => {
                if let Some(index) = self
                    .types
                    .interfaces
                    .get(*id as usize)
                    .and_then(|info| info.methods.iter().position(|m| m.name == name.name))
                {
                    let sig = &self.types.interfaces[*id as usize].methods[index];
                    Type::Function {
                        params: sig.params.iter().map(|param| param.ty.clone()).collect(),
                        ret: Box::new(sig.ret.clone()),
                    }
                } else {
                    self.error(
                        format!(
                            "接口 {} 没有方法 `{}`",
                            self.types.interfaces[*id as usize].name, name.name
                        ),
                        name.span,
                    );
                    Type::Error
                }
            }
            Type::Enum(id) => {
                let Some(info) = self.types.enums.get(*id as usize) else {
                    self.error(
                        format!("枚举 {} 不存在", self.types.enum_name(*id)),
                        name.span,
                    );
                    return Type::Error;
                };
                let Some(index) = info.members.iter().position(|m| m.name == name.name) else {
                    self.error(
                        format!("枚举 {} 没有成员 `{}`", info.name, name.name),
                        name.span,
                    );
                    return Type::Error;
                };
                if info.members.iter().any(|m| !m.fields.is_empty()) {
                    // ADT 无参变体构造：EnumName.Variant
                    let resolved = self
                        .state
                        .result
                        .enum_targets
                        .get(&span.start)
                        .copied()
                        .unwrap_or(*id);
                    self.state.result.call_targets.insert(
                        (span.start, span.end),
                        CallTarget::EnumConstruct {
                            enum_id: resolved,
                            variant_index: index,
                        },
                    );
                    Type::Enum(resolved)
                } else {
                    // C 风格枚举成员（值 = discriminant）。
                    Type::Enum(*id)
                }
            }
            Type::Str | Type::Array(_) => {
                if name.name == "length" {
                    Type::Int
                } else {
                    self.error(
                        format!("类型 {} 没有成员 `{}`", base.display(), name.name),
                        name.span,
                    );
                    Type::Error
                }
            }
            Type::Error => Type::Error,
            other => {
                self.error(
                    format!("类型 {} 没有成员 `{}`", other.display(), name.name),
                    name.span,
                );
                Type::Error
            }
        };
        if optional || matches!(object_ty, Type::Nullable(_)) {
            if matches!(object_ty, Type::Nullable(_)) && !optional {
                self.error("值可能为空，请使用 `?.`", object.span);
            }
            Type::Nullable(Box::new(result))
        } else {
            result
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[Expr], span: Span) -> Type {
        // 调用实参：`...数组字面量` 展开成多个实参（变量数组展开暂不支持）。
        let args_ty = self.call_args_ty(args);
        // 命名空间函数调用：ns.foo(...)
        if let ExprKind::Member { object, name, .. } = &callee.kind {
            if let ExprKind::Ident(ns_ident) = &object.kind {
                if let Some(SymbolId(id)) = self.lookup(&ns_ident.name) {
                    if let SymbolKind::Namespace(target) = &self.symbols[id as usize].kind {
                        let target_names =
                            self.module_names.get(target).cloned().unwrap_or_default();
                        if let Some(symbol_ids) = target_names.get(&name.name) {
                            let mut candidates = Vec::new();
                            for symbol_id in symbol_ids {
                                if let SymbolKind::Function(sig) =
                                    &self.symbols[symbol_id.0 as usize].kind
                                {
                                    candidates.push((*symbol_id, sig.clone()));
                                }
                            }
                            if let Some((symbol_id, sig)) =
                                self.pick_overload(&candidates, &args_ty, span, true)
                            {
                                self.state.result.call_targets.insert(
                                    (span.start, span.end),
                                    CallTarget::Function(symbol_id),
                                );
                                return self.record_call_result(span, sig.ret);
                            }
                            return Type::Error;
                        }
                    }
                }
            }
        }

        match &callee.kind {
            ExprKind::Ident(ident) => {
                // 闭包变量调用：f(...)
                if let Some(symbol_id) = self.lookup(&ident.name) {
                    let is_variable = matches!(
                        self.symbols[symbol_id.0 as usize].kind,
                        SymbolKind::Local { .. }
                            | SymbolKind::Param { .. }
                            | SymbolKind::Global { .. }
                    );
                    if is_variable {
                        let symbol_type = self.symbol_type(symbol_id);
                        if let Type::Function { params, ret } = &symbol_type {
                            self.state
                                .result
                                .ident_symbols
                                .insert(ident.span.start, symbol_id);
                            self.state
                                .result
                                .expr_types
                                .insert((ident.span.start, ident.span.end), symbol_type.clone());
                            let args_ty = self.call_args_ty(args);
                            if args_ty.len() != params.len() {
                                self.error(
                                    format!(
                                        "闭包调用参数数量不匹配：需要 {} 个，实际 {} 个",
                                        params.len(),
                                        args_ty.len()
                                    ),
                                    span,
                                );
                                return Type::Error;
                            }
                            for (arg_ty, param_ty) in args_ty.iter().zip(params.iter()) {
                                if !self.is_assignable(arg_ty, param_ty) {
                                    self.error(
                                        format!(
                                            "闭包参数类型不匹配：{} 不能赋给 {}",
                                            arg_ty.display(),
                                            param_ty.display()
                                        ),
                                        span,
                                    );
                                }
                            }
                            return self.record_call_result(span, (**ret).clone());
                        }
                    }
                }
                let Some(symbol_ids) = self.state.names.get(&ident.name).cloned() else {
                    self.error(format!("未定义的函数 `{}`", ident.name), ident.span);
                    return Type::Error;
                };
                let mut candidates = Vec::new();
                for symbol_id in &symbol_ids {
                    if let SymbolKind::Function(sig) = &self.symbols[symbol_id.0 as usize].kind {
                        candidates.push((*symbol_id, sig.clone()));
                    }
                }
                if candidates.is_empty() {
                    self.error(format!("`{}` 不是函数", ident.name), ident.span);
                    return Type::Error;
                }
                if let Some((symbol_id, sig)) =
                    self.pick_overload(&candidates, &args_ty, span, true)
                {
                    self.state
                        .result
                        .call_targets
                        .insert((span.start, span.end), CallTarget::Function(symbol_id));
                    return self.record_call_result(span, sig.ret);
                }
                Type::Error
            }
            ExprKind::Super => {
                let base = self
                    .this_class
                    .and_then(|id| self.types.classes[id as usize].base);
                let Some(base) = base else {
                    self.error("`super(...)` 只能在有基类的构造函数中使用", span);
                    return Type::Error;
                };
                let info = self.types.classes[base as usize].clone();
                let Some(constructor) = info
                    .methods
                    .iter()
                    .position(|method| method.name == "constructor")
                else {
                    if !args_ty.is_empty() {
                        self.error("基类没有构造函数", span);
                    }
                    return Type::Class(base);
                };
                let sig = info.methods[constructor].sig.clone();
                self.match_call_args(&sig, &args_ty, span, true);
                self.state.result.call_targets.insert(
                    (span.start, span.end),
                    CallTarget::Method {
                        class: base,
                        index: constructor,
                    },
                );
                self.record_call_result(span, Type::Class(base))
            }
            ExprKind::Member {
                object,
                name,
                optional,
            } => {
                // 类静态方法调用：ClassName.staticMethod(args)
                if let ExprKind::Ident(type_ident) = &object.kind {
                    if let Some(SymbolId(id)) = self.lookup(&type_ident.name) {
                        if let SymbolKind::Type(SymbolType::Class(class_id)) =
                            &self.symbols[id as usize].kind
                        {
                            let class_id = *class_id;
                            let info = self.types.classes[class_id as usize].clone();
                            let Some(index) =
                                info.static_methods.iter().position(|m| m.name == name.name)
                            else {
                                self.error(
                                    format!("类 {} 没有静态方法 `{}`", info.name, name.name),
                                    name.span,
                                );
                                return Type::Error;
                            };
                            let sig = info.static_methods[index].sig.clone();
                            let args_ty = self.call_args_ty(args);
                            self.match_call_args(&sig, &args_ty, span, true);
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::StaticMethod {
                                    class: class_id,
                                    index,
                                },
                            );
                            return sig.ret;
                        }
                    }
                }
                // ADT 枚举变体构造：EnumName.Variant(args)
                if let ExprKind::Ident(type_ident) = &object.kind {
                    if let Some(SymbolId(id)) = self.lookup(&type_ident.name) {
                        if let SymbolKind::Type(SymbolType::Enum(enum_id)) =
                            &self.symbols[id as usize].kind
                        {
                            let enum_id = *enum_id;
                            let info = self.types.enums[enum_id as usize].clone();
                            let Some(index) = info
                                .members
                                .iter()
                                .position(|member| member.name == name.name)
                            else {
                                self.error(
                                    format!("枚举 {} 没有变体 `{}`", info.name, name.name),
                                    name.span,
                                );
                                return Type::Error;
                            };
                            let variant = &info.members[index];
                            let resolved_enum =
                                match self.state.result.enum_targets.get(&span.start).copied() {
                                    // 带注解声明提供了目标实例（如 Option<int> 的 None）。
                                    Some(target_id) => target_id,
                                    None => {
                                        let mut type_args: HashMap<String, Type> = HashMap::new();
                                        for (arg, field) in
                                            args_ty.iter().zip(variant.fields.iter())
                                        {
                                            if let Type::TypeParam(param_name) = field {
                                                type_args
                                                    .entry(param_name.clone())
                                                    .or_insert_with(|| arg.clone());
                                            }
                                        }
                                        self.instantiate_enum_types(enum_id, &type_args)
                                    }
                                };
                            let resolved_info = self.types.enums[resolved_enum as usize].clone();
                            let resolved_variant = &resolved_info.members[index];
                            if args_ty.len() != resolved_variant.fields.len() {
                                self.error(
                                    format!(
                                        "变体 `{}` 需要 {} 个参数，实际 {}",
                                        variant.name,
                                        resolved_variant.fields.len(),
                                        args_ty.len()
                                    ),
                                    span,
                                );
                            }
                            for (i, (arg, field)) in args_ty
                                .iter()
                                .zip(resolved_variant.fields.iter())
                                .enumerate()
                            {
                                if !self.is_assignable(arg, field) {
                                    self.error(
                                        format!(
                                            "变体 `{}` 参数 {i} 类型不匹配：{} 不能赋给 {}",
                                            variant.name,
                                            arg.display(),
                                            field.display()
                                        ),
                                        span,
                                    );
                                }
                            }
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::EnumConstruct {
                                    enum_id: resolved_enum,
                                    variant_index: index,
                                },
                            );
                            return Type::Enum(resolved_enum);
                        }
                    }
                }
                let object_ty = self.check_expr(object);
                let result = match object_ty.without_nullable() {
                    Type::TypeParam(param_name) => {
                        let mut resolved = None;
                        if let Some(bounds) = self.bounds.get(param_name) {
                            for bound_ty in bounds {
                                if let Type::Interface(interface_id) = bound_ty {
                                    if let Some(index) = self.types.interfaces
                                        [*interface_id as usize]
                                        .methods
                                        .iter()
                                        .position(|m| m.name == name.name)
                                    {
                                        resolved = Some((*interface_id, index));
                                        break;
                                    }
                                }
                            }
                        }
                        match resolved {
                            Some((interface_id, index)) => {
                                let sig = self.types.interfaces[interface_id as usize].methods
                                    [index]
                                    .clone();
                                self.match_call_args(&sig, &args_ty, span, true);
                                self.state.result.call_targets.insert(
                                    (span.start, span.end),
                                    CallTarget::InterfaceMethod {
                                        interface: interface_id,
                                        index,
                                    },
                                );
                                sig.ret
                            }
                            None => {
                                self.error(
                                    format!(
                                        "类型 {} 没有可用方法 `{}`（需要 `where {}: 接口` 约束）",
                                        param_name, name.name, param_name
                                    ),
                                    name.span,
                                );
                                Type::Error
                            }
                        }
                    }
                    Type::Class(id) => {
                        let methods = self.types.class_methods_named(*id, &name.name);
                        if methods.is_empty() {
                            self.error(
                                format!(
                                    "类 {} 没有方法 `{}`",
                                    self.types.class_name(*id),
                                    name.name
                                ),
                                name.span,
                            );
                            return Type::Error;
                        }
                        let mut candidates = Vec::new();
                        for (class_id, index) in &methods {
                            let sig = self.types.classes[*class_id as usize].methods[*index]
                                .sig
                                .clone();
                            candidates.push((SymbolId(candidates.len() as u32), sig));
                        }
                        let Some((SymbolId(choice), sig)) =
                            self.pick_overload(&candidates, &args_ty, span, true)
                        else {
                            return Type::Error;
                        };
                        let (class_id, index) = methods[choice as usize];
                        self.state.result.call_targets.insert(
                            (span.start, span.end),
                            CallTarget::Method {
                                class: class_id,
                                index,
                            },
                        );
                        sig.ret
                    }
                    Type::Interface(id) => {
                        if let Some(index) =
                            self.types.interfaces.get(*id as usize).and_then(|info| {
                                info.methods.iter().position(|m| m.name == name.name)
                            })
                        {
                            let sig = self.types.interfaces[*id as usize].methods[index].clone();
                            self.match_call_args(&sig, &args_ty, span, true);
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::InterfaceMethod {
                                    interface: *id,
                                    index,
                                },
                            );
                            sig.ret
                        } else {
                            self.error(
                                format!(
                                    "接口 {} 没有方法 `{}`",
                                    self.types.interfaces[*id as usize].name, name.name
                                ),
                                name.span,
                            );
                            Type::Error
                        }
                    }
                    Type::Str => {
                        if let Some((runtime_name, param_tys, ret)) = string_method(&name.name) {
                            if args_ty.len() != param_tys.len() {
                                self.error(
                                    format!(
                                        "方法 `{}` 需要 {} 个参数，实际 {} 个",
                                        name.name,
                                        param_tys.len(),
                                        args_ty.len()
                                    ),
                                    span,
                                );
                                return Type::Error;
                            }
                            for (index, (arg_ty, param_ty)) in
                                args_ty.iter().zip(param_tys.iter()).enumerate()
                            {
                                if !self.is_assignable(arg_ty, param_ty) {
                                    self.error(
                                        format!(
                                            "方法 `{}` 参数 {index} 类型不匹配：{} 不能赋给 {}",
                                            name.name,
                                            arg_ty.display(),
                                            param_ty.display()
                                        ),
                                        span,
                                    );
                                }
                            }
                            let mut params = vec![ParamSig {
                                name: "self".to_owned(),
                                ty: Type::Str,
                                has_default: false,
                                rest: false,
                            }];
                            for (index, ty) in param_tys.iter().enumerate() {
                                params.push(ParamSig {
                                    name: format!("arg{index}"),
                                    ty: ty.clone(),
                                    has_default: false,
                                    rest: false,
                                });
                            }
                            let sig = FunctionSig {
                                module: self.state.id,
                                name: runtime_name.clone(),
                                generics: Vec::new(),
                                bounds: HashMap::new(),
                                params,
                                ret: ret.clone(),
                                extern_c: true,
                                span,
                            };
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::StrMethod { runtime_name, sig },
                            );
                            ret
                        } else {
                            self.error(format!("string 没有方法 `{}`", name.name), name.span);
                            Type::Error
                        }
                    }
                    Type::Array(inner) => {
                        if matches!(
                            name.name.as_str(),
                            "map" | "filter" | "forEach" | "some" | "every" | "find"
                        ) && args.len() == 1
                        {
                            let fn_ty = args_ty[0].clone();
                            let Type::Function {
                                params: fn_params,
                                ret: fn_ret,
                            } = &fn_ty
                            else {
                                self.error(
                                    format!("`{}` 需要一个函数参数", name.name),
                                    args[0].span,
                                );
                                return Type::Error;
                            };
                            if fn_params.len() != 1 || !self.is_assignable(inner, &fn_params[0]) {
                                self.error(
                                    format!(
                                        "`{}` 的函数参数必须接收一个 {}",
                                        name.name,
                                        inner.display()
                                    ),
                                    args[0].span,
                                );
                                return Type::Error;
                            }
                            if matches!(name.name.as_str(), "filter" | "some" | "every" | "find")
                                && !self.is_assignable(&fn_ret, &Type::Bool)
                            {
                                self.error(
                                    format!("`{}` 的函数必须返回 bool", name.name),
                                    args[0].span,
                                );
                                return Type::Error;
                            }
                            let method = match name.name.as_str() {
                                "map" => ArrayMethodKind::Map,
                                "filter" => ArrayMethodKind::Filter,
                                "forEach" => ArrayMethodKind::ForEach,
                                "some" => ArrayMethodKind::Some,
                                "every" => ArrayMethodKind::Every,
                                _ => ArrayMethodKind::Find,
                            };
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::ArrayMethod {
                                    method,
                                    elem: (**inner).clone(),
                                    ret: (**fn_ret).clone(),
                                },
                            );
                            match method {
                                ArrayMethodKind::Map => Type::Array(Box::new((**fn_ret).clone())),
                                ArrayMethodKind::Filter => Type::Array(inner.clone()),
                                ArrayMethodKind::ForEach => Type::Void,
                                ArrayMethodKind::Some | ArrayMethodKind::Every => Type::Bool,
                                ArrayMethodKind::Find => Type::Nullable(inner.clone()),
                                ArrayMethodKind::Push | ArrayMethodKind::Pop => Type::Error,
                            }
                        } else if name.name == "push" && args.len() == 1 {
                            if !self.is_assignable(&args_ty[0], inner) {
                                self.error(
                                    format!("`push` 参数必须是 {}", inner.display()),
                                    args[0].span,
                                );
                                return Type::Error;
                            }
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::ArrayMethod {
                                    method: ArrayMethodKind::Push,
                                    elem: (**inner).clone(),
                                    ret: Type::Int,
                                },
                            );
                            Type::Int
                        } else if name.name == "pop" && args.is_empty() {
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::ArrayMethod {
                                    method: ArrayMethodKind::Pop,
                                    elem: (**inner).clone(),
                                    ret: (**inner).clone(),
                                },
                            );
                            (**inner).clone()
                        } else if name.name == "join" && matches!(**inner, Type::Str) {
                            if args_ty.len() != 1 || !self.is_assignable(&args_ty[0], &Type::Str) {
                                self.error("`join` 需要一个 string 分隔符", span);
                                return Type::Error;
                            }
                            let sig = FunctionSig {
                                module: self.state.id,
                                name: "join".to_owned(),
                                generics: Vec::new(),
                                bounds: HashMap::new(),
                                params: vec![
                                    ParamSig {
                                        name: "self".to_owned(),
                                        ty: Type::Array(Box::new(Type::Str)),
                                        has_default: false,
                                        rest: false,
                                    },
                                    ParamSig {
                                        name: "sep".to_owned(),
                                        ty: Type::Str,
                                        has_default: false,
                                        rest: false,
                                    },
                                ],
                                ret: Type::Str,
                                extern_c: true,
                                span,
                            };
                            self.state.result.call_targets.insert(
                                (span.start, span.end),
                                CallTarget::StrMethod {
                                    runtime_name: "join".to_owned(),
                                    sig,
                                },
                            );
                            Type::Str
                        } else {
                            self.error(
                                format!(
                                    "{} 没有方法 `{}`",
                                    Type::Array(inner.clone()).display(),
                                    name.name
                                ),
                                name.span,
                            );
                            Type::Error
                        }
                    }
                    Type::Error => Type::Error,
                    other => {
                        self.error(
                            format!("类型 {} 不能调用方法", other.display()),
                            callee.span,
                        );
                        Type::Error
                    }
                };
                let result = if *optional || matches!(object_ty, Type::Nullable(_)) {
                    Type::Nullable(Box::new(result))
                } else {
                    result
                };
                self.record_call_result(span, result)
            }
            _ => {
                self.error("调用目标必须是函数名或方法", callee.span);
                Type::Error
            }
        }
    }

    /// 调用实参类型：`...数组字面量` 展开为多个实参类型。
    fn call_args_ty(&mut self, args: &[Expr]) -> Vec<Type> {
        let mut out = Vec::new();
        for arg in args {
            if let ExprKind::Spread(inner) = &arg.kind {
                match &inner.kind {
                    ExprKind::Array(items) => {
                        for item in items {
                            out.push(self.check_expr(item));
                        }
                    }
                    _ => {
                        self.error("调用展开暂只支持数组字面量", inner.span);
                        out.push(Type::Error);
                    }
                }
            } else {
                out.push(self.check_expr(arg));
            }
        }
        out
    }

    fn pick_overload(
        &mut self,
        candidates: &[(SymbolId, FunctionSig)],
        args: &[Type],
        span: Span,
        allow_defaults: bool,
    ) -> Option<(SymbolId, FunctionSig)> {
        let mut best: Option<(SymbolId, FunctionSig, usize, usize)> = None;
        for (id, sig) in candidates {
            let variadic = sig.params.last().map(|param| param.rest).unwrap_or(false);
            let fixed_count = if variadic {
                sig.params.len().saturating_sub(1)
            } else {
                sig.params.len()
            };
            let required = sig
                .params
                .iter()
                .filter(|param| !param.has_default && !param.rest)
                .count();
            if args.len() < required {
                continue;
            }
            if !allow_defaults && !variadic && args.len() != fixed_count {
                continue;
            }
            if !variadic && args.len() > fixed_count {
                continue;
            }
            let mut mismatches = 0usize;
            let mut exact = 0usize;
            let mut ok = true;
            let mut type_args = HashMap::new();
            for (index, arg) in args.iter().enumerate() {
                let param = if variadic && index >= fixed_count {
                    &sig.params[sig.params.len() - 1]
                } else {
                    &sig.params[index]
                };
                let param_ty = self.substitute(&param.ty, &type_args);
                if self.is_assignable(arg, &param_ty) {
                    exact += usize::from(arg == &param_ty);
                } else if let Some(inferred) = self.infer_type_arg(&param.ty, arg, &type_args) {
                    type_args.extend(inferred);
                } else {
                    mismatches += 1;
                    if mismatches > 1 {
                        ok = false;
                        break;
                    }
                }
            }
            if !ok {
                continue;
            }
            // 约束驱动反向推导：where T: Container<U> 这类接口实参含类型参数的
            // 约束，从实参类实现的同模板接口具体实例反推 U（检查期即推导，
            // 保证泛型返回类型可被替换）。
            self.derive_bound_params_checker(&sig.bounds, &mut type_args);
            let ret = self.substitute(&sig.ret, &type_args);
            let sig = FunctionSig { ret, ..sig.clone() };
            match &best {
                Some((_, _, best_mismatch, best_exact)) => {
                    if mismatches < *best_mismatch
                        || (mismatches == *best_mismatch && exact > *best_exact)
                    {
                        best = Some((*id, sig, mismatches, exact));
                    }
                }
                None => best = Some((*id, sig, mismatches, exact)),
            }
        }
        match best {
            Some((id, sig, mismatches, _)) => {
                if mismatches > 0 {
                    self.error("没有找到匹配的重载函数", span);
                    return None;
                }
                Some((id, sig))
            }
            None => {
                self.error("没有找到匹配的重载函数", span);
                None
            }
        }
    }

    fn match_call_args(
        &mut self,
        sig: &FunctionSig,
        args: &[Type],
        span: Span,
        allow_defaults: bool,
    ) {
        let _ = self.pick_overload(&[(SymbolId(0), sig.clone())], args, span, allow_defaults);
    }

    fn substitute(&self, ty: &Type, type_args: &HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => type_args.get(name).cloned().unwrap_or_else(|| ty.clone()),
            Type::Array(inner) => Type::Array(Box::new(self.substitute(inner, type_args))),
            Type::Nullable(inner) => Type::Nullable(Box::new(self.substitute(inner, type_args))),
            Type::Ptr(inner) => Type::Ptr(Box::new(self.substitute(inner, type_args))),
            Type::Function { params, ret } => Type::Function {
                params: params
                    .iter()
                    .map(|param| self.substitute(param, type_args))
                    .collect(),
                ret: Box::new(self.substitute(ret, type_args)),
            },
            other => other.clone(),
        }
    }

    fn infer_type_arg(
        &self,
        param_ty: &Type,
        arg_ty: &Type,
        known: &HashMap<String, Type>,
    ) -> Option<HashMap<String, Type>> {
        match (param_ty, arg_ty) {
            (Type::TypeParam(name), actual) => {
                if known.contains_key(name) {
                    None
                } else {
                    let mut map = HashMap::new();
                    map.insert(name.clone(), actual.clone());
                    Some(map)
                }
            }
            (Type::Array(param_inner), Type::Array(arg_inner)) => {
                self.infer_type_arg(param_inner, arg_inner, known)
            }
            (Type::Nullable(param_inner), Type::Nullable(arg_inner)) => {
                self.infer_type_arg(param_inner, arg_inner, known)
            }
            _ => None,
        }
    }

    /// 约束驱动反向推导（检查期）：`where T: Container<U>` 的接口实参若是类型
    /// 参数，从实参类实现的同模板接口具体实例反推 U 填入 type_args。这样泛型
    /// 函数的返回类型可在 pick_overload 的返回类型替换阶段被代入。
    fn derive_bound_params_checker(
        &self,
        bounds: &HashMap<String, Vec<Type>>,
        type_args: &mut HashMap<String, Type>,
    ) {
        for (param_name, bound_tys) in bounds {
            let Some(actual) = type_args.get(param_name).cloned() else {
                continue;
            };
            let Type::Class(class_id) = actual else {
                continue;
            };
            for bound_ty in bound_tys {
                let Type::Interface(bound_iface_id) = bound_ty else {
                    continue;
                };
                let Some((bound_template_id, bound_args)) = self
                    .types
                    .generic_interface_instances
                    .iter()
                    .find(|entry| *entry.1 == *bound_iface_id)
                    .map(|((t, args), _)| (*t, args.clone()))
                else {
                    continue;
                };
                for cid in self.types.class_base_chain(class_id) {
                    let Some(ifaces) = self.types.class_interfaces.get(&cid) else {
                        continue;
                    };
                    for &inst_id in ifaces {
                        let Some((t2, concrete_args)) = self
                            .types
                            .generic_interface_instances
                            .iter()
                            .find(|entry| *entry.1 == inst_id)
                            .map(|((t, args), _)| (*t, args.clone()))
                        else {
                            continue;
                        };
                        if t2 != bound_template_id {
                            continue;
                        }
                        for (bound, concrete) in bound_args.iter().zip(concrete_args.iter()) {
                            if let Type::TypeParam(name) = bound {
                                if !matches!(
                                    concrete,
                                    Type::TypeParam(_) | Type::Unknown | Type::Error
                                ) {
                                    type_args
                                        .entry(name.clone())
                                        .or_insert_with(|| concrete.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn binary_op_name(op: &BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Sub => "-",
        BinaryOp::Mul => "*",
        BinaryOp::Div => "/",
        BinaryOp::Rem => "%",
        BinaryOp::Pow => "**",
        BinaryOp::Eq => "==",
        BinaryOp::Ne => "!=",
        BinaryOp::Lt => "<",
        BinaryOp::Le => "<=",
        BinaryOp::Gt => ">",
        BinaryOp::Ge => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
        BinaryOp::Coalesce => "??",
        BinaryOp::BitAnd => "&",
        BinaryOp::BitOr => "|",
        BinaryOp::BitXor => "^",
        BinaryOp::Shl => "<<",
        BinaryOp::Shr => ">>",
    }
}

// ---------------------------------------------------------------------------
// MIR 降级
// ---------------------------------------------------------------------------

struct MirLowerer<'m, 's> {
    module: &'m Module,
    /// 全部已加载模块（跨模块泛型实例化时取模板函数体）。
    all_modules: &'m [Module],
    /// 各模块的检查结果表（跨模块泛型实例化时模板 body 的 span 查模板模块）。
    all_results: Vec<CheckResult>,
    /// 模块 id → 文件 stem（stable 符号名用，避免加载顺序/源码位置编号）。
    module_stems: HashMap<u32, String>,
    symbols: &'s [Symbol],
    types: &'s mut TypeTable,
    registry: &'s HashMap<String, Type>,
    diagnostics: &'s mut Diagnostics,
    state: &'s mut ModuleState,
    global_index_by_symbol: HashMap<u32, usize>,
    /// SymbolId → 声明名（跨模块全局变量导入时生成链接符号名，支持别名）。
    decl_names: HashMap<u32, String>,
    hidden_functions: Vec<MirFunction>,
    /// 泛型实例缓存：key → (实例函数名, 实例签名)。
    generic_instances: HashMap<String, (String, FunctionSig)>,
    generic_counter: u32,
    /// static 字段全局名 → 当前模块 MirGlobal 索引。
    static_field_globals: HashMap<String, u32>,
}

struct FnLower<'a, 'm, 's> {
    lowerer: &'a mut MirLowerer<'m, 's>,
    /// 检查结果表所属模块（跨模块泛型实例化时指向模板模块）。
    result_index: usize,
    name: String,
    /// 源码用户函数名（导出头文件用；隐藏函数为空）。
    user_name: String,
    /// 顶层 `export` 标记（仅顶层函数有意义）。
    exported: bool,
    params: Vec<MirParam>,
    ret: Type,
    locals: Vec<MirLocal>,
    name_scopes: Vec<HashMap<String, usize>>,
    global_by_symbol: HashMap<u32, usize>,
    this_class: Option<u32>,
    /// 闭包隐藏函数内：符号 ID → 环境槽序号。
    captures: HashMap<u32, usize>,
    /// 泛型实例化的类型实参（非泛型函数为空）。
    type_args: HashMap<String, Type>,
}

fn substitute_type(ty: &Type, type_args: &HashMap<String, Type>) -> Type {
    match ty {
        Type::TypeParam(name) => type_args.get(name).cloned().unwrap_or_else(|| ty.clone()),
        Type::Array(inner) => Type::Array(Box::new(substitute_type(inner, type_args))),
        Type::Nullable(inner) => Type::Nullable(Box::new(substitute_type(inner, type_args))),
        Type::Ptr(inner) => Type::Ptr(Box::new(substitute_type(inner, type_args))),
        Type::Function { params, ret } => Type::Function {
            params: params
                .iter()
                .map(|param| substitute_type(param, type_args))
                .collect(),
            ret: Box::new(substitute_type(ret, type_args)),
        },
        other => other.clone(),
    }
}

fn infer_type_arg(param: &Type, arg: &Type, known: &mut HashMap<String, Type>) {
    match (param, arg) {
        (Type::TypeParam(name), actual) => {
            known.entry(name.clone()).or_insert_with(|| actual.clone());
        }
        (Type::Array(param_inner), Type::Array(arg_inner)) => {
            infer_type_arg(param_inner, arg_inner, known);
        }
        (Type::Nullable(param_inner), Type::Nullable(arg_inner)) => {
            infer_type_arg(param_inner, arg_inner, known);
        }
        _ => {}
    }
}

fn optional_fallback(ty: &Type) -> MirExpr {
    match ty.without_nullable() {
        Type::F32 | Type::F64 => MirExpr::Float(0.0),
        ty if ty.is_reference() => MirExpr::Null,
        _ => MirExpr::Int(0),
    }
}

/// 字符串内建方法表：方法名 → (运行时符号, 参数类型, 返回类型)。接收者隐式为 string。
fn string_method(method: &str) -> Option<(String, Vec<Type>, Type)> {
    Some(match method {
        "to_upper" | "to_lower" | "trim" => (method.to_owned(), vec![], Type::Str),
        "trim_left" | "trim_right" => (method.to_owned(), vec![], Type::Str),
        "ends_with" => ("ends_with".to_owned(), vec![Type::Str], Type::Bool),
        "lines" | "split_whitespace" | "chars" => {
            (method.to_owned(), vec![], Type::Array(Box::new(Type::Str)))
        }
        "count" | "last_index_of" => (method.to_owned(), vec![Type::Str], Type::Int),
        "is_ascii" => ("is_ascii".to_owned(), vec![], Type::Bool),
        "escape" | "unescape" => (method.to_owned(), vec![], Type::Str),
        "is_empty" | "utf8_is_valid" => (method.to_owned(), vec![], Type::Bool),
        "truncate" | "ellipsis" => (method.to_owned(), vec![Type::Int], Type::Str),
        "contains" => ("contains".to_owned(), vec![Type::Str], Type::Bool),
        "index_of" => ("index_of".to_owned(), vec![Type::Str], Type::Int),
        "starts_with" => ("starts_with".to_owned(), vec![Type::Str], Type::Bool),
        "substring" => (
            "substring".to_owned(),
            vec![Type::Int, Type::Int],
            Type::Str,
        ),
        "replace" => ("replace".to_owned(), vec![Type::Str, Type::Str], Type::Str),
        "split" => (
            "split".to_owned(),
            vec![Type::Str],
            Type::Array(Box::new(Type::Str)),
        ),
        "parse_int" => ("parse_int".to_owned(), vec![], Type::Int),
        "parse_float" => ("parse_float".to_owned(), vec![], Type::F64),
        "parse_int_or" => ("parse_int_or".to_owned(), vec![Type::Int], Type::Int),
        "parse_float_or" => ("parse_float_or".to_owned(), vec![Type::F64], Type::F64),
        "is_number" => ("is_number".to_owned(), vec![], Type::Bool),
        "parse_bool" => ("parse_bool".to_owned(), vec![], Type::Bool),
        "repeat" => ("repeat".to_owned(), vec![Type::Int], Type::Str),
        "reverse" => ("reverse".to_owned(), vec![], Type::Str),
        "split_chars" => (
            "split_chars".to_owned(),
            vec![Type::Str],
            Type::Array(Box::new(Type::Str)),
        ),
        "index_of_char" => ("index_of_char".to_owned(), vec![Type::Str], Type::Int),
        "substring_chars" => (
            "utf8_substring".to_owned(),
            vec![Type::Int, Type::Int],
            Type::Str,
        ),
        "pad_left" => ("pad_left".to_owned(), vec![Type::Int, Type::Str], Type::Str),
        "pad_right" => (
            "pad_right".to_owned(),
            vec![Type::Int, Type::Str],
            Type::Str,
        ),
        "remove_prefix" | "remove_suffix" => (method.to_owned(), vec![Type::Str], Type::Str),
        "is_upper" | "is_lower" | "is_digit" => (method.to_owned(), vec![], Type::Bool),
        "capitalize" => ("capitalize".to_owned(), vec![], Type::Str),
        "is_blank" => ("is_blank".to_owned(), vec![], Type::Bool),
        "strip_whitespace" => ("strip_whitespace".to_owned(), vec![], Type::Str),
        "substring_between" | "substring_between_last" => {
            (method.to_owned(), vec![Type::Str, Type::Str], Type::Str)
        }
        "before" | "after" | "before_last" | "after_last" => {
            (method.to_owned(), vec![Type::Str], Type::Str)
        }
        "char_code" => ("char_code".to_owned(), vec![Type::Int], Type::Int),
        // 中文别名（火山风格，转发到英文实现）。
        "是否空白" => ("is_blank".to_owned(), vec![], Type::Bool),
        "删全部空白" => ("strip_whitespace".to_owned(), vec![], Type::Str),
        "取文本中间" | "取文本中间反向" => (
            if method == "取文本中间" {
                "substring_between"
            } else {
                "substring_between_last"
            }
            .to_owned(),
            vec![Type::Str, Type::Str],
            Type::Str,
        ),
        "取文本左边" => ("before".to_owned(), vec![Type::Str], Type::Str),
        "取文本右边" => ("after".to_owned(), vec![Type::Str], Type::Str),
        "取文本左边反向" => ("before_last".to_owned(), vec![Type::Str], Type::Str),
        "取文本右边反向" => ("after_last".to_owned(), vec![Type::Str], Type::Str),
        "取字符代码" => ("char_code".to_owned(), vec![Type::Int], Type::Int),
        "转大写" => ("to_upper".to_owned(), vec![], Type::Str),
        "转小写" => ("to_lower".to_owned(), vec![], Type::Str),
        "首字母大写" => ("capitalize".to_owned(), vec![], Type::Str),
        "反转文本" => ("reverse".to_owned(), vec![], Type::Str),
        "替换文本" => ("replace".to_owned(), vec![Type::Str, Type::Str], Type::Str),
        "删除前缀" => ("remove_prefix".to_owned(), vec![Type::Str], Type::Str),
        "删除后缀" => ("remove_suffix".to_owned(), vec![Type::Str], Type::Str),
        "是否包含" => ("contains".to_owned(), vec![Type::Str], Type::Bool),
        "开头为" => ("starts_with".to_owned(), vec![Type::Str], Type::Bool),
        "结尾为" => ("ends_with".to_owned(), vec![Type::Str], Type::Bool),
        _ => return None,
    })
}

/// 可变参数运行时类型标签（与 runtime.c 中 SW_TAG_* 保持一致）。
fn vararg_tag(ty: &Type) -> i64 {
    match ty.without_nullable() {
        Type::F32 | Type::F64 => 1,
        Type::Str => 2,
        Type::Bool => 3,
        Type::Char => 4,
        _ => 0,
    }
}

impl<'m, 's> MirLowerer<'m, 's> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        let file = self.state.path.clone();
        self.diagnostics.error_at(message, Some(span), Some(file));
    }

    fn symbol(&self, id: SymbolId) -> &Symbol {
        &self.symbols[id.0 as usize]
    }

    fn lower_module(&mut self) -> MirModule {
        let mut module_mir = MirModule {
            module_id: self.state.id.0,
            functions: Vec::new(),
            globals: Vec::new(),
            strings: Vec::new(),
        };

        let items = self.module.items.clone();

        // 顶层全局变量
        let mut module_global_ids = std::collections::HashSet::new();
        for item in &items {
            if let ItemKind::Variable(variable) = &item.kind {
                let symbol_id = self
                    .state
                    .names
                    .get(&variable.name.name)
                    .and_then(|ids| ids.first().copied());
                let Some(symbol_id) = symbol_id else {
                    continue;
                };
                let ty = match &self.symbol(symbol_id).kind {
                    SymbolKind::Global { ty, .. } => ty.clone(),
                    _ => Type::Error,
                };
                let mutable = matches!(
                    self.symbol(symbol_id).kind,
                    SymbolKind::Global { mutable: true, .. }
                );
                let index = module_mir.globals.len() as u32;
                let init = variable.init.as_ref().and_then(|init| self.const_mir(init));
                module_mir.globals.push(MirGlobal {
                    name: format!("sw_global_{}_{}", self.state.id.0, variable.name.name),
                    ty,
                    mutable,
                    init,
                    module: self.state.id.0,
                });
                module_global_ids.insert(symbol_id.0);
                self.global_index_by_symbol
                    .insert(symbol_id.0, index as usize);
            }
        }
        // 跨模块全局变量：本模块 names 引用的 Global 符号若不属于本模块声明，
        // 生成同名的导入条目（codegen 按 module 字段声明为 Import 外部数据）。
        let names = self.state.names.clone();
        for (_, ids) in &names {
            for id in ids {
                if module_global_ids.contains(&id.0)
                    || self.global_index_by_symbol.contains_key(&id.0)
                {
                    continue;
                }
                let symbol = self.symbol(*id);
                let SymbolKind::Global { ty, mutable } = &symbol.kind else {
                    continue;
                };
                let Some(decl_name) = self.decl_names.get(&id.0) else {
                    continue;
                };
                let index = module_mir.globals.len() as u32;
                module_mir.globals.push(MirGlobal {
                    name: format!("sw_global_{}_{}", symbol.module.0, decl_name),
                    ty: ty.clone(),
                    mutable: *mutable,
                    init: None,
                    module: symbol.module.0,
                });
                self.global_index_by_symbol.insert(id.0, index as usize);
            }
        }
        let global_by_symbol = self.global_index_by_symbol.clone();

        // 顶层函数
        for item in &items {
            if let ItemKind::Function(function) = &item.kind {
                if function.body.is_some() || function.extern_c {
                    let sig = match self.function_symbol(function) {
                        Some(SymbolKind::Function(sig)) => sig,
                        _ => continue,
                    };
                    // 泛型函数模板不直接生成 MIR，按调用点实例化。
                    if !sig.generics.is_empty() {
                        continue;
                    }
                    let module_stem = self
                        .module_stems
                        .get(&self.state.id.0)
                        .cloned()
                        .unwrap_or_else(|| "mod".to_owned());
                    let name = stable_function_name(&sig, &module_stem);
                    let result_index = self.state.id.0 as usize;
                    let mut lower = FnLower {
                        lowerer: self,
                        result_index,
                        name: name.clone(),
                        user_name: function.name.name.clone(),
                        exported: item.exported,
                        params: Vec::new(),
                        ret: sig.ret.clone(),
                        locals: Vec::new(),
                        name_scopes: Vec::new(),
                        global_by_symbol: global_by_symbol.clone(),
                        this_class: None,
                        captures: HashMap::new(),
                        type_args: HashMap::new(),
                    };
                    for param in &sig.params {
                        lower.params.push(MirParam {
                            name: param.name.clone(),
                            ty: param.ty.clone(),
                        });
                    }
                    let mir_function =
                        lower.lower_function(function.body.as_ref(), function.extern_c);
                    module_mir.functions.push(mir_function);
                }
            }
        }

        // 类方法
        for item in &items {
            if let ItemKind::Class(class) = &item.kind {
                let class_id = match self
                    .state
                    .names
                    .get(&class.name.name)
                    .and_then(|ids| ids.first().copied())
                    .map(|id| self.symbol(id))
                    .map(|symbol| match &symbol.kind {
                        SymbolKind::Type(SymbolType::Class(id)) => Some(*id),
                        _ => None,
                    })
                    .flatten()
                {
                    Some(id) => id,
                    None => continue,
                };
                // 泛型类模板的方法不直接生成，按实例化后的 id 在下方生成。
                if !self.types.classes[class_id as usize].generics.is_empty() {
                    continue;
                }
                // static 字段 → 模块级全局变量。
                let class_static_fields =
                    self.types.classes[class_id as usize].static_fields.clone();
                for (index, field) in class_static_fields.iter().enumerate() {
                    let name = format!("sw_sfield_{class_id}_{index}");
                    let gindex = module_mir.globals.len() as u32;
                    module_mir.globals.push(MirGlobal {
                        name: name.clone(),
                        ty: field.ty.clone(),
                        mutable: field.mutable,
                        init: None,
                        module: self.state.id.0,
                    });
                    self.static_field_globals.insert(name, gindex);
                }
                let mut method_index = 0usize;
                for member in &class.members {
                    let (body, name, sig) = match member {
                        ClassMember::Method(function) => {
                            if function.static_ {
                                let static_index = self.types.classes[class_id as usize]
                                    .static_methods
                                    .iter()
                                    .position(|m| m.name == function.name.name)
                                    .unwrap_or(0);
                                let name = format!(
                                    "sw_smethod_{class_id}_{static_index}_{}",
                                    function.name.name
                                );
                                let sig = self.types.classes[class_id as usize]
                                    .static_methods
                                    .get(static_index)
                                    .map(|m| m.sig.clone())
                                    .unwrap_or_else(placeholder_sig);
                                let result_index = self.state.id.0 as usize;
                                let mut lower = FnLower {
                                    lowerer: self,
                                    result_index,
                                    name: name.clone(),
                                    user_name: function.name.name.clone(),
                                    exported: false,
                                    params: Vec::new(),
                                    ret: sig.ret.clone(),
                                    locals: Vec::new(),
                                    name_scopes: Vec::new(),
                                    global_by_symbol: global_by_symbol.clone(),
                                    this_class: None,
                                    captures: HashMap::new(),
                                    type_args: HashMap::new(),
                                };
                                for param in &sig.params {
                                    lower.params.push(MirParam {
                                        name: param.name.clone(),
                                        ty: param.ty.clone(),
                                    });
                                }
                                if let Some(body) = &function.body {
                                    let mir_function = lower.lower_function(Some(body), false);
                                    module_mir.functions.push(mir_function);
                                }
                                continue;
                            }
                            let name =
                                format!("sw_m_{class_id}_{method_index}_{}", function.name.name);
                            let sig =
                                self.class_method_sig(class_id, method_index, &function.name.name);
                            method_index += 1;
                            (function.body.as_ref(), name, sig)
                        }
                        ClassMember::Constructor(constructor) => {
                            let name = format!("sw_ctor_{class_id}");
                            let sig = self.class_constructor_sig(class_id);
                            method_index += 1;
                            (Some(&constructor.body), name, sig)
                        }
                        _ => continue,
                    };
                    let Some(body) = body else { continue };
                    let result_index = self.state.id.0 as usize;
                    let mut lower = FnLower {
                        lowerer: self,
                        result_index,
                        name: name.clone(),
                        user_name: sig.name.clone(),
                        exported: false,
                        params: vec![MirParam {
                            name: "self".to_owned(),
                            ty: Type::Class(class_id),
                        }],
                        ret: sig.ret.clone(),
                        locals: Vec::new(),
                        name_scopes: Vec::new(),
                        global_by_symbol: global_by_symbol.clone(),
                        this_class: Some(class_id),
                        captures: HashMap::new(),
                        type_args: HashMap::new(),
                    };
                    for param in &sig.params {
                        lower.params.push(MirParam {
                            name: param.name.clone(),
                            ty: param.ty.clone(),
                        });
                    }
                    let mir_function = lower.lower_function(Some(body), false);
                    module_mir.functions.push(mir_function);
                }
                // 无显式构造函数的类：生成空 sw_ctor（`new` 需要）。
                let has_constructor = class
                    .members
                    .iter()
                    .any(|member| matches!(member, ClassMember::Constructor(_)));
                if !has_constructor {
                    module_mir.functions.push(MirFunction {
                        name: format!("sw_ctor_{class_id}"),
                        user_name: String::new(),
                        exported: false,
                        params: vec![MirParam {
                            name: "self".to_owned(),
                            ty: Type::Class(class_id),
                        }],
                        ret: Type::Void,
                        locals: Vec::new(),
                        body: Vec::new(),
                        extern_c: false,
                    });
                }
            }
        }

        // 泛型类实例：为每个实例化生成方法体（this_class = 实例 id，类型实参替换）。
        let instances: Vec<(u32, u32, Vec<Type>)> = self
            .types
            .generic_class_instances
            .iter()
            .map(|((orig, args), id)| (*orig, *id, args.clone()))
            .collect();
        for (orig_id, instance_id, args) in instances {
            let orig_info = self.types.classes[orig_id as usize].clone();
            let Some(class) = items.iter().find_map(|item| match &item.kind {
                ItemKind::Class(class) if class.name.name == orig_info.name => Some(class),
                _ => None,
            }) else {
                continue;
            };
            let type_args: HashMap<String, Type> = orig_info
                .generics
                .iter()
                .cloned()
                .zip(args.iter().cloned())
                .collect();
            let mut method_index = 0usize;
            for member in &class.members {
                let (body, name, sig) = match member {
                    ClassMember::Method(function) => {
                        let name =
                            format!("sw_m_{instance_id}_{method_index}_{}", function.name.name);
                        let sig =
                            self.class_method_sig(instance_id, method_index, &function.name.name);
                        method_index += 1;
                        (function.body.as_ref(), name, sig)
                    }
                    ClassMember::Constructor(constructor) => {
                        let name = format!("sw_ctor_{instance_id}");
                        let sig = self.class_constructor_sig(instance_id);
                        method_index += 1;
                        (Some(&constructor.body), name, sig)
                    }
                    _ => continue,
                };
                let Some(body) = body else { continue };
                let result_index = self.state.id.0 as usize;
                let mut lower = FnLower {
                    lowerer: self,
                    result_index,
                    name: name.clone(),
                    user_name: sig.name.clone(),
                    exported: false,
                    params: vec![MirParam {
                        name: "self".to_owned(),
                        ty: Type::Class(instance_id),
                    }],
                    ret: sig.ret.clone(),
                    locals: Vec::new(),
                    name_scopes: Vec::new(),
                    global_by_symbol: global_by_symbol.clone(),
                    this_class: Some(instance_id),
                    captures: HashMap::new(),
                    type_args: type_args.clone(),
                };
                for param in &sig.params {
                    lower.params.push(MirParam {
                        name: param.name.clone(),
                        ty: param.ty.clone(),
                    });
                }
                let mir_function = lower.lower_function(Some(body), false);
                module_mir.functions.push(mir_function);
            }
        }

        for function in std::mem::take(&mut self.hidden_functions) {
            module_mir.functions.push(function);
        }
        // @test 测试函数：无 main 时合成测试 runner（返回失败数）。
        let test_fns: Vec<(String, FunctionSig)> = items
            .iter()
            .filter_map(|item| {
                if !item.attributes.iter().any(|attr| attr.name.name == "test") {
                    return None;
                }
                let ItemKind::Function(function) = &item.kind else {
                    return None;
                };
                let Some(SymbolKind::Function(sig)) = self.function_symbol(function) else {
                    return None;
                };
                if !sig.params.is_empty() {
                    self.error("test 函数不能有参数", function.span);
                }
                if sig.ret != Type::Int && sig.ret != Type::Void && sig.ret != Type::Unknown {
                    self.error("test 函数返回类型必须为 int 或 void", function.span);
                }
                let module_stem = self
                    .module_stems
                    .get(&self.state.id.0)
                    .cloned()
                    .unwrap_or_else(|| "mod".to_owned());
                Some((stable_function_name(&sig, &module_stem), sig))
            })
            .collect();
        if !test_fns.is_empty()
            && !items.iter().any(|item| {
                matches!(
                    &item.kind,
                    ItemKind::Function(function) if function.name.name == "main"
                )
            })
        {
            let main = self.build_test_main(&test_fns);
            if let Some(main) = main {
                module_mir.functions.push(main);
            }
        }
        module_mir.strings = self.state.mir_strings.clone();
        module_mir
    }

    fn build_test_main(&mut self, test_fns: &[(String, FunctionSig)]) -> Option<MirFunction> {
        let mut locals = vec![
            MirLocal {
                name: "fail".to_owned(),
                ty: Type::Int,
                mutable: true,
            },
            MirLocal {
                name: "$ret".to_owned(),
                ty: Type::Int,
                mutable: true,
            },
            MirLocal {
                name: "$frame".to_owned(),
                ty: Type::Ptr(Box::new(Type::I8)),
                mutable: true,
            },
            MirLocal {
                name: "$exc".to_owned(),
                ty: Type::Ptr(Box::new(Type::I8)),
                mutable: false,
            },
        ];
        let mut body = vec![MirStmt::new(MirStmtKind::VarDecl {
            local: 0,
            init: Some(MirExpr::Int(0)),
        })];
        let ok_prefix = self.intern_string("[ok] ");
        let fail_prefix = self.intern_string("[FAIL] ");
        let println_sig = || FunctionSig {
            module: ModuleId(0),
            name: "sw_test_println".to_owned(),
            generics: Vec::new(),
            bounds: HashMap::new(),
            params: vec![ParamSig {
                name: "text".to_owned(),
                ty: Type::Str,
                has_default: false,
                rest: false,
            }],
            ret: Type::Void,
            extern_c: true,
            span: Span::empty(0),
        };
        let setjmp_sig = || FunctionSig {
            module: ModuleId(0),
            name: "sw_setjmp".to_owned(),
            generics: Vec::new(),
            bounds: HashMap::new(),
            params: vec![ParamSig {
                name: "buf".to_owned(),
                ty: Type::Ptr(Box::new(Type::I8)),
                has_default: false,
                rest: false,
            }],
            ret: Type::Int,
            extern_c: true,
            span: Span::empty(0),
        };
        for (name, sig) in test_fns {
            let ret_void = sig.ret == Type::Void || sig.ret == Type::Unknown;
            let call = MirExpr::Call {
                callee: MirCallee::Function {
                    module: self.state.id.0,
                    name: name.clone(),
                    sig: sig.clone(),
                },
                args: vec![],
            };
            let name_str = self.intern_string(&sig.name);
            let ok_print = MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                callee: MirCallee::Extern {
                    name: "sw_test_println".to_owned(),
                    sig: println_sig(),
                },
                args: vec![MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "string_concat".to_owned(),
                    },
                    args: vec![MirExpr::Str(ok_prefix), MirExpr::Str(name_str)],
                }],
            }));
            let fail_inc = MirStmt::new(MirStmtKind::Assign {
                target: MirTarget::Local(0),
                value: MirExpr::Binary {
                    op: MirBinary::Add,
                    left: Box::new(MirExpr::Local(0)),
                    right: Box::new(MirExpr::Int(1)),
                },
            });
            let fail_print = MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                callee: MirCallee::Extern {
                    name: "sw_test_println".to_owned(),
                    sig: println_sig(),
                },
                args: vec![MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "string_concat".to_owned(),
                    },
                    args: vec![MirExpr::Str(fail_prefix), MirExpr::Str(name_str)],
                }],
            }));
            let cond = if ret_void {
                MirExpr::Bool(false)
            } else {
                MirExpr::Binary {
                    op: MirBinary::Ne,
                    left: Box::new(MirExpr::Local(1)),
                    right: Box::new(MirExpr::Int(0)),
                }
            };
            // 正常路径：调用测试 + 按返回码判断 + 弹出异常框架。
            let mut else_stmts = Vec::new();
            if ret_void {
                else_stmts.push(MirStmt::new(MirStmtKind::Expr(call)));
            } else {
                else_stmts.push(MirStmt::new(MirStmtKind::VarDecl {
                    local: 1,
                    init: Some(call),
                }));
            }
            else_stmts.push(MirStmt::new(MirStmtKind::If {
                cond,
                then: vec![fail_inc.clone(), fail_print.clone()],
                else_: vec![ok_print],
            }));
            let frame = MirExpr::Local(2);
            else_stmts.push(MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                callee: MirCallee::Intrinsic {
                    name: "sw_try_leave".to_owned(),
                },
                args: vec![frame.clone()],
            })));
            // 异常路径：断言抛出的异常记为失败。
            let then_stmts = vec![
                MirStmt::new(MirStmtKind::VarDecl {
                    local: 3,
                    init: Some(MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "sw_try_value".to_owned(),
                        },
                        args: vec![frame.clone()],
                    }),
                }),
                MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_try_leave".to_owned(),
                    },
                    args: vec![frame.clone()],
                })),
                fail_inc,
                fail_print,
            ];
            // frame = sw_try_begin()，再用 setjmp 分派正常/异常路径。
            body.push(MirStmt::new(MirStmtKind::VarDecl {
                local: 2,
                init: Some(MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_try_begin".to_owned(),
                    },
                    args: vec![],
                }),
            }));
            body.push(MirStmt::new(MirStmtKind::If {
                cond: MirExpr::Binary {
                    op: MirBinary::Ne,
                    left: Box::new(MirExpr::Call {
                        callee: MirCallee::Extern {
                            name: "sw_setjmp".to_owned(),
                            sig: setjmp_sig(),
                        },
                        args: vec![frame],
                    }),
                    right: Box::new(MirExpr::Int(0)),
                },
                then: then_stmts,
                else_: else_stmts,
            }));
        }
        body.push(MirStmt::new(MirStmtKind::Return(Some(MirExpr::Local(0)))));
        Some(MirFunction {
            name: "sw_user_main".to_owned(),
            user_name: String::new(),
            exported: false,
            params: Vec::new(),
            ret: Type::Int,
            locals,
            body,
            extern_c: false,
        })
    }

    fn function_symbol(&self, function: &FunctionDecl) -> Option<SymbolKind> {
        self.state
            .span_symbols
            .get(&function.span.start)
            .copied()
            .map(|id| self.symbol(id).kind.clone())
    }

    fn class_method_sig(&self, class_id: u32, method_index: usize, name: &str) -> FunctionSig {
        self.types
            .classes
            .get(class_id as usize)
            .and_then(|info| info.methods.get(method_index))
            .filter(|method| method.name == name)
            .map(|method| method.sig.clone())
            .unwrap_or_else(|| placeholder_sig())
    }

    fn class_constructor_sig(&self, class_id: u32) -> FunctionSig {
        self.types
            .classes
            .get(class_id as usize)
            .and_then(|info| {
                info.methods
                    .iter()
                    .find(|method| method.name == "constructor")
            })
            .map(|method| method.sig.clone())
            .unwrap_or_else(|| placeholder_sig())
    }

    fn const_mir(&mut self, expr: &Expr) -> Option<MirExpr> {
        match &expr.kind {
            ExprKind::Integer { text, suffix } => {
                let value = parse_int(text)?;
                if matches!(
                    suffix,
                    Some(sw_frontend::IntegerSuffix::U8)
                        | Some(sw_frontend::IntegerSuffix::U16)
                        | Some(sw_frontend::IntegerSuffix::U32)
                        | Some(sw_frontend::IntegerSuffix::U64)
                        | Some(sw_frontend::IntegerSuffix::Usize)
                ) {
                    Some(MirExpr::UInt(value as u64))
                } else {
                    Some(MirExpr::Int(value))
                }
            }
            ExprKind::Float { text, .. } => text.parse::<f64>().ok().map(MirExpr::Float),
            ExprKind::Str(value) => {
                let index = self.intern_string(value);
                Some(MirExpr::Str(index))
            }
            ExprKind::Bool(value) => Some(MirExpr::Bool(*value)),
            ExprKind::Char(value) => Some(MirExpr::Char(*value)),
            ExprKind::Null => Some(MirExpr::Null),
            ExprKind::Unary { op, expr } => {
                let inner = self.const_mir(expr)?;
                match op {
                    UnaryOp::Neg => Some(MirExpr::Unary {
                        op: MirUnary::Neg,
                        expr: Box::new(inner),
                    }),
                    UnaryOp::Pos => Some(MirExpr::Unary {
                        op: MirUnary::Pos,
                        expr: Box::new(inner),
                    }),
                    UnaryOp::Not => Some(MirExpr::Unary {
                        op: MirUnary::Not,
                        expr: Box::new(inner),
                    }),
                    UnaryOp::BitNot => Some(MirExpr::Unary {
                        op: MirUnary::BitNot,
                        expr: Box::new(inner),
                    }),
                    _ => None,
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.const_mir(left)?;
                let right = self.const_mir(right)?;
                Some(MirExpr::Binary {
                    op: mir_binary(op),
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
            _ => None,
        }
    }

    fn intern_string(&mut self, value: &str) -> usize {
        let strings = &mut self.state.mir_strings;
        if let Some(index) = strings.iter().position(|existing| existing == value) {
            return index;
        }
        strings.push(value.to_owned());
        strings.len() - 1
    }
}

impl<'a, 'm, 's> FnLower<'a, 'm, 's> {
    /// 当前降级函数所属模块的检查结果表（跨模块泛型实例化时指向模板模块）。
    fn result(&self) -> &CheckResult {
        &self.lowerer.all_results[self.result_index]
    }

    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.lowerer.error(message, span);
    }

    fn declare_local(&mut self, name: &str, ty: Type, mutable: bool) -> usize {
        let ty = substitute_type(&ty, &self.type_args);
        let index = self.locals.len();
        self.locals.push(MirLocal {
            name: name.to_owned(),
            ty,
            mutable,
        });
        index
    }

    fn lookup_local(&self, name: &str) -> Option<usize> {
        for scope in self.name_scopes.iter().rev() {
            if let Some(index) = scope.get(name) {
                return Some(*index);
            }
        }
        None
    }

    fn lower_function(&mut self, body: Option<&Block>, extern_c: bool) -> MirFunction {
        // 参数作为最前面的局部变量
        self.locals.clear();
        let mut scope = HashMap::new();
        let params = self.params.clone();
        for param in &params {
            let index = self.declare_local(&param.name, param.ty.clone(), false);
            scope.insert(param.name.clone(), index);
        }
        self.name_scopes = vec![scope];

        let mut statements = Vec::new();
        if let Some(body) = body {
            statements = self.lower_stmts(&body.statements);
        }
        let locals = std::mem::take(&mut self.locals);
        MirFunction {
            name: self.name.clone(),
            user_name: self.user_name.clone(),
            exported: self.exported,
            params: self.params.clone(),
            ret: self.ret.clone(),
            locals,
            body: statements,
            extern_c,
        }
    }

    fn lower_stmts(&mut self, statements: &[Stmt]) -> Vec<MirStmt> {
        let mut result = Vec::new();
        for statement in statements {
            self.lower_stmt(statement, &mut result);
        }
        result
    }

    fn local_type(&mut self, variable: &VariableDecl) -> Type {
        if let Some(annotation) = &variable.ty {
            let ty = self.lowerer.lower_type_for_mir(annotation);
            if ty != Type::Error {
                return substitute_type(&ty, &self.type_args);
            }
        }
        if let Some(init) = &variable.init {
            return substitute_type(&self.expr_type(init), &self.type_args);
        }
        Type::Error
    }

    fn lower_stmt(&mut self, statement: &Stmt, output: &mut Vec<MirStmt>) {
        match &statement.kind {
            StmtKind::Empty => {}
            StmtKind::Block(block) => {
                self.name_scopes.push(HashMap::new());
                let inner = self.lower_stmts(&block.statements);
                self.name_scopes.pop();
                output.extend(inner);
            }
            StmtKind::Variable(variable) => {
                if let Some(pattern) = &variable.pattern {
                    let Some(init_expr) = &variable.init else {
                        return;
                    };
                    let init_ty = self.expr_type(init_expr);
                    // init 只求值一次：存临时局部，绑定从临时取。
                    let tmp = self.declare_local("$destr", init_ty.clone(), false);
                    output.push(MirStmt::new(MirStmtKind::VarDecl {
                        local: tmp,
                        init: Some(self.lower_expr(init_expr)),
                    }));
                    let tmp_expr = MirExpr::Local(tmp);
                    match pattern {
                        VariablePattern::Array(bindings) => {
                            let elem = match init_ty.without_nullable() {
                                Type::Array(inner) => (**inner).clone(),
                                _ => Type::Error,
                            };
                            for (index, binding) in bindings.iter().enumerate() {
                                let local = self.declare_local(
                                    binding,
                                    elem.clone(),
                                    variable.kind == VarKind::Let,
                                );
                                self.name_scopes
                                    .last_mut()
                                    .expect("作用域存在")
                                    .insert(binding.clone(), local);
                                output.push(MirStmt::new(MirStmtKind::VarDecl {
                                    local,
                                    init: Some(MirExpr::Index {
                                        object: Box::new(tmp_expr.clone()),
                                        index: Box::new(MirExpr::Int(index as i64)),
                                        elem: Box::new(elem.clone()),
                                    }),
                                }));
                            }
                        }
                        VariablePattern::Object(bindings) => {
                            let struct_id = match init_ty.without_nullable() {
                                Type::Struct(id) => *id,
                                _ => u32::MAX,
                            };
                            for (field_name, binding) in bindings {
                                let index =
                                    self.lowerer.types.structs.get(struct_id as usize).and_then(
                                        |info| {
                                            info.fields.iter().position(|f| f.name == *field_name)
                                        },
                                    );
                                let Some(index) = index else {
                                    continue;
                                };
                                let ty = self
                                    .lowerer
                                    .types
                                    .structs
                                    .get(struct_id as usize)
                                    .map(|info| info.fields[index].ty.clone())
                                    .unwrap_or(Type::Error);
                                let local = self.declare_local(
                                    binding,
                                    ty.clone(),
                                    variable.kind == VarKind::Let,
                                );
                                self.name_scopes
                                    .last_mut()
                                    .expect("作用域存在")
                                    .insert(binding.clone(), local);
                                output.push(MirStmt::new(MirStmtKind::VarDecl {
                                    local,
                                    init: Some(MirExpr::Field {
                                        object: Box::new(tmp_expr.clone()),
                                        index,
                                    }),
                                }));
                            }
                        }
                    }
                    return;
                }
                let ty = self.local_type(variable);
                let local =
                    self.declare_local(&variable.name.name, ty, variable.kind == VarKind::Let);
                self.name_scopes
                    .last_mut()
                    .expect("作用域存在")
                    .insert(variable.name.name.clone(), local);
                let init = variable.init.as_ref().map(|expr| self.lower_expr(expr));
                output.push(MirStmt::new(MirStmtKind::VarDecl { local, init }));
            }
            StmtKind::If { cond, then, else_ } => {
                let cond = self.lower_expr(cond);
                let then = self.lower_stmts(&[then.as_ref().clone()]);
                let else_ = else_
                    .as_ref()
                    .map(|else_| self.lower_stmts(&[else_.as_ref().clone()]))
                    .unwrap_or_default();
                output.push(MirStmt::new(MirStmtKind::If { cond, then, else_ }));
            }
            StmtKind::While { cond, body } => {
                let cond = self.lower_expr(cond);
                let body = self.lower_stmts(&[body.as_ref().clone()]);
                output.push(MirStmt::new(MirStmtKind::While { cond, body }));
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    match init {
                        ForInit::Variable(variable) => {
                            let ty = self.local_type(variable);
                            let local = self.declare_local(
                                &variable.name.name,
                                ty,
                                variable.kind == VarKind::Let,
                            );
                            self.name_scopes
                                .last_mut()
                                .expect("作用域存在")
                                .insert(variable.name.name.clone(), local);
                            let init = variable.init.as_ref().map(|expr| self.lower_expr(expr));
                            output.push(MirStmt::new(MirStmtKind::VarDecl { local, init }));
                        }
                        ForInit::Expr(expr) => {
                            self.lower_expr_stmt(expr, output);
                        }
                    }
                }
                let cond = cond.as_ref().map(|expr| self.lower_expr(expr));
                let mut body_stmts = self.lower_stmts(&[body.as_ref().clone()]);
                if let Some(update) = update {
                    self.lower_expr_stmt(update, &mut body_stmts);
                }
                let cond = cond.unwrap_or(MirExpr::Bool(true));
                output.push(MirStmt::new(MirStmtKind::While {
                    cond,
                    body: body_stmts,
                }));
            }
            StmtKind::ForEach {
                name,
                iterable,
                body,
                ..
            } => {
                let array = self.lower_expr(iterable);
                let element_ty = self.expr_type(iterable);
                let is_string = element_ty == Type::Str;
                let element_ty = match element_ty {
                    Type::Array(inner) => *inner,
                    Type::Str => Type::Char,
                    _ => Type::Error,
                };
                let element_local = self.declare_local(&name.name, element_ty.clone(), true);
                self.name_scopes
                    .last_mut()
                    .expect("作用域存在")
                    .insert(name.name.clone(), element_local);
                let index_local = self.declare_local("$index", Type::Int, true);
                let index_expr = MirExpr::Local(index_local);
                let mut body_stmts = Vec::new();
                let element_value = if is_string {
                    MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "string_char_at".to_owned(),
                        },
                        args: vec![array.clone(), index_expr.clone()],
                    }
                } else {
                    MirExpr::Index {
                        object: Box::new(array.clone()),
                        index: Box::new(index_expr.clone()),
                        elem: Box::new(element_ty.clone()),
                    }
                };
                body_stmts.push(MirStmt::new(MirStmtKind::Assign {
                    target: MirTarget::Local(element_local),
                    value: element_value,
                }));
                body_stmts.extend(self.lower_stmts(&[body.as_ref().clone()]));
                body_stmts.push(MirStmt::new(MirStmtKind::Assign {
                    target: MirTarget::Local(index_local),
                    value: MirExpr::Binary {
                        op: MirBinary::Add,
                        left: Box::new(index_expr.clone()),
                        right: Box::new(MirExpr::Int(1)),
                    },
                }));
                output.push(MirStmt::new(MirStmtKind::VarDecl {
                    local: index_local,
                    init: Some(MirExpr::Int(0)),
                }));
                output.push(MirStmt::new(MirStmtKind::While {
                    cond: MirExpr::Binary {
                        op: MirBinary::Lt,
                        left: Box::new(index_expr),
                        right: Box::new(MirExpr::Len {
                            object: Box::new(array),
                            string: is_string,
                        }),
                    },
                    body: body_stmts,
                }));
            }
            StmtKind::Switch {
                value,
                cases,
                default,
            } => {
                let value = self.lower_expr(value);
                let mut chain = default
                    .as_ref()
                    .map(|body| self.lower_switch_case_body(body))
                    .unwrap_or_default();
                for case in cases.iter().rev() {
                    let case_value = self.lower_expr(&case.value);
                    let body = self.lower_switch_case_body(&case.body);
                    let cond = MirExpr::Binary {
                        op: MirBinary::Eq,
                        left: Box::new(value.clone()),
                        right: Box::new(case_value),
                    };
                    chain = vec![MirStmt::new(MirStmtKind::If {
                        cond,
                        then: body,
                        else_: chain,
                    })];
                }
                output.extend(chain);
            }
            StmtKind::Match { value, arms } => {
                let value_ty = self.expr_type(value);
                let Type::Enum(enum_id) = value_ty.without_nullable() else {
                    return;
                };
                let value_mir = self.lower_expr(value);
                let info = self.lowerer.types.enums[*enum_id as usize].clone();
                let mut chain: Vec<MirStmt> = Vec::new();
                if let Some(wildcard) = arms
                    .iter()
                    .find(|arm| matches!(arm.pattern, Pattern::Wildcard(_)))
                {
                    chain = self.lower_stmts(&wildcard.body.statements);
                }
                for arm in arms.iter().rev() {
                    let Pattern::Variant { name, bindings, .. } = &arm.pattern else {
                        continue;
                    };
                    let Some(index) = info
                        .members
                        .iter()
                        .position(|member| member.name == name.name)
                    else {
                        continue;
                    };
                    let variant = &info.members[index];
                    let mut body = Vec::new();
                    self.name_scopes.push(HashMap::new());
                    for (bind_index, binding) in bindings.iter().enumerate() {
                        let ty = variant
                            .fields
                            .get(bind_index)
                            .cloned()
                            .unwrap_or(Type::Error);
                        let local = self.declare_local(&binding.name, ty.clone(), false);
                        self.name_scopes
                            .last_mut()
                            .expect("作用域存在")
                            .insert(binding.name.clone(), local);
                        body.push(MirStmt::new(MirStmtKind::VarDecl {
                            local,
                            init: Some(MirExpr::EnumField {
                                object: Box::new(value_mir.clone()),
                                index: bind_index,
                                elem: ty.clone(),
                            }),
                        }));
                    }
                    body.extend(self.lower_stmts(&arm.body.statements));
                    self.name_scopes.pop();
                    let cond = MirExpr::Binary {
                        op: MirBinary::Eq,
                        left: Box::new(MirExpr::EnumTag {
                            object: Box::new(value_mir.clone()),
                        }),
                        right: Box::new(MirExpr::Int(variant.discriminant)),
                    };
                    chain = vec![MirStmt::new(MirStmtKind::If {
                        cond,
                        then: body,
                        else_: chain,
                    })];
                }
                output.extend(chain);
            }
            StmtKind::Try {
                body,
                catches,
                finally,
            } => {
                let frame_ty = Type::Ptr(Box::new(Type::I8));
                let frame_local = self.declare_local("$frame", frame_ty.clone(), true);
                let frame = MirExpr::Local(frame_local);
                output.push(MirStmt::new(MirStmtKind::VarDecl {
                    local: frame_local,
                    init: Some(MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "sw_try_begin".to_owned(),
                        },
                        args: vec![],
                    }),
                }));

                // 异常路径：取值 → 弹出框架 → 类型分派 → finally → 未匹配则重抛
                let mut then_stmts = Vec::new();
                let e_local = self.declare_local("$exc", frame_ty.clone(), false);
                let e = MirExpr::Local(e_local);
                then_stmts.push(MirStmt::new(MirStmtKind::VarDecl {
                    local: e_local,
                    init: Some(MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "sw_try_value".to_owned(),
                        },
                        args: vec![frame.clone()],
                    }),
                }));
                then_stmts.push(MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_try_leave".to_owned(),
                    },
                    args: vec![frame.clone()],
                })));

                let matched_local = self.declare_local("$matched", Type::Int, false);
                then_stmts.push(MirStmt::new(MirStmtKind::VarDecl {
                    local: matched_local,
                    init: Some(MirExpr::Int(0)),
                }));
                let exception_type = || MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_exception_type".to_owned(),
                    },
                    args: vec![e.clone()],
                };
                for catch in catches {
                    let cond = match &catch.ty {
                        Some(ty) => {
                            let ty = self.lowerer.lower_type_for_mir(ty);
                            match ty {
                                Type::Str => MirExpr::Binary {
                                    op: MirBinary::Eq,
                                    left: Box::new(exception_type()),
                                    right: Box::new(MirExpr::Int(0)),
                                },
                                Type::Class(class_id) => MirExpr::Binary {
                                    op: MirBinary::Eq,
                                    left: Box::new(exception_type()),
                                    right: Box::new(MirExpr::Int(class_id as i64)),
                                },
                                // 其他类型（如 int）不能匹配 string/class 异常
                                _ => MirExpr::Bool(false),
                            }
                        }
                        None => MirExpr::Bool(true),
                    };
                    let catch_ty = catch
                        .ty
                        .as_ref()
                        .map(|ty| self.lowerer.lower_type_for_mir(ty))
                        .unwrap_or(Type::Error);
                    let catch_local = self.declare_local(&catch.name.name, catch_ty, false);
                    self.name_scopes
                        .last_mut()
                        .expect("作用域存在")
                        .insert(catch.name.name.clone(), catch_local);
                    let mut catch_body = vec![MirStmt::new(MirStmtKind::VarDecl {
                        local: catch_local,
                        init: Some(MirExpr::Call {
                            callee: MirCallee::Intrinsic {
                                name: "sw_exception_value".to_owned(),
                            },
                            args: vec![e.clone()],
                        }),
                    })];
                    catch_body.extend(self.lower_stmts(&catch.body.statements));
                    catch_body.push(MirStmt::new(MirStmtKind::Assign {
                        target: MirTarget::Local(matched_local),
                        value: MirExpr::Int(1),
                    }));
                    then_stmts.push(MirStmt::new(MirStmtKind::If {
                        cond,
                        then: catch_body,
                        else_: Vec::new(),
                    }));
                }
                if let Some(finally) = finally {
                    then_stmts.extend(self.lower_stmts(&finally.statements));
                }
                then_stmts.push(MirStmt::new(MirStmtKind::If {
                    cond: MirExpr::Binary {
                        op: MirBinary::Eq,
                        left: Box::new(MirExpr::Local(matched_local)),
                        right: Box::new(MirExpr::Int(0)),
                    },
                    then: vec![MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "sw_rethrow".to_owned(),
                        },
                        args: vec![e],
                    }))],
                    else_: Vec::new(),
                }));

                // 正常路径：body → finally → 弹出框架
                let mut else_stmts = self.lower_stmts(&body.statements);
                if let Some(finally) = finally {
                    else_stmts.extend(self.lower_stmts(&finally.statements));
                }
                else_stmts.push(MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_try_leave".to_owned(),
                    },
                    args: vec![frame],
                })));

                output.push(MirStmt::new(MirStmtKind::If {
                    cond: MirExpr::Binary {
                        op: MirBinary::Ne,
                        left: Box::new(MirExpr::Call {
                            callee: MirCallee::Extern {
                                name: "sw_setjmp".to_owned(),
                                sig: FunctionSig {
                                    module: ModuleId(0),
                                    name: "sw_setjmp".to_owned(),
                                    generics: Vec::new(),
                                    bounds: HashMap::new(),
                                    params: vec![ParamSig {
                                        name: "buf".to_owned(),
                                        ty: Type::Ptr(Box::new(Type::I8)),
                                        has_default: false,
                                        rest: false,
                                    }],
                                    ret: Type::Int,
                                    extern_c: true,
                                    span: Span::empty(0),
                                },
                            },
                            args: vec![MirExpr::Local(frame_local)],
                        }),
                        right: Box::new(MirExpr::Int(0)),
                    },
                    then: then_stmts,
                    else_: else_stmts,
                }));
            }
            StmtKind::Throw(expr) => {
                let value = self.lower_expr(expr);
                let ty = self
                    .result()
                    .expr_types
                    .get(&(expr.span.start, expr.span.end))
                    .cloned()
                    .unwrap_or(Type::Error);
                let type_id = match ty {
                    Type::Class(class_id) => class_id as i64,
                    _ => 0,
                };
                output.push(MirStmt::new(MirStmtKind::Expr(MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "sw_throw".to_owned(),
                    },
                    args: vec![value, MirExpr::Int(type_id)],
                })));
            }
            StmtKind::Defer(expr) => {
                self.error("MIR 降级暂不支持 defer（清理在后续版本实现）", expr.span);
            }
            StmtKind::Break => output.push(MirStmt::new(MirStmtKind::Break)),
            StmtKind::Continue => output.push(MirStmt::new(MirStmtKind::Continue)),
            StmtKind::Return(expr) => {
                let value = expr.as_ref().map(|expr| self.lower_expr(expr));
                output.push(MirStmt::new(MirStmtKind::Return(value)));
            }
            StmtKind::Expr(expr) => {
                self.lower_expr_stmt(expr, output);
            }
        }
    }

    fn lower_switch_case_body(&mut self, statements: &[Stmt]) -> Vec<MirStmt> {
        let mut output = Vec::new();
        for statement in statements {
            if matches!(statement.kind, StmtKind::Break) {
                break;
            }
            self.lower_stmt(statement, &mut output);
        }
        output
    }

    fn append_default_args(&mut self, symbol: SymbolId, args: &mut Vec<MirExpr>) {
        let symbol_span = self.lowerer.symbols[symbol.0 as usize].span;
        let defaults: Vec<Expr> = self
            .lowerer
            .module
            .items
            .iter()
            .find_map(|item| match &item.kind {
                ItemKind::Function(function) if item.span == symbol_span => Some(
                    function
                        .params
                        .iter()
                        .skip(args.len())
                        .filter_map(|param| param.default.clone())
                        .collect(),
                ),
                _ => None,
            })
            .unwrap_or_default();
        for default in defaults {
            args.push(self.lower_expr(&default));
        }
    }

    fn lower_expr_stmt(&mut self, expr: &Expr, output: &mut Vec<MirStmt>) {
        if let Some((target, value)) = self.lower_assign_parts(expr) {
            output.push(MirStmt::new(MirStmtKind::Assign { target, value }));
            return;
        }
        let mir = self.lower_expr(expr);
        output.push(MirStmt::new(MirStmtKind::Expr(mir)));
    }

    fn lower_assign_parts(&mut self, expr: &Expr) -> Option<(MirTarget, MirExpr)> {
        match &expr.kind {
            ExprKind::Assign { op, target, value } => {
                let target_ast = target;
                let value_ast = value;
                let target = self.lower_target(target)?;
                let value = self.lower_expr(value);
                let value = match op {
                    AssignOp::Assign => value,
                    AssignOp::Add if self.expr_type(target_ast) == Type::Str => {
                        let right_ty = self.expr_type(value_ast);
                        MirExpr::Call {
                            callee: MirCallee::Intrinsic {
                                name: "string_concat".to_owned(),
                            },
                            args: vec![
                                self.lower_expr(target_ast),
                                Self::to_string_expr(value, &right_ty),
                            ],
                        }
                    }
                    AssignOp::Add => MirExpr::Binary {
                        op: MirBinary::Add,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Sub => MirExpr::Binary {
                        op: MirBinary::Sub,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Mul => MirExpr::Binary {
                        op: MirBinary::Mul,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Div => MirExpr::Binary {
                        op: MirBinary::Div,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Rem => MirExpr::Binary {
                        op: MirBinary::Rem,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::BitAnd => MirExpr::Binary {
                        op: MirBinary::BitAnd,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::BitOr => MirExpr::Binary {
                        op: MirBinary::BitOr,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::BitXor => MirExpr::Binary {
                        op: MirBinary::BitXor,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Shl => MirExpr::Binary {
                        op: MirBinary::Shl,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Shr => MirExpr::Binary {
                        op: MirBinary::Shr,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::Coalesce => MirExpr::Binary {
                        op: MirBinary::Coalesce,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::LogicalAnd => MirExpr::Binary {
                        op: MirBinary::And,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                    AssignOp::LogicalOr => MirExpr::Binary {
                        op: MirBinary::Or,
                        left: Box::new(self.lower_expr(target_ast)),
                        right: Box::new(value),
                    },
                };
                Some((target, value))
            }
            ExprKind::Unary {
                op: UnaryOp::Inc | UnaryOp::Dec,
                expr: inner,
            }
            | ExprKind::Postfix {
                expr: inner,
                op: PostfixOp::Inc | PostfixOp::Dec,
            } => {
                let target = self.lower_target(inner)?;
                let op = if matches!(
                    &expr.kind,
                    ExprKind::Unary {
                        op: UnaryOp::Dec,
                        ..
                    }
                ) || matches!(
                    &expr.kind,
                    ExprKind::Postfix {
                        op: PostfixOp::Dec,
                        ..
                    }
                ) {
                    MirBinary::Sub
                } else {
                    MirBinary::Add
                };
                let value = MirExpr::Binary {
                    op,
                    left: Box::new(self.lower_value(inner)),
                    right: Box::new(MirExpr::Int(1)),
                };
                Some((target, value))
            }
            _ => None,
        }
    }

    /// 泛型函数按调用点单态化：推断类型实参，生成并缓存专用实例。
    fn instantiate_generic(
        &mut self,
        symbol: SymbolId,
        template: &FunctionSig,
        ast_args: &[Expr],
        span: Span,
    ) -> (String, FunctionSig) {
        let mut type_args = HashMap::new();
        for (index, param) in template.params.iter().enumerate() {
            if let Some(arg_ty) = ast_args.get(index) {
                infer_type_arg(&param.ty, &self.expr_type(arg_ty), &mut type_args);
            }
        }
        // 校验 where 约束：类型实参必须实现约束接口（沿基类链收集，与 vtable/
        // 接口赋值规则一致——接口只需被基类 implements，子类实参即满足约束）。
        for (param_name, bound_tys) in &template.bounds {
            let Some(actual) = type_args.get(param_name).cloned() else {
                continue;
            };
            for bound_ty in bound_tys {
                if let Type::Interface(bound_iface_id) = bound_ty {
                    // 约束驱动反向推导：若 bound 接口实参含类型参数（where T:
                    // Container<U>），从实参类实现的同模板接口具体实例反推 U，
                    // 再据此把 bound 实例化到具体接口做校验。
                    let derived =
                        self.derive_bound_params(&mut type_args, *bound_iface_id, &actual);
                    let implements = match actual {
                        Type::Class(class_id) => {
                            let types = &self.lowerer.types;
                            let check_id = derived.unwrap_or(*bound_iface_id);
                            types.class_base_chain(class_id).iter().any(|id| {
                                types
                                    .class_interfaces
                                    .get(id)
                                    .map(|ids| ids.contains(&check_id))
                                    .unwrap_or(false)
                            })
                        }
                        _ => false,
                    };
                    if !implements {
                        self.lowerer.error(
                            format!(
                                "类型实参 {} 未实现约束接口 {}",
                                actual.display(),
                                self.lowerer.types.interfaces[*bound_iface_id as usize].name
                            ),
                            span,
                        );
                    }
                }
            }
        }
        let mut key_parts: Vec<String> = type_args
            .iter()
            .map(|(name, ty)| format!("{name}={}", ty.display()))
            .collect();
        key_parts.sort();
        let key = format!(
            "{}:{}:{}",
            template.module.0,
            stable_function_name(
                template,
                self.lowerer
                    .module_stems
                    .get(&template.module.0)
                    .map(String::as_str)
                    .unwrap_or("mod"),
            ),
            key_parts.join(",")
        );
        if let Some((name, sig)) = self.lowerer.generic_instances.get(&key) {
            return (name.clone(), sig.clone());
        }

        let instance_id = self.lowerer.generic_counter;
        self.lowerer.generic_counter += 1;
        let instance_name = format!(
            "sw_gen_{}_{}",
            stable_function_name(
                template,
                self.lowerer
                    .module_stems
                    .get(&template.module.0)
                    .map(String::as_str)
                    .unwrap_or("mod"),
            ),
            instance_id
        );
        let instance_params: Vec<MirParam> = template
            .params
            .iter()
            .map(|param| MirParam {
                name: param.name.clone(),
                ty: substitute_type(&param.ty, &type_args),
            })
            .collect();
        let instance_ret = substitute_type(&template.ret, &type_args);
        let instance_sig = FunctionSig {
            module: template.module,
            name: instance_name.clone(),
            generics: Vec::new(),
            bounds: HashMap::new(),
            params: template
                .params
                .iter()
                .map(|param| ParamSig {
                    name: param.name.clone(),
                    ty: substitute_type(&param.ty, &type_args),
                    has_default: param.has_default,
                    rest: param.rest,
                })
                .collect(),
            ret: instance_ret.clone(),
            extern_c: false,
            span: template.span,
        };
        // 先登记再降级函数体，保证自递归调用能命中同一个实例。
        self.lowerer
            .generic_instances
            .insert(key, (instance_name.clone(), instance_sig.clone()));

        let body = self
            .lowerer
            .all_modules
            .get(template.module.0 as usize)
            .and_then(|module| {
                module.items.iter().find_map(|item| match &item.kind {
                    ItemKind::Function(function)
                        if function.name.name == template.name && item.span == template.span =>
                    {
                        function.body.clone()
                    }
                    _ => None,
                })
            });
        let Some(body) = body else {
            self.error("泛型函数体未找到", span);
            return (instance_name, instance_sig);
        };
        let global_by_symbol = self.global_by_symbol.clone();
        let lowerer = &mut *self.lowerer;
        let mut nested = FnLower {
            lowerer,
            result_index: template.module.0 as usize,
            name: instance_name.clone(),
            user_name: template.name.clone(),
            exported: false,
            params: instance_params,
            ret: instance_ret,
            locals: Vec::new(),
            name_scopes: Vec::new(),
            global_by_symbol,
            this_class: None,
            captures: HashMap::new(),
            type_args,
        };
        let instance_function = nested.lower_function(Some(&body), false);
        self.lowerer.hidden_functions.push(instance_function);
        (instance_name, instance_sig)
    }

    /// 约束驱动反向推导：对 `where T: Container<U>` 这类"接口实参含类型参数"的
    /// 约束，从实参类（T）实现的同模板接口具体实例反推 U 写入 type_args，
    /// 并把约束实例化到那个具体接口 id 返回；找不到则返回 None（沿用原 bound）。
    fn derive_bound_params(
        &self,
        type_args: &mut HashMap<String, Type>,
        bound_iface_id: u32,
        actual: &Type,
    ) -> Option<u32> {
        let types = &self.lowerer.types;
        // 反查 bound 接口的泛型模板与实参（含类型参数的实例 id → (模板, 实参)）。
        let (bound_template_id, bound_args) = types
            .generic_interface_instances
            .iter()
            .find(|entry| *entry.1 == bound_iface_id)
            .map(|((t, args), _)| (*t, args.clone()))?;
        // 反推来源：实参类实现的同模板接口具体实例（沿基类链）。
        let Type::Class(class_id) = actual else {
            return None;
        };
        let mut concrete_bound: Option<u32> = None;
        for cid in types.class_base_chain(*class_id) {
            let Some(ifaces) = types.class_interfaces.get(&cid) else {
                continue;
            };
            for &inst_id in ifaces {
                let Some((t2, concrete_args)) = types
                    .generic_interface_instances
                    .iter()
                    .find(|entry| *entry.1 == inst_id)
                    .map(|((t, args), _)| (*t, args.clone()))
                else {
                    continue;
                };
                if t2 != bound_template_id {
                    continue;
                }
                // 同一模板接口实例：把 bound 实参里的类型参数按具体实参反推。
                for (b, c) in bound_args.iter().zip(concrete_args.iter()) {
                    if let Type::TypeParam(name) = b {
                        if !matches!(c, Type::TypeParam(_) | Type::Unknown | Type::Error) {
                            type_args.entry(name.clone()).or_insert_with(|| c.clone());
                        }
                    }
                }
                concrete_bound = Some(inst_id);
                break;
            }
            if concrete_bound.is_some() {
                break;
            }
        }
        concrete_bound
    }

    /// 把约束/bound 接口 id 解析到实例化的具体接口 id，供泛型约束方法派发用。
    /// 若 bound 是含类型参数的泛型接口实例（如 Container<U>），用 type_args 替换
    /// 实参后查 generic_interface_instances，得到实参类实际注册的接口实例 id
    /// （与 IntBox 等实现的 Container<int> 槽位一致），避免 vtable 槽位错位。
    /// 非泛型接口或仍含未解析类型参数的场景返回原 id。
    fn resolve_concrete_interface(&self, bound_iface_id: u32) -> u32 {
        let types = &self.lowerer.types;
        if let Some((template_id, bound_args)) = types
            .generic_interface_instances
            .iter()
            .find(|entry| *entry.1 == bound_iface_id)
            .map(|((t, args), _)| (*t, args.clone()))
        {
            let concrete_args: Vec<Type> = bound_args
                .iter()
                .map(|ty| substitute_type(ty, &self.type_args))
                .collect();
            if concrete_args
                .iter()
                .any(|t| matches!(t, Type::TypeParam(_)))
            {
                return bound_iface_id;
            }
            if let Some(&concrete_id) = types
                .generic_interface_instances
                .get(&(template_id, concrete_args))
            {
                return concrete_id;
            }
        }
        bound_iface_id
    }

    fn lower_target(&mut self, expr: &Expr) -> Option<MirTarget> {
        match &expr.kind {
            ExprKind::Ident(ident) => {
                if let Some(local) = self.lookup_local(&ident.name) {
                    return Some(MirTarget::Local(local));
                }
                if let Some(symbol) = self.result().ident_symbols.get(&expr.span.start).copied() {
                    if let Some(global) = self.global_by_symbol.get(&symbol.0) {
                        return Some(MirTarget::Global(*global as u32));
                    }
                }
                None
            }
            ExprKind::Member { object, name, .. } => {
                let field = self.resolve_field_target(object, &name.name)?;
                let object = self.lower_expr(object);
                let index = match field {
                    FieldTarget::Struct(_, index) => index,
                    FieldTarget::Class(class_id, index) => {
                        self.lowerer.ancestor_field_count(class_id) + index
                    }
                };
                let _ = name;
                Some(MirTarget::Field {
                    object: Box::new(object),
                    index,
                })
            }
            ExprKind::Index { object, index, .. } => Some(MirTarget::Index {
                object: Box::new(self.lower_expr(object)),
                index: Box::new(self.lower_expr(index)),
                elem: Box::new(match self.expr_type(object).without_nullable() {
                    Type::Array(inner) => (**inner).clone(),
                    _ => Type::Int,
                }),
            }),
            _ => None,
        }
    }

    fn lower_value(&mut self, expr: &Expr) -> MirExpr {
        self.lower_expr(expr)
    }

    /// 把右/左操作数包装成字符串：标量经 to_string intrinsic 转换，字符串原样。
    fn to_string_expr(value: MirExpr, ty: &Type) -> MirExpr {
        if *ty == Type::Str {
            return value;
        }
        let name = match ty {
            Type::Int => "int_to_string",
            Type::F32 | Type::F64 => "float_to_string",
            Type::Bool => "bool_to_string",
            Type::Char => "char_to_string",
            _ => "int_to_string",
        };
        MirExpr::Call {
            callee: MirCallee::Intrinsic {
                name: name.to_owned(),
            },
            args: vec![value],
        }
    }

    /// struct 相等：按字段逐一比较（字符串字段按内容、嵌套 struct 递归、其余标量按值），
    /// 全部相等才为 true。
    fn struct_eq_mir(&mut self, left: MirExpr, right: MirExpr, id: u32) -> MirExpr {
        let fields = self.lowerer.types.structs[id as usize].fields.clone();
        let mut acc: Option<MirExpr> = None;
        for (index, field) in fields.iter().enumerate() {
            let lf = MirExpr::Field {
                object: Box::new(left.clone()),
                index,
            };
            let rf = MirExpr::Field {
                object: Box::new(right.clone()),
                index,
            };
            let cmp = match &field.ty {
                Type::Str => MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "string_eq".to_owned(),
                    },
                    args: vec![lf, rf],
                },
                Type::Struct(inner) => self.struct_eq_mir(lf, rf, *inner),
                _ => MirExpr::Binary {
                    op: MirBinary::Eq,
                    left: Box::new(lf),
                    right: Box::new(rf),
                },
            };
            acc = Some(match acc {
                Some(prev) => MirExpr::Binary {
                    op: MirBinary::And,
                    left: Box::new(prev),
                    right: Box::new(cmp),
                },
                None => cmp,
            });
        }
        acc.unwrap_or(MirExpr::Bool(true))
    }

    fn lower_expr(&mut self, expr: &Expr) -> MirExpr {
        match &expr.kind {
            ExprKind::Integer { text, suffix } => {
                let value = parse_int(text).unwrap_or(0);
                if matches!(
                    suffix,
                    Some(sw_frontend::IntegerSuffix::U8)
                        | Some(sw_frontend::IntegerSuffix::U16)
                        | Some(sw_frontend::IntegerSuffix::U32)
                        | Some(sw_frontend::IntegerSuffix::U64)
                        | Some(sw_frontend::IntegerSuffix::Usize)
                ) {
                    MirExpr::UInt(value as u64)
                } else {
                    MirExpr::Int(value)
                }
            }
            ExprKind::Float { text, .. } => MirExpr::Float(text.parse::<f64>().unwrap_or(0.0)),
            ExprKind::Str(value) => {
                let index = self.lowerer.intern_string(value);
                MirExpr::Str(index)
            }
            ExprKind::Char(value) => MirExpr::Char(*value),
            ExprKind::Bool(value) => MirExpr::Bool(*value),
            ExprKind::Null => MirExpr::Null,
            ExprKind::Ident(ident) => {
                if let Some(local) = self.lookup_local(&ident.name) {
                    return MirExpr::Local(local);
                }
                if let Some(symbol) = self.result().ident_symbols.get(&expr.span.start).copied() {
                    if let Some(slot) = self.captures.get(&symbol.0) {
                        return MirExpr::EnvGet { slot: *slot };
                    }
                    if let Some(global) = self.global_by_symbol.get(&symbol.0) {
                        return MirExpr::Global(*global as u32);
                    }
                }
                self.error("无法降级标识符", expr.span);
                MirExpr::Int(0)
            }
            ExprKind::This => {
                if self.this_class.is_some() {
                    MirExpr::Local(0)
                } else {
                    self.error("`this` 只能在方法中降级", expr.span);
                    MirExpr::Int(0)
                }
            }
            ExprKind::Super => {
                self.error("`super` 只能用于调用基类构造函数或方法", expr.span);
                MirExpr::Null
            }
            ExprKind::Group(inner) => self.lower_expr(inner),
            ExprKind::Cast { expr: inner, ty } => {
                let to = substitute_type(&self.lowerer.lower_type_for_mir(ty), &self.type_args);
                MirExpr::Cast {
                    expr: Box::new(self.lower_expr(inner)),
                    to,
                }
            }
            ExprKind::Unary { op, expr: inner } => {
                match op {
                    UnaryOp::Inc | UnaryOp::Dec => {
                        if let Some(target) = self.lower_target(inner) {
                            let op = if *op == UnaryOp::Dec {
                                MirBinary::Sub
                            } else {
                                MirBinary::Add
                            };
                            let value = MirExpr::Binary {
                                op,
                                left: Box::new(self.lower_expr(inner)),
                                right: Box::new(MirExpr::Int(1)),
                            };
                            return MirExpr::Assign {
                                target,
                                value: Box::new(value),
                            };
                        }
                        self.error("`++`/`--` 需要可赋值的数值目标", inner.span);
                        return MirExpr::Int(0);
                    }
                    _ => {}
                };
                let mir_op = match op {
                    UnaryOp::Not => MirUnary::Not,
                    UnaryOp::Neg => MirUnary::Neg,
                    UnaryOp::Pos => MirUnary::Pos,
                    UnaryOp::BitNot => MirUnary::BitNot,
                    UnaryOp::Await => {
                        self.error("await 降级暂不支持", expr.span);
                        MirUnary::Not
                    }
                    UnaryOp::Inc | UnaryOp::Dec => unreachable!(),
                };
                MirExpr::Unary {
                    op: mir_op,
                    expr: Box::new(self.lower_expr(inner)),
                }
            }
            ExprKind::Binary { op, left, right } => {
                if *op == BinaryOp::Add {
                    let left_ty = self.expr_type(left);
                    let right_ty = self.expr_type(right);
                    if left_ty == Type::Str && Checker::concatable_with_string(&right_ty) {
                        let left = self.lower_expr(left);
                        let right = Self::to_string_expr(self.lower_expr(right), &right_ty);
                        return MirExpr::Call {
                            callee: MirCallee::Intrinsic {
                                name: "string_concat".to_owned(),
                            },
                            args: vec![left, right],
                        };
                    }
                    if right_ty == Type::Str
                        && left_ty != Type::Str
                        && Checker::concatable_with_string(&left_ty)
                    {
                        let left = Self::to_string_expr(self.lower_expr(left), &left_ty);
                        let right = self.lower_expr(right);
                        return MirExpr::Call {
                            callee: MirCallee::Intrinsic {
                                name: "string_concat".to_owned(),
                            },
                            args: vec![left, right],
                        };
                    }
                }
                if matches!(*op, BinaryOp::Eq | BinaryOp::Ne) {
                    if let Type::Struct(id) = self.expr_type(left) {
                        let left_mir = self.lower_expr(left);
                        let right_mir = self.lower_expr(right);
                        let eq = self.struct_eq_mir(left_mir, right_mir, id);
                        return if *op == BinaryOp::Ne {
                            MirExpr::Unary {
                                op: MirUnary::Not,
                                expr: Box::new(eq),
                            }
                        } else {
                            eq
                        };
                    }
                }
                if *op == BinaryOp::Pow {
                    let is_float = self.expr_type(left).is_float();
                    let name = if is_float { "pow_f64" } else { "pow_i64" };
                    return MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: name.to_owned(),
                        },
                        args: vec![self.lower_expr(left), self.lower_expr(right)],
                    };
                }
                if *op == BinaryOp::Rem && self.expr_type(left).is_float() {
                    return MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "frem_f64".to_owned(),
                        },
                        args: vec![self.lower_expr(left), self.lower_expr(right)],
                    };
                }
                if matches!(op, BinaryOp::Eq | BinaryOp::Ne) && self.expr_type(left) == Type::Str {
                    let name = if *op == BinaryOp::Eq {
                        "string_eq"
                    } else {
                        "string_ne"
                    };
                    return MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: name.to_owned(),
                        },
                        args: vec![self.lower_expr(left), self.lower_expr(right)],
                    };
                }
                MirExpr::Binary {
                    op: mir_binary(op),
                    left: Box::new(self.lower_expr(left)),
                    right: Box::new(self.lower_expr(right)),
                }
            }
            ExprKind::Assign { .. } => {
                if let Some((target, value)) = self.lower_assign_parts(expr) {
                    return MirExpr::Assign {
                        target,
                        value: Box::new(value),
                    };
                }
                self.error("赋值表达式降级失败", expr.span);
                MirExpr::Int(0)
            }
            ExprKind::Conditional { cond, then, else_ } => MirExpr::Select {
                cond: Box::new(self.lower_expr(cond)),
                then: Box::new(self.lower_expr(then)),
                else_: Box::new(self.lower_expr(else_)),
            },
            ExprKind::Call { callee, args } => {
                let optional_receiver =
                    matches!(&callee.kind, ExprKind::Member { optional: true, .. });
                // 闭包调用：callee 是 lambda，或 callee 是函数类型的局部/参数/全局
                let closure_ty = match &callee.kind {
                    ExprKind::Lambda { .. } => self.expr_type(callee),
                    ExprKind::Ident(_) => {
                        let symbol = self
                            .result()
                            .ident_symbols
                            .get(&callee.span.start)
                            .copied()
                            .map(|id| self.lowerer.symbol(id));
                        match symbol {
                            Some(symbol) => match &symbol.kind {
                                SymbolKind::Local { ty, .. }
                                | SymbolKind::Param { ty }
                                | SymbolKind::Global { ty, .. } => ty.clone(),
                                _ => Type::Error,
                            },
                            None => Type::Error,
                        }
                    }
                    _ => Type::Error,
                };
                if let Type::Function {
                    params: fn_params,
                    ret: fn_ret,
                } = &closure_ty
                {
                    let mut values = vec![self.lower_expr(callee)];
                    for arg in args {
                        values.push(self.lower_expr(arg));
                    }
                    let mut sig = FunctionSig {
                        module: self.lowerer.state.id,
                        name: "$closure".to_owned(),
                        generics: Vec::new(),
                        bounds: HashMap::new(),
                        params: vec![ParamSig {
                            name: "$env".to_owned(),
                            ty: Type::Ptr(Box::new(Type::I8)),
                            has_default: false,
                            rest: false,
                        }],
                        ret: (**fn_ret).clone(),
                        extern_c: false,
                        span: expr.span,
                    };
                    for (index, param_ty) in fn_params.iter().enumerate() {
                        sig.params.push(ParamSig {
                            name: format!("arg{index}"),
                            ty: param_ty.clone(),
                            has_default: false,
                            rest: false,
                        });
                    }
                    return MirExpr::Call {
                        callee: MirCallee::Closure { sig },
                        args: values,
                    };
                }
                let target = self
                    .result()
                    .call_targets
                    .get(&(expr.span.start, expr.span.end))
                    .cloned();
                let ast_args = args;
                let variadic_fixed = match &target {
                    Some(CallTarget::Function(symbol)) => {
                        let sig = match &self.lowerer.symbol(*symbol).kind {
                            SymbolKind::Function(sig) => sig.clone(),
                            _ => placeholder_sig(),
                        };
                        if sig.params.last().map(|param| param.rest).unwrap_or(false) {
                            Some(sig.params.len().saturating_sub(1))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };
                let mut args: Vec<MirExpr> = Vec::new();
                for (index, arg) in ast_args.iter().enumerate() {
                    if let Some(fixed) = variadic_fixed {
                        if index >= fixed {
                            break;
                        }
                    }
                    if let ExprKind::Spread(inner) = &arg.kind {
                        if let ExprKind::Array(items) = &inner.kind {
                            for item in items {
                                args.push(self.lower_expr(item));
                            }
                            continue;
                        }
                    }
                    args.push(self.lower_expr(arg));
                }
                if let Some(fixed) = variadic_fixed {
                    let mut packed: Vec<(i64, MirExpr)> = Vec::new();
                    for arg in ast_args.iter().skip(fixed) {
                        if let ExprKind::Spread(inner) = &arg.kind {
                            if let ExprKind::Array(items) = &inner.kind {
                                for item in items {
                                    let ty = self.expr_type(item);
                                    packed.push((vararg_tag(&ty), self.lower_expr(item)));
                                }
                                continue;
                            }
                        }
                        let ty = self.expr_type(arg);
                        packed.push((vararg_tag(&ty), self.lower_expr(arg)));
                    }
                    args.push(MirExpr::VarArgs(packed));
                }
                if variadic_fixed.is_none() {
                    if let Some(CallTarget::Function(symbol)) = &target {
                        self.append_default_args(*symbol, &mut args);
                    }
                }
                if let Some(CallTarget::EnumConstruct {
                    enum_id,
                    variant_index,
                }) = &target
                {
                    let info = self.lowerer.types.enums[*enum_id as usize].clone();
                    let variant = &info.members[*variant_index];
                    return MirExpr::EnumNew {
                        tag: variant.discriminant,
                        fields: args,
                    };
                }
                if let Some(CallTarget::ArrayMethod { method, elem, ret }) = &target {
                    let object = match &callee.kind {
                        ExprKind::Member { object, .. } => self.lower_expr(object),
                        _ => MirExpr::Int(0),
                    };
                    if matches!(method, ArrayMethodKind::Push | ArrayMethodKind::Pop) {
                        let (name, params) = match method {
                            ArrayMethodKind::Push => ("sw_array_push", 1usize),
                            _ => ("sw_array_pop", 0usize),
                        };
                        let mut sig = FunctionSig {
                            module: self.lowerer.state.id,
                            name: name.to_owned(),
                            generics: Vec::new(),
                            bounds: HashMap::new(),
                            params: vec![ParamSig {
                                name: "self".to_owned(),
                                ty: Type::Array(Box::new(elem.clone())),
                                has_default: false,
                                rest: false,
                            }],
                            ret: Type::Int,
                            extern_c: true,
                            span: expr.span,
                        };
                        if params == 1 {
                            sig.params.push(ParamSig {
                                name: "value".to_owned(),
                                ty: elem.clone(),
                                has_default: false,
                                rest: false,
                            });
                        } else {
                            sig.ret = elem.clone();
                        }
                        let mut call_args = vec![object];
                        if params == 1 {
                            call_args.push(args.first().cloned().unwrap_or(MirExpr::Int(0)));
                        }
                        return MirExpr::Call {
                            callee: MirCallee::Extern {
                                name: name.to_owned(),
                                sig,
                            },
                            args: call_args,
                        };
                    }
                    let closure = args.first().cloned().unwrap_or(MirExpr::Int(0));
                    let fn_ty = self.expr_type(&ast_args[0]);
                    let (fn_params, fn_ret) = match &fn_ty {
                        Type::Function { params, ret } => (params.clone(), (**ret).clone()),
                        _ => (Vec::new(), Type::Error),
                    };
                    let mut sig = FunctionSig {
                        module: self.lowerer.state.id,
                        name: "$closure".to_owned(),
                        generics: Vec::new(),
                        bounds: HashMap::new(),
                        params: vec![ParamSig {
                            name: "$env".to_owned(),
                            ty: Type::Ptr(Box::new(Type::I8)),
                            has_default: false,
                            rest: false,
                        }],
                        ret: fn_ret,
                        extern_c: false,
                        span: expr.span,
                    };
                    for (index, param_ty) in fn_params.iter().enumerate() {
                        sig.params.push(ParamSig {
                            name: format!("arg{index}"),
                            ty: param_ty.clone(),
                            has_default: false,
                            rest: false,
                        });
                    }
                    return match method {
                        ArrayMethodKind::Map => MirExpr::ArrayMap {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                            ret_elem: ret.clone(),
                        },
                        ArrayMethodKind::Filter => MirExpr::ArrayFilter {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                        },
                        ArrayMethodKind::ForEach => MirExpr::ArrayIterate {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                            mode: IterateMode::ForEach,
                        },
                        ArrayMethodKind::Some => MirExpr::ArrayIterate {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                            mode: IterateMode::Some,
                        },
                        ArrayMethodKind::Every => MirExpr::ArrayIterate {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                            mode: IterateMode::Every,
                        },
                        ArrayMethodKind::Find => MirExpr::ArrayIterate {
                            object: Box::new(object),
                            closure: Box::new(closure),
                            sig,
                            elem: elem.clone(),
                            mode: IterateMode::Find,
                        },
                        ArrayMethodKind::Push | ArrayMethodKind::Pop => unreachable!(),
                    };
                }
                let callee = match target {
                    Some(CallTarget::EnumConstruct { .. }) => unreachable!(),
                    Some(CallTarget::ArrayMethod { .. }) => unreachable!(),
                    Some(CallTarget::StaticMethod { class, index }) => {
                        let sig = self
                            .lowerer
                            .types
                            .classes
                            .get(class as usize)
                            .and_then(|info| info.static_methods.get(index))
                            .map(|m| m.sig.clone())
                            .unwrap_or_else(placeholder_sig);
                        MirCallee::Function {
                            module: self.lowerer.state.id.0,
                            name: format!("sw_smethod_{class}_{index}_{}", sig.name),
                            sig,
                        }
                    }
                    Some(CallTarget::Function(symbol)) => {
                        let sig = match &self.lowerer.symbol(symbol).kind {
                            SymbolKind::Function(sig) => sig.clone(),
                            _ => placeholder_sig(),
                        };
                        if sig.extern_c {
                            MirCallee::Extern {
                                name: sig.name.clone(),
                                sig: sig.clone(),
                            }
                        } else if !sig.generics.is_empty() {
                            let (instance_name, instance_sig) =
                                self.instantiate_generic(symbol, &sig, ast_args, expr.span);
                            MirCallee::Function {
                                module: sig.module.0,
                                name: instance_name,
                                sig: instance_sig,
                            }
                        } else {
                            MirCallee::Function {
                                module: sig.module.0,
                                name: stable_function_name(
                                    &sig,
                                    self.lowerer
                                        .module_stems
                                        .get(&sig.module.0)
                                        .map(String::as_str)
                                        .unwrap_or("mod"),
                                ),
                                sig: sig.clone(),
                            }
                        }
                    }
                    Some(CallTarget::Method { class, index }) => {
                        match &callee.kind {
                            ExprKind::Member { object, .. } => {
                                if matches!(object.kind, ExprKind::Super) {
                                    args.insert(0, MirExpr::Local(0));
                                } else {
                                    args.insert(0, self.lower_expr(object));
                                }
                            }
                            // super(...) 基类构造函数调用：接收者是当前对象（this = 局部 0）。
                            ExprKind::Super => {
                                args.insert(0, MirExpr::Local(0));
                            }
                            _ => {}
                        }
                        let class_info = self.lowerer.types.classes.get(class as usize);
                        let method_name = class_info
                            .and_then(|info| info.methods.get(index))
                            .map(|method| method.name.clone())
                            .unwrap_or_default();
                        let method_sig = class_info
                            .and_then(|info| info.methods.get(index))
                            .map(|method| method.sig.clone())
                            .unwrap_or_else(placeholder_sig);
                        let callee_name = if method_name == "constructor" {
                            format!("sw_ctor_{class}")
                        } else {
                            format!("sw_m_{class}_{index}_{method_name}")
                        };
                        MirCallee::Method {
                            class,
                            name: callee_name,
                            sig: method_sig,
                        }
                    }
                    Some(CallTarget::InterfaceMethod { interface, index }) => {
                        match &callee.kind {
                            ExprKind::Member { object, .. } => {
                                args.insert(0, self.lower_expr(object));
                            }
                            _ => {}
                        }
                        let iface_info = self.lowerer.types.interfaces.get(interface as usize);
                        let method_sig = iface_info
                            .and_then(|info| info.methods.get(index))
                            .cloned()
                            .unwrap_or_else(placeholder_sig);
                        // 泛型实例化时用 type_args 替换接口方法签名里的类型参数
                        // （如 `where T: Container<U>` 的方法返回 U → 具体类型）。
                        let method_sig = FunctionSig {
                            module: method_sig.module,
                            name: method_sig.name,
                            generics: Vec::new(),
                            bounds: HashMap::new(),
                            params: method_sig
                                .params
                                .iter()
                                .map(|param| ParamSig {
                                    name: param.name.clone(),
                                    ty: substitute_type(&param.ty, &self.type_args),
                                    has_default: param.has_default,
                                    rest: param.rest,
                                })
                                .collect(),
                            ret: substitute_type(&method_sig.ret, &self.type_args),
                            extern_c: method_sig.extern_c,
                            span: method_sig.span,
                        };
                        let interface = self.resolve_concrete_interface(interface);
                        MirCallee::InterfaceMethod {
                            interface,
                            index,
                            sig: method_sig,
                        }
                    }
                    Some(CallTarget::StrMethod { runtime_name, sig }) => {
                        match &callee.kind {
                            ExprKind::Member { object, .. } => {
                                args.insert(0, self.lower_expr(object));
                            }
                            _ => {}
                        }
                        MirCallee::Extern {
                            name: runtime_name,
                            sig,
                        }
                    }
                    None => {
                        self.error("调用目标未解析", expr.span);
                        MirCallee::Intrinsic {
                            name: "unresolved".to_owned(),
                        }
                    }
                };
                let call = MirExpr::Call { callee, args };
                if optional_receiver {
                    if let MirExpr::Call { args, .. } = &call {
                        if let Some(receiver) = args.first().cloned() {
                            let zero = MirExpr::Int(0);
                            let cond = MirExpr::Binary {
                                op: MirBinary::Ne,
                                left: Box::new(receiver),
                                right: Box::new(zero),
                            };
                            let fallback = optional_fallback(&self.expr_type(expr));
                            return MirExpr::Select {
                                cond: Box::new(cond),
                                then: Box::new(call),
                                else_: Box::new(fallback),
                            };
                        }
                    }
                }
                call
            }
            ExprKind::Member {
                object,
                name,
                optional,
            } => {
                // 类静态字段访问：ClassName.field → 模块级全局。
                if let Some(StaticMemberTarget::Field(class, index)) = self
                    .result()
                    .static_member_targets
                    .get(&expr.span.start)
                    .copied()
                {
                    let gname = format!("sw_sfield_{class}_{index}");
                    if let Some(&gindex) = self.lowerer.static_field_globals.get(&gname) {
                        return MirExpr::Global(gindex);
                    }
                    return MirExpr::Int(0);
                }
                // ADT 无参变体构造（EnumName.Variant）：检查阶段已登记 EnumConstruct。
                if let Some(CallTarget::EnumConstruct {
                    enum_id,
                    variant_index,
                }) = self
                    .result()
                    .call_targets
                    .get(&(expr.span.start, expr.span.end))
                    .cloned()
                {
                    let info = self.lowerer.types.enums[enum_id as usize].clone();
                    let variant = &info.members[variant_index];
                    return MirExpr::EnumNew {
                        tag: variant.discriminant,
                        fields: vec![],
                    };
                }
                let access = if name.name == "length"
                    && matches!(
                        self.expr_type(object).without_nullable(),
                        Type::Array(_) | Type::Str
                    ) {
                    if *self.expr_type(object).without_nullable() == Type::Str {
                        // string.length 按字符数（UTF-8 码点）。
                        MirExpr::Call {
                            callee: MirCallee::Intrinsic {
                                name: "string_char_len".to_owned(),
                            },
                            args: vec![self.lower_expr(object)],
                        }
                    } else {
                        MirExpr::Len {
                            object: Box::new(self.lower_expr(object)),
                            string: false,
                        }
                    }
                } else if let Some(field) = self.resolve_field_target(object, &name.name) {
                    match field {
                        FieldTarget::Struct(_, index) => MirExpr::Field {
                            object: Box::new(self.lower_expr(object)),
                            index,
                        },
                        FieldTarget::Class(class_id, index) => MirExpr::Field {
                            object: Box::new(self.lower_expr(object)),
                            index: self.lowerer.ancestor_field_count(class_id) + index,
                        },
                    }
                } else {
                    // 枚举成员访问 Color.Red
                    let object_ty = self.expr_type(object);
                    if let Type::Enum(enum_id) = object_ty {
                        let value =
                            self.lowerer
                                .types
                                .enums
                                .get(enum_id as usize)
                                .and_then(|info| {
                                    info.members
                                        .iter()
                                        .find(|member| member.name == name.name)
                                        .map(|member| member.discriminant)
                                });
                        if let Some(value) = value {
                            MirExpr::Int(value)
                        } else {
                            self.error("成员访问未解析", expr.span);
                            MirExpr::Int(0)
                        }
                    } else {
                        self.error("成员访问未解析", expr.span);
                        MirExpr::Int(0)
                    }
                };
                if *optional {
                    let object_mir = self.lower_expr(object);
                    let zero = MirExpr::Int(0);
                    let cond = MirExpr::Binary {
                        op: MirBinary::Ne,
                        left: Box::new(object_mir),
                        right: Box::new(zero),
                    };
                    let fallback = optional_fallback(&self.expr_type(expr));
                    return MirExpr::Select {
                        cond: Box::new(cond),
                        then: Box::new(access),
                        else_: Box::new(fallback),
                    };
                }
                access
            }
            ExprKind::Index {
                object,
                index,
                optional,
            } => {
                let access = if self.expr_type(object) == Type::Str {
                    // s[i] 按字符（UTF-8 码点）。
                    MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "string_char_at".to_owned(),
                        },
                        args: vec![self.lower_expr(object), self.lower_expr(index)],
                    }
                } else {
                    let elem = match self.expr_type(object).without_nullable() {
                        Type::Array(inner) => (**inner).clone(),
                        _ => Type::Int,
                    };
                    MirExpr::Index {
                        object: Box::new(self.lower_expr(object)),
                        index: Box::new(self.lower_expr(index)),
                        elem: Box::new(elem),
                    }
                };
                if *optional {
                    let object_mir = self.lower_expr(object);
                    let zero = MirExpr::Int(0);
                    let cond = MirExpr::Binary {
                        op: MirBinary::Ne,
                        left: Box::new(object_mir),
                        right: Box::new(zero),
                    };
                    let fallback = optional_fallback(&self.expr_type(expr));
                    return MirExpr::Select {
                        cond: Box::new(cond),
                        then: Box::new(access),
                        else_: Box::new(fallback),
                    };
                }
                access
            }
            ExprKind::Slice { object, start, end } => {
                let object_ty = self.expr_type(object);
                let elem_size = match object_ty.without_nullable() {
                    Type::Array(inner) if matches!(&**inner, Type::U8) => 1,
                    _ => 8,
                };
                let object_mir = self.lower_expr(object);
                let start_mir = start
                    .as_ref()
                    .map(|expr| self.lower_expr(expr))
                    .unwrap_or(MirExpr::Int(0));
                let end_mir =
                    end.as_ref()
                        .map(|expr| self.lower_expr(expr))
                        .unwrap_or(MirExpr::Len {
                            object: Box::new(object_mir.clone()),
                            string: false,
                        });
                MirExpr::Call {
                    callee: MirCallee::Intrinsic {
                        name: "array_slice".to_owned(),
                    },
                    args: vec![object_mir, start_mir, end_mir, MirExpr::Int(elem_size)],
                }
            }
            ExprKind::Postfix { expr: inner, op } => {
                if let Some(target) = self.lower_target(inner) {
                    let mir_op = match op {
                        PostfixOp::Inc => MirUnary::Inc,
                        PostfixOp::Dec => MirUnary::Dec,
                    };
                    return MirExpr::Postfix { target, op: mir_op };
                }
                self.error("`++`/`--` 需要可赋值的数值目标", inner.span);
                MirExpr::Int(0)
            }
            ExprKind::TryOp(inner) => {
                let inner_ty = self.expr_type(inner);
                let Type::Enum(enum_id) = inner_ty.without_nullable() else {
                    return MirExpr::Int(0);
                };
                let info = self.lowerer.types.enums[*enum_id as usize].clone();
                let (ok_idx, _, err_idx, _) = match (
                    info.members
                        .iter()
                        .position(|m| m.name == "Ok" && m.fields.len() == 1),
                    info.members
                        .iter()
                        .position(|m| m.name == "Err" && m.fields.len() == 1),
                ) {
                    (Some(ok), Some(err)) => (
                        ok,
                        info.members[ok].fields[0].clone(),
                        err,
                        info.members[err].fields[0].clone(),
                    ),
                    _ => return MirExpr::Int(0),
                };
                let ret_enum = match self.ret.without_nullable() {
                    Type::Enum(ret_id) => *ret_id,
                    _ => *enum_id,
                };
                let ret_info = self.lowerer.types.enums[ret_enum as usize].clone();
                let ret_err_tag = ret_info
                    .members
                    .iter()
                    .find(|m| m.name == "Err")
                    .map(|m| m.discriminant)
                    .unwrap_or(0);
                MirExpr::TryPropagate {
                    object: Box::new(self.lower_expr(inner)),
                    err_tag: info.members[err_idx].discriminant,
                    ret_err_tag,
                    elem: info.members[ok_idx].fields[0].clone(),
                }
            }
            ExprKind::MatchExpr { value, arms } => {
                let value_ty = self.expr_type(value);
                let Type::Enum(enum_id) = value_ty.without_nullable() else {
                    return MirExpr::Int(0);
                };
                let value_mir = self.lower_expr(value);
                let info = self.lowerer.types.enums[*enum_id as usize].clone();
                let mut mir_arms = Vec::new();
                for arm in arms {
                    match &arm.pattern {
                        Pattern::Wildcard(_) => {
                            mir_arms.push(MatchArmMir {
                                tag: None,
                                bindings: Vec::new(),
                                body: self.lower_expr(&arm.body),
                            });
                        }
                        Pattern::Variant { name, bindings, .. } => {
                            let Some(index) = info
                                .members
                                .iter()
                                .position(|member| member.name == name.name)
                            else {
                                continue;
                            };
                            let variant = &info.members[index];
                            self.name_scopes.push(HashMap::new());
                            let mut mir_bindings = Vec::new();
                            for (bind_index, binding) in bindings.iter().enumerate() {
                                let ty = variant
                                    .fields
                                    .get(bind_index)
                                    .cloned()
                                    .unwrap_or(Type::Error);
                                let local = self.declare_local(&binding.name, ty.clone(), false);
                                self.name_scopes
                                    .last_mut()
                                    .expect("作用域存在")
                                    .insert(binding.name.clone(), local);
                                mir_bindings.push((local, ty));
                            }
                            let body = self.lower_expr(&arm.body);
                            self.name_scopes.pop();
                            mir_arms.push(MatchArmMir {
                                tag: Some(variant.discriminant),
                                bindings: mir_bindings,
                                body,
                            });
                        }
                    }
                }
                MirExpr::MatchExpr {
                    value: Box::new(value_mir),
                    arms: mir_arms,
                    ret: self.expr_type(expr),
                }
            }
            ExprKind::Spread(inner) => {
                self.error("`...` 展开只能用于数组/对象字面量或调用参数", expr.span);
                self.lower_expr(inner)
            }
            ExprKind::Array(items) => {
                let elem = self
                    .result()
                    .expr_types
                    .get(&(expr.span.start, expr.span.end))
                    .cloned()
                    .unwrap_or(Type::Error);
                let elem = match elem {
                    Type::Array(inner) => *inner,
                    other => other,
                };
                let elem = substitute_type(&elem, &self.type_args);
                MirExpr::Array {
                    elem: Box::new(elem),
                    items: items
                        .iter()
                        .map(|item| match &item.kind {
                            ExprKind::Spread(inner) => {
                                MirExpr::ArraySpread(Box::new(self.lower_expr(inner)))
                            }
                            _ => self.lower_expr(item),
                        })
                        .collect(),
                }
            }
            ExprKind::Object(fields) => {
                let target = self
                    .result()
                    .object_types
                    .get(&expr.span.start)
                    .cloned()
                    .unwrap_or(Type::Error);
                let target = substitute_type(&target, &self.type_args);
                match target {
                    Type::Struct(id) => {
                        let field_names: Vec<String> = self.lowerer.types.structs[id as usize]
                            .fields
                            .iter()
                            .map(|field| field.name.clone())
                            .collect();
                        let mut field_values = Vec::new();
                        for field in fields {
                            if let Some(spread_expr) = &field.spread {
                                let spread_ty = self.expr_type(spread_expr);
                                if let Type::Struct(spread_id) = spread_ty.without_nullable() {
                                    if *spread_id == id {
                                        let spread_mir = self.lower_expr(spread_expr);
                                        for index in 0..field_names.len() {
                                            field_values.push((
                                                index,
                                                MirExpr::Field {
                                                    object: Box::new(spread_mir.clone()),
                                                    index,
                                                },
                                            ));
                                        }
                                    }
                                }
                                continue;
                            }
                            let name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            if let Some(index) = field_names
                                .iter()
                                .position(|field_name| *field_name == name)
                            {
                                let value = self.lower_expr(&field.value);
                                field_values.push((index, value));
                            }
                        }
                        MirExpr::Struct {
                            ty: Type::Struct(id),
                            fields: field_values,
                        }
                    }
                    _ => {
                        self.error("对象字面量降级只支持 struct", expr.span);
                        MirExpr::Null
                    }
                }
            }
            ExprKind::New { args, .. } => {
                let class = self
                    .result()
                    .new_types
                    .get(&expr.span.start)
                    .cloned()
                    .unwrap_or(Type::Error);
                let class = match class {
                    Type::Class(id) => id,
                    _ => {
                        self.error("new 目标不是 class", expr.span);
                        0
                    }
                };
                let ctor_sig = self
                    .lowerer
                    .types
                    .classes
                    .get(class as usize)
                    .and_then(|info| {
                        info.methods
                            .iter()
                            .find(|method| method.name == "constructor")
                            .map(|method| method.sig.clone())
                    })
                    .unwrap_or_else(placeholder_sig);
                MirExpr::New {
                    class,
                    sig: ctor_sig,
                    args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                }
            }
            ExprKind::Template(parts) => {
                let mut values = Vec::new();
                for part in parts {
                    match part {
                        TemplatePart::Text(text) => {
                            let index = self.lowerer.intern_string(text);
                            values.push(MirExpr::Str(index));
                        }
                        TemplatePart::Expr(expr) => {
                            let ty = self.expr_type(expr);
                            let value = self.lower_expr(expr);
                            let value = if ty == Type::Str {
                                value
                            } else {
                                let intrinsic = match ty {
                                    Type::Int | Type::Isize => "int_to_string",
                                    Type::UInt | Type::Usize => "uint_to_string",
                                    Type::Char => "char_to_string",
                                    Type::Bool => "bool_to_string",
                                    _ if ty.is_integer() => "int_to_string",
                                    _ => "float_to_string",
                                };
                                MirExpr::Call {
                                    callee: MirCallee::Intrinsic {
                                        name: intrinsic.to_owned(),
                                    },
                                    args: vec![value],
                                }
                            };
                            values.push(value);
                        }
                    }
                }
                let mut result = values.pop().unwrap_or_else(|| MirExpr::Str(0));
                while let Some(value) = values.pop() {
                    result = MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "string_concat".to_owned(),
                        },
                        args: vec![value, result],
                    };
                }
                result
            }
            ExprKind::Lambda { params, body } => {
                let lambda_type = self.expr_type(expr);
                let (lambda_params, lambda_ret) = match lambda_type {
                    Type::Function { params, ret } => (params, *ret),
                    other => {
                        self.error(format!("闭包类型未知：{}", other.display()), expr.span);
                        return MirExpr::Null;
                    }
                };
                let param_names: Vec<String> =
                    params.iter().map(|param| param.name.name.clone()).collect();
                let mut captures = Vec::new();
                let mut seen = HashSet::new();
                match body {
                    LambdaBody::Expr(inner) => {
                        self.collect_captures_expr(inner, &param_names, &mut captures, &mut seen);
                    }
                    LambdaBody::Block(block) => {
                        for statement in &block.statements {
                            self.collect_captures_stmt(
                                statement,
                                &param_names,
                                &mut captures,
                                &mut seen,
                            );
                        }
                    }
                }

                let hidden_name =
                    format!("sw_closure_{}_{}", self.lowerer.state.id.0, expr.span.start);
                let env_ty = Type::Ptr(Box::new(Type::I8));
                let mut hidden_params = vec![MirParam {
                    name: "$env".to_owned(),
                    ty: env_ty.clone(),
                }];
                for (index, param) in params.iter().enumerate() {
                    hidden_params.push(MirParam {
                        name: param.name.name.clone(),
                        ty: lambda_params.get(index).cloned().unwrap_or(Type::Error),
                    });
                }
                let mut hidden_sig = FunctionSig {
                    module: self.lowerer.state.id,
                    name: hidden_name.clone(),
                    generics: Vec::new(),
                    bounds: HashMap::new(),
                    params: Vec::new(),
                    ret: lambda_ret.clone(),
                    extern_c: false,
                    span: expr.span,
                };
                hidden_sig.params.push(ParamSig {
                    name: "$env".to_owned(),
                    ty: env_ty,
                    has_default: false,
                    rest: false,
                });
                for (index, param) in params.iter().enumerate() {
                    hidden_sig.params.push(ParamSig {
                        name: param.name.name.clone(),
                        ty: lambda_params[index].clone(),
                        has_default: false,
                        rest: false,
                    });
                }

                let capture_map: HashMap<u32, usize> = captures
                    .iter()
                    .enumerate()
                    .map(|(slot, (_, symbol_id))| (*symbol_id, slot))
                    .collect();
                let body_block = match body {
                    LambdaBody::Block(block) => block.clone(),
                    LambdaBody::Expr(inner) => Block {
                        statements: vec![Stmt {
                            kind: StmtKind::Return(Some(inner.as_ref().clone())),
                            span: inner.span,
                        }],
                        span: inner.span,
                    },
                };
                let lowerer = &mut *self.lowerer;
                let mut nested = FnLower {
                    lowerer,
                    result_index: self.result_index,
                    name: hidden_name.clone(),
                    user_name: String::new(),
                    exported: false,
                    params: hidden_params,
                    ret: lambda_ret,
                    locals: Vec::new(),
                    name_scopes: Vec::new(),
                    global_by_symbol: self.global_by_symbol.clone(),
                    this_class: None,
                    captures: capture_map,
                    type_args: self.type_args.clone(),
                };
                let hidden_function = nested.lower_function(Some(&body_block), false);
                self.lowerer.hidden_functions.push(hidden_function);

                let capture_values: Vec<MirExpr> = captures
                    .iter()
                    .filter_map(|(name, _)| self.lookup_local(name).map(MirExpr::Local))
                    .collect();
                MirExpr::ClosureNew {
                    name: hidden_name,
                    captures: capture_values,
                    sig: hidden_sig,
                }
            }
        }
    }

    fn collect_captures_expr(
        &self,
        expr: &Expr,
        lambda_params: &[String],
        out: &mut Vec<(String, u32)>,
        seen: &mut HashSet<u32>,
    ) {
        match &expr.kind {
            ExprKind::Ident(ident) => {
                if lambda_params.iter().any(|name| name == &ident.name) {
                    return;
                }
                let Some(symbol) = self.result().ident_symbols.get(&expr.span.start).copied()
                else {
                    return;
                };
                let is_local = matches!(
                    self.lowerer.symbol(symbol).kind,
                    SymbolKind::Local { .. } | SymbolKind::Param { .. }
                );
                if is_local && seen.insert(symbol.0) {
                    out.push((ident.name.clone(), symbol.0));
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.collect_captures_expr(left, lambda_params, out, seen);
                self.collect_captures_expr(right, lambda_params, out, seen);
            }
            ExprKind::Unary { expr: inner, .. }
            | ExprKind::Group(inner)
            | ExprKind::Cast { expr: inner, .. }
            | ExprKind::Postfix { expr: inner, .. } => {
                self.collect_captures_expr(inner, lambda_params, out, seen);
            }
            ExprKind::Assign { target, value, .. } => {
                self.collect_captures_expr(target, lambda_params, out, seen);
                self.collect_captures_expr(value, lambda_params, out, seen);
            }
            ExprKind::Conditional { cond, then, else_ } => {
                self.collect_captures_expr(cond, lambda_params, out, seen);
                self.collect_captures_expr(then, lambda_params, out, seen);
                self.collect_captures_expr(else_, lambda_params, out, seen);
            }
            ExprKind::Call { callee, args } => {
                self.collect_captures_expr(callee, lambda_params, out, seen);
                for arg in args {
                    self.collect_captures_expr(arg, lambda_params, out, seen);
                }
            }
            ExprKind::Member { object, .. } | ExprKind::Index { object, .. } => {
                self.collect_captures_expr(object, lambda_params, out, seen);
            }
            ExprKind::Slice { object, start, end } => {
                self.collect_captures_expr(object, lambda_params, out, seen);
                for bound in [start.as_deref(), end.as_deref()].into_iter().flatten() {
                    self.collect_captures_expr(bound, lambda_params, out, seen);
                }
            }
            ExprKind::Array(items) => {
                for item in items {
                    self.collect_captures_expr(item, lambda_params, out, seen);
                }
            }
            ExprKind::TryOp(inner) => {
                self.collect_captures_expr(inner, lambda_params, out, seen);
            }
            ExprKind::MatchExpr { value, arms } => {
                self.collect_captures_expr(value, lambda_params, out, seen);
                for arm in arms {
                    self.collect_captures_expr(&arm.body, lambda_params, out, seen);
                }
            }
            ExprKind::Spread(inner) => {
                self.collect_captures_expr(inner, lambda_params, out, seen);
            }
            ExprKind::Object(fields) => {
                for field in fields {
                    self.collect_captures_expr(&field.value, lambda_params, out, seen);
                }
            }
            ExprKind::New { args, .. } => {
                for arg in args {
                    self.collect_captures_expr(arg, lambda_params, out, seen);
                }
            }
            ExprKind::Template(parts) => {
                for part in parts {
                    if let TemplatePart::Expr(inner) = part {
                        self.collect_captures_expr(inner, lambda_params, out, seen);
                    }
                }
            }
            ExprKind::Lambda { .. }
            | ExprKind::Str(_)
            | ExprKind::Integer { .. }
            | ExprKind::Float { .. }
            | ExprKind::Char(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::This
            | ExprKind::Super => {}
        }
    }

    fn collect_captures_stmt(
        &self,
        statement: &Stmt,
        lambda_params: &[String],
        out: &mut Vec<(String, u32)>,
        seen: &mut HashSet<u32>,
    ) {
        match &statement.kind {
            StmtKind::Block(block) => {
                for statement in &block.statements {
                    self.collect_captures_stmt(statement, lambda_params, out, seen);
                }
            }
            StmtKind::Variable(variable) => {
                if let Some(init) = &variable.init {
                    self.collect_captures_expr(init, lambda_params, out, seen);
                }
            }
            StmtKind::If { cond, then, else_ } => {
                self.collect_captures_expr(cond, lambda_params, out, seen);
                self.collect_captures_stmt(then, lambda_params, out, seen);
                if let Some(else_) = else_ {
                    self.collect_captures_stmt(else_, lambda_params, out, seen);
                }
            }
            StmtKind::While { cond, body } => {
                self.collect_captures_expr(cond, lambda_params, out, seen);
                self.collect_captures_stmt(body, lambda_params, out, seen);
            }
            StmtKind::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(init) = init {
                    match init {
                        ForInit::Variable(variable) => {
                            if let Some(init) = &variable.init {
                                self.collect_captures_expr(init, lambda_params, out, seen);
                            }
                        }
                        ForInit::Expr(expr) => {
                            self.collect_captures_expr(expr, lambda_params, out, seen);
                        }
                    }
                }
                if let Some(cond) = cond {
                    self.collect_captures_expr(cond, lambda_params, out, seen);
                }
                if let Some(update) = update {
                    self.collect_captures_expr(update, lambda_params, out, seen);
                }
                self.collect_captures_stmt(body, lambda_params, out, seen);
            }
            StmtKind::ForEach { iterable, body, .. } => {
                self.collect_captures_expr(iterable, lambda_params, out, seen);
                self.collect_captures_stmt(body, lambda_params, out, seen);
            }
            StmtKind::Switch {
                value,
                cases,
                default,
            } => {
                self.collect_captures_expr(value, lambda_params, out, seen);
                for case in cases {
                    self.collect_captures_expr(&case.value, lambda_params, out, seen);
                    for statement in &case.body {
                        self.collect_captures_stmt(statement, lambda_params, out, seen);
                    }
                }
                if let Some(statements) = default {
                    for statement in statements {
                        self.collect_captures_stmt(statement, lambda_params, out, seen);
                    }
                }
            }
            StmtKind::Match { value, arms } => {
                self.collect_captures_expr(value, lambda_params, out, seen);
                for arm in arms {
                    for statement in &arm.body.statements {
                        self.collect_captures_stmt(statement, lambda_params, out, seen);
                    }
                }
            }
            StmtKind::Try {
                body,
                catches,
                finally,
            } => {
                for statement in &body.statements {
                    self.collect_captures_stmt(statement, lambda_params, out, seen);
                }
                for catch in catches {
                    for statement in &catch.body.statements {
                        self.collect_captures_stmt(statement, lambda_params, out, seen);
                    }
                }
                if let Some(finally) = finally {
                    for statement in &finally.statements {
                        self.collect_captures_stmt(statement, lambda_params, out, seen);
                    }
                }
            }
            StmtKind::Throw(expr) | StmtKind::Defer(expr) => {
                self.collect_captures_expr(expr, lambda_params, out, seen);
            }
            StmtKind::Return(expr) => {
                if let Some(expr) = expr {
                    self.collect_captures_expr(expr, lambda_params, out, seen);
                }
            }
            StmtKind::Expr(expr) => {
                self.collect_captures_expr(expr, lambda_params, out, seen);
            }
            StmtKind::Break | StmtKind::Continue | StmtKind::Empty => {}
        }
    }

    fn expr_type(&self, expr: &Expr) -> Type {
        // 成员访问表达式与对象标识符共用起始偏移，expr_types 会被覆盖；
        // Ident 直接从符号表取类型，避免碰撞。
        match &expr.kind {
            ExprKind::Str(_) => return Type::Str,
            ExprKind::Integer { .. } => return Type::Int,
            ExprKind::Float { suffix, .. } => {
                return if matches!(suffix, Some(sw_frontend::FloatSuffix::F32)) {
                    Type::F32
                } else {
                    Type::F64
                };
            }
            ExprKind::Bool(_) => return Type::Bool,
            ExprKind::Char(_) => return Type::Char,
            ExprKind::Null => return Type::Null,
            ExprKind::This => {
                if let Some(class_id) = self.this_class {
                    return Type::Class(class_id);
                }
            }
            ExprKind::Ident(_) => {
                if let Some(id) = self.result().ident_symbols.get(&expr.span.start).copied() {
                    if let Some(kind) = self.lowerer.symbols.get(id.0 as usize).map(|s| &s.kind) {
                        let ty = match kind {
                            SymbolKind::Local { ty, .. }
                            | SymbolKind::Param { ty }
                            | SymbolKind::Global { ty, .. } => Some(ty.clone()),
                            SymbolKind::Function(sig) => Some(Type::Function {
                                params: sig.params.iter().map(|param| param.ty.clone()).collect(),
                                ret: Box::new(sig.ret.clone()),
                            }),
                            _ => None,
                        };
                        if let Some(ty) = ty {
                            let resolved = substitute_type(&ty, &self.type_args);
                            return resolved;
                        }
                    }
                }
            }
            ExprKind::Member { object, name, .. } => {
                if name.name == "length" {
                    let object_ty = self.expr_type(object);
                    if matches!(object_ty, Type::Str | Type::Array(_)) {
                        return Type::Int;
                    }
                }
                let object_ty = self.expr_type(object);
                match object_ty.without_nullable() {
                    Type::Struct(id) => {
                        if let Some(field) = self
                            .lowerer
                            .types
                            .structs
                            .get(*id as usize)
                            .and_then(|info| info.fields.iter().find(|f| f.name == name.name))
                        {
                            return substitute_type(&field.ty, &self.type_args);
                        }
                    }
                    Type::Class(id) => {
                        if let Some((class_id, index)) =
                            self.lowerer.types.find_class_field(*id, &name.name)
                        {
                            if let Some(field) = self
                                .lowerer
                                .types
                                .classes
                                .get(class_id as usize)
                                .and_then(|info| info.fields.get(index))
                            {
                                return substitute_type(&field.ty, &self.type_args);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        self.result()
            .expr_types
            .get(&(expr.span.start, expr.span.end))
            .cloned()
            // 泛型实例化降级时，把记录的类型里的 TypeParam 用实例 type_args 替换
            // （如约束接口方法返回的 U → 具体类型），否则 codegen 收到 TypeParam 会拒。
            .map(|ty| substitute_type(&ty, &self.type_args))
            .unwrap_or(Type::Error)
    }

    /// 按对象类型 + 字段名解析字段目标（不依赖 span 表，避免嵌套成员链的起始偏移碰撞）。
    fn resolve_field_target(&self, object: &Expr, name: &str) -> Option<FieldTarget> {
        match self.expr_type(object).without_nullable() {
            Type::Struct(id) => self
                .lowerer
                .types
                .structs
                .get(*id as usize)
                .and_then(|info| info.fields.iter().position(|f| f.name == name))
                .map(|index| FieldTarget::Struct(*id, index)),
            Type::Class(id) => self
                .lowerer
                .types
                .find_class_field(*id, name)
                .map(|(class_id, index)| FieldTarget::Class(class_id, index)),
            _ => None,
        }
    }
}

impl<'m, 's> MirLowerer<'m, 's> {
    fn ancestor_field_count(&self, class_id: u32) -> usize {
        let chain = self.types.class_base_chain(class_id);
        chain
            .iter()
            .skip(1) // 展平顺序基类在前：跳过自身，统计全部基类字段
            .map(|id| self.types.classes[*id as usize].fields.len())
            .sum()
    }

    fn lower_type_for_mir(&mut self, ty: &TypeRef) -> Type {
        let mut resolver = TypeResolver::new(
            self.symbols,
            &mut *self.types,
            self.registry,
            &self.state.names,
        );
        resolver.lower(ty, &[])
    }
}

fn stable_function_name(sig: &FunctionSig, module_stem: &str) -> String {
    if sig.name == "main" {
        // 入口由运行时 main 调用，用户 main 改名为 sw_user_main。
        return "sw_user_main".to_owned();
    }
    // 固定名：模块文件名 + 函数名；重载用参数类型缩写消歧（不依赖源码位置）。
    let abbrev: String = sig
        .params
        .iter()
        .map(|param| type_abbrev(&param.ty))
        .collect::<Vec<_>>()
        .join("");
    let mut name = format!("sw_fn_{module_stem}_{}", sig.name);
    if !abbrev.is_empty() {
        name.push('_');
        name.push_str(&abbrev);
    }
    name
}

/// 参数类型 → 稳定缩写（用于重载消歧）。
fn type_abbrev(ty: &Type) -> String {
    match ty.without_nullable() {
        Type::Int
        | Type::UInt
        | Type::I8
        | Type::I16
        | Type::I32
        | Type::I64
        | Type::Isize
        | Type::U8
        | Type::U16
        | Type::U32
        | Type::U64
        | Type::Usize => "i".to_string(),
        Type::F32 | Type::F64 => "f".to_string(),
        Type::Str => "s".to_string(),
        Type::Bool => "b".to_string(),
        Type::Char => "c".to_string(),
        Type::Array(inner) => format!("a{}", type_abbrev(inner)),
        Type::Void => "v".to_string(),
        _ => "o".to_string(),
    }
}

fn mir_binary(op: &BinaryOp) -> MirBinary {
    match op {
        BinaryOp::Add => MirBinary::Add,
        BinaryOp::Sub => MirBinary::Sub,
        BinaryOp::Mul => MirBinary::Mul,
        BinaryOp::Div => MirBinary::Div,
        BinaryOp::Rem => MirBinary::Rem,
        BinaryOp::Pow => MirBinary::Pow,
        BinaryOp::Eq => MirBinary::Eq,
        BinaryOp::Ne => MirBinary::Ne,
        BinaryOp::Lt => MirBinary::Lt,
        BinaryOp::Le => MirBinary::Le,
        BinaryOp::Gt => MirBinary::Gt,
        BinaryOp::Ge => MirBinary::Ge,
        BinaryOp::And => MirBinary::And,
        BinaryOp::Or => MirBinary::Or,
        BinaryOp::Coalesce => MirBinary::Coalesce,
        BinaryOp::BitAnd => MirBinary::BitAnd,
        BinaryOp::BitOr => MirBinary::BitOr,
        BinaryOp::BitXor => MirBinary::BitXor,
        BinaryOp::Shl => MirBinary::Shl,
        BinaryOp::Shr => MirBinary::Shr,
    }
}
