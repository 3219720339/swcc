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

#[derive(Default)]
struct CheckResult {
    expr_types: HashMap<usize, Type>,
    ident_symbols: HashMap<usize, SymbolId>,
    call_targets: HashMap<usize, CallTarget>,
    field_targets: HashMap<usize, FieldTarget>,
    new_types: HashMap<usize, Type>,
    object_types: HashMap<usize, Type>,
    /// 函数声明起始偏移 → 所属类（方法）。
    method_classes: HashMap<usize, u32>,
}

#[derive(Clone, Debug)]
enum CallTarget {
    Function(SymbolId),
    Method { class: u32, index: usize },
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum FieldTarget {
    Struct(u32, usize),
    Class(u32, usize),
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

    fn alloc_symbol(&mut self) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            kind: SymbolKind::Global {
                ty: Type::Error,
                mutable: false,
            },
            exported: false,
            span: Span::empty(0),
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
        self.finalize(id)?;
        self.module_names.insert(id, self.state(id).names.clone());
        self.check_globals(id);
        self.check_all_functions(id);
        self.lower_mir(id);

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
                                final_: class.final_,
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
                    let id = self.alloc_symbol();
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
            let id = self.alloc_symbol();
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
                ImportKind::SideEffect => {}
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
                    let id = self.alloc_symbol();
                    self.symbols.push(Symbol {
                        kind: SymbolKind::Namespace(target_id),
                        exported: false,
                        span: alias.span,
                    });
                    self.bind_import(module_id, alias.name.clone(), vec![id], alias.span)?;
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
                    }
                }
                ItemKind::Enum(enumeration) => {
                    let id = self.type_id_for(module_id, &enumeration.name.name);
                    let mut members = Vec::new();
                    let mut next_value = 0i64;
                    for member in &enumeration.members {
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
                        members.push((member.name.name.clone(), value));
                        next_value = value + 1;
                    }
                    if let Some(SymbolKind::Type(SymbolType::Enum(id))) = id {
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
                        let resolver = TypeResolver::new(
                            &self.symbols,
                            &self.types,
                            &self.registry,
                            &self.state(module_id).names,
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
                    let (fields, methods) = self.finalize_class_members(
                        module_id,
                        class,
                        &generics,
                        class_id.unwrap_or(0),
                    );
                    if let Some(id) = class_id {
                        let info = &mut self.types.classes[id as usize];
                        info.fields = fields;
                        info.methods = methods;
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
                    let resolver = TypeResolver::new(
                        &self.symbols,
                        &self.types,
                        &self.registry,
                        &self.state(module_id).names,
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
                    let sig = self.build_function_sig(module_id, function, &generics, None);
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
                        let resolver = TypeResolver::new(
                            &self.symbols,
                            &self.types,
                            &self.registry,
                            &self.state(module_id).names,
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
        let resolver = TypeResolver::new(
            &self.symbols,
            &self.types,
            &self.registry,
            &self.state(module_id).names,
        );
        fields
            .iter()
            .map(|field| FieldInfo {
                name: field.name.name.clone(),
                ty: resolver.lower(&field.ty, generics),
                mutable: !field.modifiers.contains(&MemberModifier::Final),
                span: field.span,
            })
            .collect()
    }

    fn finalize_class_members(
        &mut self,
        module_id: ModuleId,
        class: &ClassDecl,
        generics: &[String],
        class_id: u32,
    ) -> (Vec<FieldInfo>, Vec<MethodInfo>) {
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        for member in &class.members {
            match member {
                ClassMember::Field(field) => {
                    let resolver = TypeResolver::new(
                        &self.symbols,
                        &self.types,
                        &self.registry,
                        &self.state(module_id).names,
                    );
                    fields.push(FieldInfo {
                        name: field.name.name.clone(),
                        ty: resolver.lower(&field.ty, generics),
                        mutable: !field.modifiers.contains(&MemberModifier::Final),
                        span: field.span,
                    });
                }
                ClassMember::Method(function) => {
                    let sig =
                        self.build_function_sig(module_id, function, generics, Some(class_id));
                    methods.push(MethodInfo {
                        name: function.name.name.clone(),
                        sig,
                        virtual_: false,
                        override_: false,
                        span: function.span,
                    });
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
        (fields, methods)
    }

    fn build_function_sig(
        &mut self,
        module_id: ModuleId,
        function: &FunctionDecl,
        generics: &[String],
        this_class: Option<u32>,
    ) -> FunctionSig {
        let resolver = TypeResolver::new(
            &self.symbols,
            &self.types,
            &self.registry,
            &self.state(module_id).names,
        );
        let params = function
            .params
            .iter()
            .map(|param| ParamSig {
                name: param.name.name.clone(),
                ty: resolver.lower(&param.ty, generics),
                has_default: param.default.is_some(),
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
        let resolver = TypeResolver::new(
            &self.symbols,
            &self.types,
            &self.registry,
            &self.state(module_id).names,
        );
        let params = constructor
            .params
            .iter()
            .map(|param| ParamSig {
                name: param.name.name.clone(),
                ty: resolver.lower(&param.ty, generics),
                has_default: param.default.is_some(),
            })
            .collect();
        FunctionSig {
            module: module_id,
            name: "constructor".to_owned(),
            generics: generics.to_vec(),
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
                            let sig = class_info
                                .methods
                                .iter()
                                .find(|method| method.name == method_name)
                                .map(|method| method.sig.clone())
                                .unwrap_or_else(|| FunctionSig {
                                    module: module_id,
                                    name: method_name.clone(),
                                    generics: Vec::new(),
                                    params: Vec::new(),
                                    ret: Type::Void,
                                    extern_c: false,
                                    span,
                                });
                            self.check_function_body(
                                module_id,
                                span,
                                Some(class_id),
                                &sig,
                                &generics,
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
        body: &Block,
    ) {
        let symbols = &mut self.symbols;
        let types = &self.types;
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
            generics: generics.to_vec(),
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
        let types = &self.types;
        let registry = &self.registry;
        let diagnostics = &mut self.diagnostics;
        let state = &mut self.states[module_id.0 as usize];
        let mir = {
            let mut lowerer = MirLowerer {
                module,
                symbols,
                types,
                registry,
                diagnostics,
                state,
                global_index_by_symbol: HashMap::new(),
                hidden_functions: Vec::new(),
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
    types: &'a TypeTable,
    registry: &'a HashMap<String, Type>,
    names: &'a HashMap<String, Vec<SymbolId>>,
}

impl<'a> TypeResolver<'a> {
    fn new(
        symbols: &'a [Symbol],
        types: &'a TypeTable,
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

    fn lower(&self, ty: &TypeRef, generics: &[String]) -> Type {
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
        &self,
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
                            SymbolType::Struct(id) => Type::Struct(*id),
                            SymbolType::Enum(id) => Type::Enum(*id),
                            SymbolType::Class(id) => Type::Class(*id),
                            SymbolType::Interface(id) => Type::Interface(*id),
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
}

// ---------------------------------------------------------------------------
// 类型检查器
// ---------------------------------------------------------------------------

struct Checker<'s> {
    symbols: &'s mut Vec<Symbol>,
    types: &'s TypeTable,
    registry: &'s HashMap<String, Type>,
    module_names: &'s HashMap<ModuleId, HashMap<String, Vec<SymbolId>>>,
    diagnostics: &'s mut Diagnostics,
    state: &'s mut ModuleState,
    scopes: Vec<HashMap<String, SymbolId>>,
    ret: Type,
    this_class: Option<u32>,
    loop_depth: usize,
    generics: Vec<String>,
    saw_return_value: bool,
}

impl<'s> Checker<'s> {
    fn error(&mut self, message: impl Into<String>, span: Span) {
        let file = self.state.path.clone();
        self.diagnostics.error_at(message, Some(span), Some(file));
    }

    fn alloc_local(&mut self) -> SymbolId {
        let id = SymbolId(self.symbols.len() as u32);
        self.symbols.push(Symbol {
            kind: SymbolKind::Param { ty: Type::Error },
            exported: false,
            span: Span::empty(0),
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
        let resolver =
            TypeResolver::new(self.symbols, self.types, self.registry, &self.state.names);
        resolver.lower(ty, &self.generics)
    }

    fn is_assignable(&self, from: &Type, to: &Type) -> bool {
        if matches!(from, Type::Error | Type::Unknown) {
            return true;
        }
        if from == to {
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
            (Type::Array(from_inner), Type::Array(to_inner)) => {
                self.is_assignable(from_inner, to_inner)
            }
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
            StmtKind::Break | StmtKind::Continue => {
                if self.loop_depth == 0 {
                    self.error("break/continue 只能在循环内使用", statement.span);
                }
            }
            StmtKind::Return(expr) => match expr {
                Some(expr) => {
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
        let ty = if let Some(annotation) = &variable.ty {
            let ty = self.lower_type(annotation);
            if let Some(init) = &variable.init {
                if matches!(init.kind, ExprKind::Object(_)) {
                    self.state
                        .result
                        .object_types
                        .insert(init.span.start, ty.clone());
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
            .insert(expr.span.start, ty.clone());
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
            ExprKind::Postfix { expr: inner, .. } => {
                let ty = self.check_expr(inner);
                if !ty.is_numeric() || !self.is_lvalue(inner) {
                    self.error("`++`/`--` 需要可赋值的数值目标", inner.span);
                }
                ty
            }
            ExprKind::Array(items) => {
                let mut element = None;
                for item in items {
                    let ty = self.check_expr(item);
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
                        let info = &self.types.structs[id as usize];
                        for field in fields {
                            let field_name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            if !info.fields.iter().any(|f| f.name == field_name) {
                                self.error(
                                    format!("结构体 {} 没有字段 `{field_name}`", info.name),
                                    expr.span,
                                );
                            }
                            self.check_expr(&field.value);
                        }
                        Type::Struct(id)
                    }
                    Type::Class(id) => {
                        let info = &self.types.classes[id as usize];
                        for field in fields {
                            let field_name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            if self.types.find_class_field(id, &field_name).is_none() {
                                self.error(
                                    format!("类 {} 没有字段 `{field_name}`", info.name),
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
                        let args_ty: Vec<Type> =
                            args.iter().map(|arg| self.check_expr(arg)).collect();
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

    fn check_binary(&mut self, op: &BinaryOp, left: &Expr, right: &Expr, span: Span) -> Type {
        let left_ty = self.check_expr(left);
        let right_ty = self.check_expr(right);
        let (left_ty, right_ty) = self.adapt_literal_operands(op, left, right, left_ty, right_ty);
        match op {
            BinaryOp::Add => {
                if left_ty == Type::Str && right_ty == Type::Str {
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
        let object_ty = self.check_expr(object);
        let base = object_ty.without_nullable().clone();
        let result = match &base {
            Type::Class(id) => {
                if let Some((_, index)) = self.types.find_class_field(*id, &name.name) {
                    self.state
                        .result
                        .field_targets
                        .insert(span.start, FieldTarget::Class(*id, index));
                    let (class, _) = self.types.find_class_field(*id, &name.name).unwrap();
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
            Type::Enum(id) => {
                let has_member = self
                    .types
                    .enums
                    .get(*id as usize)
                    .map(|info| info.members.iter().any(|(m, _)| m == &name.name))
                    .unwrap_or(false);
                if has_member {
                    Type::Enum(*id)
                } else {
                    self.error(
                        format!(
                            "枚举 {} 没有成员 `{}`",
                            self.types.enum_name(*id),
                            name.name
                        ),
                        name.span,
                    );
                    Type::Error
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
        // 命名空间函数调用：ns.foo(...)
        if let ExprKind::Member { object, name, .. } = &callee.kind {
            if let ExprKind::Ident(ns_ident) = &object.kind {
                if let Some(SymbolId(id)) = self.lookup(&ns_ident.name) {
                    if let SymbolKind::Namespace(target) = &self.symbols[id as usize].kind {
                        let target_names =
                            self.module_names.get(target).cloned().unwrap_or_default();
                        if let Some(symbol_ids) = target_names.get(&name.name) {
                            let args_ty: Vec<Type> =
                                args.iter().map(|arg| self.check_expr(arg)).collect();
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
                                self.state
                                    .result
                                    .call_targets
                                    .insert(span.start, CallTarget::Function(symbol_id));
                                return sig.ret;
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
                                .insert(ident.span.start, symbol_type.clone());
                            let args_ty: Vec<Type> =
                                args.iter().map(|arg| self.check_expr(arg)).collect();
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
                            return (**ret).clone();
                        }
                    }
                }
                let args_ty: Vec<Type> = args.iter().map(|arg| self.check_expr(arg)).collect();
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
                        .insert(span.start, CallTarget::Function(symbol_id));
                    return sig.ret;
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
                let args_ty: Vec<Type> = args.iter().map(|arg| self.check_expr(arg)).collect();
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
                    span.start,
                    CallTarget::Method {
                        class: base,
                        index: constructor,
                    },
                );
                Type::Class(base)
            }
            ExprKind::Member { object, name, .. } => {
                let object_ty = self.check_expr(object);
                let args_ty: Vec<Type> = args.iter().map(|arg| self.check_expr(arg)).collect();
                match object_ty.without_nullable() {
                    Type::Class(id) => {
                        let Some((class_id, index)) = self.types.find_class_method(*id, &name.name)
                        else {
                            self.error(
                                format!(
                                    "类 {} 没有方法 `{}`",
                                    self.types.class_name(*id),
                                    name.name
                                ),
                                name.span,
                            );
                            return Type::Error;
                        };
                        let sig = self.types.classes[class_id as usize].methods[index]
                            .sig
                            .clone();
                        self.match_call_args(&sig, &args_ty, span, true);
                        self.state.result.call_targets.insert(
                            span.start,
                            CallTarget::Method {
                                class: class_id,
                                index,
                            },
                        );
                        sig.ret
                    }
                    Type::Error => Type::Error,
                    other => {
                        self.error(
                            format!("类型 {} 不能调用方法", other.display()),
                            callee.span,
                        );
                        Type::Error
                    }
                }
            }
            _ => {
                self.error("调用目标必须是函数名或方法", callee.span);
                Type::Error
            }
        }
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
            let required = sig.params.iter().filter(|param| !param.has_default).count();
            if args.len() < required {
                continue;
            }
            if !allow_defaults && args.len() != sig.params.len() {
                continue;
            }
            if args.len() > sig.params.len() {
                continue;
            }
            let mut mismatches = 0usize;
            let mut exact = 0usize;
            let mut ok = true;
            let mut type_args = HashMap::new();
            for (index, arg) in args.iter().enumerate() {
                let param_ty = self.substitute(&sig.params[index].ty, &type_args);
                if self.is_assignable(arg, &param_ty) {
                    exact += usize::from(arg == &param_ty);
                } else if let Some(inferred) =
                    self.infer_type_arg(&sig.params[index].ty, arg, &type_args)
                {
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
    symbols: &'s [Symbol],
    types: &'s TypeTable,
    registry: &'s HashMap<String, Type>,
    diagnostics: &'s mut Diagnostics,
    state: &'s mut ModuleState,
    global_index_by_symbol: HashMap<u32, usize>,
    hidden_functions: Vec<MirFunction>,
}

struct FnLower<'a, 'm, 's> {
    lowerer: &'a mut MirLowerer<'m, 's>,
    name: String,
    params: Vec<MirParam>,
    ret: Type,
    locals: Vec<MirLocal>,
    name_scopes: Vec<HashMap<String, usize>>,
    global_by_symbol: HashMap<u32, usize>,
    this_class: Option<u32>,
    /// 闭包隐藏函数内：符号 ID → 环境槽序号。
    captures: HashMap<u32, usize>,
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
                });
                self.global_index_by_symbol
                    .insert(symbol_id.0, index as usize);
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
                    let name = stable_function_name(&sig);
                    let mut lower = FnLower {
                        lowerer: self,
                        name: name.clone(),
                        params: Vec::new(),
                        ret: sig.ret.clone(),
                        locals: Vec::new(),
                        name_scopes: Vec::new(),
                        global_by_symbol: global_by_symbol.clone(),
                        this_class: None,
                        captures: HashMap::new(),
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
                let mut method_index = 0usize;
                for member in &class.members {
                    let (body, name, sig) = match member {
                        ClassMember::Method(function) => {
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
                    let mut lower = FnLower {
                        lowerer: self,
                        name: name.clone(),
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
        }

        for function in std::mem::take(&mut self.hidden_functions) {
            module_mir.functions.push(function);
        }
        module_mir.strings = self.state.mir_strings.clone();
        module_mir
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
    fn error(&mut self, message: impl Into<String>, span: Span) {
        self.lowerer.error(message, span);
    }

    fn declare_local(&mut self, name: &str, ty: Type, mutable: bool) -> usize {
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
                return ty;
            }
        }
        if let Some(init) = &variable.init {
            return self.expr_type(init);
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
                let element_ty = match element_ty {
                    Type::Array(inner) => *inner,
                    _ => Type::Error,
                };
                let element_local = self.declare_local(&name.name, element_ty, true);
                self.name_scopes
                    .last_mut()
                    .expect("作用域存在")
                    .insert(name.name.clone(), element_local);
                let index_local = self.declare_local("$index", Type::Int, true);
                let index_expr = MirExpr::Local(index_local);
                let mut body_stmts = Vec::new();
                body_stmts.push(MirStmt::new(MirStmtKind::Assign {
                    target: MirTarget::Local(element_local),
                    value: MirExpr::Index {
                        object: Box::new(array.clone()),
                        index: Box::new(index_expr.clone()),
                    },
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
                let mut chain: Vec<MirStmt> = Vec::new();
                for case in cases {
                    let case_value = self.lower_expr(&case.value);
                    let body = self.lower_stmts(&case.body);
                    let cond = MirExpr::Binary {
                        op: MirBinary::Eq,
                        left: Box::new(value.clone()),
                        right: Box::new(case_value),
                    };
                    chain.push(MirStmt::new(MirStmtKind::If {
                        cond,
                        then: body,
                        else_: Vec::new(),
                    }));
                }
                if let Some(default) = default {
                    let body = self.lower_stmts(default);
                    if let Some(last) = chain.last_mut() {
                        if let MirStmtKind::If { else_, .. } = &mut last.kind {
                            *else_ = body;
                        }
                    } else {
                        chain.extend(body);
                    }
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
                                    params: vec![ParamSig {
                                        name: "buf".to_owned(),
                                        ty: Type::Ptr(Box::new(Type::I8)),
                                        has_default: false,
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
                    .lowerer
                    .state
                    .result
                    .expr_types
                    .get(&expr.span.start)
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

    fn lower_expr_stmt(&mut self, expr: &Expr, output: &mut Vec<MirStmt>) {
        if let Some((target, value)) = self.lower_assign_expr(expr) {
            output.push(MirStmt::new(MirStmtKind::Assign { target, value }));
            return;
        }
        let mir = self.lower_expr(expr);
        output.push(MirStmt::new(MirStmtKind::Expr(mir)));
    }

    fn lower_assign_expr(&mut self, expr: &Expr) -> Option<(MirTarget, MirExpr)> {
        match &expr.kind {
            ExprKind::Assign { op, target, value } => {
                let target_ast = target;
                let target = self.lower_target(target)?;
                let value = self.lower_expr(value);
                let value = match op {
                    AssignOp::Assign => value,
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

    fn lower_target(&mut self, expr: &Expr) -> Option<MirTarget> {
        match &expr.kind {
            ExprKind::Ident(ident) => {
                if let Some(local) = self.lookup_local(&ident.name) {
                    return Some(MirTarget::Local(local));
                }
                if let Some(symbol) = self
                    .lowerer
                    .state
                    .result
                    .ident_symbols
                    .get(&expr.span.start)
                    .copied()
                {
                    if let Some(global) = self.global_by_symbol.get(&symbol.0) {
                        return Some(MirTarget::Global(*global as u32));
                    }
                }
                None
            }
            ExprKind::Member { object, name, .. } => {
                let field = self
                    .lowerer
                    .state
                    .result
                    .field_targets
                    .get(&expr.span.start)
                    .cloned()?;
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
            }),
            _ => None,
        }
    }

    fn lower_value(&mut self, expr: &Expr) -> MirExpr {
        self.lower_expr(expr)
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
                if let Some(symbol) = self
                    .lowerer
                    .state
                    .result
                    .ident_symbols
                    .get(&expr.span.start)
                    .copied()
                {
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
                let to = self.lowerer.lower_type_for_mir(ty);
                MirExpr::Cast {
                    expr: Box::new(self.lower_expr(inner)),
                    to,
                }
            }
            ExprKind::Unary { op, expr: inner } => {
                let mir_op = match op {
                    UnaryOp::Not => MirUnary::Not,
                    UnaryOp::Neg => MirUnary::Neg,
                    UnaryOp::Pos => MirUnary::Pos,
                    UnaryOp::BitNot => MirUnary::BitNot,
                    UnaryOp::Inc | UnaryOp::Dec => {
                        self.error("`++`/`--` 表达式降级暂不支持，请用作语句", expr.span);
                        MirUnary::Inc
                    }
                    UnaryOp::Await => {
                        self.error("await 降级暂不支持", expr.span);
                        MirUnary::Not
                    }
                };
                MirExpr::Unary {
                    op: mir_op,
                    expr: Box::new(self.lower_expr(inner)),
                }
            }
            ExprKind::Binary { op, left, right } => {
                if *op == BinaryOp::Add && self.expr_type(left) == Type::Str {
                    let left = self.lower_expr(left);
                    let right = self.lower_expr(right);
                    return MirExpr::Call {
                        callee: MirCallee::Intrinsic {
                            name: "string_concat".to_owned(),
                        },
                        args: vec![left, right],
                    };
                }
                MirExpr::Binary {
                    op: mir_binary(op),
                    left: Box::new(self.lower_expr(left)),
                    right: Box::new(self.lower_expr(right)),
                }
            }
            ExprKind::Assign { .. } => {
                self.error("赋值表达式降级暂不支持，请用作语句", expr.span);
                MirExpr::Int(0)
            }
            ExprKind::Conditional { cond, then, else_ } => MirExpr::Select {
                cond: Box::new(self.lower_expr(cond)),
                then: Box::new(self.lower_expr(then)),
                else_: Box::new(self.lower_expr(else_)),
            },
            ExprKind::Call { callee, args } => {
                // 闭包调用：callee 是 lambda，或 callee 是函数类型的局部/参数/全局
                let closure_ty = match &callee.kind {
                    ExprKind::Lambda { .. } => self.expr_type(callee),
                    ExprKind::Ident(_) => {
                        let symbol = self
                            .lowerer
                            .state
                            .result
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
                        params: vec![ParamSig {
                            name: "$env".to_owned(),
                            ty: Type::Ptr(Box::new(Type::I8)),
                            has_default: false,
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
                        });
                    }
                    return MirExpr::Call {
                        callee: MirCallee::Closure { sig },
                        args: values,
                    };
                }
                let target = self
                    .lowerer
                    .state
                    .result
                    .call_targets
                    .get(&expr.span.start)
                    .cloned();
                let mut args: Vec<MirExpr> = args.iter().map(|arg| self.lower_expr(arg)).collect();
                let callee = match target {
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
                        } else {
                            MirCallee::Function {
                                module: sig.module.0,
                                name: stable_function_name(&sig),
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
                            // super(...) 基类构造函数调用：无接收者
                            ExprKind::Super => {}
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
                    None => {
                        self.error("调用目标未解析", expr.span);
                        MirCallee::Intrinsic {
                            name: "unresolved".to_owned(),
                        }
                    }
                };
                MirExpr::Call { callee, args }
            }
            ExprKind::Member { object, name, .. } => {
                if name.name == "length"
                    && matches!(self.expr_type(object), Type::Array(_) | Type::Str)
                {
                    return MirExpr::Len {
                        object: Box::new(self.lower_expr(object)),
                    };
                }
                let field = self
                    .lowerer
                    .state
                    .result
                    .field_targets
                    .get(&expr.span.start)
                    .cloned();
                match field {
                    Some(FieldTarget::Struct(_, index)) => MirExpr::Field {
                        object: Box::new(self.lower_expr(object)),
                        index,
                    },
                    Some(FieldTarget::Class(class_id, index)) => MirExpr::Field {
                        object: Box::new(self.lower_expr(object)),
                        index: self.lowerer.ancestor_field_count(class_id) + index,
                    },
                    None => {
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
                                            .find(|(member, _)| member == &name.name)
                                            .map(|(_, value)| *value)
                                    });
                            if let Some(value) = value {
                                return MirExpr::Int(value);
                            }
                        }
                        self.error("成员访问未解析", expr.span);
                        MirExpr::Int(0)
                    }
                }
            }
            ExprKind::Index { object, index, .. } => MirExpr::Index {
                object: Box::new(self.lower_expr(object)),
                index: Box::new(self.lower_expr(index)),
            },
            ExprKind::Postfix { .. } => {
                self.error("`++`/`--` 表达式降级暂不支持，请用作语句", expr.span);
                MirExpr::Int(0)
            }
            ExprKind::Array(items) => {
                let elem = self
                    .lowerer
                    .state
                    .result
                    .expr_types
                    .get(&expr.span.start)
                    .cloned()
                    .unwrap_or(Type::Error);
                let elem = match elem {
                    Type::Array(inner) => *inner,
                    other => other,
                };
                MirExpr::Array {
                    elem: Box::new(elem),
                    items: items.iter().map(|item| self.lower_expr(item)).collect(),
                }
            }
            ExprKind::Object(fields) => {
                let target = self
                    .lowerer
                    .state
                    .result
                    .object_types
                    .get(&expr.span.start)
                    .cloned()
                    .unwrap_or(Type::Error);
                match target {
                    Type::Struct(id) => {
                        let info = &self.lowerer.types.structs[id as usize];
                        let mut field_values = Vec::new();
                        for field in fields {
                            let name = match &field.key {
                                ObjectKey::Ident(ident) => ident.name.clone(),
                                ObjectKey::Str(value) => value.clone(),
                            };
                            if let Some(index) = info.fields.iter().position(|f| f.name == name) {
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
                    .lowerer
                    .state
                    .result
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
                MirExpr::New {
                    class,
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
                    params: Vec::new(),
                    ret: lambda_ret.clone(),
                    extern_c: false,
                    span: expr.span,
                };
                hidden_sig.params.push(ParamSig {
                    name: "$env".to_owned(),
                    ty: env_ty,
                    has_default: false,
                });
                for (index, param) in params.iter().enumerate() {
                    hidden_sig.params.push(ParamSig {
                        name: param.name.name.clone(),
                        ty: lambda_params[index].clone(),
                        has_default: false,
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
                    name: hidden_name.clone(),
                    params: hidden_params,
                    ret: lambda_ret,
                    locals: Vec::new(),
                    name_scopes: Vec::new(),
                    global_by_symbol: self.global_by_symbol.clone(),
                    this_class: None,
                    captures: capture_map,
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
                let Some(symbol) = self
                    .lowerer
                    .state
                    .result
                    .ident_symbols
                    .get(&expr.span.start)
                    .copied()
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
            ExprKind::Array(items) => {
                for item in items {
                    self.collect_captures_expr(item, lambda_params, out, seen);
                }
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
        self.lowerer
            .state
            .result
            .expr_types
            .get(&expr.span.start)
            .cloned()
            .unwrap_or(Type::Error)
    }
}

impl<'m, 's> MirLowerer<'m, 's> {
    fn ancestor_field_count(&self, class_id: u32) -> usize {
        let chain = self.types.class_base_chain(class_id);
        chain
            .iter()
            .take_while(|id| **id != class_id)
            .map(|id| self.types.classes[*id as usize].fields.len())
            .sum()
    }

    fn lower_type_for_mir(&self, ty: &TypeRef) -> Type {
        let resolver =
            TypeResolver::new(self.symbols, self.types, self.registry, &self.state.names);
        resolver.lower(ty, &[])
    }
}

fn stable_function_name(sig: &FunctionSig) -> String {
    if sig.name == "main" {
        return "main".to_owned();
    }
    format!("sw_fn_{}_{}_{}", sig.module.0, sig.name, sig.span.start)
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
