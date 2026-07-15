//! Compile-time trait-bound detection — minimal vendored replacement for
//! `impls::impls!`.
//!
//! `impls!($Type : $Trait)` evaluates to a `const bool` that is `true` if
//! `$Type: $Trait` and `false` otherwise. The macro is consumed only by code
//! that `crudcrate-derive` generates (specifically the bidirectional-relation
//! check in `relation_validator`); it isn't intended as a public utility.
//!
//! Mechanism: the standard autoref/sentinel-trait specialization trick — a
//! blanket fallback trait sets `IMPLS = false`, and a more specific concrete
//! `Wrapper<T>` impl bound on `T: $Trait` overrides it with `IMPLS = true`.
//! See <https://github.com/nvzqz/impls> for the original source.

/// See module docs.
#[macro_export]
macro_rules! impls {
    ($type:ty: $($trait_expr:tt)+) => {{
        // Local-only trait so this macro composes inside any context without
        // polluting the surrounding namespace. `dead_code` is allowed because the
        // fallback goes unused whenever `$type: $trait` holds (the inherent
        // `Wrapper` const wins), which is exactly the positive case.
        #[allow(dead_code)]
        trait DoesNotImpl {
            const IMPLS: bool = false;
        }
        impl<T: ?Sized> DoesNotImpl for T {}

        struct Wrapper<T: ?Sized>(::core::marker::PhantomData<T>);

        #[allow(dead_code)]
        impl<T: ?Sized + $($trait_expr)+> Wrapper<T> {
            const IMPLS: bool = true;
        }

        <Wrapper<$type>>::IMPLS
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn detects_implemented_trait() {
        assert!(impls!(String: Clone));
        assert!(impls!(i32: Copy));
        assert!(impls!(Vec<u8>: IntoIterator));
    }

    #[test]
    fn detects_missing_trait() {
        // `*mut u8` is not `Send` (raw pointers aren't auto-Send).
        assert!(!impls!(*mut u8: Send));
    }

    #[test]
    fn handles_generic_trait_path() {
        assert!(impls!(String: From<&'static str>));
        assert!(!impls!(String: From<i32>));
    }
}
