use quickjs_sys as sys;

use crate::value::Value;

pub struct Object {
    pub(crate) value: Value,
}

impl Object {
    pub fn set(&mut self, key: &str, val: Value) -> bool {
        let mut cstr = key.as_bytes().to_vec();

        cstr.push(0);

        unsafe {
            sys::JS_SetPropertyStr(
                self.value.context.as_ptr(),
                self.value.value,
                cstr.as_ptr() as *const i8,
                sys::Helper_JS_DupValue(self.value.context.as_ptr(), val.value),
            ) >= 0
        }
    }

    pub fn get(&self, key: &str) -> Result<Value, Value> {
        let mut cstr = key.as_bytes().to_vec();

        cstr.push(0);

        let v = unsafe {
            sys::JS_GetPropertyStr(
                self.value.context.as_ptr(),
                self.value.value,
                cstr.as_ptr() as *const i8,
            )
        };
        let val = Value { context: self.value.context.clone(), value: v };

        if val.is_exception() {
            Err(val)
        } else {
            Ok(val)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Runtime;

    #[test]
    fn new() {
        let mut rt = Runtime::default();
        let ctx = rt.context();

        let _ = ctx.object();
    }
}
