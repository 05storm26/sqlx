use crate::backend::{Backend, BackendAssocRawQuery};

pub struct Pg;

impl<'q> BackendAssocRawQuery<'q, Pg> for Pg {
    type RawQuery = super::PgRawQuery<'q>;
}

impl Backend for Pg {
    type RawConnection = super::PgRawConnection;
    type Row = super::PgRow;
}

// Generates tuple FromSqlRow impls for this backend
impl_from_sql_row_tuples_for_backend!(Pg);
