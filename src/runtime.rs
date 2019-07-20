use std::ffi::{CStr, CString};
use std::fmt;
use std::i64;
use std::f64;
use std::ptr;
use std::rc::Rc;
use std::slice;
use std::str;

use quickjs_sys::{
    JSContext, JSRefCountHeader, JSRuntime, JSValue, JS_Eval, JS_FreeContext,
    JS_FreeRuntime, JS_NewContext, JS_NewRuntime, JS_SetRuntimeInfo,
    __JS_FreeValueRT, js_init_module_os, js_init_module_std,
    js_std_add_helpers, js_std_dump_error, JS_GetException, JS_NewInt64,
    JS_NewStringLen, JS_ToCStringLen, JS_ToInt64, JS_EVAL_FLAG_SHEBANG,
    JS_EVAL_FLAG_STRICT, JS_EVAL_FLAG_STRIP, JS_EVAL_TYPE_MODULE, JS_TAG_BOOL,
    JS_TAG_EXCEPTION, JS_TAG_FIRST, JS_TAG_FLOAT64, JS_TAG_INT, JS_TAG_STRING, Helper_JS_NewFloat64, JS_ToFloat64, JS_ToBool, Helper_JS_NewBool, JS_NewArray, Helper_JS_FreeValue,
};

use crate::Value;

struct RuntimePtr {
    info: CString,
    runtime: *mut JSRuntime,
}

impl Drop for RuntimePtr {
    fn drop(&mut self) {
        if !self.runtime.is_null() {
            unsafe {
                JS_FreeRuntime(self.runtime as *mut _);
                self.runtime = ptr::null::<JSRuntime>() as *mut _;
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
            let rt = JS_NewRuntime();

            assert!(!rt.is_null());
            Runtime {
                ptr: Rc::new(RuntimePtr {
                    runtime: rt,
                    info: CString::default(),
                }),
            }
        }
    }
}

impl Runtime {
    pub fn context(&mut self) -> Context {
        unsafe {
            let ctx = JS_NewContext(self.ptr.runtime as *mut _);
            assert!(!ctx.is_null());

            js_std_add_helpers(
                ctx,
                1,
                [b"<none>\n".as_ptr() as *mut i8].as_mut_ptr(),
            );

            /* system modules */
            js_init_module_std(ctx, b"std\0".as_ptr() as *const i8);
            js_init_module_os(ctx, b"os\0".as_ptr() as *const i8);

            Context {
                ptr: Rc::new(ContextPtr {
                    context: ctx,
                    runtime: self.ptr.clone(),
                }),
            }
        }
    }
}

pub struct ContextPtr {
    pub(crate) context: *mut JSContext,
    runtime: Rc<RuntimePtr>,
}

impl Drop for ContextPtr {
    fn drop(&mut self) {
        if !self.context.is_null() {
            unsafe {
                JS_FreeContext(self.context as *mut _);
            }
            self.context = ptr::null::<JSContext>() as *mut _;
        }
    }
}

pub struct Context {
    pub(crate) ptr: Rc<ContextPtr>,
}

impl Context {
    pub fn eval(
        &mut self,
        input: &str,
        filename: &str,
        skip_shebang: bool,
        strict: bool,
        strip: bool,
    ) -> Result<Value, Value> {
        let input =
            CString::new(input).expect("Eval: input is not a valid string");
        let filename =
            CString::new(filename).expect("Eval: filename not a valid string");
        let mut flags = 0i32;

        if skip_shebang {
            flags |= JS_EVAL_FLAG_SHEBANG as i32;
        }

        if strict {
            flags |= JS_EVAL_FLAG_STRICT as i32;
        }

        if strip {
            flags |= JS_EVAL_FLAG_STRIP as i32;
        }

        let val = unsafe {
            let v = JS_Eval(
                self.ptr.context as *mut _,
                input.as_ptr(),
                input.as_bytes().len(),
                filename.as_ptr(),
                flags | JS_EVAL_TYPE_MODULE as i32,
            );

            Value { value: v, context: self.ptr.clone() }
        };

        if val.is_exception() {
            unsafe {
                let ex = JS_GetException(self.ptr.context as *mut _);
                Err(Value { value: ex, context: self.ptr.clone() })
            }
        } else {
            Ok(val)
        }
    }

    /*
    JS_DefinePropertyValueUint32(ctx, obj, 0, JS_NewInt32(ctx, info.dwSize.X), JS_PROP_C_W_E);
    JS_DefinePropertyValueUint32(ctx, obj, 1, JS_NewInt32(ctx, info.dwSize.Y), JS_PROP_C_W_E);
    */
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
        let val = ctx
            .eval(r#"print('Hello, World\n');"#, "<test>", false, false, false)
            .unwrap();
    }

    //#[test]
    //fn eval_multiple_ctx() {
    //    let mut rt = Runtime::default();
    //    let ctx1 = rt.context();
    //    let mut ctx2 = rt.context();

    //    let val = ctx2
    //        .eval(r#"print('Hello, World\n');"#, "<test>", false, false, false)
    //        .unwrap();
    //}
}
