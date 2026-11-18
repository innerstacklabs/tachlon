//! The one integration-test binary of the app crate: each file in this directory is a module
//! of it, so an engine edit relinks one test program, not a hundred. Run one old
//! file with its module name: `cargo test -p app --test it xer::`.
//!
//! A new test file is a new `mod` line here. `common` and the `architecture` and
//! `json_contract` directories are modules too.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

#[path = "architecture/main.rs"]
mod architecture;
mod assign_key;
mod baseline;
mod baseline_add;
mod baseline_bars;
mod baseline_frames;
mod baseline_open;
mod baseline_records;
mod baseline_summary_rows;
mod baseline_task;
mod baseline_variance;
mod before_edit;
mod before_edit_values;
mod blank;
mod capabilities;
mod check_filter;
mod checks;
mod checks_baseline;
mod checks_report;
mod checks_schedule;
mod clear_filters;
mod collapse;
mod collapse_links;
mod commits;
mod commits_outline;
mod common;
mod compare_file;
mod constraint_filter;
mod critical_finished;
mod critical_path;
mod data_date_edit;
mod date_origin;
mod deadline;
mod demo_whole_hours;
mod details;
mod document_bar_look;
mod document_finish_by;
mod document_key;
mod document_limit;
mod document_options;
mod document_round_trip;
mod document_stream;
mod document_write;
mod drag_actuals;
mod drag_day_start;
mod drag_finished;
mod drag_frames;
mod drag_held;
mod drag_milestone;
mod drag_resize;
mod drag_threshold;
mod duration_cone;
mod duration_fallback_fault;
mod duration_relative;
mod edit;
mod explain;
mod export_mspdi;
mod export_xer;
mod filter;
mod filter_code;
mod filter_critical_path;
mod filter_float_lost;
mod filter_moved;
mod filter_near_critical;
mod find;
mod finish_edit;
mod fit_open;
mod fit_project;
mod float_calendar;
mod float_text;
mod fly;
mod frame_scene;
mod gesture;
mod go_to_date;
mod group;
mod history;
mod insert;
mod insert_activity_id;
mod insert_many;
#[path = "json_contract/main.rs"]
mod json_contract;
mod json_escape;
mod keep_place;
mod layout_round_trip;
mod ledger_timeline;
mod levels;
mod link_gesture;
mod links;
mod links_replay;
mod links_set;
mod list_csv;
mod list_xlsx;
mod locale_heads;
mod locale_names;
mod locale_press;
mod locale_print;
mod locale_text;
mod locale_texts;
mod longest_path;
mod lookahead;
mod motion;
mod motion_frame;
mod move_row;
mod moved;
mod moved_critical;
mod moved_data_date;
mod moved_drag;
mod moved_hold;
mod moved_outline;
mod moved_reduced;
mod mspdi_timing;
mod names;
mod nav;
mod offscreen;
mod open;
mod open_codes;
mod open_data_date;
mod open_data_date_floor;
mod open_file_dates;
mod open_motion;
mod open_names;
mod open_order_rules;
mod open_progress;
mod open_select;
mod open_start_order;
mod outline;
mod outline_contracts;
mod percent_edit;
mod percent_edit_rounding;
mod plan_text;
mod predecessors;
mod print_columns;
mod print_dates;
mod print_legend;
mod print_lookahead;
mod print_pages;
mod print_scene;
mod print_window;
mod progress_edit;
mod progress_rollup;
mod progress_text;
mod proposal;
mod range;
mod range_text;
mod read_only;
mod reopen;
mod reparent;
mod resource_filter;
mod resources_card;
mod results;
mod reveal;
mod rollback;
mod schedule;
mod select;
mod set_limit;
mod start_constraint;
mod start_edit;
mod status_filter;
mod stress_grouped;
mod structure;
mod structure_rounds;
mod summary;
mod summary_duration;
mod task_fields;
mod theme;
mod update_tasks;
mod what_moves;
mod whatif;
mod xer;
