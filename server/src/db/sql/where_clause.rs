//! Shared WHERE machinery for [`Sql`](super::Sql) and
//! [`Update`](super::Update). Same fragment shape, same `$` →
//! placeholder rewrite, same paren-wrapping rule, so the storage
//! type and the renderer live here once.

use sqlx::{Postgres, QueryBuilder};

use super::binds::BindOne;

pub(super) struct WherePart<'a> {
    pub fragment: &'a str,
    pub binds: Vec<Box<dyn BindOne<'a> + Send + 'a>>,
}

/// Render a vec of [`WherePart`]s into an existing builder. Pulled
/// out because both `Sql` and `Update` need the same logic
/// (paren-wrap each fragment, AND between, expand `$` to numbered
/// placeholders).
pub(super) fn push_wheres<'a>(qb: &mut QueryBuilder<'a, Postgres>, wheres: Vec<WherePart<'a>>) {
    for (i, w) in wheres.into_iter().enumerate() {
        qb.push(if i == 0 { " WHERE " } else { " AND " });
        qb.push("(");
        let parts: Vec<&str> = w.fragment.split('$').collect();
        assert_eq!(
            parts.len() - 1,
            w.binds.len(),
            "and_where: {} `$` placeholders but {} bind values: {:?}",
            parts.len() - 1,
            w.binds.len(),
            w.fragment,
        );
        qb.push(parts[0]);
        let mut binds = w.binds.into_iter();
        for part in &parts[1..] {
            let bind = binds.next().expect("counted above");
            bind.push(qb);
            qb.push(*part);
        }
        qb.push(")");
    }
}
