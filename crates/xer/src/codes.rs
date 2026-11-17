/// A calendar type; `CA_Base` and `CA_Project` are in public samples, `CA_Rsrc` is unverified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CalendarType {
    /// A base calendar (`CA_Base`).
    Base,
    /// A project calendar (`CA_Project`).
    Project,
    /// A resource calendar (`CA_Rsrc`, unverified).
    Resource,
    /// An unrecognized non-empty code.
    Other(String),
}

impl CalendarType {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "CA_Base" => Self::Base,
            "CA_Project" => Self::Project,
            "CA_Rsrc" => Self::Resource,
            _ => Self::Other(code.to_owned()),
        }
    }
}

/// An activity type. `TT_Task` is confirmed in public samples; the other spellings are unverified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaskType {
    /// A regular task (`TT_Task`).
    Task,
    /// A start milestone (`TT_Mile`, unverified).
    StartMilestone,
    /// A finish milestone (`TT_FinMile`, unverified).
    FinishMilestone,
    /// A resource-dependent activity (`TT_Rsrc`, unverified).
    ResourceDependent,
    /// A level-of-effort activity (`TT_LOE`, unverified).
    LevelOfEffort,
    /// A work breakdown structure summary (`TT_WBS`, unverified).
    WbsSummary,
    /// An unrecognized non-empty code.
    Other(String),
}

impl TaskType {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "TT_Task" => Self::Task,
            "TT_Mile" => Self::StartMilestone,
            "TT_FinMile" => Self::FinishMilestone,
            "TT_Rsrc" => Self::ResourceDependent,
            "TT_LOE" => Self::LevelOfEffort,
            "TT_WBS" => Self::WbsSummary,
            _ => Self::Other(code.to_owned()),
        }
    }
}

/// An activity status; the three `TK_*` spellings are in public samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusCode {
    /// An activity that has not started (`TK_NotStart`).
    NotStarted,
    /// An active activity (`TK_Active`).
    Active,
    /// A completed activity (`TK_Complete`).
    Complete,
    /// An unrecognized non-empty code.
    Other(String),
}

impl StatusCode {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "TK_NotStart" => Self::NotStarted,
            "TK_Active" => Self::Active,
            "TK_Complete" => Self::Complete,
            _ => Self::Other(code.to_owned()),
        }
    }
}

/// A scheduling constraint type; every spelling is in a P6 probe file that P6 imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConstraintType {
    /// A start-on constraint (`CS_MSO`).
    StartOn,
    /// A start-on-or-after constraint (`CS_MSOA`).
    StartOnOrAfter,
    /// A start-on-or-before constraint (`CS_MSOB`).
    StartOnOrBefore,
    /// A finish-on constraint (`CS_MEO`).
    FinishOn,
    /// A finish-on-or-after constraint (`CS_MEOA`).
    FinishOnOrAfter,
    /// A finish-on-or-before constraint (`CS_MEOB`).
    FinishOnOrBefore,
    /// An as-late-as-possible constraint (`CS_ALAP`).
    AsLateAsPossible,
    /// A mandatory-start constraint (`CS_MANDSTART`).
    MandatoryStart,
    /// A mandatory-finish constraint (`CS_MANDFIN`).
    MandatoryFinish,
    /// An unrecognized non-empty code.
    Other(String),
}

impl ConstraintType {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "CS_MSO" => Self::StartOn,
            "CS_MSOA" => Self::StartOnOrAfter,
            "CS_MSOB" => Self::StartOnOrBefore,
            "CS_MEO" => Self::FinishOn,
            "CS_MEOA" => Self::FinishOnOrAfter,
            "CS_MEOB" => Self::FinishOnOrBefore,
            "CS_ALAP" => Self::AsLateAsPossible,
            "CS_MANDSTART" => Self::MandatoryStart,
            "CS_MANDFIN" => Self::MandatoryFinish,
            _ => Self::Other(code.to_owned()),
        }
    }
}

/// A predecessor relationship type whose XER code spellings are unverified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PredType {
    /// A finish-to-start relationship (`PR_FS`, unverified).
    FinishToStart,
    /// A finish-to-finish relationship (`PR_FF`, unverified).
    FinishToFinish,
    /// A start-to-start relationship (`PR_SS`, unverified).
    StartToStart,
    /// A start-to-finish relationship (`PR_SF`, unverified).
    StartToFinish,
    /// An unrecognized non-empty code.
    Other(String),
}

impl PredType {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "PR_FS" => Self::FinishToStart,
            "PR_FF" => Self::FinishToFinish,
            "PR_SS" => Self::StartToStart,
            "PR_SF" => Self::StartToFinish,
            _ => Self::Other(code.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ConstraintType, PredType, StatusCode, TaskType};

    #[test]
    fn task_type_codes_are_mapped_exactly() {
        let cases = [
            ("TT_Task", TaskType::Task),
            ("TT_Mile", TaskType::StartMilestone),
            ("TT_FinMile", TaskType::FinishMilestone),
            ("TT_Rsrc", TaskType::ResourceDependent),
            ("TT_LOE", TaskType::LevelOfEffort),
            ("TT_WBS", TaskType::WbsSummary),
            ("unknown", TaskType::Other("unknown".to_owned())),
        ];
        for (code, expected) in cases {
            assert_eq!(TaskType::from_code(code), expected);
        }
    }

    #[test]
    fn status_codes_are_mapped_exactly() {
        let cases = [
            ("TK_NotStart", StatusCode::NotStarted),
            ("TK_Active", StatusCode::Active),
            ("TK_Complete", StatusCode::Complete),
            ("unknown", StatusCode::Other("unknown".to_owned())),
        ];
        for (code, expected) in cases {
            assert_eq!(StatusCode::from_code(code), expected);
        }
    }

    #[test]
    fn constraint_type_codes_are_mapped_exactly() {
        let cases = [
            ("CS_MSO", ConstraintType::StartOn),
            ("CS_MSOA", ConstraintType::StartOnOrAfter),
            ("CS_MSOB", ConstraintType::StartOnOrBefore),
            ("CS_MEO", ConstraintType::FinishOn),
            ("CS_MEOA", ConstraintType::FinishOnOrAfter),
            ("CS_MEOB", ConstraintType::FinishOnOrBefore),
            ("CS_ALAP", ConstraintType::AsLateAsPossible),
            ("CS_MANDSTART", ConstraintType::MandatoryStart),
            ("CS_MANDFIN", ConstraintType::MandatoryFinish),
            ("unknown", ConstraintType::Other("unknown".to_owned())),
        ];
        for (code, expected) in cases {
            assert_eq!(ConstraintType::from_code(code), expected);
        }
    }

    #[test]
    fn predecessor_type_codes_are_mapped_exactly() {
        let cases = [
            ("PR_FS", PredType::FinishToStart),
            ("PR_FF", PredType::FinishToFinish),
            ("PR_SS", PredType::StartToStart),
            ("PR_SF", PredType::StartToFinish),
            ("unknown", PredType::Other("unknown".to_owned())),
        ];
        for (code, expected) in cases {
            assert_eq!(PredType::from_code(code), expected);
        }
    }
}
