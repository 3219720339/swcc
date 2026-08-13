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
    MirBinary, MirCallee, MirExpr, MirFunction, MirGlobal, MirModule, MirStmt, MirStmtKind,
    MirTarget, MirUnary, TypeTable,
};
use target_lexicon::Triple;

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
    /// Mach-O 汇编层符号带下划线前缀（Darwin 的 C ABI），ELF/COFF 不需要。
    symbol_prefix: &'static str,
}

impl Generator {
    fn new(target: Option<&str>) -> Result<Self, CodegenError> {
        let mut flag_builder = settings::builder();
        flag_builder.set("opt_level", "speed").unwrap();
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
            symbol_prefix: match target {
                Some(target) if target.contains("darwin") => "_",
                _ => "",
            },
        })
    }

    fn prefixed(&self, name: &str) -> String {
        format!("{}{}", self.symbol_prefix, name)
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
            self.declare_global(index as u32, global)?;
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
                .declare_function(
                    self.prefixed(&function.name).as_str(),
                    Linkage::Export,
                    &sig,
                )
                .map_err(|error| error.to_string())?;
            exports.insert(function.name.clone(), func_id);
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

    fn declare_global(&mut self, index: u32, global: &MirGlobal) -> Result<(), CodegenError> {
        let data_id = self
            .module
            .declare_data(global.name.as_str(), Linkage::Local, global.mutable, false)
            .map_err(|error| error.to_string())?;
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
            .declare_function(self.prefixed(name).as_str(), Linkage::Import, sig)
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

        let mut local_types: Vec<Type> = function
            .params
            .iter()
            .map(|param| param.ty.clone())
            .collect();
        local_types.extend(function.locals.iter().map(|local| local.ty.clone()));

        let mut lower = LowerCtx {
            builder,
            refs,
            slots: Vec::new(),
            loops: Vec::new(),
            module: &self.module,
            types,
            class_field_counts: class_field_counts(types),
            struct_field_counts: struct_field_counts(types),
            struct_field_types: struct_field_types(types),
            class_field_types: class_field_types(types),
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
        | MirExpr::Len { object: inner }
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
            for item in items {
                visit_expr(item, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                visit_expr(value, generator, mir, exports, ctx, refs)?;
            }
        }
        MirExpr::New { class, args } => {
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
            }
            for arg in args {
                visit_expr(arg, generator, mir, exports, ctx, refs)?;
            }
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
        MirCallee::Method { name, sig, .. } => (name.clone(), signature_of_sig(sig, isa)?),
        MirCallee::Extern { name, sig } => (name.clone(), signature_of_sig(sig, isa)?),
        MirCallee::Intrinsic { name } => {
            let runtime_name = intrinsic_name(name);
            let sig = intrinsic_signature(runtime_name, isa);
            (runtime_name.to_owned(), sig)
        }
        MirCallee::Closure { sig } => ("$closure".to_owned(), signature_of_sig(sig, isa)?),
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
        "int_to_string" => "sw_int_to_string",
        "uint_to_string" => "sw_uint_to_string",
        "float_to_string" => "sw_float_to_string",
        "char_to_string" => "sw_char_to_string",
        "bool_to_string" => "sw_bool_to_string",
        _ if name.starts_with("sw_") => name,
        _ => "sw_unimplemented",
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
        Type::Void => return Err("void 不能作为参数或返回值类型".into()),
        Type::Struct(_) => types::I64,
        Type::Interface(_) | Type::TypeParam(_) | Type::Unknown | Type::Error => {
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

fn struct_field_counts(types: &TypeTable) -> HashMap<u32, usize> {
    types
        .structs
        .iter()
        .enumerate()
        .map(|(index, info)| (index as u32, info.fields.len()))
        .collect()
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

struct LowerCtx<'a, 'f> {
    builder: FunctionBuilder<'f>,
    refs: RefTable,
    slots: Vec<StackSlot>,
    /// (循环头块, 循环出口块)
    loops: Vec<(Block, Block)>,
    module: &'a ObjectModule,
    types: &'a TypeTable,
    class_field_counts: HashMap<u32, usize>,
    struct_field_counts: HashMap<u32, usize>,
    struct_field_types: HashMap<u32, Vec<Type>>,
    class_field_types: HashMap<u32, Vec<Type>>,
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

    fn struct_size(&self, ty: &Type) -> usize {
        match ty {
            Type::Struct(id) => self.struct_field_counts.get(id).copied().unwrap_or(0) * 8,
            _ => 8,
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
                let float = self.field_is_float(object, *index);
                let object = self.expr(object)?;
                let offset = (*index as i32) * 8;
                let ty = if float { types::F64 } else { types::I64 };
                Ok(self
                    .builder
                    .ins()
                    .load(ty, MemFlagsData::new(), object, offset))
            }
            MirTarget::Index { object, index } => {
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let data = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, 16);
                let eight = self.builder.ins().iconst(types::I64, 8);
                let scaled = self.builder.ins().imul(index, eight);
                let address = self.builder.ins().iadd(data, scaled);
                Ok(self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), address, 0))
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
                let object = self.expr(object)?;
                let offset = (*index as i32) * 8;
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), value, object, offset);
                Ok(())
            }
            MirTarget::Index { object, index } => {
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let data = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, 16);
                let eight = self.builder.ins().iconst(types::I64, 8);
                let scaled = self.builder.ins().imul(index, eight);
                let address = self.builder.ins().iadd(data, scaled);
                self.builder
                    .ins()
                    .store(MemFlagsData::new(), value, address, 0);
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
                    MirCallee::Intrinsic { .. } => (Type::Void, false),
                };
                if closure_struct {
                    return Err("闭包暂不支持 struct 参数".into());
                }
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
                    _ => {
                        let name = match callee {
                            MirCallee::Function { name, .. }
                            | MirCallee::Method { name, .. }
                            | MirCallee::Extern { name, .. } => name.clone(),
                            MirCallee::Intrinsic { name } => intrinsic_name(name).to_owned(),
                            MirCallee::Closure { .. } => unreachable!(),
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
                let float = self.field_is_float(object, *index);
                let object = self.expr(object)?;
                let offset = (*index as i32) * 8;
                let ty = if float { types::F64 } else { types::I64 };
                self.builder
                    .ins()
                    .load(ty, MemFlagsData::new(), object, offset)
            }
            MirExpr::Index { object, index } => {
                let object = self.expr(object)?;
                let index = self.expr(index)?;
                let data = self
                    .builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, 16);
                let eight = self.builder.ins().iconst(types::I64, 8);
                let scaled = self.builder.ins().imul(index, eight);
                let address = self.builder.ins().iadd(data, scaled);
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), address, 0)
            }
            MirExpr::Len { object } => {
                let object = self.expr(object)?;
                self.builder
                    .ins()
                    .load(types::I64, MemFlagsData::new(), object, 0)
            }
            MirExpr::Array { elem, items } => {
                let elem_size = if matches!(**elem, Type::F32 | Type::F64) {
                    8
                } else {
                    8
                };
                let count = self.builder.ins().iconst(types::I64, items.len() as i64);
                let elem_size_value = self.builder.ins().iconst(types::I64, elem_size);
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
                    self.call_import(
                        "sw_array_set",
                        array_set_signature(self.module.isa()),
                        &[array, index_value, item],
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
                    let offset = (*index as i32) * 8;
                    self.builder
                        .ins()
                        .store(MemFlagsData::new(), value, address, offset);
                }
                address
            }
            MirExpr::New { class, args } => {
                let field_count = self.class_field_counts.get(class).copied().unwrap_or(0);
                let size = self
                    .builder
                    .ins()
                    .iconst(types::I64, (field_count * 8) as i64);
                let object = self.call_import(
                    "sw_object_new",
                    object_new_signature(self.module.isa()),
                    &[size],
                )?;
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
                } else if self.expr_is_float(inner) {
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
