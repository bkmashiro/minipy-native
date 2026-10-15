//! Source-dependent Cranelift lowering. The only raw access boundary is here.
use crate::{
    buffer::Quote,
    ir::{self, BinOp, CompileError, Expr, Stmt},
};
use cranelift_codegen::{
    ir::{AbiParam, InstBuilder, MemFlags, Value, condcodes::IntCC, types},
    settings::{self, Configurable},
};
use cranelift_frontend::Switch;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::cell::RefCell;
thread_local! {
    // Compiler scratch only: clear all function/code state between unknown sources.
    static SCRATCH: RefCell<Option<(cranelift_codegen::Context, FunctionBuilderContext)>> = const { RefCell::new(None) };
}

#[repr(C)]
pub(crate) struct Frame {
    pub locals: [i64; ir::MAX_LOCALS],
    pub index: u64,
    pub pc: u64,
    pub total: i64,
    pub batch: i64,
    pub target: u64,
    pub remaining: u64,
    pub chunk: u64,
    pub live: u64,
}
impl Frame {
    pub(crate) fn new(inputs: &[i64], remaining: u64, chunk: usize) -> Self {
        let mut locals = [0; ir::MAX_LOCALS];
        locals[..inputs.len()].copy_from_slice(inputs);
        Self {
            locals,
            index: 0,
            pc: 0,
            total: 0,
            batch: 0,
            target: 0,
            remaining,
            chunk: chunk as u64,
            live: 0,
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(crate) struct View {
    pub ptr: *const Quote,
    pub len: u64,
}
type Entry = unsafe extern "C" fn(*mut Frame, *const View, *mut crate::runtime::Output) -> i64;
/// Not Send/Sync: this first dispatcher and code ownership are thread-local.
pub struct CodeUnit {
    module: Option<JITModule>,
    entry: Entry,
    pub code_bytes: usize,
    /// Actual lowered CLIF when explicitly requested; empty for ordinary compilation.
    pub clif: String,
    pub(crate) inputs: Vec<ir::Type>,
    pub(crate) buffers: usize,
    pub(crate) legacy: bool,
    #[cfg(feature = "tool-json")]
    pub(crate) json: Option<std::rc::Rc<crate::tool::Plan>>,
}
impl Drop for CodeUnit {
    fn drop(&mut self) {
        // No exported function pointer or surviving task can call after owner drop.
        if let Some(module) = self.module.take() {
            unsafe {
                module.free_memory();
            }
        }
    }
}
impl CodeUnit {
    pub fn is_legacy(&self) -> bool {
        self.legacy
    }
    pub fn compile(source: &str) -> Result<Self, CompileError> {
        Self::compile_with_opt(source, "speed")
    }
    pub fn compile_with_opt(source: &str, opt: &str) -> Result<Self, CompileError> {
        Self::compile_internal(source, opt, false)
    }
    /// Compile with the actual lowered CLIF retained for diagnostics.
    /// Pass timing is separately enabled with the `compile-timing` Cargo feature.
    pub fn compile_with_ir(source: &str, opt: &str) -> Result<Self, CompileError> {
        Self::compile_internal(source, opt, true)
    }
    fn compile_internal(source: &str, opt: &str, retain_ir: bool) -> Result<Self, CompileError> {
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        let program = crate::frontend::parse(source)?;
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| {
            p.frontend_ns = crate::compiler_profile::elapsed(start)
        });
        Self::lower(program, opt, retain_ir)
    }
    /// Bind the declared Host input schema at compile time. No runtime names.
    pub fn compile_script(source: &str, inputs: &[(&str, ir::Type)]) -> Result<Self, CompileError> {
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        let program = crate::frontend::parse_with_inputs(source, inputs)?;
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| {
            p.frontend_ns = crate::compiler_profile::elapsed(start)
        });
        Self::lower(program, "speed", false)
    }
    #[cfg(feature = "tool-json")]
    pub fn compile_with_tools(
        source: &str,
        inputs: &[(&str, ir::Type)],
        tools: &[crate::tool::ToolSchema],
    ) -> Result<Self, CompileError> {
        Self::lower(
            crate::frontend::parse_with_tools(source, inputs, tools)?,
            "speed",
            false,
        )
    }
    fn lower(program: ir::Program, opt: &str, retain_ir: bool) -> Result<Self, CompileError> {
        let (mut ctx, mut fc) = SCRATCH.with(|s| s.borrow_mut().take()).unwrap_or_else(|| {
            (
                cranelift_codegen::Context::new(),
                FunctionBuilderContext::new(),
            )
        });
        let result = Self::lower_into(program, opt, retain_ir, &mut ctx, &mut fc);
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        ctx.clear();
        SCRATCH.with(|s| *s.borrow_mut() = Some((ctx, fc)));
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| p.cleanup_ns = crate::compiler_profile::elapsed(start));
        result
    }
    fn lower_into(
        program: ir::Program,
        opt: &str,
        retain_ir: bool,
        ctx: &mut cranelift_codegen::Context,
        fc: &mut FunctionBuilderContext,
    ) -> Result<Self, CompileError> {
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        let mut flags = settings::builder();
        flags.set("opt_level", opt).map_err(error)?;
        let isa = cranelift_native::builder()
            .map_err(error)?
            .finish(settings::Flags::new(flags))
            .map_err(error)?;
        if isa.pointer_type() != types::I64 {
            return Err(CompileError::new(
                ir::AdmissionKind::Backend,
                "only 64-bit hosts supported",
            ));
        }
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        builder.symbol("minipy_print", crate::runtime::native_print as *const u8);
        #[cfg(feature = "tool-json")]
        if program.json.roots > 0 {
            builder.symbol(
                "minipy_json_read",
                crate::json_view::native_read as *const u8,
            );
            builder.symbol(
                "minipy_json_release",
                crate::json_view::native_release as *const u8,
            );
            if program.json.string_paths {
                builder.symbol(
                    "minipy_json_string",
                    crate::json_view::native_string as *const u8,
                );
            }
        }
        let mut module = JITModule::new(builder);
        module.clear_context(ctx);
        ctx.func.signature.params = vec![AbiParam::new(types::I64); 3];
        ctx.func.signature.returns.push(AbiParam::new(types::I64));
        let mut print_sig = module.make_signature();
        print_sig.params = vec![AbiParam::new(types::I64); 3];
        print_sig.returns.push(AbiParam::new(types::I64));
        let print = module
            .declare_function("minipy_print", Linkage::Import, &print_sig)
            .map_err(error)?;
        let print = module.declare_func_in_func(print, &mut ctx.func);
        #[cfg(feature = "tool-json")]
        let json_read = if program.json.roots > 0 {
            let id = module
                .declare_function("minipy_json_read", Linkage::Import, &print_sig)
                .map_err(error)?;
            Some(module.declare_func_in_func(id, &mut ctx.func))
        } else {
            None
        };
        #[cfg(feature = "tool-json")]
        let json_string = if program.json.string_paths {
            let mut sig = module.make_signature();
            sig.params = vec![AbiParam::new(types::I64); 4];
            sig.returns.push(AbiParam::new(types::I64));
            let id = module
                .declare_function("minipy_json_string", Linkage::Import, &sig)
                .map_err(error)?;
            Some(module.declare_func_in_func(id, &mut ctx.func))
        } else {
            None
        };
        #[cfg(feature = "tool-json")]
        let json_release = if program.json.roots > 0 {
            let mut sig = module.make_signature();
            sig.params = vec![AbiParam::new(types::I64); 2];
            let id = module
                .declare_function("minipy_json_release", Linkage::Import, &sig)
                .map_err(error)?;
            Some(module.declare_func_in_func(id, &mut ctx.func))
        } else {
            None
        };
        let id = module
            .declare_function("scan", Linkage::Local, &ctx.func.signature)
            .map_err(error)?;
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| {
            p.setup_ns = crate::compiler_profile::elapsed(start);
            p.scalar_slots = program.locals.len();
            p.buffer_slots = program.buffers;
        });
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        {
            let mut b = FunctionBuilder::new(&mut ctx.func, fc);
            let entry = b.create_block();
            b.append_block_params_for_function_params(entry);
            b.switch_to_block(entry);
            let params = b.block_params(entry).to_vec();
            let frame = params[0];
            let views = params[1];
            let output = params[2];
            let vars: Vec<_> = program
                .locals
                .iter()
                .map(|_| b.declare_var(types::I64))
                .collect();
            for (i, var) in vars.iter().enumerate() {
                let v = b
                    .ins()
                    .load(types::I64, MemFlags::trusted(), frame, (i * 8) as i32);
                b.def_var(*var, v);
            }
            let overflow = b.create_block();
            let budget = b.create_block();
            let output_error = b.create_block();
            let start = b.create_block();
            let dispatch = b.create_block();
            b.ins().jump(dispatch, &[]);
            // Fill the resume switch after lowering has assigned all PCs.
            b.switch_to_block(start);
            let mut flow = Flow {
                b: &mut b,
                vars: &vars,
                frame,
                views,
                output,
                print,
                #[cfg(feature = "tool-json")]
                json_read,
                #[cfg(feature = "tool-json")]
                json_string,
                #[cfg(feature = "tool-json")]
                json_release,

                overflow,
                budget,
                output_error,
                resumes: vec![start],
                row: None,
                inputs: program.inputs,
            };
            flow.stmts(&program.statements, Live::default());
            let resumes = flow.resumes;
            #[cfg(feature = "compiler-profile")]
            crate::compiler_profile::update(|p| p.resume_states = resumes.len());
            b.switch_to_block(dispatch);
            let pc = b.ins().load(types::I64, MemFlags::trusted(), frame, 264);
            let mut switch = Switch::new();
            for (i, block) in resumes.into_iter().enumerate() {
                switch.set_entry(i as u128, block);
            }
            switch.emit(&mut b, pc, budget);
            b.switch_to_block(overflow);
            let one = b.ins().iconst(types::I64, 1);
            b.ins().return_(&[one]);
            b.switch_to_block(budget);
            let status = b.ins().iconst(types::I64, 5);
            b.ins().return_(&[status]);
            b.switch_to_block(output_error);
            let status = b.ins().iconst(types::I64, 6);
            b.ins().return_(&[status]);
            b.seal_all_blocks();
            b.finalize();
        }
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| {
            p.lowering_ns = crate::compiler_profile::elapsed(start)
        });
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::inventory(&ctx.func, true);
        let clif = if retain_ir {
            ctx.func.display().to_string()
        } else {
            String::new()
        };
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        module.define_function(id, ctx).map_err(error)?;
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| p.backend_ns = crate::compiler_profile::elapsed(start));
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::inventory(&ctx.func, false);
        let code_bytes = ctx
            .compiled_code()
            .ok_or_else(|| CompileError::new(ir::AdmissionKind::Backend, "missing machine code"))?
            .code_info()
            .total_size as usize;
        #[cfg(feature = "compiler-profile")]
        let start = std::time::Instant::now();
        module.finalize_definitions().map_err(error)?;
        let ptr = module.get_finalized_function(id);
        // Signature and frame offsets above are fixed, audited, and layout-tested.
        let entry: Entry = unsafe { std::mem::transmute(ptr) };
        #[cfg(feature = "compiler-profile")]
        crate::compiler_profile::update(|p| {
            p.finalize_ns = crate::compiler_profile::elapsed(start)
        });
        Ok(Self {
            module: Some(module),
            entry,
            code_bytes,
            clif,
            inputs: program.locals[..program.inputs].to_vec(),
            buffers: program.buffers,
            legacy: program.legacy,
            #[cfg(feature = "tool-json")]
            json: (!program.json.calls.is_empty()).then(|| std::rc::Rc::new(program.json)),
        })
    }
    pub(crate) fn resume(
        &self,
        frame: &mut Frame,
        views: &[View],
        output: &mut crate::runtime::Output,
    ) -> i64 {
        // Runtime-owned views are validated immutable leases. Lowering only uses
        // statically bound buffer/field slots and index < validated length.
        debug_assert_eq!(views.len(), self.buffers);
        unsafe { (self.entry)(frame, views.as_ptr(), output) }
    }
}
fn error(e: impl std::fmt::Display) -> CompileError {
    CompileError::new(ir::AdmissionKind::Backend, e.to_string())
}
/// Compile structured statements directly to native CFG. PCs exist only at
/// actual Host waits and row-chunk yields, never as an interpreted instruction stream.
struct Flow<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    vars: &'a [Variable],
    frame: Value,
    views: Value,
    output: Value,
    print: cranelift_codegen::ir::FuncRef,
    #[cfg(feature = "tool-json")]
    json_read: Option<cranelift_codegen::ir::FuncRef>,
    #[cfg(feature = "tool-json")]
    json_string: Option<cranelift_codegen::ir::FuncRef>,
    #[cfg(feature = "tool-json")]
    json_release: Option<cranelift_codegen::ir::FuncRef>,
    overflow: cranelift_codegen::ir::Block,
    budget: cranelift_codegen::ir::Block,
    output_error: cranelift_codegen::ir::Block,
    resumes: Vec<cranelift_codegen::ir::Block>,
    row: Option<Value>,
    inputs: usize,
}
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Live {
    scalars: u32,
    buffers: u64,
    #[cfg(feature = "tool-json")]
    json: u32,
}
impl Live {
    fn union(self, other: Self) -> Self {
        Self {
            scalars: self.scalars | other.scalars,
            buffers: self.buffers | other.buffers,
            #[cfg(feature = "tool-json")]
            json: self.json | other.json,
        }
    }
}
fn used(e: &Expr) -> u32 {
    match e {
        Expr::Local(i) => 1u32 << i,
        Expr::Binary(_, a, b) | Expr::Min(a, b) | Expr::Max(a, b) => used(a) | used(b),
        Expr::Not(e) | Expr::Abs(e) => used(e),
        _ => 0,
    }
}
fn buffer_used(e: &Expr) -> u64 {
    match e {
        Expr::BufferLen(i) => 1u64 << i,
        Expr::Binary(_, a, b) | Expr::Min(a, b) | Expr::Max(a, b) => {
            buffer_used(a) | buffer_used(b)
        }
        Expr::Not(e) | Expr::Abs(e) => buffer_used(e),
        _ => 0,
    }
}
fn live_before(ss: &[Stmt], following: Live) -> Live {
    ss.iter()
        .rev()
        .fold(following, |live, s| live_statement(s, live))
}
fn live_loop(buffer: usize, body: &[Stmt], following: Live) -> Live {
    let mut live = following;
    live.buffers |= 1u64 << buffer;
    loop {
        let next = live.union(live_before(body, live));
        if next == live {
            return live;
        }
        live = next;
    }
}
#[cfg(feature = "tool-json")]
fn json_used(e: &Expr) -> u32 {
    match e {
        Expr::JsonI64 { root, .. }
        | Expr::JsonView { root, .. }
        | Expr::JsonString { root, .. }
        | Expr::Contains { root, .. } => 1 << root,
        Expr::Binary(_, a, b) | Expr::Min(a, b) | Expr::Max(a, b) => json_used(a) | json_used(b),
        Expr::Not(e) | Expr::Abs(e) => json_used(e),
        _ => 0,
    }
}
fn live_statement(s: &Stmt, mut live: Live) -> Live {
    match s {
        Stmt::Set(_, e) | Stmt::Load(_, e) | Stmt::Eval(e) | Stmt::Print(e, _) => {
            live.buffers |= buffer_used(e)
        }
        Stmt::Record(_, a, b) => live.buffers |= buffer_used(a) | buffer_used(b),
        #[cfg(feature = "tool-json")]
        Stmt::Tool { args, .. } => {
            for e in args {
                live.buffers |= buffer_used(e);
            }
        }
        Stmt::If(_, _, _) => {}
        Stmt::Return(_) | Stmt::For(_, _) => {}
    }
    #[cfg(feature = "tool-json")]
    {
        match s {
            Stmt::Set(_, e) | Stmt::Load(_, e) | Stmt::Eval(e) | Stmt::Print(e, _) => {
                live.json |= json_used(e)
            }
            Stmt::Record(_, a, b) => live.json |= json_used(a) | json_used(b),

            _ => {}
        }
    }
    match s {
        Stmt::Set(i, e) => live.scalars = (live.scalars & !(1u32 << i)) | used(e),
        Stmt::Record(i, batch, total) => {
            live.scalars = (live.scalars & !(1u32 << i)) | used(batch) | used(total)
        }
        Stmt::Load(i, batch) => {
            live.buffers &= !(1u64 << i);
            live.scalars |= used(batch);
        }
        Stmt::Print(e, _) | Stmt::Eval(e) => live.scalars |= used(e),
        Stmt::Return(e) => {
            return Live {
                scalars: used(e),
                buffers: buffer_used(e),
                #[cfg(feature = "tool-json")]
                json: json_used(e),
            };
        }
        Stmt::If(e, yes, no) => {
            live = live_before(yes, live).union(live_before(no, live));
            live.scalars |= used(e);
            live.buffers |= buffer_used(e);
            #[cfg(feature = "tool-json")]
            {
                live.json |= json_used(e);
            }
        }
        Stmt::For(i, body) => live = live_loop(*i, body, live),
        #[cfg(feature = "tool-json")]
        Stmt::Tool {
            target, json, args, ..
        } => {
            if *json {
                live.json &= !(1 << target);
            } else {
                live.scalars &= !(1 << target);
            }
            for e in args {
                live.scalars |= used(e);
                live.json |= json_used(e);
            }
        }
    }
    live
}
impl Flow<'_, '_> {
    fn expr(&mut self, expr: &Expr) -> Value {
        Emit {
            b: self.b,
            vars: self.vars,
            row: self.row,
            views: self.views,
            overflow: self.overflow,
            #[cfg(feature = "tool-json")]
            json: self
                .json_read
                .map(|read| (self.output, self.frame, read, self.output_error)),
            #[cfg(feature = "tool-json")]
            json_string: self.json_string,
        }
        .expr(expr)
    }
    fn store(&mut self, offset: i32, value: Value) {
        self.b
            .ins()
            .store(MemFlags::trusted(), value, self.frame, offset);
    }
    fn constant(&mut self, offset: i32, value: i64) {
        let v = self.b.ins().iconst(types::I64, value);
        self.store(offset, v);
    }
    fn load(&mut self, offset: i32) -> Value {
        self.b
            .ins()
            .load(types::I64, MemFlags::trusted(), self.frame, offset)
    }
    fn spill(&mut self, live: u32) {
        for (i, var) in self.vars.iter().enumerate() {
            // Host inputs are immutable and already resident. Other slots only
            // need writeback when the next native turn can read their value.
            if i >= self.inputs && live & (1u32 << i) != 0 {
                let v = self.b.use_var(*var);
                self.store((i * 8) as i32, v);
            }
        }
    }
    fn suspend(&mut self, status: i64, pc: usize, live: Live) {
        #[cfg(feature = "tool-json")]
        if let Some(helper) = self.json_release {
            let mask = self.b.ins().iconst(types::I64, live.json as i64);
            self.b.ins().call(helper, &[self.output, mask]);
        }
        self.spill(live.scalars);
        self.constant(264, pc as i64);
        self.constant(312, live.buffers as i64);
        let status = self.b.ins().iconst(types::I64, status);
        self.b.ins().return_(&[status]);
    }
    fn continuation(&mut self) -> (usize, cranelift_codegen::ir::Block) {
        let block = self.b.create_block();
        let pc = self.resumes.len();
        self.resumes.push(block);
        (pc, block)
    }
    fn stmts(&mut self, ss: &[Stmt], following: Live) {
        let mut after = vec![following; ss.len()];
        let mut live = following;
        for (i, s) in ss.iter().enumerate().rev() {
            after[i] = live;
            live = live_statement(s, live);
        }
        for (position, stmt) in ss.iter().enumerate() {
            let mut live = after[position];
            match stmt {
                Stmt::Set(i, e) => {
                    let value = self.expr(e);
                    self.b.def_var(self.vars[*i], value);
                }
                Stmt::Eval(e) => {
                    self.expr(e);
                }
                Stmt::If(e, yes, no) => {
                    let y = self.b.create_block();
                    let n = self.b.create_block();
                    let join = self.b.create_block();
                    Emit {
                        b: self.b,
                        vars: self.vars,
                        row: self.row,
                        views: self.views,
                        overflow: self.overflow,
                        #[cfg(feature = "tool-json")]
                        json: self
                            .json_read
                            .map(|read| (self.output, self.frame, read, self.output_error)),
                        #[cfg(feature = "tool-json")]
                        json_string: self.json_string,
                    }
                    .predicate(e, y, n);
                    self.b.switch_to_block(y);
                    self.stmts(yes, live);
                    self.b.ins().jump(join, &[]);
                    self.b.switch_to_block(n);
                    self.stmts(no, live);
                    self.b.ins().jump(join, &[]);
                    self.b.switch_to_block(join);
                }
                #[cfg(feature = "tool-json")]
                Stmt::Tool {
                    call,
                    target,
                    json,
                    args,
                } => {
                    let a = args
                        .first()
                        .map(|e| self.expr(e))
                        .unwrap_or_else(|| self.b.ins().iconst(types::I64, 0));
                    let b = args
                        .get(1)
                        .map(|e| self.expr(e))
                        .unwrap_or_else(|| self.b.ins().iconst(types::I64, 0));
                    self.store(280, a);
                    self.store(272, b);
                    self.constant(288, *target as i64);
                    if !*json {
                        live.scalars &= !(1 << target);
                    }
                    let (pc, next) = self.continuation();
                    self.suspend(8 + *call as i64, pc, live);
                    self.b.switch_to_block(next);
                }
                Stmt::Load(target, batch) => {
                    let v = self.expr(batch);
                    self.store(280, v);
                    self.constant(288, *target as i64);
                    let (pc, next) = self.continuation();
                    self.suspend(2, pc, live);
                    self.b.switch_to_block(next);
                }
                Stmt::Record(target, batch, total) => {
                    let batch = self.expr(batch);
                    let total = self.expr(total);
                    self.store(280, batch);
                    self.store(272, total);
                    self.constant(288, *target as i64);
                    let (pc, next) = self.continuation();
                    let terminal = matches!(&ss[position + 1..], [Stmt::Return(Expr::Local(i))] if i == target);
                    // The Host supplies the new return value in this scalar slot.
                    live.scalars &= !(1u32 << target);
                    self.suspend(if terminal { 7 } else { 3 }, pc, live);
                    self.b.switch_to_block(next);
                }
                Stmt::Print(e, ty) => {
                    let value = self.expr(e);
                    let kind = self
                        .b
                        .ins()
                        .iconst(types::I64, i64::from(*ty == ir::Type::Bool));
                    let call = self.b.ins().call(self.print, &[self.output, value, kind]);
                    let failed = self.b.inst_results(call)[0];
                    let next = self.b.create_block();
                    self.b.ins().brif(failed, self.output_error, &[], next, &[]);
                    self.b.switch_to_block(next);
                }
                Stmt::For(buffer, body) => self.scan(*buffer, body, live),
                Stmt::Return(e) => {
                    let value = self.expr(e);
                    self.store(272, value);
                    self.constant(312, 0);
                    let done = self.b.ins().iconst(types::I64, 0);
                    self.b.ins().return_(&[done]);
                }
            }
        }
    }
    fn scan(&mut self, buffer: usize, body: &[Stmt], following: Live) {
        let live = live_loop(buffer, body, following);
        self.constant(256, 0);
        let (pc, chunk) = self.continuation();
        self.b.ins().jump(chunk, &[]);
        self.b.switch_to_block(chunk);
        let index = self.load(256);
        let len = self.b.ins().load(
            types::I64,
            MemFlags::trusted(),
            self.views,
            (buffer * 16 + 8) as i32,
        );
        let rows = self.b.ins().load(
            types::I64,
            MemFlags::trusted(),
            self.views,
            (buffer * 16) as i32,
        );
        let left = self.b.ins().isub(len, index);
        let quota = self.load(296);
        let max_chunk = self.load(304);
        let count = self.b.ins().umin(left, max_chunk);
        let count = self.b.ins().umin(count, quota);
        let left_nonzero = self.b.ins().icmp_imm(IntCC::NotEqual, left, 0);
        let empty_quota = self.b.ins().icmp_imm(IntCC::Equal, quota, 0);
        let exhausted = self.b.ins().band(left_nonzero, empty_quota);
        let reserve = self.b.create_block();
        self.b.ins().brif(exhausted, self.budget, &[], reserve, &[]);
        self.b.switch_to_block(reserve);
        let remaining = self.b.ins().isub(quota, count);
        self.store(296, remaining);
        let end = self.b.ins().iadd(index, count);
        let ix = self.b.declare_var(types::I64);
        self.b.def_var(ix, index);
        let header = self.b.create_block();
        let row_body = self.b.create_block();
        let finish = self.b.create_block();
        self.b.ins().jump(header, &[]);
        self.b.switch_to_block(header);
        let i = self.b.use_var(ix);
        let more = self.b.ins().icmp(IntCC::UnsignedLessThan, i, end);
        self.b.ins().brif(more, row_body, &[], finish, &[]);
        self.b.switch_to_block(row_body);
        let offset = self.b.ins().ishl_imm(i, 4);
        self.row = Some(self.b.ins().iadd(rows, offset));
        self.stmts(body, live);
        self.row = None;
        let i = self.b.use_var(ix);
        let next = self.b.ins().iadd_imm(i, 1);
        self.b.def_var(ix, next);
        self.b.ins().jump(header, &[]);
        self.b.switch_to_block(finish);
        let i = self.b.use_var(ix);
        self.store(256, i);
        let more = self.b.ins().icmp(IntCC::UnsignedLessThan, i, len);
        let yield_block = self.b.create_block();
        let after = self.b.create_block();
        self.b.ins().brif(more, yield_block, &[], after, &[]);
        self.b.switch_to_block(yield_block);
        self.suspend(4, pc, live);
        self.b.switch_to_block(after);
    }
}
struct Emit<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    vars: &'a [Variable],
    row: Option<Value>,
    views: Value,
    overflow: cranelift_codegen::ir::Block,
    #[cfg(feature = "tool-json")]
    json: Option<(
        Value,
        Value,
        cranelift_codegen::ir::FuncRef,
        cranelift_codegen::ir::Block,
    )>,
    #[cfg(feature = "tool-json")]
    json_string: Option<cranelift_codegen::ir::FuncRef>,
}
impl Emit<'_, '_> {
    // Predicate consumers need control flow, not a materialized bool join.
    // Recurse left-to-right; only the selected RHS may run (including errors).
    // Value consumers still use expr below, including bool assignment/print.
    fn predicate(
        &mut self,
        e: &Expr,
        yes: cranelift_codegen::ir::Block,
        no: cranelift_codegen::ir::Block,
    ) {
        match e {
            Expr::Not(x) => self.predicate(x, no, yes),
            Expr::Binary(BinOp::And, l, r) => {
                let rhs = self.b.create_block();
                self.predicate(l, rhs, no);
                self.b.switch_to_block(rhs);
                self.predicate(r, yes, no);
            }
            Expr::Binary(BinOp::Or, l, r) => {
                let rhs = self.b.create_block();
                self.predicate(l, yes, rhs);
                self.b.switch_to_block(rhs);
                self.predicate(r, yes, no);
            }
            _ => {
                let value = self.expr(e);
                self.b.ins().brif(value, yes, &[], no, &[]);
            }
        }
    }
    fn expr(&mut self, e: &Expr) -> Value {
        match e {
            Expr::Int(v) => self.b.ins().iconst(types::I64, *v),
            Expr::Bool(v) => self.b.ins().iconst(types::I64, i64::from(*v)),
            Expr::Local(i) => self.b.use_var(self.vars[*i]),
            Expr::BufferLen(i) => self.b.ins().load(
                types::I64,
                MemFlags::trusted(),
                self.views,
                (i * 16 + 8) as i32,
            ),
            Expr::Abs(x) => {
                let v = self.expr(x);
                let min = self.b.ins().iconst(types::I64, i64::MIN);
                let overflow = self.b.ins().icmp(IntCC::Equal, v, min);
                let next = self.b.create_block();
                self.b.ins().brif(overflow, self.overflow, &[], next, &[]);
                self.b.switch_to_block(next);
                let negative = self.b.ins().icmp_imm(IntCC::SignedLessThan, v, 0);
                let negated = self.b.ins().ineg(v);
                self.b.ins().select(negative, negated, v)
            }
            Expr::Min(a, b) | Expr::Max(a, b) => {
                let a = self.expr(a);
                let b = self.expr(b);
                if matches!(e, Expr::Min(..)) {
                    self.b.ins().smin(a, b)
                } else {
                    self.b.ins().smax(a, b)
                }
            }
            #[cfg(feature = "tool-json")]
            Expr::JsonI64 { slot, .. } | Expr::JsonView { slot, .. } => {
                let (output, frame, helper, error) = self.json.expect("frontend bound JSON plan");
                let slot = if matches!(e, Expr::JsonView { .. }) {
                    -(*slot as i64) - 1
                } else {
                    *slot as i64
                };
                let slot = self.b.ins().iconst(types::I64, slot);
                let result = self.b.ins().iadd_imm(frame, 272);
                let call = self.b.ins().call(helper, &[output, slot, result]);
                let failed = self.b.inst_results(call)[0];
                let next = self.b.create_block();
                self.b.ins().brif(failed, error, &[], next, &[]);
                self.b.switch_to_block(next);
                self.b
                    .ins()
                    .load(types::I64, MemFlags::trusted(), frame, 272)
            }
            #[cfg(feature = "tool-json")]
            Expr::JsonString { slot, .. } | Expr::Contains { slot, .. } => {
                let (output, frame, _, error) = self.json.expect("frontend bound JSON plan");
                let needle = match e {
                    Expr::Contains { needle, .. } => *needle as i64,
                    _ => -1,
                };
                let slot = self.b.ins().iconst(types::I64, *slot as i64);
                let needle = self.b.ins().iconst(types::I64, needle);
                let result = self.b.ins().iadd_imm(frame, 272);
                let call = self.b.ins().call(
                    self.json_string.expect("string helper"),
                    &[output, slot, needle, result],
                );
                let failed = self.b.inst_results(call)[0];
                let next = self.b.create_block();
                self.b.ins().brif(failed, error, &[], next, &[]);
                self.b.switch_to_block(next);
                self.b
                    .ins()
                    .load(types::I64, MemFlags::trusted(), frame, 272)
            }
            Expr::Field(f) => self.b.ins().load(
                types::I64,
                MemFlags::trusted(),
                self.row.expect("frontend prohibits fields outside scan"),
                match f {
                    ir::Field::Price => 0,
                    ir::Field::Quantity => 8,
                },
            ),
            Expr::Not(x) => {
                let v = self.expr(x);
                let x = self.b.ins().icmp_imm(IntCC::Equal, v, 0);
                self.b.ins().uextend(types::I64, x)
            }
            Expr::Binary(op, l, r) => {
                let a = self.expr(l);
                if matches!(op, BinOp::And | BinOp::Or) {
                    let rhs = self.b.create_block();
                    let skip = self.b.create_block();
                    let join = self.b.create_block();
                    self.b.append_block_param(join, types::I64);
                    if matches!(op, BinOp::And) {
                        self.b.ins().brif(a, rhs, &[], skip, &[]);
                    } else {
                        self.b.ins().brif(a, skip, &[], rhs, &[]);
                    }
                    self.b.switch_to_block(skip);
                    self.b.ins().jump(join, &[a.into()]);
                    self.b.switch_to_block(rhs);
                    let v = self.expr(r);
                    self.b.ins().jump(join, &[v.into()]);
                    self.b.switch_to_block(join);
                    return self.b.block_params(join)[0];
                }
                let c = self.expr(r);
                if matches!(op, BinOp::Add | BinOp::Sub | BinOp::Mul) {
                    let (v, o) = match op {
                        BinOp::Add => self.b.ins().sadd_overflow(a, c),
                        BinOp::Sub => self.b.ins().ssub_overflow(a, c),
                        _ => self.b.ins().smul_overflow(a, c),
                    };
                    let next = self.b.create_block();
                    self.b.ins().brif(o, self.overflow, &[], next, &[]);
                    self.b.switch_to_block(next);
                    v
                } else {
                    let cc = match op {
                        BinOp::Eq => IntCC::Equal,
                        BinOp::Ne => IntCC::NotEqual,
                        BinOp::Lt => IntCC::SignedLessThan,
                        BinOp::Le => IntCC::SignedLessThanOrEqual,
                        BinOp::Gt => IntCC::SignedGreaterThan,
                        BinOp::Ge => IntCC::SignedGreaterThanOrEqual,
                        _ => unreachable!(),
                    };
                    let v = self.b.ins().icmp(cc, a, c);
                    self.b.ins().uextend(types::I64, v)
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout() {
        assert_eq!(std::mem::offset_of!(Frame, index), 256);
        assert_eq!(std::mem::offset_of!(Frame, pc), 264);
        assert_eq!(std::mem::offset_of!(Frame, total), 272);
        assert_eq!(std::mem::offset_of!(Frame, batch), 280);
        assert_eq!(std::mem::offset_of!(Frame, target), 288);
        assert_eq!(std::mem::offset_of!(Frame, remaining), 296);
        assert_eq!(std::mem::offset_of!(Frame, chunk), 304);
        assert_eq!(std::mem::offset_of!(Frame, live), 312);
        assert_eq!(std::mem::size_of::<Frame>(), 320);
        assert_eq!(std::mem::size_of::<View>(), 16);
    }
}
