package demo.phase1

use std.collections.HashMap
use std.io

pub const DEFAULT_PORT: Int = 8080
static BUILD_LABEL: String = "dev"

type UserMap[K, V] = HashMap[K, V]
opaque type UserId = U64

pub struct Buffer[T] {
    value: T,
}

pub struct Tagged {
    `type`: String,
}

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}

pub trait Writer[T: io.Flush] {
    fn write(var self, value: T) -> Result[Int, IoError]
    fn flush(var self) -> Result[Int, IoError]
}

impl[T: io.Flush] Writer[T] for Buffer[T]
where
    T: io.Flush
{
    pub fn write(var self, value: T) -> Result[Int, IoError] {
        self.value = value
    }

    pub fn flush(var self) -> Result[Int, IoError] {
        return self.write(self.value)
    }
}

extend String {
    fn to_port(self) -> Int {
        unsafe {
            parse_int(self)
        }
    }
}

pub fn merge[K: Eq, V](left: UserMap[K, V], right: UserMap[K, V]) -> UserMap[K, V]
where
    K: Eq + Hash,
    V: Clone
{
    return right
}

pub fn keyword_passthrough(`type`: String) -> Tagged {
    let _value = `type`
    return Tagged { `type`: _value }
}

pub extern "c" {
    fn strlen(ptr: *const U8) -> USize
}

extern "c" pub unsafe fn q_add(left: I32, right: I32) -> I32
