//! Cranelift 后端：MIR → 机器码 → ELF/COFF/Mach-O 目标文件。
//!
//! 标量统一按 64 位表示（整数/布尔/字符/指针），浮点按 64 位；
//! 字符串/数组/对象以堆指针形式传递（运行时 ABI 见 runtime/runtime.c）。

use std::collections::HashMap;
use std::sync::Arc;

use cranelift_codegen::ir::{
    AbiParam, Block, InstBuilder, MemFlagsData, Signature, StackSlot, StackSlotData, StackSlotKind,
    UserFuncName, Value,
    condcodes::{FloatCC, IntCC},
    types,
};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{DataDescription, Init, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use sw_semantic::symbols::FunctionSig;
use sw_semantic::types::Type;
use sw_semantic::{
    MirBinary, MirCallee, MirExpr, MirFunction, MirGlobal, MirModule, MirParam, MirStmt,
    MirStmtKind, MirTarget, MirUnary, TypeTable,
};
use target_lexicon::Triple;

/// 可变参数运行时类型标签（与 runtime.c 中 SW_TAG_* 保持一致）。
const SW_TAG_FLOAT: i64 = 1;

#[derive(Debug)]
pub struct CodegenError {
    pub message: String,
}

impl From<String> for CodegenError {
    fn from(message: String) -> Self {
        Self { message }
    }
}

impl From<&str> for CodegenError {
    fn from(message: &str) -> Self {
        Self {
            message: message.to_owned(),
        }
    }
}

pub fn compile_module(mir: &MirModule, types: &TypeTable) -> Result<Vec<u8>, CodegenError> {
    let generator = Generator::new(None)?;
    generator.run(mir, types)
}

/// 按目标 triple 编译：x86_64/aarch64 的 Windows/Linux/macOS 均可生成对应对象文件。
pub fn compile_module_for_target(
    mir: &MirModule,
    types: &TypeTable,
    target: &str,
) -> Result<Vec<u8>, CodegenError> {
    let generator = Generator::new(Some(target))?;
    generator.run(mir, types)
}

struct Generator {
    module: ObjectModule,
    imports: HashMap<String, cranelift_module::FuncId>,
    string_data: HashMap<usize, cranelift_module::DataId>,
    global_data: HashMap<u32, cranelift_module::DataId>,
    /// class id → vtable 数据（类对象头部引用）。
    vtable_data: HashMap<u32, cranelift_module::DataId>,
}

impl Generator {
    fn new(target: Option<&str>) -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").unwrap();
        flag_builder.set("is_pic", "true").unwrap();
        let flags = settings::Flags::new(flag_builder);
        let isa_builder = match target {
            Some(target) => {
                let triple: Triple = target
                    .parse()
                    .map_err(|error| format!("无效的目标 triple `{target}`：{error}"))?;
                cranelift_codegen::isa::lookup(triple).map_err(|error| error.to_string())?
            }
            None => cranelift_native::builder().map_err(|error| error.to_string())?,
        };
        let isa: Arc<dyn cranelift_codegen::isa::TargetIsa> = isa_builder
            .finish(flags)
            .map_err(|error| error.to_string())?;
        let builder = ObjectBuilder::new(
            isa,
            "sw_module".to_owned(),
            cranelift_module::default_libcall_names(),
        )
        .map_err(|error| error.to_string())?;
        let module = ObjectModule::new(builder);
        Ok(Self {
            module,
            imports: HashMap::new(),
            string_data: HashMap::new(),
            global_data: HashMap::new(),
            vtable_data: HashMap::new(),
        })
    }

    fn run(mut self, mir: &MirModule, types: &TypeTable) -> Result<Vec<u8>, CodegenError> {
        // 字符串字面量数据
        for (index, text) in mir.strings.iter().enumerate() {
            let data_id = self
                .module
                .declare_data(
                    format!("sw_str_{index}").as_str(),
                    Linkage::Local,
                    false,
                    false,
                )
                .map_err(|error| error.to_string())?;
            let mut description = DataDescription::new();
            description.init = Init::Bytes {
                contents: text.as_bytes().to_vec().into_boxed_slice(),
            };
            self.module
                .define_data(data_id, &description)
                .map_err(|error| error.to_string())?;
            self.string_data.insert(index, data_id);
        }

        // 全局变量
        for (index, global) in mir.globals.iter().enumerate() {
            self.declare_global(index as u32, global, mir.module_id)?;
        }

        // 先声明全部导出函数，保证互相调用（含递归）都能解析
        let mut exports: HashMap<String, cranelift_module::FuncId> = HashMap::new();
        for function in &mir.functions {
            if function.extern_c {
                continue;
            }
            let sig = signature_of(&function.params, &function.ret, self.module.isa())?;
            let func_id = self
                .module
                .declare_function(&function.name, Linkage::Export, &sig)
                .map_err(|error| error.to_string())?;
            exports.insert(function.name.clone(), func_id);
        }

        // 接口 vtable：按全局接口槽位布局生成每个类的派发表。
        let (interface_slot_bases, interface_slot_total) = interface_slot_bases(types);
        if interface_slot_total > 0 {
            for (class_id, class) in types.classes.iter().enumerate() {
                let data_id = self
                    .module
                    .declare_data(
                        format!("sw_vt_{class_id}").as_str(),
                        Linkage::Local,
                        false,
                        false,
                    )
                    .map_err(|error| error.to_string())?;
                let mut desc = DataDescription::new();
                desc.define(vec![0u8; interface_slot_total * 8].into_boxed_slice());
                let mut inherited_interfaces = Vec::new();
                let mut current = Some(class_id as u32);
                while let Some(id) = current {
                    if let Some(interfaces) = types.class_interfaces.get(&id) {
                        for interface_id in interfaces {
                            if !inherited_interfaces.contains(interface_id) {
                                inherited_interfaces.push(*interface_id);
                            }
                        }
                    }
                    current = types.classes[id as usize].base;
                }
                for interface_id in inherited_interfaces {
                    let Some(&base) = interface_slot_bases.get(&interface_id) else {
                        continue;
                    };
                    let interface = &types.interfaces[interface_id as usize];
                    for (method_index, method) in interface.methods.iter().enumerate() {
                        if let Some((impl_class, impl_index)) =
                            types.find_class_method(class_id as u32, &method.name)
                        {
                            let fn_name = format!("sw_m_{impl_class}_{impl_index}_{}", method.name);
                            if let Some(func_id) = exports.get(&fn_name) {
                                let func_ref =
                                    self.module.declare_func_in_data(*func_id, &mut desc);
                                desc.write_function_addr(
                                    ((base + method_index) * 8) as u32,
                                    func_ref,
                                );
                            }
                        }
                    }
                }
                self.module
                    .define_data(data_id, &desc)
                    .map_err(|error| error.to_string())?;
                self.vtable_data.insert(class_id as u32, data_id);
            }
        }

        // 定义函数体
        for function in &mir.functions {
            if function.extern_c {
                continue;
            }
            let func_id = exports.get(&function.name).copied().ok_or("函数未声明")?;
            self.define_function(mir, types, function, func_id, &exports)?;
        }

        let product = self.module.finish();
        Ok(product.object.write().map_err(|error| error.to_string())?)
    }

    fn declare_global(
        &mut self,
        index: u32,
        global: &MirGlobal,
        current_module: u32,
    ) -> Result<(), CodegenError> {
        // 本模块定义的全局导出为可链接符号；跨模块引用声明为 Import 外部数据。
        let defining = global.module == current_module;
        let data_id = self
            .module
            .declare_data(
                global.name.as_str(),
                if defining {
                    Linkage::Export
                } else {
                    Linkage::Import
                },
                global.mutable,
                false,
            )
            .map_err(|error| error.to_string())?;
        if defining {
            let mut description = DataDescription::new();
            let bytes = global
                .init
                .as_ref()
                .and_then(const_i64)
                .map(|value| value.to_le_bytes().to_vec())
                .unwrap_or_else(|| vec![0u8; 8]);
            description.init = Init::Bytes {
                contents: bytes.into_boxed_slice(),
            };
            self.module
                .define_data(data_id, &description)
                .map_err(|error| error.to_string())?;
        }
        self.global_data.insert(index, data_id);
        Ok(())
    }

    fn declare_import(
        &mut self,
        name: &str,
        sig: &Signature,
    ) -> Result<cranelift_module::FuncId, CodegenError> {
        if let Some(func_id) = self.imports.get(name) {
            return Ok(*func_id);
        }
        let func_id = self
            .module
            .declare_function(name, Linkage::Import, sig)
            .map_err(|error| error.to_string())?;
        self.imports.insert(name.to_owned(), func_id);
        Ok(func_id)
    }

    fn define_function(
        &mut self,
        mir: &MirModule,
        types: &TypeTable,
        function: &MirFunction,
        func_id: cranelift_module::FuncId,
        exports: &HashMap<String, cranelift_module::FuncId>,
    ) -> Result<(), CodegenError> {
        let sig = signature_of(&function.params, &function.ret, self.module.isa())?;
        let mut ctx = self.module.make_context();
        ctx.func.signature = sig;
        ctx.func.name = UserFuncName::user(0, func_id.as_u32());

        // 预声明函数体内引用的外部符号
        let mut refs = RefTable::default();
        collect_refs(self, mir, function, exports, &mut ctx, &mut refs)?;

        let mut func_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let entry = builder.create_block();
        builder.switch_to_block(entry);
        builder.append_block_params_for_function_params(entry);

        // MirFunction.locals 已包含参数（FnLower 把参数 declare_local 进同一张表），
        // MirExpr::Local 的 index 直接对应 locals 顺序；不能把 params 再拼一遍，
        // 否则 index >= 参数个数的局部类型错位（潜伏 bug，float 局部在含参数
        // 函数中会被当 int/其它类型）。
        let local_types: Vec<Type> = function
            .locals
            .iter()
            .map(|local| local.ty.clone())
            .collect();

        let struct_layout = struct_layout(types);
        let class_layout = class_layout(types, &struct_layout.1);
        let mut lower = LowerCtx {
            builder,
            refs,
            slots: Vec::new(),
            loops: Vec::new(),
            module: &self.module,
            types,
            class_field_counts: class_field_counts(types),
            struct_field_offsets: struct_layout.0,
            struct_sizes: struct_layout.1,
            class_field_offsets: class_layout.0,
            class_sizes: class_layout.1,
            struct_field_types: struct_field_types(types),
            class_field_types: class_field_types(types),
            vtable_data: self.vtable_data.clone(),
            interface_slot_bases: interface_slot_bases(types).0,
            local_types,
            ret_ty: function.ret.clone(),
            sret: None,
            strings: &mir.strings,
            last_terminated: false,
        };

        // 参数入槽
        let param_values = lower.builder.block_params(entry).to_vec();
        let has_sret = is_struct_ret(&function.ret);
        if has_sret {
            lower.sret = Some(param_values[0]);
        }
        let param_offset = usize::from(has_sret);
        for (index, param) in function.params.iter().enumerate() {
            let slot = lower.slot_for(index);
            let value = param_values[index + param_offset];
            if matches!(param.ty, Type::Struct(_)) {
                let address = lower.builder.ins().stack_addr(types::I64, slot, 0);
                lower.copy_struct(&param.ty, value, address)?;
            } else if matches!(param.ty, Type::F32 | Type::F64) {
                lower.builder.ins().stack_store(types::I64, value, slot, 0);
            } else {
                lower.builder.ins().stack_store(types::I64, value, slot, 0);
            }
        }
        lower.stmts(&function.body)?;
        // 空函数体兜底：补一个 void 返回，避免“块未填充”。
        if !lower.last_terminated {
            if function.ret == Type::Void
                || function.ret == Type::Unknown
                || is_struct_ret(&function.ret)
            {
                lower.builder.ins().return_(&[]);
            } else if function.ret.is_float() {
                let zero = lower.builder.ins().f64const(0.0);
                lower.builder.ins().return_(&[zero]);
            } else {
                let zero = lower.builder.ins().iconst(types::I64, 0);
                lower.builder.ins().return_(&[zero]);
            }
        }
        lower.builder.seal_all_blocks();
        lower.builder.finalize(self.module.isa().frontend_config());
        ctx.verify(self.module.isa()).map_err(|error| {
            CodegenError::from(format!("函数 {} 的 IR 校验失败：{error}", function.name))
        })?;

        Ok(self
            .module
            .define_function(func_id, &mut ctx)
            .map_err(|error| error.to_string())?)
    }
}

