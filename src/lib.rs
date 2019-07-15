use std::ffi::{CStr, CString};
use std::fmt;
use std::i64;
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
    JS_TAG_EXCEPTION, JS_TAG_FIRST, JS_TAG_FLOAT64, JS_TAG_INT, JS_TAG_STRING,
};

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

struct ContextPtr {
    context: *mut JSContext,
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
    ptr: Rc<ContextPtr>,
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

    pub fn string(&self, val: &str) -> Value {
        unsafe {
            Value {
                value: JS_NewStringLen(
                    self.ptr.context,
                    val.as_ptr() as *const i8,
                    val.len() as i32,
                ),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn integer(&self, val: i64) -> Value {
        unsafe {
            Value {
                value: JS_NewInt64(self.ptr.context, val),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn float(&self, val: f32) -> Value {
        unimplemented!()
    }
    pub fn boolean(&self, val: bool) -> Value {
        unimplemented!()
    }
    pub fn array(&self, val: &[Value]) -> Value {
        unimplemented!()
    }
}

pub struct Value {
    value: JSValue,
    context: Rc<ContextPtr>,
}

impl Drop for Value {
    fn drop(&mut self) {
        if self.value.tag as u64 >= JS_TAG_FIRST as u64 {
            unsafe {
                let p = self.value.u.ptr as *mut JSRefCountHeader;
                (*p).ref_count -= 1;
                if (*p).ref_count <= 0 {
                    __JS_FreeValueRT(self.context.runtime.runtime, self.value);
                }
            }
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            let mut sz = 0i32;
            let p = JS_ToCStringLen(
                self.context.context as *mut _,
                &mut sz,
                self.value,
                0,
            );
            assert!(sz >= 0);

            let s = slice::from_raw_parts(p as *const u8, sz as usize);

            write!(f, "{}", str::from_utf8(s).unwrap_or("<encoding error>"))
        }
    }
}

impl Value {
    #[inline]
    pub fn is_exception(&self) -> bool {
        self.value.tag as u64 == JS_TAG_EXCEPTION as u64
    }

    #[inline]
    pub fn is_string(&self) -> bool {
        self.value.tag as i32 == JS_TAG_STRING
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        self.value.tag as i32 == JS_TAG_INT
    }

    #[inline]
    pub fn is_boolean(&self) -> bool {
        self.value.tag as i32 == JS_TAG_BOOL
    }

    #[inline]
    pub fn is_float(&self) -> bool {
        self.value.tag as i32 == JS_TAG_FLOAT64
    }

    pub fn as_string(&self) -> Option<String> {
        if self.is_string() {
            Some(format!("{:?}", self))
        } else {
            None
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        if self.is_integer() {
            let mut ret = 0i64;
            let rc = unsafe {
                JS_ToInt64(self.context.context, &mut ret, self.value)
            };

            if rc == 0 {
                Some(ret)
            } else {
                None
            }
        } else {
            None
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

    #[test]
    fn values() {
        let mut rt = Runtime::default();
        let mut ctx = rt.context();

        // strings
        let s1 = ctx.string("Hello, World");
        let s2 = ctx.string("");
        let s3 = ctx.string("\0");

        assert!(s1.is_string());
        assert_eq!(&s1.as_string().unwrap(), "Hello, World");

        assert!(s2.is_string());
        assert_eq!(&s2.as_string().unwrap(), "");

        assert!(s3.is_string());
        assert_eq!(&s3.as_string().unwrap(), "\0");

        // int
        let i1 = ctx.integer(42);
        let i2 = ctx.integer(0);
        let i3 = ctx.integer(-1);
        let i4 = ctx.integer(i64::MAX);
        let i5 = ctx.integer(i64::MIN);

        assert!(i1.is_integer());
        assert_eq!(i1.as_i64().unwrap(), 42);

        assert!(i2.is_integer());
        assert_eq!(i2.as_i64().unwrap(), 0);

        assert!(i3.is_integer());
        assert_eq!(i3.as_i64().unwrap(), -1);

        assert!(i4.is_integer());
        assert_eq!(i4.as_i64().unwrap(), i64::MAX);

        assert!(i5.is_integer());
        assert_eq!(i5.as_i64().unwrap(), i64::MIN);
    }
}
