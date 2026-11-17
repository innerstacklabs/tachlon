use std::collections::HashSet;

use xer::ProjectRow;

use super::ProjectSelection;
use crate::notes::{Notes, Section};
use crate::{ImportError, XerProject};

pub(super) fn select_projects<'a>(
    data: &'a xer::Data,
    selection: &ProjectSelection,
    notes: &mut Notes,
) -> Result<Vec<&'a ProjectRow>, ImportError> {
    let summaries = data.projects();
    // `export_flag = N` filtering is unverified on a real P6 export (ruling 3, gs#116 corpus).
    let exported: Vec<_> = summaries
        .iter()
        .filter(|project| project.exported)
        .collect();
    let candidate_ids: HashSet<u32> = if exported.is_empty() {
        notes.add(
            Section::Project,
            "No project in the file is marked for export, so every project was read; no dates are affected",
            None,
        );
        summaries.iter().map(|project| project.proj_id).collect()
    } else {
        exported.iter().map(|project| project.proj_id).collect()
    };

    let requested: HashSet<u32> = match selection {
        ProjectSelection::Unspecified if candidate_ids.len() > 1 => {
            let projects = summaries
                .iter()
                .filter(|project| candidate_ids.contains(&project.proj_id))
                .map(|project| XerProject {
                    proj_id: project.proj_id,
                    short_name: project.proj_short_name.clone(),
                    activities: project.activity_count,
                })
                .collect();
            return Err(ImportError::ProjectChoice { projects });
        }
        ProjectSelection::Unspecified | ProjectSelection::All => candidate_ids.clone(),
        ProjectSelection::Only(ids) => {
            let all: HashSet<u32> = data.project_rows.iter().map(|row| row.proj_id).collect();
            for &proj_id in ids {
                if !all.contains(&proj_id) {
                    return Err(ImportError::UnknownProject { proj_id });
                }
            }
            ids.iter().copied().collect()
        }
    };

    let selected: Vec<_> = data
        .project_rows
        .iter()
        .filter(|project| requested.contains(&project.proj_id))
        .collect();
    for project in &data.project_rows {
        if !requested.contains(&project.proj_id) {
            let id = project.proj_id.to_string();
            notes.add(Section::Project, "Projects that were not chosen, or are not marked for export, were not read; the dates of the projects that were read are not affected", Some(&id));
        }
    }
    Ok(selected)
}