#[derive(Default)]
struct RefTable {
    func_refs: HashMap<String, cranelift_codegen::ir::FuncRef>,
    global_refs: HashMap<u32, cranelift_codegen::ir::GlobalValue>,
    string_refs: HashMap<usize, cranelift_codegen::ir::GlobalValue>,
    closure_sig_refs: HashMap<String, cranelift_codegen::ir::SigRef>,
}

/// 预扫描函数体，把引用的外部符号（调用目标、字符串、全局）声明进 Context。
fn collect_refs(
    generator: &mut Generator,
    mir: &MirModule,
    function: &MirFunction,
    exports: &HashMap<String, cranelift_module::FuncId>,
    ctx: &mut cranelift_codegen::Context,
    refs: &mut RefTable,
) -> Result<(), CodegenError> {
    for statement in &function.body {
        visit_stmt(statement, generator, mir, exports, ctx, refs)?;
    }
    Ok(())
}

fn visit_stmt(
    statement: &MirStmt,
    generator: &mut Generator,
    mir: &MirModule,
    exports: &HashMap<String, cranelift_module::FuncId>,
    ctx: &mut cranelift_codegen::Context,
    refs: &mut RefTable,
) -> Result<(), CodegenError> {
    match &statement.kind {
        MirStmtKind::VarDecl { init, .. } => {
            if let Some(expr) = init {
                visit_expr(expr, generator, mir, exports, ctx, refs)?;
            }
            Ok(())
        }
        MirStmtKind::Assign { target, value } => {
            visit_target(target, generator, mir, exports, ctx, refs)?;
            visit_expr(value, generator, mir, exports, ctx, refs)
        }
        MirStmtKind::If { cond, then, else_ } => {
            visit_expr(cond, generator, mir, exports, ctx, refs)?;
            for statement in then {
                visit_stmt(statement, generator, mir, exports, ctx, refs)?;
            }
            for statement in else_ {
                visit_stmt(statement, generator, mir, exports, ctx, refs)?;
            }
            Ok(())
        }
        MirStmtKind::While { cond, body } => {
            visit_expr(cond, generator, mir, exports, ctx, refs)?;
            for statement in body {
                visit_stmt(statement, generator, mir, exports, ctx, refs)?;
            }
            Ok(())
        }
        MirStmtKind::Return(value) => {
            if let Some(expr) = value {
                visit_expr(expr, generator, mir, exports, ctx, refs)?;
            }
            Ok(())
        }
        MirStmtKind::Expr(expr) => visit_expr(expr, generator, mir, exports, ctx, refs),
        MirStmtKind::Break | MirStmtKind::Continue => Ok(()),
    }
}

fn visit_target(
    target: &MirTarget,
    generator: &mut Generator,
    mir: &MirModule,
    exports: &HashMap<String, cranelift_module::FuncId>,
    ctx: &mut cranelift_codegen::Context,
    refs: &mut RefTable,
) -> Result<(), CodegenError> {
    match target {
        MirTarget::Local(_) | MirTarget::Global(_) => Ok(()),
        MirTarget::Field { object, .. } | MirTarget::Index { object, .. } => {
            visit_expr(object, generator, mir, exports, ctx, refs)
        }
    }
}

