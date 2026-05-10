//! Heterogeneous bind storage. The builders need to keep a `Vec` of
//! values of *different* types until render time, but `Encode` /
//! `Type` aren't object-safe directly. [`BindOne`] is the trivial
//! shim that makes them so; [`IntoBinds`] turns a tuple of values
//! into the boxed-trait-object form.

use sqlx::{Encode, Postgres, QueryBuilder, Type};

/// Object-safe shim around the `Encode + Type` trait pair so we can
/// store heterogeneous binds in one `Vec`. `pub` only because it
/// leaks into [`IntoBinds`]'s return type; not part of the public
/// API (see the `#[doc(hidden)]` re-export in the parent module).
pub trait BindOne<'a>: Send {
    fn push(self: Box<Self>, qb: &mut QueryBuilder<'a, Postgres>);
}

impl<'a, T> BindOne<'a> for T
where
    T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,
{
    fn push(self: Box<Self>, qb: &mut QueryBuilder<'a, Postgres>) {
        qb.push_bind(*self);
    }
}

/// Tuples up to 8 implement this. `$` placeholders in the fragment
/// pull from the tuple in declaration order.
pub trait IntoBinds<'a> {
    fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>>;
}

impl<'a> IntoBinds<'a> for () {
    fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>> {
        Vec::new()
    }
}

macro_rules! impl_into_binds {
    ($($T:ident),+ $(,)?) => {
        impl<'a, $($T,)+> IntoBinds<'a> for ($($T,)+)
        where
            $($T: 'a + Send + Type<Postgres> + Encode<'a, Postgres>,)+
        {
            fn into_binds(self) -> Vec<Box<dyn BindOne<'a> + Send + 'a>> {
                #[allow(non_snake_case)]
                let ($($T,)+) = self;
                let v: Vec<Box<dyn BindOne<'a> + Send + 'a>> =
                    vec![ $( Box::new($T) as Box<dyn BindOne<'a> + Send + 'a>, )+ ];
                v
            }
        }
    };
}

impl_into_binds!(T1);
impl_into_binds!(T1, T2);
impl_into_binds!(T1, T2, T3);
impl_into_binds!(T1, T2, T3, T4);
impl_into_binds!(T1, T2, T3, T4, T5);
impl_into_binds!(T1, T2, T3, T4, T5, T6);
impl_into_binds!(T1, T2, T3, T4, T5, T6, T7);
impl_into_binds!(T1, T2, T3, T4, T5, T6, T7, T8);
