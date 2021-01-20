pub mod v1 {
    pub use crate::syscalls::*;
    pub use crate::{print, println};
    pub use core_imports::*;

    #[doc(hidden)]
    pub mod core_imports {
        pub use core;
        pub use core::convert::{AsMut, AsRef, From, Into};
        pub use core::iter::{DoubleEndedIterator, ExactSizeIterator, Extend, IntoIterator, Iterator};
        pub use core::marker::{Send, Sized, Sync, Unpin};
        pub use core::mem::drop;
        pub use core::ops::{Drop, Fn, FnMut, FnOnce};
        pub use core::option::Option::{self, None, Some};
        pub use core::prelude::v1::{
            asm, assert, cfg, column, compile_error, concat, env, file, format_args, include, include_bytes,
            include_str, line, module_path, option_env, stringify,
        };
        pub use core::prelude::v1::{
            global_allocator, Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd,
        };
        pub use core::result::Result::{self, Err, Ok};
    }
}
