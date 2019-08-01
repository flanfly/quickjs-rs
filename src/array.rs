use quickjs_sys::{
    JS_GetPropertyInternal, JS_GetPropertyUint32, JS_NewAtom,
    JS_SetPropertyUint32,
};

use crate::value::Value;

pub struct Array {
    pub(crate) value: Value,
}

impl Array {
    pub fn len(&self) -> Result<usize, Value> {
        unsafe {
            let len_atm = JS_NewAtom(
                self.value.context.as_ptr(),
                b"length\0".as_ptr() as *const i8,
            );
            let l = JS_GetPropertyInternal(
                self.value.context.as_ptr(),
                self.value.value,
                len_atm,
                self.value.value,
                0,
            );
            let l = Value { value: l, context: self.value.context.clone() };

            match l.as_integer() {
                Some(i) if i >= 0 => Ok(i as usize),
                Some(_) => unreachable!(),
                None => Err(l),
            }
        }
    }

    pub fn iter<'a>(&'a self) -> ArrayIterator<'a> {
        ArrayIterator { array: self, pos: 0 }
    }

    pub fn set(&mut self, index: u32, val: Value) -> bool {
        unsafe {
            JS_SetPropertyUint32(
                self.value.context.as_ptr(),
                self.value.value,
                index,
                val.value,
            ) == 0
        }
    }
    pub fn get(&self, idx: u32) -> Result<Value, Value> {
        let v = unsafe {
            JS_GetPropertyUint32(
                self.value.context.as_ptr(),
                self.value.value,
                idx,
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

pub struct ArrayIterator<'a> {
    array: &'a Array,
    pos: usize,
}

impl<'a> Iterator for ArrayIterator<'a> {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        match self.array.get(self.pos as u32) {
            Ok(v) => {
                self.pos += 1;
                Some(v)
            }
            Err(_) => None,
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

        let _a1 = ctx.array(&[]);
    }
}