fn visit_expr(
    expr: &MirExpr,
    generator: &mut Generator,
    mir: &MirModule,
    exports: &HashMap<String, cranelift_module::FuncId>,
    ctx: &mut cranelift_codegen::Context,
    refs: &mut RefTable,
) -> Result<(), CodegenError> {
    match expr {
        MirExpr::Str(id) => {
            let data_id = generator
                .string_data
                .get(id)
                .copied()
                .ok_or("字符串未声明")?;
            let gv = generator
                .module
                .declare_data_in_func(data_id, &mut ctx.func);
            refs.string_refs.insert(*id, gv);
            let sig = string_from_literal_signature(generator.module.isa());
            let func_id = generator.declare_import("sw_string_from_literal", &sig)?;
            let func_ref = generator
                .module
                .declare_func_in_func(func_id, &mut ctx.func);
            refs.func_refs
                .insert("sw_string_from_literal".to_owned(), func_ref);
        }
        MirExpr::Global(index) => {
            let data_id = generator
                .global_data
                .get(index)
                .copied()
                .ok_or("全局未声明")?;
            let gv = generator
                .module
                .declare_data_in_func(data_id, &mut ctx.func);
            refs.global_refs.insert(*index, gv);
        }
        MirExpr::Call { callee, args } => {
            // 接口方法经 vtable 间接调用，无需直接符号导入。
            if let MirCallee::InterfaceMethod { .. } = callee {
                for arg in args {
                    visit_expr(arg, generator, mir, exports, ctx, refs)?;
                }
                return Ok(());
            }
            if let MirCallee::Closure { sig } = callee {
                let key = format!("{:?}", sig);
                let cranelift_sig = signature_of_sig(sig, generator.module.isa())?;
                let sig_ref = ctx.func.import_signature(cranelift_sig);
                refs.closure_sig_refs.insert(key, sig_ref);
            } else {
                let (name, sig) = callee_signature(callee, generator)?;
                let func_id = if let Some(func_id) = exports.get(&name) {
                    *func_id
                } else {
                    generator.declare_import(&name, &sig)?
                };
                let func_ref = generator
                    .module
                    .declare_func_in_func(func_id, &mut ctx.func);
                refs.func_refs.insert(name.clone(), func_ref);
            }
            for arg in args {
                visit_expr(arg, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::ClosureNew { name, captures, .. } => {
            if let Some(func_id) = exports.get(name) {
                refs.func_refs.insert(
                    name.clone(),
                    generator
                        .module
                        .declare_func_in_func(*func_id, &mut ctx.func),
                );
            }
            let closure_new = generator.declare_import(
                "sw_closure_new",
                &closure_new_signature(generator.module.isa()),
            )?;
            let env_set = generator
                .declare_import("sw_env_set", &env_set_signature(generator.module.isa()))?;
            refs.func_refs.insert(
                "sw_closure_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(closure_new, &mut ctx.func),
            );
            refs.func_refs.insert(
                "sw_env_set".to_owned(),
                generator
                    .module
                    .declare_func_in_func(env_set, &mut ctx.func),
            );
            for capture in captures {
                visit_expr(capture, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::EnvGet { .. } => {}
        MirExpr::Unary { expr: inner, .. }
        | MirExpr::Cast { expr: inner, .. }
        | MirExpr::Len { object: inner, .. }
        | MirExpr::Field { object: inner, .. }
        | MirExpr::Index { object: inner, .. } => {
            visit_expr(inner, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::Binary { left, right, .. } => {
            visit_expr(left, generator, mir, exports, ctx, refs)?;
            visit_expr(right, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::Select { cond, then, else_ } => {
            visit_expr(cond, generator, mir, exports, ctx, refs)?;
            visit_expr(then, generator, mir, exports, ctx, refs)?;
            visit_expr(else_, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::Assign { target, value } => {
            visit_target(target, generator, mir, exports, ctx, refs)?;
            visit_expr(value, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::Postfix { target, .. } => {
            visit_target(target, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::Array { items, .. } => {
            let array_new = generator
                .declare_import("sw_array_new", &array_new_signature(generator.module.isa()))?;
            let array_set = generator
                .declare_import("sw_array_set", &array_set_signature(generator.module.isa()))?;
            let array_set_u8 = generator.declare_import(
                "sw_array_set_u8",
                &array_set_signature(generator.module.isa()),
            )?;
            refs.func_refs.insert(
                "sw_array_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(array_new, &mut ctx.func),
            );
            refs.func_refs.insert(
                "sw_array_set_u8".to_owned(),
                generator
                    .module
                    .declare_func_in_func(array_set_u8, &mut ctx.func),
            );
            refs.func_refs.insert(
                "sw_array_set".to_owned(),
                generator
                    .module
                    .declare_func_in_func(array_set, &mut ctx.func),
            );
            for item in items {
                visit_expr(item, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::VarArgs(items) => {
            let array_new = generator
                .declare_import("sw_array_new", &array_new_signature(generator.module.isa()))?;
            let array_set = generator
                .declare_import("sw_array_set", &array_set_signature(generator.module.isa()))?;
            refs.func_refs.insert(
                "sw_array_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(array_new, &mut ctx.func),
            );
            refs.func_refs.insert(
                "sw_array_set".to_owned(),
                generator
                    .module
                    .declare_func_in_func(array_set, &mut ctx.func),
            );
            for (_, item) in items {
                visit_expr(item, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                visit_expr(value, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::New { class, sig, args } => {
            let object_new = generator.declare_import(
                "sw_object_new",
                &object_new_signature(generator.module.isa()),
            )?;
            refs.func_refs.insert(
                "sw_object_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(object_new, &mut ctx.func),
            );
            let ctor_name = format!("sw_ctor_{class}");
            if let Some(func_id) = exports.get(&ctor_name) {
                refs.func_refs.insert(
                    ctor_name.clone(),
                    generator
                        .module
                        .declare_func_in_func(*func_id, &mut ctx.func),
                );
            } else {
                // 跨模块：构造函数在其它模块中定义，按签名声明为导入（链接时解析）。
                // 构造函数 MIR 参数 = [self, ...用户参数]。
                let mut ctor_params = vec![sw_semantic::symbols::ParamSig {
                    name: "self".to_owned(),
                    ty: Type::Class(*class),
                    has_default: false,
                    rest: false,
                }];
                ctor_params.extend(sig.params.iter().cloned());
                let full_sig = FunctionSig {
                    params: ctor_params,
                    ..sig.clone()
                };
                let cranelift_sig = signature_of_sig(&full_sig, generator.module.isa())?;
                let func_id = generator.declare_import(&ctor_name, &cranelift_sig)?;
                refs.func_refs.insert(
                    ctor_name,
                    generator
                        .module
                        .declare_func_in_func(func_id, &mut ctx.func),
                );
            }
            for arg in args {
                visit_expr(arg, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::EnumNew { fields, .. } => {
            let object_new = generator.declare_import(
                "sw_object_new",
                &object_new_signature(generator.module.isa()),
            )?;
            refs.func_refs.insert(
                "sw_object_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(object_new, &mut ctx.func),
            );
            for field in fields {
                visit_expr(field, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::EnumTag { object } | MirExpr::EnumField { object, .. } => {
            visit_expr(object, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::TryPropagate { object, .. } => {
            let object_new = generator.declare_import(
                "sw_object_new",
                &object_new_signature(generator.module.isa()),
            )?;
            refs.func_refs.insert(
                "sw_object_new".to_owned(),
                generator
                    .module
                    .declare_func_in_func(object_new, &mut ctx.func),
            );
            visit_expr(object, generator, mir, exports, ctx, refs)?;
        }
        MirExpr::ArrayMap {
            object,
            closure,
            sig,
            ..
        }
        | MirExpr::ArrayFilter {
            object,
            closure,
            sig,
            ..
        } => {
            for (name, sig) in [
                ("sw_array_new", array_new_signature(generator.module.isa())),
                ("sw_array_set", array_set_signature(generator.module.isa())),
            ] {
                let func_id = generator.declare_import(name, &sig)?;
                refs.func_refs.insert(
                    name.to_owned(),
                    generator
                        .module
                        .declare_func_in_func(func_id, &mut ctx.func),
                );
            }
            let key = format!("{:?}", sig);
            let cranelift_sig = signature_of_sig(sig, generator.module.isa())?;
            let sig_ref = ctx.func.import_signature(cranelift_sig);
            refs.closure_sig_refs.insert(key, sig_ref);
            visit_expr(object, generator, mir, exports, ctx, refs)?;
            visit_expr(closure, generator, mir, exports, ctx, refs)?;
        }
        _ => {}
    }
    Ok(())
}

fn callee_signature(
    callee: &MirCallee,
    generator: &Generator,
) -> Result<(String, Signature), CodegenError> {
    callee_signature_for(callee, generator.module.isa())
}

fn callee_signature_for(
    callee: &MirCallee,
    isa: &dyn cranelift_codegen::isa::TargetIsa,
) -> Result<(String, Signature), CodegenError> {
    Ok(match callee {
        MirCallee::Function { name, sig, .. } => (name.clone(), signature_of_sig(sig, isa)?),
        MirCallee::Method {
            class, name, sig, ..
        } => {
            // 跨模块方法：导入签名需手动补 self 接收者（MIR 方法签名不含 self）。
            let mut full_params = vec![sw_semantic::symbols::ParamSig {
                name: "self".to_owned(),
                ty: Type::Class(*class),
                has_default: false,
                rest: false,
            }];
            full_params.extend(sig.params.iter().cloned());
            let full_sig = FunctionSig {
                params: full_params,
                ..sig.clone()
            };
            (name.clone(), signature_of_sig(&full_sig, isa)?)
        }
        MirCallee::Extern { name, sig } => (
            extern_c_symbol(name).to_owned(),
            signature_of_sig(sig, isa)?,
        ),
        MirCallee::Intrinsic { name } => {
            let runtime_name = intrinsic_name(name);
            let sig = intrinsic_signature(runtime_name, isa);
            (runtime_name.to_owned(), sig)
        }
        MirCallee::Closure { sig } => ("$closure".to_owned(), signature_of_sig(sig, isa)?),
        MirCallee::InterfaceMethod { sig, .. } => {
            ("$iface".to_owned(), signature_of_sig(sig, isa)?)
        }
    })
}

fn closure_new_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn env_set_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig
}

fn intrinsic_name(name: &str) -> &str {
    match name {
        "string_concat" => "sw_string_concat",
        "pow_f64" => "sw_pow_f64",
        "pow_i64" => "sw_pow_i64",
        "frem_f64" => "sw_frem_f64",
        "string_eq" => "string_eq",
        "string_ne" => "string_ne",
        "string_char_at" => "utf8_char_at",
        "string_char_len" => "utf8_len",
        "int_to_string" => "sw_int_to_string",
        "uint_to_string" => "sw_uint_to_string",
        "float_to_string" => "sw_float_to_string",
        "char_to_string" => "sw_char_to_string",
        "bool_to_string" => "sw_bool_to_string",
        "array_slice" => "sw_array_slice",
        _ if name.starts_with("sw_") => name,
        _ => "sw_unimplemented",
    }
}

/// extern c 函数 → 实际链接符号名。
/// 标准库的 extern 声明沿用 Sw 侧名字（open/close/write 等），但静态链接时若与
/// libc 同名，会把 libc 内部调用劫持到我们的包装函数（例如 musl 的 opendir 内部
/// 调用 open()，被 Sw 的 open(sw_string*, sw_string*) 接住后按野指针解引用崩溃）。
/// 这里统一映射成 sw_ 前缀的唯一符号，运行时侧对应改名。新增 extern 标准库函数
/// 时若与 libc 符号重名，需在本表登记。
fn extern_c_symbol(name: &str) -> &str {
    match name {
        "open" => "sw_open",
        "close" => "sw_close",
        "write" => "sw_write",
        "abs" => "sw_abs",
        "mkdir" => "sw_mkdir",
        "rename" => "sw_rename",
        "remove" => "sw_remove",
        "getenv" => "sw_getenv",
        "spawn" => "sw_spawn",
        "wait" => "sw_wait",
        "poll" => "sw_poll",
        "kill" => "sw_kill",
        "run" => "sw_run",
        "run_with_input" => "sw_run_with_input",
        "run_status" => "sw_run_status",
        "platform" => "sw_platform",
        "base64_encode" => "sw_base64_encode",
        "base64_decode" => "sw_base64_decode",
        "hex_encode" => "sw_hex_encode",
        "hex_decode" => "sw_hex_decode",
        "url_encode" => "sw_url_encode",
        "url_decode" => "sw_url_decode",
        "ends_with" => "sw_ends_with",
        "trim_left" => "sw_trim_left",
        "trim_right" => "sw_trim_right",
        "lines" => "sw_lines",
        "split_whitespace" => "sw_split_whitespace",
        "count" => "sw_count",
        "last_index_of" => "sw_last_index_of",
        "chars" => "sw_chars",
        "from_utf8_bytes" => "sw_from_utf8_bytes",
        "to_utf8_bytes" => "sw_to_utf8_bytes",
        "is_ascii" => "sw_is_ascii",
        "escape" => "sw_escape",
        "unescape" => "sw_unescape",
        "sign" => "sw_sign",
        "rand_float" => "sw_rand_float",
        "rand_range" => "sw_rand_range",
        "pi" => "sw_pi",
        "e" => "sw_e",
        "time_format" => "sw_time_format",
        "time_from_parts" => "sw_time_from_parts",
        "timezone_offset_sec" => "sw_timezone_offset_sec",
        "eprintln" => "sw_eprintln",
        "eprint" => "sw_eprint",
        "read_all_stdin" => "sw_read_all_stdin",
        "cwd" => "sw_cwd",
        "chdir" => "sw_chdir",
        "temp_dir" => "sw_temp_dir",
        "home_dir" => "sw_home_dir",
        "hostname" => "sw_hostname",
        "cpu_count" => "sw_cpu_count",
        "env_keys" => "sw_env_keys",
        "setenv" => "sw_setenv",
        "file_size_path" => "sw_file_size_path",
        "file_mtime" => "sw_file_mtime",
        "is_file" => "sw_is_file",
        "chmod" => "sw_chmod",
        "touch" => "sw_touch",
        "copy_dir" => "sw_copy_dir",
        "remove_all" => "sw_remove_all",
        "glob" => "sw_glob",
        "unsetenv" => "sw_unsetenv",
        "desktop_dir" => "sw_desktop_dir",
        "documents_dir" => "sw_documents_dir",
        "downloads_dir" => "sw_downloads_dir",
        "pictures_dir" => "sw_pictures_dir",
        "music_dir" => "sw_music_dir",
        "videos_dir" => "sw_videos_dir",
        "config_dir" => "sw_config_dir",
        "system_dir" => "sw_system_dir",
        "username" => "sw_username",
        "pid" => "sw_pid",
        "arch" => "sw_arch",
        "path_absolute" => "sw_path_absolute",
        "path_normalize" => "sw_path_normalize",
        "is_absolute" => "sw_is_absolute",
        "path_parts" => "sw_path_parts",
        "expand_home" => "sw_expand_home",
        "mkdir_p" => "sw_mkdir_p",
        "disk_free" => "sw_disk_free",
        "disk_total" => "sw_disk_total",
        "is_symlink" => "sw_is_symlink",
        "read_symlink" => "sw_read_symlink",
        "file_mode" => "sw_file_mode",
        "is_empty" => "sw_is_empty",
        "utf8_is_valid" => "sw_utf8_is_valid",
        "truncate" => "sw_truncate",
        "ellipsis" => "sw_ellipsis",
        "deg_to_rad" => "sw_deg_to_rad",
        "rad_to_deg" => "sw_rad_to_deg",
        "is_nan" => "sw_is_nan",
        "is_infinite" => "sw_is_infinite",
        "tau" => "sw_tau",
        "parse_datetime" => "sw_parse_datetime",
        "base64url_encode" => "sw_base64url_encode",
        "base64url_decode" => "sw_base64url_decode",
        "html_escape" => "sw_html_escape",
        "sort_int" => "sw_sort_int",
        "sort_float" => "sw_sort_float",
        "sort_string" => "sw_sort_string",
        "reverse_int" => "sw_reverse_int",
        "reverse_float" => "sw_reverse_float",
        "reverse_string" => "sw_reverse_string",
        "min_int" => "sw_min_int",
        "max_int" => "sw_max_int",
        "sum_int" => "sw_sum_int",
        "min_float" => "sw_min_float",
        "max_float" => "sw_max_float",
        "sum_float" => "sw_sum_float",
        "unique_string" => "sw_unique_string",
        "map_new" => "sw_map_new",
        "map_set" => "sw_map_set",
        "map_get" => "sw_map_get",
        "map_has" => "sw_map_has",
        "map_remove" => "sw_map_remove",
        "map_len" => "sw_map_len",
        "map_keys" => "sw_map_keys",
        "net_connect" => "sw_net_connect",
        "net_send" => "sw_net_send",
        "net_recv" => "sw_net_recv",
        "net_close" => "sw_net_close",
        "net_listen" => "sw_net_listen",
        "net_accept" => "sw_net_accept",
        "net_port" => "sw_net_port",
        _ => name,
    }
}

fn intrinsic_signature(name: &str, isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    match name {
        "sw_string_concat" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_try_begin" => {
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_security_cookie" | "sw_func_name_addr" => {
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_try_value" | "sw_exception_type" | "sw_exception_value" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_try_leave" | "sw_rethrow" => {
            sig.params.push(AbiParam::new(types::I64));
        }
        "sw_throw" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
        }
        "sw_closure_new" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_env_set" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
        }
        "sw_env_get" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_pow_f64" => {
            sig.params.push(AbiParam::new(types::F64));
            sig.params.push(AbiParam::new(types::F64));
            sig.returns.push(AbiParam::new(types::F64));
        }
        "sw_frem_f64" => {
            sig.params.push(AbiParam::new(types::F64));
            sig.params.push(AbiParam::new(types::F64));
            sig.returns.push(AbiParam::new(types::F64));
        }
        "sw_pow_i64" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "string_eq" | "string_ne" | "utf8_char_at" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_array_slice" => {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "sw_float_to_string" => {
            sig.params.push(AbiParam::new(types::F64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        _ => {
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
    }
    sig
}

fn string_from_literal_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn signature_of(
    params: &[sw_semantic::MirParam],
    ret: &Type,
    isa: &dyn cranelift_codegen::isa::TargetIsa,
) -> Result<Signature, CodegenError> {
    let mut sig = Signature::new(isa.default_call_conv());
    // 结构体返回值：隐藏 sret 指针参数，函数本身返回 void。
    if is_struct_ret(ret) {
        sig.params.push(AbiParam::new(types::I64));
    }
    for param in params {
        sig.params.push(AbiParam::new(abi_type(&param.ty)?));
    }
    if *ret != Type::Void && *ret != Type::Unknown && !is_struct_ret(ret) {
        sig.returns.push(AbiParam::new(abi_type(ret)?));
    }
    Ok(sig)
}

fn signature_of_sig(
    sig: &FunctionSig,
    isa: &dyn cranelift_codegen::isa::TargetIsa,
) -> Result<Signature, CodegenError> {
    let mut cranelift_sig = Signature::new(isa.default_call_conv());
    if is_struct_ret(&sig.ret) {
        cranelift_sig.params.push(AbiParam::new(types::I64));
    }
    for param in &sig.params {
        cranelift_sig
            .params
            .push(AbiParam::new(abi_type(&param.ty)?));
    }
    if sig.ret != Type::Void && sig.ret != Type::Unknown && !is_struct_ret(&sig.ret) {
        cranelift_sig
            .returns
            .push(AbiParam::new(abi_type(&sig.ret)?));
    }
    Ok(cranelift_sig)
}

/// 标量统一按 64 位表示；浮点按 64 位；指针/引用按 64 位；
/// struct 参数按地址传递（调用方持有副本，被调方入口复制进自己的槽）。
fn abi_type(ty: &Type) -> Result<cranelift_codegen::ir::Type, CodegenError> {
    Ok(match ty {
        Type::F32 | Type::F64 => types::F64,
        Type::Any => types::I64,
        Type::Void => return Err("void 不能作为参数或返回值类型".into()),
        Type::Struct(_) | Type::Interface(_) => types::I64,
        Type::TypeParam(_) | Type::Unknown | Type::Error => {
            return Err(format!("后端暂不支持类型 {}", ty.display()).into());
        }
        _ => types::I64,
    })
}

fn is_struct_ret(ret: &Type) -> bool {
    matches!(ret, Type::Struct(_))
}

fn const_i64(expr: &MirExpr) -> Option<i64> {
    match expr {
        MirExpr::Int(value) => Some(*value),
        MirExpr::UInt(value) => Some(*value as i64),
        MirExpr::Bool(value) => Some(i64::from(*value)),
        MirExpr::Char(value) => Some(*value as i64),
        MirExpr::Unary {
            op: MirUnary::Neg,
            expr,
        } => const_i64(expr).map(|value| -value),
        MirExpr::Unary {
            op: MirUnary::Pos,
            expr,
        } => const_i64(expr),
        MirExpr::Binary { op, left, right } => {
            let (left, right) = (const_i64(left)?, const_i64(right)?);
            match op {
                MirBinary::Add => left.checked_add(right),
                MirBinary::Sub => left.checked_sub(right),
                MirBinary::Mul => left.checked_mul(right),
                MirBinary::Div => left.checked_div(right),
                MirBinary::Rem => left.checked_rem(right),
                _ => None,
            }
        }
        _ => None,
    }
}

fn class_field_counts(types: &TypeTable) -> HashMap<u32, usize> {
    types
        .classes
        .iter()
        .enumerate()
        .map(|(index, class)| {
            let own = class.fields.len();
            let mut count = own;
            let mut base = class.base;
            while let Some(id) = base {
                count += types.classes[id as usize].fields.len();
                base = types.classes[id as usize].base;
            }
            (index as u32, count)
        })
        .collect()
}

/// struct 布局：每个字段的字节偏移（标量 8 字节，嵌套 struct 内联其大小）。
/// 返回 (字段偏移表, 总大小表)。
fn struct_layout(types: &TypeTable) -> (HashMap<u32, Vec<usize>>, HashMap<u32, usize>) {
    let mut offsets: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    let total = types.structs.len();
    let mut resolved = vec![false; total];
    let mut progressed = true;
    while progressed {
        progressed = false;
        for id in 0..total {
            if resolved[id] {
                continue;
            }
            let info = &types.structs[id];
            let mut ok = true;
            let mut off = 0usize;
            let mut field_offsets = Vec::with_capacity(info.fields.len());
            for field in &info.fields {
                field_offsets.push(off);
                match &field.ty {
                    Type::Struct(inner) => match sizes.get(inner) {
                        Some(size) => off += size,
                        None => {
                            ok = false;
                            break;
                        }
                    },
                    _ => off += 8,
                }
            }
            if ok {
                offsets.insert(id as u32, field_offsets);
                sizes.insert(id as u32, off);
                resolved[id] = true;
                progressed = true;
            }
        }
    }
    // 循环依赖兜底：按每字段 8 字节布局（语义层应已拦截直接自引用）。
    for id in 0..total {
        if !resolved[id] {
            let count = types.structs[id].fields.len();
            sizes.insert(id as u32, count * 8);
            offsets.insert(id as u32, (0..count).map(|index| index * 8).collect());
        }
    }
    (offsets, sizes)
}

/// 类字段布局：基类优先展平（与 MIR 字段序号一致），标量 8 字节、
/// struct 值字段内联其大小；对象头部第 0 个字是 vtable 指针。
/// 返回 (字段偏移表, 对象总大小表)。
fn class_layout(
    types: &TypeTable,
    struct_sizes: &HashMap<u32, usize>,
) -> (HashMap<u32, Vec<usize>>, HashMap<u32, usize>) {
    let mut offsets: HashMap<u32, Vec<usize>> = HashMap::new();
    let mut sizes: HashMap<u32, usize> = HashMap::new();
    for (index, class) in types.classes.iter().enumerate() {
        let mut chain = vec![class];
        let mut base = class.base;
        while let Some(id) = base {
            chain.push(&types.classes[id as usize]);
            base = types.classes[id as usize].base;
        }
        chain.reverse();
        let mut off = 8usize; // vtable 指针
        let mut field_offsets = Vec::new();
        for field in chain.iter().flat_map(|class| class.fields.iter()) {
            field_offsets.push(off);
            match &field.ty {
                Type::Struct(inner) => {
                    off += struct_sizes.get(inner).copied().unwrap_or(8);
                }
                _ => off += 8,
            }
        }
        offsets.insert(index as u32, field_offsets);
        sizes.insert(index as u32, off);
    }
    (offsets, sizes)
}

fn struct_field_types(types: &TypeTable) -> HashMap<u32, Vec<Type>> {
    types
        .structs
        .iter()
        .enumerate()
        .map(|(index, info)| {
            (
                index as u32,
                info.fields.iter().map(|field| field.ty.clone()).collect(),
            )
        })
        .collect()
}

/// 类字段按基类优先展平（与 MIR 字段序号一致）。
fn class_field_types(types: &TypeTable) -> HashMap<u32, Vec<Type>> {
    types
        .classes
        .iter()
        .enumerate()
        .map(|(index, class)| {
            let mut chain = vec![class];
            let mut base = class.base;
            while let Some(id) = base {
                chain.push(&types.classes[id as usize]);
                base = types.classes[id as usize].base;
            }
            chain.reverse();
            let fields = chain
                .iter()
                .flat_map(|class| class.fields.iter())
                .map(|field| field.ty.clone())
                .collect();
            (index as u32, fields)
        })
        .collect()
}

/// 接口方法在全局 vtable 槽位中的起始偏移：按接口声明顺序累加方法数。
fn interface_slot_bases(types: &TypeTable) -> (HashMap<u32, usize>, usize) {
    let mut bases = HashMap::new();
    let mut total = 0usize;
    for (id, info) in types.interfaces.iter().enumerate() {
        bases.insert(id as u32, total);
        total += info.methods.len();
    }
    (bases, total)
}

struct LowerCtx<'a, 'f> {
    builder: FunctionBuilder<'f>,
    refs: RefTable,
    slots: Vec<StackSlot>,
    /// (循环头块, 循环出口块)
    loops: Vec<(Block, Block)>,
    module: &'a ObjectModule,
    types: &'a TypeTable,
    class_field_counts: HashMap<u32, usize>,
    struct_field_offsets: HashMap<u32, Vec<usize>>,
    struct_sizes: HashMap<u32, usize>,
    class_field_offsets: HashMap<u32, Vec<usize>>,
    class_sizes: HashMap<u32, usize>,
    struct_field_types: HashMap<u32, Vec<Type>>,
    class_field_types: HashMap<u32, Vec<Type>>,
    /// class id → vtable 数据。
    vtable_data: HashMap<u32, cranelift_module::DataId>,
    /// interface id → vtable 槽位起始偏移。
    interface_slot_bases: HashMap<u32, usize>,
    /// 参数 + 局部变量的类型（槽索引与 MIR 局部索引一致）。
    local_types: Vec<Type>,
    ret_ty: Type,
    /// 结构体返回值的隐藏 sret 指针（入口参数）。
    sret: Option<Value>,
    strings: &'a [String],
    /// 最后一条 MIR 语句是否以终止指令（return/break/continue）结束。
    last_terminated: bool,
}

impl<'a, 'f> LowerCtx<'a, 'f> {
    fn new_slot(&mut self) -> StackSlot {
        let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            8,
            3,
        ));
        self.slots.push(slot);
        slot
    }

    fn stmts(&mut self, statements: &[MirStmt]) -> Result<(), CodegenError> {
        for statement in statements {
            self.stmt(statement)?;
        }
        Ok(())
    }

    fn stmt(&mut self, statement: &MirStmt) -> Result<(), CodegenError> {
        self.last_terminated = false;
        match &statement.kind {
            MirStmtKind::VarDecl { local, init } => {
                let slot = self.slot_for(*local);
                let is_struct = self.is_struct_local(*local);
                let is_float = self.is_float_local(*local);
                match init {
                    Some(init) => {
                        let value = self.expr(init)?;
                        if is_struct {
                            let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                            self.copy_struct(&self.local_types[*local].clone(), value, address)?;
                        } else if is_float {
                            self.builder.ins().stack_store(types::I64, value, slot, 0);
                        } else {
                            self.builder.ins().stack_store(types::I64, value, slot, 0);
                        }
                        Ok(())
                    }
                    None => {
                        if is_struct {
                            let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                            let ty = self.local_types[*local].clone();
                            self.zero_struct(&ty, address)
                        } else if is_float {
                            let zero = self.builder.ins().f64const(0.0);
                            self.builder.ins().stack_store(types::I64, zero, slot, 0);
                            Ok(())
                        } else {
                            let zero = self.builder.ins().iconst(types::I64, 0);
                            self.builder.ins().stack_store(types::I64, zero, slot, 0);
                            Ok(())
                        }
                    }
                }
            }
            MirStmtKind::Assign { target, value } => {
                let value = self.expr(value)?;
                self.store_target(target, value)
            }
            MirStmtKind::If { cond, then, else_ } => {
                let cond = self.expr(cond)?;
                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let join_block = self.builder.create_block();
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_true = self.builder.ins().icmp(IntCC::NotEqual, cond, zero);
                self.builder
                    .ins()
                    .brif(is_true, then_block, &[], else_block, &[]);
                self.builder.switch_to_block(then_block);
                self.stmts(then)?;
                self.builder.ins().jump(join_block, &[]);
                self.builder.switch_to_block(else_block);
                self.stmts(else_)?;
                self.builder.ins().jump(join_block, &[]);
                self.builder.switch_to_block(join_block);
                self.builder.seal_block(join_block);
                self.last_terminated = false;
                Ok(())
            }
            MirStmtKind::While { cond, body } => {
                let header = self.builder.create_block();
                let body_block = self.builder.create_block();
                let exit = self.builder.create_block();
                self.builder.ins().jump(header, &[]);
                self.builder.switch_to_block(header);
                let cond = self.expr(cond)?;
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_true = self.builder.ins().icmp(IntCC::NotEqual, cond, zero);
                self.builder.ins().brif(is_true, body_block, &[], exit, &[]);
                self.builder.switch_to_block(body_block);
                self.loops.push((header, exit));
                self.stmts(body)?;
                self.loops.pop();
                self.builder.ins().jump(header, &[]);
                self.builder.switch_to_block(exit);
                self.builder.seal_block(exit);
                self.last_terminated = false;
                Ok(())
            }
            MirStmtKind::Return(value) => {
                match value {
                    Some(value) => {
                        let value = self.expr(value)?;
                        if is_struct_ret(&self.ret_ty) {
                            let sret = self.sret.ok_or("结构体返回值缺少 sret 参数")?;
                            let ret_ty = self.ret_ty.clone();
                            self.copy_struct(&ret_ty, value, sret)?;
                            self.builder.ins().return_(&[]);
                        } else {
                            self.builder.ins().return_(&[value]);
                        }
                    }
                    None => {
                        self.builder.ins().return_(&[]);
                    }
                }
                self.fresh_block_after_terminator();
                self.last_terminated = true;
                Ok(())
            }
            MirStmtKind::Expr(expr) => {
                self.expr(expr)?;
                Ok(())
            }
            MirStmtKind::Break => {
                let exit = self
                    .loops
                    .last()
                    .map(|(_, exit)| *exit)
                    .ok_or("break 不在循环内")?;
                self.builder.ins().jump(exit, &[]);
                self.fresh_block_after_terminator();
                self.last_terminated = true;
                Ok(())
            }
            MirStmtKind::Continue => {
                let header = self
                    .loops
                    .last()
                    .map(|(header, _)| *header)
                    .ok_or("continue 不在循环内")?;
                self.builder.ins().jump(header, &[]);
                self.fresh_block_after_terminator();
                self.last_terminated = true;
                Ok(())
            }
        }
    }

    /// 终止指令（return/break/continue）之后不能继续往当前块加指令；
    /// 切到一个新的未填充块，后续语句落在不可达区域。
    fn fresh_block_after_terminator(&mut self) {
        let block = self.builder.create_block();
        self.builder.switch_to_block(block);
    }

    fn slot_for(&mut self, local: usize) -> StackSlot {
        while self.slots.len() <= local {
            let size = self
                .local_types
                .get(self.slots.len())
                .map(|ty| self.struct_size(ty))
                .unwrap_or(8);
            let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                size as u32,
                3,
            ));
            self.slots.push(slot);
        }
        self.slots[local]
    }

    fn is_struct_local(&self, local: usize) -> bool {
        matches!(self.local_types.get(local), Some(Type::Struct(_)))
    }

    fn is_float_local(&self, local: usize) -> bool {
        matches!(
            self.local_types.get(local),
            Some(Type::F32) | Some(Type::F64)
        )
    }

    fn expr_owner_type(&self, expr: &MirExpr) -> Option<Type> {
        match expr {
            MirExpr::Local(local) => self.local_types.get(*local).cloned(),
            MirExpr::Field { object, index } => {
                let owner = self.expr_owner_type(object)?;
                self.field_type(&owner, *index)
            }
            MirExpr::New { class, .. } => Some(Type::Class(*class)),
            MirExpr::Struct { ty, .. } => Some(ty.clone()),
            MirExpr::Call { callee, .. } => {
                let sig = match callee {
                    MirCallee::Function { sig, .. }
                    | MirCallee::Method { sig, .. }
                    | MirCallee::Extern { sig, .. }
                    | MirCallee::InterfaceMethod { sig, .. }
                    | MirCallee::Closure { sig, .. } => Some(sig),
                    _ => None,
                };
                match sig {
                    Some(sig) if matches!(sig.ret, Type::Struct(_)) => Some(sig.ret.clone()),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn field_type(&self, owner: &Type, index: usize) -> Option<Type> {
        match owner {
            Type::Struct(id) => self.struct_field_types.get(id)?.get(index).cloned(),
            Type::Class(id) => self.class_field_types.get(id)?.get(index).cloned(),
            _ => None,
        }
    }

    fn field_is_float(&self, object: &MirExpr, index: usize) -> bool {
        self.expr_owner_type(object)
            .and_then(|owner| self.field_type(&owner, index))
            .map(|ty| matches!(ty, Type::F32 | Type::F64))
            .unwrap_or(false)
    }

    /// 从数组表达式推断元素类型（u8 紧凑布局 1 字节，其余 8 字节）。
    fn expr_array_elem(&self, expr: &MirExpr) -> Option<Type> {
        match expr {
            MirExpr::Local(local) => match self.local_types.get(*local) {
                Some(Type::Array(inner)) => Some((**inner).clone()),
                _ => None,
            },
            MirExpr::Field { object, index } => {
                let owner = self.expr_owner_type(object)?;
                match self.field_type(&owner, *index)? {
                    Type::Array(inner) => Some(*inner),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn array_elem_size(&self, expr: &MirExpr) -> usize {
        match self.expr_array_elem(expr) {
            Some(Type::U8) => 1,
            Some(Type::Struct(id)) => self.struct_sizes.get(&id).copied().unwrap_or(8),
            _ => 8,
        }
    }

    fn struct_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Struct(id) => self.struct_sizes.get(id).copied().unwrap_or(8),
            _ => 8,
        }
    }

    /// 字段字节偏移：struct 用内联偏移表；class 暂按每字段 8 字节（vtable 头在接口批次引入）。
    fn field_offset(&self, owner: &Type, index: usize) -> usize {
        match owner {
            Type::Struct(id) => self
                .struct_field_offsets
                .get(id)
                .and_then(|offsets| offsets.get(index))
                .copied()
                .unwrap_or(index * 8),
            // 类字段内联布局（基类优先展平，struct 值字段按大小内联），头部 +8 是 vtable。
            Type::Class(id) => self
                .class_field_offsets
                .get(id)
                .and_then(|offsets| offsets.get(index))
                .copied()
                .unwrap_or(8 + index * 8),
            _ => index * 8,
        }
    }

    fn copy_struct(&mut self, ty: &Type, src: Value, dst: Value) -> Result<(), CodegenError> {
        let size = self.struct_size(ty);
        for offset in (0..size).step_by(8) {
            let offset = offset as i32;
            let word = self
                .builder
                .ins()
                .load(types::I64, MemFlagsData::new(), src, offset);
            self.builder
                .ins()
                .store(MemFlagsData::new(), word, dst, offset);
        }
        Ok(())
    }

    /// 数组元素地址 = data 指针 + index * 步长（u8 为 1、struct 为其内联大小、其余 8）。
    fn index_address(&mut self, object: Value, index: Value, elem_size: usize) -> Value {
        let data = self
            .builder
            .ins()
            .load(types::I64, MemFlagsData::new(), object, 16);
        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
        let scaled = self.builder.ins().imul(index, stride);
        self.builder.ins().iadd(data, scaled)
    }

    fn zero_struct(&mut self, ty: &Type, dst: Value) -> Result<(), CodegenError> {
        let size = self.struct_size(ty);
        for offset in (0..size).step_by(8) {
            let offset = offset as i32;
            let zero = self.builder.ins().iconst(types::I64, 0);
            self.builder
                .ins()
                .store(MemFlagsData::new(), zero, dst, offset);
        }
        Ok(())
    }

    fn load_target(&mut self, target: &MirTarget) -> Result<Value, CodegenError> {
        match target {
            MirTarget::Local(local) => {
                let slot = self.slot_for(*local);
                if self.is_float_local(*local) {
                    Ok(self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::F64, slot, 0))
                } else {
                    Ok(self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0))
                }
            }
            MirTarget::Global(index) => {
                let gv = self
                    .refs
                    .global_refs
                    .get(index)
                    .copied()
                    .ok_or("全局未声明")?;
                let address = self.builder.ins().symbol_value(types::I64, gv);
                Ok(self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), address, 0))
            }
            MirTarget::Field { object, index } => {
                let owner = self.expr_owner_type(object).unwrap_or(Type::Error);
                let offset = self.field_offset(&owner, *index) as i32;
                let float = self.field_is_float(object, *index);
                let object = self.expr(object)?;
                let ty = if float { types::F64 } else { types::I64 };
                Ok(self
                    .builder
                    .ins()
                    .load(ty, MemFlagsData::new(), object, offset))
            }
            MirTarget::Index {
                object,
                index,
                elem,
            } => {
                let elem_size = self.array_elem_size(object);
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let address = self.index_address(object, index, elem_size);
                if elem_size == 1 {
                    let byte = self
                        .builder
                        .ins()
                        .load(types::I8, MemFlagsData::new(), address, 0);
                    Ok(self.builder.ins().uextend(types::I64, byte))
                } else if matches!(&**elem, Type::Struct(_)) {
                    // struct 元素：返回元素地址（值语义由调用方复制）。
                    Ok(address)
                } else {
                    let value =
                        self.builder
                            .ins()
                            .load(types::I64, MemFlagsData::new(), address, 0);
                    Ok(if elem.is_float() {
                        self.builder
                            .ins()
                            .bitcast(types::F64, MemFlagsData::new(), value)
                    } else {
                        value
                    })
                }
            }
        }
    }

    fn store_target(&mut self, target: &MirTarget, value: Value) -> Result<(), CodegenError> {
        match target {
            MirTarget::Local(local) => {
                let slot = self.slot_for(*local);
                if self.is_struct_local(*local) {
                    let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                    self.copy_struct(&self.local_types[*local].clone(), value, address)
                } else if self.is_float_local(*local) {
                    self.builder.ins().stack_store(types::I64, value, slot, 0);
                    Ok(())
                } else {
                    self.builder.ins().stack_store(types::I64, value, slot, 0);
                    Ok(())
                }
            }
            MirTarget::Global(index) => {
                let gv = self
                    .refs
                    .global_refs
                    .get(index)
                    .copied()
                    .ok_or("全局未声明")?;
                let address = self.builder.ins().symbol_value(types::I64, gv);
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), value, address, 0);
                Ok(())
            }
            MirTarget::Field { object, index } => {
                let owner = self.expr_owner_type(object).unwrap_or(Type::Error);
                let offset = self.field_offset(&owner, *index) as i32;
                let object = self.expr(object)?;
                if let Some(Type::Struct(_)) = self.field_type(&owner, *index) {
                    let dst = self.builder.ins().iadd_imm(object, offset as i64);
                    let field_ty = self.field_type(&owner, *index).unwrap();
                    self.copy_struct(&field_ty, value, dst)
                } else {
                    self.builder
                        .ins()
                        .store(MemFlagsData::new(), value, object, offset);
                    Ok(())
                }
            }
            MirTarget::Index {
                object,
                index,
                elem,
            } => {
                let elem_size = self.array_elem_size(object);
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let address = self.index_address(object, index, elem_size);
                if matches!(&**elem, Type::Struct(_)) {
                    return self.copy_struct(elem, value, address);
                }
                let value = if elem.is_float() {
                    self.builder
                        .ins()
                        .bitcast(types::I64, MemFlagsData::new(), value)
                } else {
                    value
                };
                if elem_size == 1 {
                    let byte = self.builder.ins().ireduce(types::I8, value);
                    self.builder
                        .ins()
                        .store(MemFlagsData::new(), byte, address, 0);
                } else {
                    self.builder
                        .ins()
                        .store(MemFlagsData::new(), value, address, 0);
                }
                Ok(())
            }
        }
    }

    fn expr(&mut self, expr: &MirExpr) -> Result<Value, CodegenError> {
        Ok(match expr {
            MirExpr::Int(value) => self.builder.ins().iconst(types::I64, *value),
            MirExpr::UInt(value) => self.builder.ins().iconst(types::I64, *value as i64),
            MirExpr::Float(value) => self.builder.ins().f64const(*value),
            MirExpr::Bool(value) => self.builder.ins().iconst(types::I64, i64::from(*value)),
            MirExpr::Char(value) => self.builder.ins().iconst(types::I64, *value as i64),
            MirExpr::Null => self.builder.ins().iconst(types::I64, 0),
            MirExpr::Str(id) => {
                let gv = self
                    .refs
                    .string_refs
                    .get(id)
                    .copied()
                    .ok_or("字符串未声明")?;
                let address = self.builder.ins().symbol_value(types::I64, gv);
                let len_value = self.string_len(*id) as i64;
                let len = self.builder.ins().iconst(types::I64, len_value);
                self.call_import(
                    "sw_string_from_literal",
                    string_from_literal_signature(self.module.isa()),
                    &[address, len],
                )?
            }
            MirExpr::Local(local) => {
                let slot = self.slot_for(*local);
                if self.is_struct_local(*local) {
                    self.builder.ins().stack_addr(types::I64, slot, 0)
                } else if self.is_float_local(*local) {
                    self.builder
                        .ins()
                        .stack_load(types::I64, types::F64, slot, 0)
                } else {
                    self.builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0)
                }
            }
            MirExpr::Global(index) => {
                let gv = self
                    .refs
                    .global_refs
                    .get(index)
                    .copied()
                    .ok_or("全局未声明")?;
                let address = self.builder.ins().symbol_value(types::I64, gv);
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), address, 0)
            }
            MirExpr::Unary { op, expr: inner } => {
                let value = self.expr(inner)?;
                match op {
                    MirUnary::Neg => {
                        if self.builder.func.dfg.value_type(value) == types::F64 {
                            self.builder.ins().fneg(value)
                        } else {
                            self.builder.ins().ineg(value)
                        }
                    }
                    MirUnary::Pos => value,
                    MirUnary::Not => {
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        let is_zero = self.builder.ins().icmp(IntCC::Equal, value, zero);
                        self.builder.ins().uextend(types::I64, is_zero)
                    }
                    MirUnary::BitNot => {
                        let all_ones = self.builder.ins().iconst(types::I64, -1);
                        self.builder.ins().bxor(value, all_ones)
                    }
                    MirUnary::Inc | MirUnary::Dec => {
                        return Err("自增/自减已降级为赋值".into());
                    }
                }
            }
            MirExpr::Binary { op, left, right } => {
                let left = self.expr(left)?;
                let right = self.expr(right)?;
                let is_float = self.builder.func.dfg.value_type(left) == types::F64
                    || self.builder.func.dfg.value_type(right) == types::F64;
                match op {
                    MirBinary::Add => {
                        if is_float {
                            self.builder.ins().fadd(left, right)
                        } else {
                            self.builder.ins().iadd(left, right)
                        }
                    }
                    MirBinary::Sub => {
                        if is_float {
                            self.builder.ins().fsub(left, right)
                        } else {
                            self.builder.ins().isub(left, right)
                        }
                    }
                    MirBinary::Mul => {
                        if is_float {
                            self.builder.ins().fmul(left, right)
                        } else {
                            self.builder.ins().imul(left, right)
                        }
                    }
                    MirBinary::Div => {
                        if is_float {
                            self.builder.ins().fdiv(left, right)
                        } else {
                            self.builder.ins().sdiv(left, right)
                        }
                    }
                    MirBinary::Rem => {
                        if is_float {
                            return Err("浮点取模应已降级为内建调用".into());
                        }
                        self.builder.ins().srem(left, right)
                    }
                    MirBinary::Pow => return Err("`**` 后端暂不支持".into()),
                    MirBinary::And => self.builder.ins().band(left, right),
                    MirBinary::Or => self.builder.ins().bor(left, right),
                    MirBinary::BitAnd => self.builder.ins().band(left, right),
                    MirBinary::BitOr => self.builder.ins().bor(left, right),
                    MirBinary::BitXor => self.builder.ins().bxor(left, right),
                    MirBinary::Shl => self.builder.ins().ishl(left, right),
                    MirBinary::Shr => self.builder.ins().sshr(left, right),
                    MirBinary::Eq => self.bool_cmp(IntCC::Equal, left, right),
                    MirBinary::Ne => self.bool_cmp(IntCC::NotEqual, left, right),
                    MirBinary::Lt => self.bool_cmp(IntCC::SignedLessThan, left, right),
                    MirBinary::Le => self.bool_cmp(IntCC::SignedLessThanOrEqual, left, right),
                    MirBinary::Gt => self.bool_cmp(IntCC::SignedGreaterThan, left, right),
                    MirBinary::Ge => self.bool_cmp(IntCC::SignedGreaterThanOrEqual, left, right),
                    MirBinary::Coalesce => {
                        let zero = self.builder.ins().iconst(types::I64, 0);
                        let is_nonzero = self.builder.ins().icmp(IntCC::NotEqual, left, zero);
                        self.builder.ins().select(is_nonzero, left, right)
                    }
                }
            }
            MirExpr::Call { callee, args } => {
                // 结构体返回值：签名首参数为 sret 指针，调用后返回该地址；
                // 结构体实参：表达式本身产出地址，直接按 I64 传入（被调方入口复制）。
                let (ret_type, closure_struct) = match callee {
                    MirCallee::Closure { sig } => (
                        sig.ret.clone(),
                        sig.params.iter().any(|p| matches!(p.ty, Type::Struct(_))),
                    ),
                    MirCallee::Function { sig, .. }
                    | MirCallee::Method { sig, .. }
                    | MirCallee::Extern { sig, .. } => (sig.ret.clone(), false),
                    MirCallee::InterfaceMethod { sig, .. } => (sig.ret.clone(), false),
                    MirCallee::Intrinsic { .. } => (Type::Void, false),
                };
                let _ = closure_struct;
                let mut call_args = Vec::new();
                let mut sret = None;
                if is_struct_ret(&ret_type) {
                    let size = self.struct_size(&ret_type);
                    let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        size as u32,
                        3,
                    ));
                    let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                    call_args.push(address);
                    sret = Some(address);
                }
                for arg in args {
                    call_args.push(self.expr(arg)?);
                }
                let call = match callee {
                    MirCallee::Closure { sig } => {
                        let key = format!("{:?}", sig);
                        let sig_ref = self
                            .refs
                            .closure_sig_refs
                            .get(&key)
                            .copied()
                            .ok_or("闭包签名未导入")?;
                        let mut values = call_args.into_iter();
                        let sret_arg = if sret.is_some() { values.next() } else { None };
                        let closure = values.next().ok_or("闭包调用缺少接收者")?;
                        let fn_ptr =
                            self.builder
                                .ins()
                                .load(types::I64, MemFlagsData::new(), closure, 0);
                        let env =
                            self.builder
                                .ins()
                                .load(types::I64, MemFlagsData::new(), closure, 8);
                        let mut final_args = Vec::new();
                        if let Some(sret_arg) = sret_arg {
                            final_args.push(sret_arg);
                        }
                        final_args.push(env);
                        final_args.extend(values);
                        self.builder
                            .ins()
                            .call_indirect(sig_ref, fn_ptr, &final_args)
                    }
                    MirCallee::InterfaceMethod {
                        interface,
                        index,
                        sig,
                    } => {
                        let mut values = call_args.into_iter();
                        let sret_arg = if sret.is_some() { values.next() } else { None };
                        let receiver = values.next().ok_or("接口调用缺少接收者")?;
                        let vt =
                            self.builder
                                .ins()
                                .load(types::I64, MemFlagsData::new(), receiver, 0);
                        let base = self
                            .interface_slot_bases
                            .get(interface)
                            .copied()
                            .unwrap_or(0);
                        let slot = ((base + index) * 8) as i32;
                        let fn_ptr =
                            self.builder
                                .ins()
                                .load(types::I64, MemFlagsData::new(), vt, slot);
                        let mut call_sig = Signature::new(self.module.isa().default_call_conv());
                        if sret.is_some() {
                            call_sig.params.push(AbiParam::new(types::I64));
                        }
                        call_sig.params.push(AbiParam::new(types::I64));
                        for param in &sig.params {
                            call_sig.params.push(AbiParam::new(abi_type(&param.ty)?));
                        }
                        if sig.ret != Type::Void
                            && sig.ret != Type::Unknown
                            && !is_struct_ret(&sig.ret)
                        {
                            call_sig.returns.push(AbiParam::new(abi_type(&sig.ret)?));
                        }
                        let sig_ref = self.builder.import_signature(call_sig);
                        let mut final_args = Vec::new();
                        if let Some(sret_arg) = sret_arg {
                            final_args.push(sret_arg);
                        }
                        final_args.push(receiver);
                        final_args.extend(values);
                        self.builder
                            .ins()
                            .call_indirect(sig_ref, fn_ptr, &final_args)
                    }
                    _ => {
                        let name = match callee {
                            MirCallee::Function { name, .. } | MirCallee::Method { name, .. } => {
                                name.clone()
                            }
                            MirCallee::Extern { name, .. } => extern_c_symbol(name).to_owned(),
                            MirCallee::Intrinsic { name } => intrinsic_name(name).to_owned(),
                            MirCallee::Closure { .. } => unreachable!(),
                            MirCallee::InterfaceMethod { .. } => unreachable!(),
                        };
                        let func_ref = self
                            .refs
                            .func_refs
                            .get(&name)
                            .copied()
                            .ok_or("调用目标未声明")?;
                        self.builder.ins().call(func_ref, &call_args)
                    }
                };
                let results = self.builder.inst_results(call);
                if let Some(sret) = sret {
                    sret
                } else if results.is_empty() {
                    self.builder.ins().iconst(types::I64, 0)
                } else {
                    results[0]
                }
            }
            MirExpr::Field { object, index } => {
                let owner = self.expr_owner_type(object).unwrap_or(Type::Error);
                let offset = self.field_offset(&owner, *index) as i32;
                let float = self.field_is_float(object, *index);
                let object = self.expr(object)?;
                if let Some(field_ty @ Type::Struct(_)) = self.field_type(&owner, *index) {
                    let _ = field_ty;
                    return Ok(self.builder.ins().iadd_imm(object, offset as i64));
                }
                let ty = if float { types::F64 } else { types::I64 };
                self.builder
                    .ins()
                    .load(ty, MemFlagsData::new(), object, offset)
            }
            MirExpr::Index {
                object,
                index,
                elem,
            } => {
                let elem_size = self.array_elem_size(object);
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let address = self.index_address(object, index, elem_size);
                if elem_size == 1 {
                    let byte = self
                        .builder
                        .ins()
                        .load(types::I8, MemFlagsData::new(), address, 0);
                    self.builder.ins().uextend(types::I64, byte)
                } else if matches!(&**elem, Type::Struct(_)) {
                    address
                } else {
                    let value =
                        self.builder
                            .ins()
                            .load(types::I64, MemFlagsData::new(), address, 0);
                    if elem.is_float() {
                        self.builder
                            .ins()
                            .bitcast(types::F64, MemFlagsData::new(), value)
                    } else {
                        value
                    }
                }
            }
            MirExpr::Len { object, string } => {
                let object = self.expr(object)?;
                let offset = if *string { 8 } else { 0 };
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, offset)
            }
            MirExpr::Array { elem, items } => {
                let elem_size = if matches!(**elem, Type::U8) {
                    1
                } else if let Type::Struct(id) = &**elem {
                    self.struct_sizes.get(id).copied().unwrap_or(8)
                } else {
                    8
                };
                let is_struct = matches!(&**elem, Type::Struct(_));
                let count = self.builder.ins().iconst(types::I64, items.len() as i64);
                let elem_size_value = self.builder.ins().iconst(types::I64, elem_size as i64);
                let array = self.call_import(
                    "sw_array_new",
                    array_new_signature(self.module.isa()),
                    &[elem_size_value, count],
                )?;
                let slot = self.new_slot();
                self.builder.ins().stack_store(types::I64, array, slot, 0);
                for (index, item) in items.iter().enumerate() {
                    let item = self.expr(item)?;
                    let index_value = self.builder.ins().iconst(types::I64, index as i64);
                    let array = self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0);
                    if elem_size == 1 {
                        self.call_import(
                            "sw_array_set_u8",
                            array_set_signature(self.module.isa()),
                            &[array, index_value, item],
                        )?;
                    } else if is_struct {
                        let data =
                            self.builder
                                .ins()
                                .load(types::I64, MemFlagsData::new(), array, 16);
                        let stride = self.builder.ins().iconst(types::I64, elem_size as i64);
                        let offset = self.builder.ins().imul(index_value, stride);
                        let dst = self.builder.ins().iadd(data, offset);
                        self.copy_struct(elem, item, dst)?;
                    } else {
                        let item = if elem.is_float() {
                            self.builder
                                .ins()
                                .bitcast(types::I64, MemFlagsData::new(), item)
                        } else {
                            item
                        };
                        self.call_import(
                            "sw_array_set",
                            array_set_signature(self.module.isa()),
                            &[array, index_value, item],
                        )?;
                    }
                }
                self.builder
                    .ins()
                    .stack_load(types::I64, types::I64, slot, 0)
            }
            MirExpr::VarArgs(items) => {
                let count = self.builder.ins().iconst(types::I64, items.len() as i64);
                let elem_size = self.builder.ins().iconst(types::I64, 16);
                let array = self.call_import(
                    "sw_array_new",
                    array_new_signature(self.module.isa()),
                    &[elem_size, count],
                )?;
                let slot = self.new_slot();
                self.builder.ins().stack_store(types::I64, array, slot, 0);
                for (index, (tag, item)) in items.iter().enumerate() {
                    let value = self.expr(item)?;
                    let value = if *tag == SW_TAG_FLOAT {
                        self.builder
                            .ins()
                            .bitcast(types::I64, MemFlagsData::new(), value)
                    } else {
                        value
                    };
                    let array = self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0);
                    let tag_index = self.builder.ins().iconst(types::I64, (index * 2) as i64);
                    let tag_value = self.builder.ins().iconst(types::I64, *tag);
                    self.call_import(
                        "sw_array_set",
                        array_set_signature(self.module.isa()),
                        &[array, tag_index, tag_value],
                    )?;
                    let array = self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0);
                    let value_index = self
                        .builder
                        .ins()
                        .iconst(types::I64, (index * 2 + 1) as i64);
                    self.call_import(
                        "sw_array_set",
                        array_set_signature(self.module.isa()),
                        &[array, value_index, value],
                    )?;
                }
                self.builder
                    .ins()
                    .stack_load(types::I64, types::I64, slot, 0)
            }
            MirExpr::Struct { ty, fields } => {
                let size = self.struct_size(ty);
                let slot = self.builder.create_sized_stack_slot(StackSlotData::new(
                    StackSlotKind::ExplicitSlot,
                    size as u32,
                    3,
                ));
                let address = self.builder.ins().stack_addr(types::I64, slot, 0);
                self.zero_struct(ty, address)?;
                for (index, field) in fields {
                    let value = self.expr(field)?;
                    let offset = self.field_offset(ty, *index) as i32;
                    if let Some(field_ty @ Type::Struct(_)) = self.field_type(ty, *index) {
                        let dst = self.builder.ins().iadd_imm(address, offset as i64);
                        self.copy_struct(&field_ty, value, dst)?;
                    } else {
                        self.builder
                            .ins()
                            .store(MemFlagsData::new(), value, address, offset);
                    }
                }
                address
            }
            MirExpr::New { class, args, .. } => {
                // 对象头部第 0 个字是 vtable 指针，字段内联（可能含 struct 值字段）。
                let object_size = self.class_sizes.get(class).copied().unwrap_or(8);
                let size = self.builder.ins().iconst(types::I64, object_size as i64);
                let object = self.call_import(
                    "sw_object_new",
                    object_new_signature(self.module.isa()),
                    &[size],
                )?;
                if let Some(data_id) = self.vtable_data.get(class) {
                    let gv = self
                        .module
                        .declare_data_in_func(*data_id, &mut self.builder.func);
                    let vt = self.builder.ins().symbol_value(types::I64, gv);
                    self.builder.ins().store(MemFlagsData::new(), vt, object, 0);
                }
                let slot = self.new_slot();
                self.builder.ins().stack_store(types::I64, object, slot, 0);
                let mut values = vec![object];
                for arg in args {
                    values.push(self.expr(arg)?);
                }
                let ctor_name = format!("sw_ctor_{class}");
                let func_ref = self
                    .refs
                    .func_refs
                    .get(&ctor_name)
                    .copied()
                    .ok_or("构造函数未声明")?;
                self.builder.ins().call(func_ref, &values);
                self.builder
                    .ins()
                    .stack_load(types::I64, types::I64, slot, 0)
            }
            MirExpr::Cast { expr: inner, to } => {
                let value = self.expr(inner)?;
                if to.is_float() {
                    self.builder.ins().fcvt_from_sint(types::F64, value)
                } else if self.builder.func.dfg.value_type(value) == types::F64 {
                    self.builder.ins().fcvt_to_sint(types::I64, value)
                } else {
                    value
                }
            }
            MirExpr::Select { cond, then, else_ } => {
                let cond = self.expr(cond)?;
                let then_block = self.builder.create_block();
                let else_block = self.builder.create_block();
                let join_block = self.builder.create_block();
                let zero = self.builder.ins().iconst(types::I64, 0);
                let is_true = self.builder.ins().icmp(IntCC::NotEqual, cond, zero);
                self.builder
                    .ins()
                    .brif(is_true, then_block, &[], else_block, &[]);
                let slot = self.new_slot();
                self.builder.switch_to_block(then_block);
                let then_value = self.expr(then)?;
                let value_ty = self.builder.func.dfg.value_type(then_value);
                self.builder
                    .ins()
                    .stack_store(types::I64, then_value, slot, 0);
                self.builder.ins().jump(join_block, &[]);
                self.builder.switch_to_block(else_block);
                let else_value = self.expr(else_)?;
                self.builder
                    .ins()
                    .stack_store(types::I64, else_value, slot, 0);
                self.builder.ins().jump(join_block, &[]);
                self.builder.switch_to_block(join_block);
                self.builder.seal_block(join_block);
                self.builder.ins().stack_load(types::I64, value_ty, slot, 0)
            }
            MirExpr::Assign { target, value } => {
                let value = self.expr(value)?;
                self.store_target(target, value)?;
                value
            }
            MirExpr::Postfix { target, op } => {
                let old = self.load_target(target)?;
                let float = self.builder.func.dfg.value_type(old) == types::F64;
                let one = if float {
                    self.builder.ins().f64const(1.0)
                } else {
                    self.builder.ins().iconst(types::I64, 1)
                };
                let new = match op {
                    MirUnary::Inc => {
                        if float {
                            self.builder.ins().fadd(old, one)
                        } else {
                            self.builder.ins().iadd(old, one)
                        }
                    }
                    MirUnary::Dec => {
                        if float {
                            self.builder.ins().fsub(old, one)
                        } else {
                            self.builder.ins().isub(old, one)
                        }
                    }
                    _ => return Err("Postfix 只支持 ++/--".into()),
                };
                self.store_target(target, new)?;
                old
            }
            MirExpr::ClosureNew { name, captures, .. } => {
                let func_ref = self
                    .refs
                    .func_refs
                    .get(name)
                    .copied()
                    .ok_or("闭包函数未声明")?;
                let fn_addr = self.builder.ins().func_addr(types::I64, func_ref);
                let count = self.builder.ins().iconst(types::I64, captures.len() as i64);
                let closure = self.call_import(
                    "sw_closure_new",
                    closure_new_signature(self.module.isa()),
                    &[fn_addr, count],
                )?;
                let slot = self.new_slot();
                self.builder.ins().stack_store(types::I64, closure, slot, 0);
                for (index, capture) in captures.iter().enumerate() {
                    let value = self.expr(capture)?;
                    let closure = self
                        .builder
                        .ins()
                        .stack_load(types::I64, types::I64, slot, 0);
                    let index_value = self.builder.ins().iconst(types::I64, index as i64);
                    self.call_import(
                        "sw_env_set",
                        env_set_signature(self.module.isa()),
                        &[closure, index_value, value],
                    )?;
                }
                self.builder
                    .ins()
                    .stack_load(types::I64, types::I64, slot, 0)
            }
            MirExpr::EnvGet { slot } => {
                let env_slot = self.slot_for(0);
                let env = self
                    .builder
                    .ins()
                    .stack_load(types::I64, types::I64, env_slot, 0);
                let offset = (*slot as i32) * 8;
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), env, offset)
            }
            MirExpr::EnumNew { tag, fields } => {
                let size = (1 + fields.len()) as i64 * 8;
                let size_value = self.builder.ins().iconst(types::I64, size);
                let ptr = self.call_import(
                    "sw_object_new",
                    object_new_signature(self.module.isa()),
                    &[size_value],
                )?;
                let tag_value = self.builder.ins().iconst(types::I64, *tag);
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), tag_value, ptr, 0);
                for (index, field) in fields.iter().enumerate() {
                    let value = self.expr(field)?;
                    let value = if self.builder.func.dfg.value_type(value) == types::F64 {
                        self.builder
                            .ins()
                            .bitcast(types::I64, MemFlagsData::new(), value)
                    } else {
                        value
                    };
                    self.builder.ins().store(
                        MemFlagsData::new(),
                        value,
                        ptr,
                        ((index + 1) * 8) as i32,
                    );
                }
                ptr
            }
            MirExpr::EnumTag { object } => {
                let object = self.expr(object)?;
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, 0)
            }
            MirExpr::EnumField {
                object,
                index,
                elem,
            } => {
                let object = self.expr(object)?;
                let offset = ((index + 1) * 8) as i32;
                if elem.is_float() {
                    let bits =
                        self.builder
                            .ins()
                            .load(types::I64, MemFlagsData::new(), object, offset);
                    self.builder
                        .ins()
                        .bitcast(types::F64, MemFlagsData::new(), bits)
                } else {
                    self.builder
                        .ins()
                        .load(types::I64, MemFlagsData::new(), object, offset)
                }
            }
            MirExpr::TryPropagate {
                object,
                err_tag,
                ret_err_tag,
                elem,
            } => {
                let value = self.expr(object)?;
                let tag = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), value, 0);
                let err_tag_value = self.builder.ins().iconst(types::I64, *err_tag);
                let is_err = self.builder.ins().icmp(IntCC::Equal, tag, err_tag_value);
                let err_block = self.builder.create_block();
                let ok_block = self.builder.create_block();
                self.builder
                    .ins()
                    .brif(is_err, err_block, &[], ok_block, &[]);
                self.builder.switch_to_block(err_block);
                let err_field = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), value, 8);
                let size_value = self.builder.ins().iconst(types::I64, 16);
                let err_obj = self.call_import(
                    "sw_object_new",
                    object_new_signature(self.module.isa()),
                    &[size_value],
                )?;
                let ret_tag = self.builder.ins().iconst(types::I64, *ret_err_tag);
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), ret_tag, err_obj, 0);
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), err_field, err_obj, 8);
                self.builder.ins().return_(&[err_obj]);
                self.builder.switch_to_block(ok_block);
                self.builder.seal_block(ok_block);
                let ok_field = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), value, 8);
                if elem.is_float() {
                    self.builder
                        .ins()
                        .bitcast(types::F64, MemFlagsData::new(), ok_field)
                } else {
                    ok_field
                }
            }
            MirExpr::ArrayMap { .. } | MirExpr::ArrayFilter { .. } => {
                let (is_filter, object, closure, sig, elem, ret_elem) = match expr {
                    MirExpr::ArrayMap {
                        object,
                        closure,
                        sig,
                        elem,
                        ret_elem,
                    } => (false, object, closure, sig, elem, ret_elem),
                    MirExpr::ArrayFilter {
                        object,
                        closure,
                        sig,
                        elem,
                    } => (true, object, closure, sig, elem, &Type::Error),
                    _ => unreachable!(),
                };
                let array = self.expr(object)?;
                let closure_obj = self.expr(closure)?;
                let len = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), array, 0);
                let elem_size: i64 = if matches!(*elem, Type::U8) {
                    1
                } else if let Type::Struct(id) = *elem {
                    self.struct_sizes.get(&id).copied().unwrap_or(8) as i64
                } else {
                    8
                };
                let elem_size_value = self.builder.ins().iconst(types::I64, elem_size);
                let new_array = self.call_import(
                    "sw_array_new",
                    array_new_signature(self.module.isa()),
                    &[elem_size_value, len],
                )?;
                let index_slot = self.new_slot();
                let write_slot = self.new_slot();
                let zero = self.builder.ins().iconst(types::I64, 0);
                self.builder
                    .ins()
                    .stack_store(types::I64, zero, index_slot, 0);
                self.builder
                    .ins()
                    .stack_store(types::I64, zero, write_slot, 0);
                let key = format!("{:?}", sig);
                let sig_ref = self
                    .refs
                    .closure_sig_refs
                    .get(&key)
                    .copied()
                    .ok_or("闭包签名未导入")?;
                let header = self.builder.create_block();
                let body = self.builder.create_block();
                let exit = self.builder.create_block();
                self.builder.ins().jump(header, &[]);
                self.builder.switch_to_block(header);
                let index = self
                    .builder
                    .ins()
                    .stack_load(types::I64, types::I64, index_slot, 0);
                let cond = self.builder.ins().icmp(IntCC::UnsignedLessThan, index, len);
                self.builder.ins().brif(cond, body, &[], exit, &[]);
                self.builder.switch_to_block(body);
                let index = self
                    .builder
                    .ins()
                    .stack_load(types::I64, types::I64, index_slot, 0);
                let data = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), array, 16);
                let stride = self.builder.ins().iconst(types::I64, elem_size);
                let offset = self.builder.ins().imul(index, stride);
                let addr = self.builder.ins().iadd(data, offset);
                let item = if elem_size == 1 {
                    let byte = self
                        .builder
                        .ins()
                        .load(types::I8, MemFlagsData::new(), addr, 0);
                    self.builder.ins().uextend(types::I64, byte)
                } else if matches!(*elem, Type::Struct(_)) {
                    addr
                } else {
                    let value = self
                        .builder
                        .ins()
                        .load(types::I64, MemFlagsData::new(), addr, 0);
                    if elem.is_float() {
                        self.builder
                            .ins()
                            .bitcast(types::F64, MemFlagsData::new(), value)
                    } else {
                        value
                    }
                };
                let fn_ptr =
                    self.builder
                        .ins()
                        .load(types::I64, MemFlagsData::new(), closure_obj, 0);
                let env = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), closure_obj, 8);
                let call = self
                    .builder
                    .ins()
                    .call_indirect(sig_ref, fn_ptr, &[env, item]);
                let result = self.builder.inst_results(call)[0];
                if is_filter {
                    let zero_i64 = self.builder.ins().iconst(types::I64, 0);
                    let keep = self.builder.ins().icmp(IntCC::NotEqual, result, zero_i64);
                    let keep_block = self.builder.create_block();
                    let skip_block = self.builder.create_block();
                    self.builder
                        .ins()
                        .brif(keep, keep_block, &[], skip_block, &[]);
                    self.builder.switch_to_block(keep_block);
                    let write_index =
                        self.builder
                            .ins()
                            .stack_load(types::I64, types::I64, write_slot, 0);
                    let stored = if elem.is_float() {
                        self.builder
                            .ins()
                            .bitcast(types::I64, MemFlagsData::new(), item)
                    } else {
                        item
                    };
                    self.call_import(
                        "sw_array_set",
                        array_set_signature(self.module.isa()),
                        &[new_array, write_index, stored],
                    )?;
                    let next = self.builder.ins().iadd_imm(write_index, 1);
                    self.builder
                        .ins()
                        .stack_store(types::I64, next, write_slot, 0);
                    self.builder.ins().jump(skip_block, &[]);
                    self.builder.switch_to_block(skip_block);
                    self.builder.seal_block(skip_block);
                } else {
                    let stored = if ret_elem.is_float() {
                        self.builder
                            .ins()
                            .bitcast(types::I64, MemFlagsData::new(), result)
                    } else {
                        result
                    };
                    self.call_import(
                        "sw_array_set",
                        array_set_signature(self.module.isa()),
                        &[new_array, index, stored],
                    )?;
                }
                let index = self
                    .builder
                    .ins()
                    .stack_load(types::I64, types::I64, index_slot, 0);
                let next_index = self.builder.ins().iadd_imm(index, 1);
                self.builder
                    .ins()
                    .stack_store(types::I64, next_index, index_slot, 0);
                self.builder.ins().jump(header, &[]);
                self.builder.switch_to_block(exit);
                self.builder.seal_block(header);
                self.builder.seal_block(body);
                self.builder.seal_block(exit);
                if is_filter {
                    let written =
                        self.builder
                            .ins()
                            .stack_load(types::I64, types::I64, write_slot, 0);
                    self.builder
                        .ins()
                        .store(MemFlagsData::new(), written, new_array, 0);
                }
                new_array
            }
        })
    }

    fn expr_is_float(&self, expr: &MirExpr) -> bool {
        match expr {
            MirExpr::Float(_) => true,
            MirExpr::Cast { to, .. } => to.is_float(),
            _ => false,
        }
    }

    fn bool_cmp(&mut self, cond: IntCC, left: Value, right: Value) -> Value {
        let is_float = self.builder.func.dfg.value_type(left) == types::F64
            || self.builder.func.dfg.value_type(right) == types::F64;
        let result = if is_float {
            let cc = match cond {
                IntCC::Equal => FloatCC::Equal,
                IntCC::NotEqual => FloatCC::NotEqual,
                IntCC::SignedLessThan => FloatCC::LessThan,
                IntCC::SignedLessThanOrEqual => FloatCC::LessThanOrEqual,
                IntCC::SignedGreaterThan => FloatCC::GreaterThan,
                IntCC::SignedGreaterThanOrEqual => FloatCC::GreaterThanOrEqual,
                _ => {
                    return self.builder.ins().iconst(types::I64, 0);
                }
            };
            self.builder.ins().fcmp(cc, left, right)
        } else {
            self.builder.ins().icmp(cond, left, right)
        };
        self.builder.ins().uextend(types::I64, result)
    }

    fn call_import(
        &mut self,
        name: &str,
        sig: Signature,
        args: &[Value],
    ) -> Result<Value, CodegenError> {
        let func_ref = self
            .refs
            .func_refs
            .get(name)
            .copied()
            .ok_or_else(|| format!("运行函数 {name} 未声明"))?;
        let call = self.builder.ins().call(func_ref, args);
        let results = self.builder.inst_results(call);
        let _ = sig;
        Ok(if results.is_empty() {
            self.builder.ins().iconst(types::I64, 0)
        } else {
            results[0]
        })
    }

    fn string_len(&self, id: usize) -> usize {
        self.strings.get(id).map(|text| text.len()).unwrap_or(0)
    }
}

fn array_new_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn array_set_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}

fn object_new_signature(isa: &dyn cranelift_codegen::isa::TargetIsa) -> Signature {
    let mut sig = Signature::new(isa.default_call_conv());
    sig.params.push(AbiParam::new(types::I64));
    sig.returns.push(AbiParam::new(types::I64));
    sig
}
