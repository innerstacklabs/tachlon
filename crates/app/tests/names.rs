use app::{
    SEED,
    meta::{TaskMeta, outline_path},
    names,
};

#[test]
fn generated_names_are_deterministic() {
    let first = names::generate(SEED, 2_000);
    assert_eq!(first, names::generate(SEED, 2_000));
    assert_ne!(first, names::generate(SEED + 1, 2_000));
}

#[test]
fn paint_colors_the_same_two_in_five_tasks_from_six_colors_on_every_run() {
    let mut meta = names::generate(SEED, 10_000);
    names::paint(&mut meta, SEED);
    let colors: Vec<_> = meta.iter().map(|t| t.color).collect();
    let painted = colors.iter().flatten().count();
    assert!(
        (3_800..=4_200).contains(&painted),
        "{painted} of 10000 painted"
    );
    assert_eq!(
        colors
            .iter()
            .flatten()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        6
    );
    let mut again = names::generate(SEED, 10_000);
    names::paint(&mut again, SEED);
    assert_eq!(meta, again);
}

#[test]
fn demo_metadata_matches_its_tasks_and_generator() {
    let n = 2_000;
    let layout = app::outline::demo_layout(n);
    assert_eq!(layout.meta.len(), layout.schedule.tasks().len());
    assert_eq!(layout.meta.len(), n);

    let generated = names::generate(SEED, n);
    for (position, task) in generated.iter().enumerate() {
        let actual = &layout.meta[layout.wbs[position].0 as usize];
        assert_eq!(actual.name, task.name);
        assert_eq!(actual.outline_level, task.outline_level);
        assert_eq!(
            actual.parent,
            task.parent.map(|parent| layout.wbs[parent.0 as usize])
        );
    }
}

#[test]
fn names_and_outline_form_valid_paths() {
    let metadata = names::generate(SEED, 2_000);
    assert_outline(&metadata);
}

#[test]
fn small_sizes_are_supported() {
    for n in 0..=3 {
        let metadata = names::generate(SEED, n);
        assert_eq!(metadata.len(), n);
        assert_outline(&metadata);
    }
}

fn assert_outline(metadata: &[TaskMeta]) {
    for (index, task) in metadata.iter().enumerate() {
        assert_ne!(task.name, "");
        assert!(!is_placeholder_name(&task.name));
        assert!((1..=3).contains(&task.outline_level));
        // A name is the task's own: the path is derived, one segment per outline level.
        assert!(!task.name.contains(" / "), "{:?} holds a path", task.name);
        assert_eq!(
            outline_path(metadata, index).split(" / ").count(),
            task.outline_level as usize
        );

        match task.parent {
            None => assert_eq!(task.outline_level, 1),
            Some(parent) => {
                let parent_index = parent.0 as usize;
                assert!(parent_index < index);
                let parent = &metadata[parent_index];
                assert_eq!(parent.outline_level + 1, task.outline_level);
                assert_eq!(
                    outline_path(metadata, index),
                    format!("{} / {}", outline_path(metadata, parent_index), task.name)
                );
            }
        }

        let mut cursor = Some(index);
        for _ in 0..3 {
            let Some(current) = cursor else {
                break;
            };
            cursor = metadata[current].parent.map(|parent| parent.0 as usize);
        }
        assert!(cursor.is_none(), "parent cycle at task {index}");
    }
}

fn is_placeholder_name(name: &str) -> bool {
    name.strip_prefix("Task ").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}
