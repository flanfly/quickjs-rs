use std::f64;
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
    js_std_add_helpers, js_std_dump_error, Helper_JS_DupValue,
    Helper_JS_FreeValue, Helper_JS_NewBool, Helper_JS_NewFloat64,
    JS_GetException, JS_NewArray, JS_NewInt64, JS_NewStringLen, JS_ToBool,
    JS_ToCStringLen, JS_ToFloat64, JS_ToInt64, JS_EVAL_FLAG_SHEBANG,
    JS_EVAL_FLAG_STRICT, JS_EVAL_FLAG_STRIP, JS_EVAL_TYPE_MODULE, JS_TAG_BOOL,
    JS_TAG_EXCEPTION, JS_TAG_FIRST, JS_TAG_FLOAT64, JS_TAG_INT, JS_TAG_STRING,
};

use crate::array::Array;
use crate::runtime::{Context, ContextPtr};

pub struct Value {
    pub(crate) value: JSValue,
    pub(crate) context: Rc<ContextPtr>,
}

impl Drop for Value {
    fn drop(&mut self) {
        unsafe { Helper_JS_FreeValue(self.context.context, self.value) }
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

impl From<Array> for Value {
    fn from(a: Array) -> Self {
        a.value
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.context.context == other.context.context
            && self.value.tag == other.value.tag
            && unsafe { self.value.u.ptr == self.value.u.ptr }
    }
}

impl Eq for Value {}

impl Clone for Value {
    fn clone(&self) -> Value {
        let v = unsafe { Helper_JS_DupValue(self.context.context, self.value) };
        Value { value: v, context: self.context.clone() }
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

    #[inline]
    pub fn is_number(&self) -> bool {
        self.is_float() || self.is_integer()
    }

    pub fn as_string(&self) -> Option<String> {
        if self.is_string() {
            Some(format!("{:?}", self))
        } else {
            None
        }
    }

    pub fn as_integer(&self) -> Option<i64> {
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

    pub fn as_float(&self) -> Option<f64> {
        if self.is_number() {
            let mut ret = 0f64;
            let rc = unsafe {
                JS_ToFloat64(self.context.context, &mut ret, self.value)
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

    pub fn as_boolean(&self) -> Option<bool> {
        if self.is_boolean() {
            let rc = unsafe { JS_ToBool(self.context.context, self.value) };

            match rc {
                0 => Some(false),
                1 => Some(true),
                -1 => None,
                _ => {
                    unreachable!();
                }
            }
        } else {
            None
        }
    }
}

impl Context {
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

    pub fn float(&self, val: f64) -> Value {
        unsafe {
            Value {
                value: Helper_JS_NewFloat64(self.ptr.context, val),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn boolean(&self, val: bool) -> Value {
        unsafe {
            Value {
                value: Helper_JS_NewBool(
                    self.ptr.context,
                    if val { 1 } else { 0 },
                ),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn array(&self, vals: &[Value]) -> Result<Array, Value> {
        let val = unsafe {
            Value {
                value: JS_NewArray(self.ptr.context),
                context: self.ptr.clone(),
            }
        };

        if val.is_exception() {
            Err(val)
        } else {
            let mut ary = Array { value: val };

            for (i, v) in vals.into_iter().enumerate() {
                assert!(ary.set(i as u32, v.clone()));
            }

            Ok(ary)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;

    #[test]
    fn strings() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

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

        assert_eq!(&s3.clone().as_string().unwrap(), "\0");
        assert_eq!(s3.clone(), s3);
    }

    #[test]
    fn integer() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        // int
        let i1 = ctx.integer(42);
        let i2 = ctx.integer(0);
        let i3 = ctx.integer(-1);
        let i4 = ctx.integer(0x7fffffff);
        let i5 = ctx.integer(-0x7fffffff);

        assert!(i1.is_integer());
        assert_eq!(i1.as_integer().unwrap(), 42);

        assert!(i2.is_integer());
        assert_eq!(i2.as_integer().unwrap(), 0);

        assert!(i3.is_integer());
        assert_eq!(i3.as_integer().unwrap(), -1);

        assert!(i4.is_integer());
        assert_eq!(i4.as_integer().unwrap(), 0x7fffffff);

        assert!(i5.is_integer());
        assert_eq!(i5.as_integer().unwrap(), -0x7fffffff);
    }

    #[test]
    fn float() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        // float
        let f1 = ctx.float(42.0);
        let f2 = ctx.float(0.0);
        let f3 = ctx.float(-1.0);
        let f4 = ctx.float(f64::INFINITY);
        let f5 = ctx.float(f64::NEG_INFINITY);
        let f6 = ctx.float(f64::NAN);

        assert!(f1.is_number());
        assert_eq!(f1.as_float().unwrap(), 42.0);

        assert!(f2.is_number());
        assert_eq!(f2.as_float().unwrap(), 0.0);

        assert!(f3.is_number());
        assert_eq!(f3.as_float().unwrap(), -1.0);

        assert!(f4.is_float());
        assert_eq!(f4.as_float().unwrap(), f64::INFINITY);

        assert!(f5.is_float());
        assert_eq!(f5.as_float().unwrap(), f64::NEG_INFINITY);

        assert!(f6.is_float());
        assert!(f6.as_float().unwrap().is_nan());
    }

    #[test]
    fn bool() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        // bool
        let b1 = ctx.boolean(true);
        let b2 = ctx.boolean(false);

        assert!(b1.is_boolean());
        assert!(b1.as_boolean().unwrap());

        assert!(b2.is_boolean());
        assert!(!b2.as_boolean().unwrap());
    }

    #[test]
    fn arrays() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        let ary1 = ctx.array(&[]).unwrap();

        let a =
            (0..100).into_iter().map(|x| ctx.integer(x)).collect::<Vec<_>>();
        let ary2 = ctx.array(&a).unwrap();

        assert_eq!(ary2.len().unwrap(), 100);
        for i in 0..100 {
            assert_eq!(
                ary2.get(i as u32).unwrap().as_integer().unwrap(),
                i as i64
            );
        }
    }
}
