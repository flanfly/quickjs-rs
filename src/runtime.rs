use std::ffi::CString;
use std::ptr;
use std::rc::Rc;
use std::str;

use quickjs_sys as sys;

use crate::Value;

struct RuntimePtr {
    runtime: *mut sys::JSRuntime,
}

impl Drop for RuntimePtr {
    fn drop(&mut self) {
        if !self.runtime.is_null() {
            unsafe {
                sys::JS_FreeRuntime(self.runtime as *mut _);
                self.runtime = ptr::null::<sys::JSRuntime>() as *mut _;
            }
        }
    }
}

pub struct Runtime {
    ptr: Rc<RuntimePtr>,
}

impl Default for Runtime {
    fn default() -> Self {
        unsafe {
            let rt = sys::JS_NewRuntime();

            assert!(!rt.is_null());
            Runtime { ptr: Rc::new(RuntimePtr { runtime: rt }) }
        }
    }
}

impl Runtime {
    pub fn context(&mut self) -> Context {
        unsafe {
            let ctx = sys::JS_NewContext(self.ptr.runtime as *mut _);
            assert!(!ctx.is_null());

            sys::js_std_add_helpers(
                ctx,
                1,
                [b"<none>\n".as_ptr() as *mut i8].as_mut_ptr(),
            );

            /* system modules */
            sys::js_init_module_std(ctx, b"std\0".as_ptr() as *const i8);
            sys::js_init_module_os(ctx, b"os\0".as_ptr() as *const i8);

            Context {
                ptr: ContextPtr::Owned(Rc::new(ContextPtrOwned {
                    context: ctx,
                    runtime: self.ptr.clone(),
                })),
            }
        }
    }
}

#[derive(Clone)]
pub struct ContextPtrOwned {
    pub(crate) context: *mut sys::JSContext,
    runtime: Rc<RuntimePtr>,
}

#[derive(Clone)]
pub(crate) enum ContextPtr {
    Owned(Rc<ContextPtrOwned>),
    Borrowed(*mut sys::JSContext),
}

impl Drop for ContextPtrOwned {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe {
                sys::JS_FreeContext(self.context);
            }
            self.context = ptr::null::<sys::JSContext>() as *mut _;
        }
    }
}

impl ContextPtr {
    pub(crate) fn as_ptr(&self) -> *mut sys::JSContext {
        match self {
            &ContextPtr::Owned(ref ctx) => ctx.context,
            &ContextPtr::Borrowed(ptr) => ptr,
        }
    }
}

pub struct Context {
    pub(crate) ptr: ContextPtr,
}

impl Context {
    pub fn eval(
        &mut self,
        input: &str,
        filename: &str,
        strict: bool,
        strip: bool,
    ) -> Result<Value, Value> {
        let input =
            CString::new(input).expect("Eval: input is not a valid string");
        let filename =
            CString::new(filename).expect("Eval: filename not a valid string");
        let mut flags = 0i32;

        if strict {
            flags |= sys::JS_EVAL_FLAG_STRICT as i32;
        }

        if strip {
            flags |= sys::JS_EVAL_FLAG_STRIP as i32;
        }

        let val = unsafe {
            let v = sys::JS_Eval(
                self.ptr.as_ptr(),
                input.as_ptr(),
                input.as_bytes().len(),
                filename.as_ptr(),
                flags | sys::JS_EVAL_TYPE_MODULE as i32,
            );

            Value { value: v, context: self.ptr.clone() }
        };

        if val.is_exception() {
            unsafe {
                let ex = sys::JS_GetException(self.ptr.as_ptr());
                Err(Value { value: ex, context: self.ptr.clone() })
            }
        } else {
            Ok(val)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_single_ctx() {
        let mut ctx = {
            let mut rt = Runtime::default();
            rt.context()
        };
        let _ = ctx
            .eval(r#"print('Hello, World\n');"#, "<test>", false, false)
            .unwrap();
    }

    #[test]
    fn eval_multiple_ctx() {
        let mut rt = Runtime::default();
        let _ = rt.context();
        let mut ctx2 = rt.context();

        let _ = ctx2
            .eval(r#"print('Hello, World\n');"#, "<test>", false, false)
            .unwrap();
    }
}
