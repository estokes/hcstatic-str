//! Global, permanent, packed, hashconsed, short string storage.
//!
//! * supports strings up to 256 bytes
//! * derefs to a &str, but uses only 1 word on the stack and len + 1 bytes on the heap
//! * the actual bytes are stored packed into 1 MiB allocations to
//!   avoid the overhead of lots of small mallocs
//! * Copy!
//! * hashconsed, the same &str will always produce a pointer to the same memory
//!
//! CAN NEVER BE DEALLOCATED

use anyhow::bail;
use fxhash::FxHashSet;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::{borrow::Borrow, collections::HashSet, hash::Hash, ops::Deref, slice, str};

const CHUNK_SIZE: usize = 1 * 1024 * 1024;

struct Chunk {
    data: Vec<u8>,
    pos: usize,
}

impl Chunk {
    fn new() -> &'static mut Self {
	Box::leak(Box::new(Chunk {
	    data: vec![0; CHUNK_SIZE],
	    pos: 0
	}))
    }

    fn insert(&mut self, str: &str) -> (*mut Chunk, Str) {
        let str = str.as_bytes();
        let mut t = self;
        loop {
            if CHUNK_SIZE - t.pos > str.len() {
                t.data[t.pos] = str.len() as u8;
                t.data[t.pos + 1..t.pos + 1 + str.len()].copy_from_slice(str);
                let res = Str(t.data.as_ptr().wrapping_add(t.pos));
                t.pos += 1 + str.len();
                break (t, res);
            } else {
                t = Self::new();
            }
        }
    }
}

struct Root {
    all: FxHashSet<Str>,
    root: *mut Chunk,
}

unsafe impl Send for Root {}
unsafe impl Sync for Root {}

static ROOT: Lazy<Mutex<Root>> = Lazy::new(|| {
    Mutex::new(Root {
        all: HashSet::default(),
        root: Chunk::new(),
    })
});

/// This is a pointer into static memory that holds the actual str
/// slice. This type is 1 word on the stack, the length is stored in
/// the heap as a byte. Deref is quite cheap, there is no locking to
/// deref. Only try_from can be expensive since it performs the
/// hashconsing.
#[derive(Clone, Copy)]
pub struct Str(*const u8);

unsafe impl Send for Str {}
unsafe impl Sync for Str {}

impl Str {
    fn get(&self) -> &'static str {
        unsafe {
            let len = *self.0 as usize;
            let ptr = self.0.wrapping_add(1);
            let slice = slice::from_raw_parts(ptr, len);
            str::from_utf8_unchecked(slice)
        }
    }
}

impl Deref for Str {
    type Target = str;

    fn deref(&self) -> &'static Self::Target {
	self.get()
    }
}

impl Borrow<str> for Str {
    fn borrow(&self) -> &'static str {
	self.get()
    }
}

impl AsRef<str> for Str {
    fn as_ref(&self) -> &'static str {
	self.get()
    }
}

impl Hash for Str {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (&**self).hash(state)
    }
}

impl PartialEq for Str {
    fn eq(&self, other: &Self) -> bool {
        &**self == &**other
    }
}

impl Eq for Str {}

impl PartialOrd for Str {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (&**self).partial_cmp(&**other)
    }
}

impl Ord for Str {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (&**self).cmp(&**other)
    }
}

impl TryFrom<&str> for Str {
    type Error = anyhow::Error;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.as_bytes().len() > u8::MAX as usize {
            bail!("string is too long")
        } else {
            let mut root = ROOT.lock();
	    match root.all.get(s) {
                Some(t) => Ok(*t),
                None => unsafe {
		    let (r, t) = (*root.root).insert(s);
		    root.root = r;
		    root.all.insert(t);
		    Ok(t)
                }
	    }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::{thread_rng, Rng};

    fn rand(size: usize) -> String {
        let mut s = String::new();
        for _ in 0..size {
            s.push(thread_rng().gen())
        }
        s
    }

    #[test]
    fn test_single() {
        let s = rand(32);
        let t0 = Str::try_from(s.as_str()).unwrap();
        assert_eq!(&*t0, &*s);
        let t1 = Str::try_from(s.as_str()).unwrap();
        assert_eq!(t0.0, t1.0);
    }

    #[test]
    fn test_lots() {
        for _ in 0..1000000 {
            let s = rand(32);
            let t0 = Str::try_from(s.as_str()).unwrap();
            assert_eq!(&*t0, &*s);
            let t1 = Str::try_from(s.as_str()).unwrap();
            assert_eq!(t0.0, t1.0)
        }
    }
}
