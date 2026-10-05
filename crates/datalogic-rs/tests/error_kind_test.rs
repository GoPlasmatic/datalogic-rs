//! `ErrorKind` variants that must exist in every build, whatever features
//! are enabled, so downstream code can match on them without mirroring
//! datalogic's feature set.

use datalogic_rs::{Error, ErrorKind};

/// `BudgetExceeded` is raised only with the `budget` feature, but the
/// variant is always present, so a host can match on it rather than on
/// `tag()`.
#[test]
fn budget_exceeded_exists_in_every_build() {
    let err = Error::from(ErrorKind::BudgetExceeded {
        budget: 10,
        spent: 12,
    });
    assert_eq!(err.tag(), "BudgetExceeded");
    assert!(matches!(
        err.kind,
        ErrorKind::BudgetExceeded {
            budget: 10,
            spent: 12
        }
    ));
    assert!(err.to_string().contains("10"), "{err}");
}
