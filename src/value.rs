use std::f64;
use std::fmt;
use std::i64;
use std::slice;
use std::str;

use quickjs_sys as sys;

use crate::array::Array;
use crate::object::Object;
use crate::runtime::{Context, ContextPtr};

pub struct Value {
    pub(crate) value: sys::JSValue,
    pub(crate) context: ContextPtr,
}

impl Drop for Value {
    fn drop(&mut self) {
        unsafe { sys::Helper_JS_FreeValue(self.context.as_ptr(), self.value) }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        unsafe {
            let mut sz = 0i32;
            let p = sys::JS_ToCStringLen(
                self.context.as_ptr() as *mut _,
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
        self.context.as_ptr() == other.context.as_ptr()
            && self.value.tag == other.value.tag
            && unsafe { self.value.u.ptr == self.value.u.ptr }
    }
}

impl Eq for Value {}

impl Clone for Value {
    fn clone(&self) -> Value {
        let v = unsafe {
            sys::Helper_JS_DupValue(self.context.as_ptr(), self.value)
        };
        Value { value: v, context: self.context.clone() }
    }
}

impl Value {
    pub fn is_exception(&self) -> bool {
        unsafe { sys::Helper_JS_IsException(self.value) != 0 }
    }

    pub fn is_string(&self) -> bool {
        unsafe { sys::Helper_JS_IsString(self.value) != 0 }
    }

    pub fn is_integer(&self) -> bool {
        unsafe { sys::Helper_JS_IsInteger(self.value) != 0 }
    }

    pub fn is_boolean(&self) -> bool {
        unsafe { sys::Helper_JS_IsBool(self.value) != 0 }
    }

    pub fn is_number(&self) -> bool {
        unsafe { sys::JS_IsNumber(self.value) != 0 }
    }

    pub fn is_undefined(&self) -> bool {
        unsafe { sys::Helper_JS_IsUndefined(self.value) != 0 }
    }

    pub fn is_null(&self) -> bool {
        unsafe { sys::Helper_JS_IsNull(self.value) != 0 }
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
                sys::JS_ToInt64(self.context.as_ptr(), &mut ret, self.value)
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
                sys::JS_ToFloat64(self.context.as_ptr(), &mut ret, self.value)
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
            let rc =
                unsafe { sys::JS_ToBool(self.context.as_ptr(), self.value) };

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

    pub fn call(&self, this: Value, args: &[Value]) -> Value {
        unsafe {
            let c = self.context.as_ptr();
            let mut v = args
                .into_iter()
                .map(|x| sys::Helper_JS_DupValue(c, x.value))
                .collect::<Vec<sys::JSValue>>();
            let t = sys::Helper_JS_DupValue(c, this.value);
            let ret =
                sys::JS_Call(c, self.value, t, v.len() as i32, v.as_mut_ptr());

            Value { context: self.context.clone(), value: ret }
        }
    }
}

impl Context {
    pub fn undefined(&self) -> Value {
        unsafe {
            Value {
                value: sys::Helper_JS_NewUndefined(),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn null(&self) -> Value {
        unsafe {
            Value { value: sys::Helper_JS_NewNull(), context: self.ptr.clone() }
        }
    }

    pub fn exception(&self) -> Value {
        unsafe {
            Value {
                value: sys::Helper_JS_NewException(),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn string(&self, val: &str) -> Value {
        let mut cstr = val.as_bytes().to_vec();

        cstr.push(0);

        unsafe {
            Value {
                value: sys::JS_NewStringLen(
                    self.ptr.as_ptr(),
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
                value: sys::JS_NewInt64(self.ptr.as_ptr(), val),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn float(&self, val: f64) -> Value {
        unsafe {
            Value {
                value: sys::Helper_JS_NewFloat64(self.ptr.as_ptr(), val),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn boolean(&self, val: bool) -> Value {
        unsafe {
            Value {
                value: sys::Helper_JS_NewBool(
                    self.ptr.as_ptr(),
                    if val { 1 } else { 0 },
                ),
                context: self.ptr.clone(),
            }
        }
    }

    pub fn array(&self, vals: &[Value]) -> Result<Array, Value> {
        let val = unsafe {
            Value {
                value: sys::JS_NewArray(self.ptr.as_ptr()),
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

    pub fn object(&self) -> Result<Object, Value> {
        let val = unsafe {
            Value {
                value: sys::JS_NewObject(self.ptr.as_ptr()),
                context: self.ptr.clone(),
            }
        };

        if val.is_exception() {
            Err(val)
        } else {
            Ok(Object { value: val })
        }
    }

    pub fn function(
        &self,
        nam: &str,
        f: extern "C" fn(
            *mut sys::JSContext,
            sys::JSValue,
            i32,
            *mut sys::JSValue,
        ) -> sys::JSValue,
    ) -> Result<Value, Value> {
        let val = unsafe {
            Value {
                value: sys::JS_NewCFunction2(
                    self.ptr.as_ptr(),
                    Some(f),
                    nam.as_ptr() as *const i8,
                    nam.len() as i32,
                    sys::JSCFunctionEnum_JS_CFUNC_generic,
                    0,
                ),
                context: self.ptr.clone(),
            }
        };

        if val.is_exception() {
            Err(val)
        } else {
            Ok(val)
        }
    }
}

#[macro_export]
macro_rules! wrap_fn {
    ($name:ident, $target:ident) => {
        extern "C" fn $name(
            ctx: *mut sys::JSContext,
            this: sys::JSValue,
            argc: i32,
            argv: *mut sys::JSValue,
        ) -> sys::JSValue {
            assert!(!ctx.is_null());

            let ctx = $crate::Context {
                ptr: $crate::runtime::ContextPtr::Borrowed(ctx),
            };
            let mut args = Vec::<$crate::Value>::with_capacity(argc as usize);
            let this = $crate::Value { value: this, context: ctx.ptr.clone() };

            for idx in 0..argc {
                let arg = unsafe {
                    $crate::Value {
                        value: *argv.offset(idx as isize),
                        context: ctx.ptr.clone(),
                    }
                };

                args.push(arg);
            }

            $target(&ctx, this, &args).value
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Runtime;

    #[test]
    fn unit() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        let undef = ctx.undefined();
        let null = ctx.null();
        let ex = ctx.exception();

        assert!(undef.is_undefined());
        assert!(null.is_null());
        assert!(ex.is_exception());
    }

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

        assert!(f4.is_number());
        assert_eq!(f4.as_float().unwrap(), f64::INFINITY);

        assert!(f5.is_number());
        assert_eq!(f5.as_float().unwrap(), f64::NEG_INFINITY);

        assert!(f6.is_number());
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

        let _ = ctx.array(&[]).unwrap();

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

    fn test_func1(ctx: &Context, _: Value, _: &[Value]) -> Value {
        ctx.integer(0)
    }

    wrap_fn!(js_test_func1, test_func1);

    #[test]
    fn func() {
        let mut rt = Runtime::default();
        let ctx = rt.context();
        let this = ctx.integer(3);
        let exp = ctx.integer(0);
        let f = ctx.function("testFunc", js_test_func1).unwrap();

        assert_eq!(f.call(this, &[]), exp);
    }

    fn test_func2(ctx: &Context, _: Value, args: &[Value]) -> Value {
        ctx.integer(
            args[0].as_integer().unwrap() + args[1].as_integer().unwrap(),
        )
    }

    wrap_fn!(js_test_func2, test_func2);

    #[test]
    fn func2() {
        let mut rt = Runtime::default();
        let ctx = rt.context();
        let this = ctx.integer(3);
        let i = ctx.integer(23);
        let j = ctx.integer(42);
        let exp = ctx.integer(23 + 42);
        let f = ctx.function("testFunc", js_test_func2).unwrap();

        assert_eq!(f.call(this, &[i, j]), exp);
    }

    #[test]
    fn object() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        let mut obj = ctx.object().unwrap();

        assert!(obj.set("a", ctx.integer(1)));
        assert!(obj.set("b", ctx.integer(2)));
        assert!(obj.set("c", ctx.string("Test")));

        assert_eq!(obj.get("a").unwrap(), ctx.integer(1));
    }
}
