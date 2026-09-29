//! Splits the real job log from the run whose step split was wrong.
//!
//! The fixture is the log exactly as the job endpoint returned it, so the split
//! is checked against every line rather than a trimmed sample.

use git_backend::github::api::split_job_log_for_test;

const LOG: &str = include_str!("fixtures/job-log.txt");
const STEPS: &str = include_str!("fixtures/job-steps.txt");

#[test]
fn every_step_of_the_real_log_gets_exactly_its_own_output() {
    let steps = split_job_log_for_test(STEPS, LOG);
    let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Set up job",
            "Run actions/checkout@v7",
            "Setup Node",
            "Install frontend dependencies",
            "Oxfmt",
            "Oxlint",
            "Typecheck",
            "Frontend tests",
            "Post job cleanup",
        ]
    );

    // The three steps that all report the second 19:12:40 are the ones a
    // timestamp split cannot separate.
    let oxfmt = &steps[4];
    assert!(
        oxfmt
            .log
            .contains("All matched files use the correct format."),
        "oxfmt lost its last line: {:?}",
        oxfmt.log
    );
    let oxlint = &steps[5];
    assert!(
        !oxlint.log.contains("correct format"),
        "oxlint stole oxfmt's output: {:?}",
        oxlint.log
    );
    assert!(
        oxlint.log.contains("Found 0 warnings and 0 errors.")
            && oxlint
                .log
                .contains("Finished in 245ms on 292 files with 128 rules"),
        "oxlint lost its own output: {:?}",
        oxlint.log
    );
    let typecheck = &steps[6];
    assert!(
        !typecheck.log.contains("Test Files  50 passed"),
        "typecheck stole the test output: {:?}",
        typecheck.log
    );
    assert!(
        typecheck.log.contains("tsc --noEmit"),
        "typecheck lost its own output: {:?}",
        typecheck.log
    );
    // The teardown starts at the runner's `Post job cleanup.` line, so the
    // test step keeps its whole output including the final summary, which is
    // stamped only 130ms before the teardown begins.
    let tests = &steps[7];
    assert!(
        tests.log.contains("Test Files  50 passed (50)"),
        "the test step lost its summary: {:?}",
        tests
            .log
            .lines()
            .last()
            .map(|line| &line[..line.len().min(60)])
    );
    assert!(
        !tests.log.contains("Cleaning up orphan processes"),
        "the teardown leaked back into the test step"
    );
}

#[test]
fn the_install_lands_in_the_install_step() {
    let steps = split_job_log_for_test(STEPS, LOG);
    let install = &steps[3];
    assert!(
        install.log.contains("npm ci"),
        "the install step lost its echo: {:?}",
        install.log
    );
    assert!(
        install.log.contains("found 0 vulnerabilities"),
        "the install step lost its last line: {:?}",
        install.log
    );
}

/// Every line of the log reaches some step, apart from the runner's own
/// `##[group]` headings, which are the runner's collapsible sections rather
/// than output.
///
/// The count comes from `spans_by_line` rather than from splitting `log`
/// again, since a step whose last line is blank loses that line to the join.
#[test]
fn no_output_line_of_the_real_log_is_dropped() {
    let steps = split_job_log_for_test(STEPS, LOG);
    let kept: usize = steps.iter().map(|step| step.spans_by_line.len()).sum();
    let output_lines = LOG.lines().filter(|line| !is_runner_heading(line)).count();
    assert_eq!(
        kept, output_lines,
        "kept {kept} of {output_lines} output lines"
    );
}

/// The runner's own collapsible headings, which are hidden rather than shown.
/// A log line carries a leading timestamp, so it is stripped before the
/// heading is recognised.
fn is_runner_heading(line: &str) -> bool {
    let message = line.split_once(' ').map_or(line, |(_, message)| message);
    message.starts_with("##[group]") || message.starts_with("##[endgroup]")
}

/// The runner's own setup output precedes the first `Run` marker, so it belongs
/// to the `Set up job` step rather than to the first workflow step.
#[test]
fn the_setup_output_lands_in_set_up_job() {
    let steps = split_job_log_for_test(STEPS, LOG);
    let setup = &steps[0];
    assert!(
        setup.log.starts_with("Current runner version: '2.337.0'"),
        "set up job lost the runner's first line: {:?}",
        setup.log.lines().next()
    );
    for expected in [
        "Hosted Compute Agent",
        "Prepare workflow directory",
        "Download action repository 'actions/checkout@v7'",
        "Complete job name: frontend",
    ] {
        assert!(
            setup.log.contains(expected),
            "set up job lost {expected:?}: {:?}",
            setup.log
        );
    }
    // The first workflow step starts at its own marker, not at the setup.
    let checkout = &steps[1];
    assert!(
        !checkout.log.contains("Current runner version"),
        "checkout stole the setup output: {:?}",
        checkout.log.lines().next()
    );
    assert!(
        checkout.log.contains("Syncing repository") || checkout.log.contains("git init"),
        "checkout lost its own output: {:?}",
        checkout.log
    );
}
/// The runner's trailing `Post ...` and `Complete job` steps have no marker in
/// the log and all report the same second, so they collapse into one cleanup
/// step that holds the teardown instead of being empty.
#[test]
fn the_post_steps_collapse_into_one_step_holding_the_teardown() {
    let steps = split_job_log_for_test(STEPS, LOG);
    let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Set up job",
            "Run actions/checkout@v7",
            "Setup Node",
            "Install frontend dependencies",
            "Oxfmt",
            "Oxlint",
            "Typecheck",
            "Frontend tests",
            "Post job cleanup",
        ]
    );

    let cleanup = steps.last().expect("the cleanup step");
    assert!(
        cleanup.log.contains("Post job cleanup.")
            && cleanup.log.contains("Cleaning up orphan processes"),
        "the cleanup step did not get the teardown: {:?}",
        cleanup.log.lines().take(3).collect::<Vec<_>>()
    );
    for expected in [
        "Post job cleanup.",
        "Cache hit occurred on the primary key",
        "Removing HTTP extra header",
        "Removing credentials config",
        "Cleaning up orphan processes",
    ] {
        assert!(
            cleanup.log.contains(expected),
            "the cleanup step lost {expected:?}"
        );
    }

    // The teardown is no longer sitting in the last workflow step.
    let tests = &steps[7];
    assert!(
        !tests.log.contains("Cleaning up orphan processes"),
        "the teardown is still in the test step"
    );
    assert!(
        tests.log.contains("Test Files  50 passed (50)"),
        "the test step lost its own output"
    );
}
