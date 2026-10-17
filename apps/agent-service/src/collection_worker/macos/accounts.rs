use super::*;
pub(super) fn collect(
    c: &Context<'_>,
    section: w::AccountSection,
    selection: &w::AccountSelection,
    rows: &mut Rows<'_>,
) -> Result<()> {
    let section = match section {
        w::AccountSection::Accounts => 0,
        w::AccountSection::Administrators => 1,
        w::AccountSection::Sessions => 2,
    };
    native::accounts(
        c,
        section,
        selection,
        rows.query.budget.max_rows.into(),
        |bytes| {
            let row: Row = serde_json::from_slice(bytes).map_err(|_| Status::Invalid)?;
            rows.push(row)?;
            Ok(())
        },
    )
}
