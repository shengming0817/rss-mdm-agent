//! Source discovery and approved output have independent bounds.
//! ref: osquery/osquery@5.23.1 osquery/tables/system/darwin/user_groups.mm.
use super::*;
const SOURCE_MAX_ROWS: usize = 4096;
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
    collect_from(rows, |source_limit, emit| {
        native::accounts(c, section, selection, source_limit, emit)
    })
}
fn collect_from(
    rows: &mut Rows<'_>,
    read: impl FnOnce(usize, &mut dyn FnMut(&[u8]) -> Result<()>) -> Result<()>,
) -> Result<()> {
    // Native discovery may examine unselected direct members. Its finite source
    // bound must not inherit max_rows; Rows alone bounds the approved output.
    read(SOURCE_MAX_ROWS, &mut |bytes| {
        let row: Row = serde_json::from_slice(bytes).map_err(|_| Status::Invalid)?;
        rows.push(row)?;
        Ok(())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    const FIRST: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const SECOND: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    fn read_fixture(rows: &mut Rows<'_>, selected: &[&str], source_count: usize) -> Result<()> {
        collect_from(rows, |source_limit, emit| {
            // Explicit native-source fixture: models source enumeration before the
            // native Selected filter, then emits the same closed wire metadata.
            if source_count > source_limit {
                return Err(Status::Truncated.into());
            }
            for member in [FIRST, SECOND] {
                if !selected.contains(&member) {
                    continue;
                }
                let r = row(&[
                    ("id", format!("admin:{member}").into()),
                    ("member", member.into()),
                    ("member_type", "user".into()),
                    ("resolved", "resolved".into()),
                ]);
                emit(&serde_json::to_vec(&r).unwrap())?;
            }
            Ok(())
        })
    }
    fn query(selected: &[&str]) -> w::NativeQuery {
        let mut q = super::super::tests::query(w::NativeQueryDefinition::System {
            query: w::NativeSystemQuery::MacosAccounts {
                section: w::AccountSection::Administrators,
                selection: w::AccountSelection::Selected {
                    identities: selected.iter().map(|s| (*s).into()).collect(),
                },
            },
        });
        q.budget.max_rows = 1;
        q
    }
    #[test]
    fn one_approved_administrator_fits_despite_other_source_members() {
        let q = query(&[SECOND]);
        let mut rows = Rows::new(&q, 100);
        let status = read_fixture(&mut rows, &[SECOND], 2);
        let result = rows.finish(status.map_err(|e| match e {
            Fault::Query(s) => s,
            _ => panic!("fixture cannot lose transport"),
        }));
        assert!(result.complete_for(&q, 100));
        assert_eq!(result.rows[0]["member"], SECOND);
    }
    #[test]
    fn approved_output_overflow_and_source_overflow_stay_distinct() {
        let q = query(&[FIRST, SECOND]);
        let mut rows = Rows::new(&q, 100);
        assert!(matches!(
            read_fixture(&mut rows, &[FIRST, SECOND], 2),
            Err(Fault::Query(Status::Truncated))
        ));
        let result = rows.finish(Err(Status::Truncated));
        assert_eq!(result.row_count, 1);
        assert!(!result.enumeration_complete);
        let mut rows = Rows::new(&q, 100);
        assert!(matches!(
            read_fixture(&mut rows, &[FIRST], SOURCE_MAX_ROWS + 1),
            Err(Fault::Query(Status::Truncated))
        ));
        assert_eq!(rows.result.row_count, 0);
    }
}
