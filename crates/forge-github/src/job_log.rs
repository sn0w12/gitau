use forge_core::dto::ActionStep;

/// A timestamp in either shape GitHub uses.
///
/// A log line is stamped in RFC 3339 with a zone and fractional seconds
/// (`2026-09-26T19:12:21.6689146Z`), while a step's `started_at` arrives as
/// `2026-09-28 01:06:36` with a space and no zone. Both are UTC. The step
/// stamps are not used to split the log, only to label it.
fn parse_stamp(stamp: &str) -> Option<Timestamp> {
    if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(stamp) {
        return Some(parsed.with_timezone(&chrono::Utc));
    }
    chrono::NaiveDateTime::parse_from_str(stamp, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|naive| naive.and_utc())
}

type Timestamp = chrono::DateTime<chrono::Utc>;

/// The leading stamp on a log line, and the message that follows it.
///
/// GitHub separates them with a space, so the first word is the stamp. A line
/// carrying only a stamp is blank output: it is still stamped, since the stamp
/// is the entire line, and its message is empty.
fn split_log_line(line: &str) -> (Option<Timestamp>, &str) {
    let (stamp, message) = line.split_once(' ').unwrap_or((line, ""));
    match parse_stamp(stamp) {
        Some(when) => (Some(when), message),
        // An unstamped continuation line is its own body, not a message whose
        // stamp went missing.
        None => (None, line),
    }
}

/// Whether a step is a real workflow step, as opposed to the runner's own
/// `Set up job` / `Complete job` and the `Post ...` teardown steps.
///
/// Only a real step gets a `##[group]Run <command>` line, and only the real
/// steps carry user output worth showing, so the markers line up with these
/// and with nothing else.
fn is_workflow_step(name: &str) -> bool {
    !matches!(name, "Set up job" | "Complete job") && !name.starts_with("Post ")
}

/// The name the collapsed teardown step is shown under.
const CLEANUP_STEP_NAME: &str = "Post job cleanup";

/// The first line of the teardown, which the runner writes once every step has
/// finished. It carries no marker, so this line is what the teardown is found
/// by. A run that never reaches it has no teardown to show.
const CLEANUP_MARKER: &str = "Post job cleanup.";

/// The last line a runner writes, as it tears down the processes the job
/// started. Without a teardown to hold it, this belongs to `Complete job`,
/// which is the step the runner is in when it writes the line.
const COMPLETE_MARKER: &str = "Cleaning up orphan processes";

/// Collapses the runner's trailing steps into one.
///
/// The runner emits a `Post <action>` step for every action that registered a
/// teardown, plus a `Complete job` step, and none of them write a marker, so
/// the teardown cannot be split between them. Left alone they are N steps that
/// are all empty while their output sits in the last workflow step, so they
/// become one step that holds all of it, whatever the workflow happens to have.
///
/// A run that never writes the teardown has no such output to gather, and
/// `Complete job` is then where the runner's own last line belongs, so the
/// steps are left as they are.
pub(crate) fn collapse_teardown(steps: Vec<ActionStep>, log: &str) -> Vec<ActionStep> {
    let Some(last_workflow) = steps.iter().rposition(|step| is_workflow_step(&step.name)) else {
        return steps;
    };
    let trailing = &steps[last_workflow + 1..];
    if trailing.len() < 2 || !log.lines().any(|line| has_message(line, CLEANUP_MARKER)) {
        return steps;
    }
    let mut collapsed: Vec<ActionStep> = steps[..=last_workflow].to_vec();
    collapsed.push(ActionStep {
        number: trailing[0].number,
        name: CLEANUP_STEP_NAME.to_string(),
        status: trailing[0].status.clone(),
        // The teardown only fails when the job was cancelled or the runner
        // itself broke, so the steps agreeing on success is the common case and
        // the first failure is the one worth reporting.
        conclusion: trailing.iter().find_map(|step| step.conclusion.clone()),
        started_at: trailing
            .iter()
            .filter_map(|step| step.started_at.clone())
            .min(),
        completed_at: trailing
            .iter()
            .filter_map(|step| step.completed_at.clone())
            .max(),
        log: String::new(),
        spans_by_line: Vec::new(),
    });
    collapsed
}

/// Routes each log line to the step that wrote it.
///
/// A `##[group]Run <command>` line is the first line of a step, and everything
/// up to the next one is that step's output. Those markers are the real
/// boundaries. The step stamps from the job endpoint are not usable on their
/// own: they are second-resolution while the lines carry sub-second stamps, so
/// a step that ran inside a single second has a window that other steps'
/// output lands inside, and a step's final second overlaps the next step's
/// first. In a real run `Oxfmt`, `Oxlint` and `Typecheck` all report
/// `19:12:40`, which leaves no way to tell their output apart by time.
///
/// The markers appear in the same order as the workflow steps, so the two are
/// zipped. The lines before the first marker are the runner's own setup, which
/// belongs to the `Set up job` step that precedes the first workflow step.
///
/// The teardown is found by the runner's own `Post job cleanup.` line rather
/// than by time, because the stamps cannot place it: the last workflow step and
/// the teardown both report the same second, and in a real run the last step's
/// final summary is written 130ms before the teardown begins.
pub(crate) fn slice_log_by_steps(
    steps: &[ActionStep],
    log: &str,
    table: &mut ansi::AnsiTable,
) -> Vec<StepLines> {
    let coloured = log.contains('\u{1b}');
    let mut buckets: Vec<StepLines> = steps.iter().map(|_| StepLines::default()).collect();

    // The steps a marker can claim, paired with the index of the bucket each
    // one writes to.
    let real: Vec<usize> = steps
        .iter()
        .enumerate()
        .filter(|(_, step)| is_workflow_step(&step.name))
        .map(|(index, _)| index)
        .collect();
    if real.is_empty() {
        return buckets;
    }

    // The runner's own steps sit before and after the workflow steps and get
    // the output that no marker claims. A run with neither, which is a
    // composite action, falls back to the first workflow step.
    let setup = steps[..real[0]]
        .iter()
        .position(|step| step.name == "Set up job")
        .unwrap_or(0);
    // The step the teardown belongs to, which is the first of the runner's
    // trailing steps, and the step the runner's own last line belongs to when
    // the run never wrote a teardown.
    let trailing = (real[real.len() - 1] + 1..steps.len()).next();
    let complete = steps
        .iter()
        .position(|step| step.name == "Complete job")
        .unwrap_or(trailing.unwrap_or(setup));

    // The step the current marker opened, which starts on the setup step so
    // the runner's own output has somewhere to go.
    let mut current = setup;
    let mut markers = 0;
    for line in log.lines() {
        let (_, message) = split_log_line(line);
        // A marker opens the step it names. Every other `##[group]` title is a
        // runner section or an action's internal, which the runner shows as
        // its own collapsible heading and which says nothing about the step, so
        // those lines are hidden along with the `##[endgroup]` that closes
        // them.
        if let Some(title) = message.strip_prefix("##[group]") {
            if is_step_title(title) {
                current = *real.get(markers).unwrap_or(&current);
                markers += 1;
            }
            continue;
        }
        // The teardown starts at the runner's own line, so the switch happens
        // before the line is stored and that line opens the cleanup step. With
        // no teardown the runner's last line goes to `Complete job` instead,
        // which is the step it is in when it writes it.
        if has_message(line, CLEANUP_MARKER) {
            if let Some(trailing) = trailing {
                current = trailing;
            }
        } else if has_message(line, COMPLETE_MARKER) {
            current = complete;
        }
        // A line the runner echoed is shown without its `[command]` prefix and
        // in the runner's own colour, so it reads as the runner's line rather
        // than as something a build tool printed.
        if let Some(parsed) = ansi::parse_command_line(message, table) {
            buckets[current].push(parsed);
            continue;
        }
        let body = ansi::strip_workflow_commands(message);
        // A line that was blank apart from its stamp is blank output, so it
        // keeps its place and the step's spacing survives.
        if !body.is_empty() || message.is_empty() {
            buckets[current].push(ansi::parse_line(&body, table, coloured));
        }
    }
    buckets
}

/// Whether a `##[group]` title names a workflow step's command. The other
/// titles are the runner's own sections and an action's internals, and they
/// nest inside a step rather than starting one.
fn is_step_title(title: &str) -> bool {
    title.starts_with("Run ")
}

/// Whether a log line carries `marker` as its whole message, ignoring the
/// leading timestamp and any surrounding space.
fn has_message(line: &str, marker: &str) -> bool {
    split_log_line(line).1.trim() == marker
}

/// Splits a job log across the steps the job endpoint reported, from the
/// tab-separated dump the dev-only dump button produces.
///
/// The integration test drives the real split through this, so the fixture in
/// `tests/fixtures` is checked against the same code path the API uses rather
/// than against a reimplementation of it.
pub fn split_job_log_for_test(step_dump: &str, log: &str) -> Vec<ActionStep> {
    let steps: Vec<ActionStep> = step_dump
        .lines()
        .filter_map(|line| {
            let field = |key: &str| {
                line.split('\t')
                    .find_map(|part| part.strip_prefix(key))
                    .map(str::to_string)
            };
            Some(ActionStep {
                number: field("number: ")?.parse().ok()?,
                name: field("name: ")?,
                status: field("status: ").unwrap_or_default(),
                conclusion: field("conclusion: "),
                started_at: field("started_at: "),
                completed_at: field("completed_at: "),
                log: String::new(),
                spans_by_line: Vec::new(),
            })
        })
        .collect();
    let mut steps = collapse_teardown(steps, log);
    let mut table = ansi::AnsiTable::default();
    let buckets = slice_log_by_steps(&steps, log, &mut table);
    for (step, bucket) in steps.iter_mut().zip(buckets) {
        bucket.into_step(step);
    }
    steps
}

/// One step's log as parallel lines and span triples, so the frontend can
/// render it with the same component as a code fence.
#[derive(Default)]
pub(crate) struct StepLines {
    lines: Vec<String>,
    spans_by_line: Vec<Vec<u32>>,
}

impl StepLines {
    fn push(&mut self, parsed: ansi::ParsedLine) {
        self.lines.push(parsed.text);
        self.spans_by_line.push(parsed.spans);
    }

    pub(crate) fn into_step(self, step: &mut ActionStep) {
        step.log = self.lines.join("\n");
        step.spans_by_line = self.spans_by_line;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, start: &str, end: &str) -> ActionStep {
        ActionStep {
            number: 1,
            name: name.to_string(),
            status: "completed".into(),
            conclusion: Some("success".into()),
            started_at: Some(start.to_string()),
            completed_at: Some(end.to_string()),
            log: String::new(),
            spans_by_line: Vec::new(),
        }
    }

    /// Slices and renders, since the point of the change is what a step's log
    /// looks like rather than which bucket a line lands in. The teardown is
    /// collapsed first, so this drives the same path the API does.
    fn slice(steps: &[ActionStep], log: &str) -> Vec<ActionStep> {
        let mut table = ansi::AnsiTable::default();
        let mut steps = collapse_teardown(steps.to_vec(), log);
        let buckets = slice_log_by_steps(&steps, log, &mut table);
        for (step, bucket) in steps.iter_mut().zip(buckets) {
            bucket.into_step(step);
        }
        steps
    }

    /// The real step list and log from the run whose split was wrong, trimmed
    /// to the lines that matter. The step stamps are copied verbatim,
    /// including the three steps that all report `19:12:40`, because that
    /// collision is what a timestamp-based split cannot resolve.
    const REAL_LOG: &str = concat!(
        // Setup output, before any step marker.
        "2026-09-26T19:12:21.3522162Z Current runner version: '2.337.0'\n",
        "2026-09-26T19:12:21.3549723Z ##[group]Runner Image Provisioner\n",
        "2026-09-26T19:12:21.3550662Z Hosted Compute Agent\n",
        "2026-09-26T19:12:21.3554915Z ##[endgroup]\n",
        // Step 1, checkout.
        "2026-09-26T19:12:22.0354624Z ##[group]Run actions/checkout@v7\n",
        "2026-09-26T19:12:22.0355590Z with:\n",
        "2026-09-26T19:12:22.0356326Z   repository: sn0w12/gitau\n",
        "2026-09-26T19:12:22.0367279Z ##[endgroup]\n",
        "2026-09-26T19:12:22.6990933Z ##[group]Checking out the ref\n",
        "2026-09-26T19:12:22.7477192Z   git switch -\n",
        "2026-09-26T19:12:22.7480774Z HEAD is now at e27a631\n",
        "2026-09-26T19:12:22.7484497Z ##[endgroup]\n",
        "2026-09-26T19:12:22.7528669Z [command]/usr/bin/git log -1 --format=%H\n",
        "2026-09-26T19:12:22.8068627Z ##[group]Run actions/setup-node@v7\n",
        "2026-09-26T19:12:22.8070163Z with:\n",
        "2026-09-26T19:12:22.8088013Z ##[endgroup]\n",
        "2026-09-26T19:12:23.3945855Z ##[group]Environment details\n",
        "2026-09-26T19:12:23.8114175Z node: v24.21.0\n",
        "2026-09-26T19:12:23.8114175Z ##[endgroup]\n",
        // Step 2, the install.
        "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
        "2026-09-26T19:12:24.9808755Z ##[endgroup]\n",
        "2026-09-26T19:12:25.1000000Z \u{1b}[36;1mnpm ci\u{1b}[0m\n",
        "2026-09-26T19:12:38.9722213Z found 0 vulnerabilities\n",
        // Step 3, oxfmt. Its last line is stamped 40.153, inside the single
        // second oxlint reports, so a timestamp split gives it to oxlint.
        "2026-09-26T19:12:39.0831110Z ##[group]Run npm run format:check\n",
        "2026-09-26T19:12:39.0900212Z ##[endgroup]\n",
        "2026-09-26T19:12:39.2514996Z Checking formatting...\n",
        "2026-09-26T19:12:40.1530110Z All matched files use the correct format.\n",
        // Step 4, oxlint, whose whole window is that same second.
        "2026-09-26T19:12:40.1848978Z ##[group]Run npm run lint\n",
        "2026-09-26T19:12:40.1914984Z ##[endgroup]\n",
        "2026-09-26T19:12:40.5859108Z Finished in 245ms on 292 files with 128 rules.\n",
        // Step 5, typecheck.
        "2026-09-26T19:12:40.6040189Z ##[group]Run npm run typecheck\n",
        "2026-09-26T19:12:40.6106442Z ##[endgroup]\n",
        "2026-09-26T19:12:41.1000000Z tsc --noEmit\n",
        // Step 6, the tests, which start after typecheck's reported end.
        "2026-09-26T19:12:43.2560593Z ##[group]Run npm test\n",
        "2026-09-26T19:12:43.2624553Z ##[endgroup]\n",
        "2026-09-26T19:12:43.4000000Z Test Files  55 passed (55)\n",
        // The teardown, which the runner opens with its own line.
        "2026-09-26T19:13:20.1866212Z Post job cleanup.\n",
        "2026-09-26T19:13:20.3017463Z Cache hit occurred on the primary key.\n",
        "2026-09-26T19:13:20.5709221Z Cleaning up orphan processes\n",
    );

    /// The real step list for `REAL_LOG`, with the stamps GitHub reported for
    /// it copied verbatim. The three steps reporting `19:12:40` are the ones a
    /// timestamp split cannot separate, and the `Set up job` / `Post ...` /
    /// `Complete job` steps are the ones that carry no marker.
    fn real_steps() -> Vec<ActionStep> {
        vec![
            step("Set up job", "2026-09-26T19:12:21Z", "2026-09-26T19:12:21Z"),
            step(
                "Run actions/checkout@v7",
                "2026-09-26T19:12:21Z",
                "2026-09-26T19:12:22Z",
            ),
            step("Setup Node", "2026-09-26T19:12:22Z", "2026-09-26T19:12:24Z"),
            step(
                "Install frontend dependencies",
                "2026-09-26T19:12:24Z",
                "2026-09-26T19:12:39Z",
            ),
            step("Oxfmt", "2026-09-26T19:12:39Z", "2026-09-26T19:12:40Z"),
            step("Oxlint", "2026-09-26T19:12:40Z", "2026-09-26T19:12:40Z"),
            step("Typecheck", "2026-09-26T19:12:40Z", "2026-09-26T19:12:43Z"),
            step(
                "Frontend tests",
                "2026-09-26T19:12:43Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Setup Node",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Run actions/checkout@v7",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Complete job",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
        ]
    }

    #[test]
    fn both_timestamp_shapes_parse() {
        assert_eq!(
            parse_stamp("2026-09-26 19:12:21").map(|at| at.to_rfc3339()),
            Some("2026-09-26T19:12:21+00:00".to_string())
        );
        assert_eq!(
            parse_stamp("2026-09-26T19:12:21.6689146Z").map(|at| at.to_rfc3339()),
            Some("2026-09-26T19:12:21.668914600+00:00".to_string())
        );
        assert_eq!(parse_stamp("not a time"), None);
    }

    #[test]
    fn every_step_of_a_real_log_receives_its_own_output() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The marker names the command, not the step, so the step list is
        // matched in order. The runner's own setup output precedes every
        // marker, so it belongs to `Set up job`, and the first marker opens
        // checkout.
        assert_eq!(
            steps[0].log,
            "Current runner version: '2.337.0'\nHosted Compute Agent"
        );
        assert_eq!(
            steps[1].log,
            "with:\n  repository: sn0w12/gitau\n  git switch -\nHEAD is now at e27a631\n\
                 /usr/bin/git log -1 --format=%H"
        );
        assert_eq!(steps[2].log, "with:\nnode: v24.21.0");
        assert_eq!(steps[3].log, "npm ci\nfound 0 vulnerabilities");
        assert_eq!(
            steps[4].log,
            "Checking formatting...\nAll matched files use the correct format."
        );
        assert_eq!(
            steps[5].log,
            "Finished in 245ms on 292 files with 128 rules."
        );
        assert_eq!(steps[6].log, "tsc --noEmit");
        assert_eq!(steps[7].log, "Test Files  55 passed (55)");
        assert_eq!(
            steps[8].log,
            "Post job cleanup.\nCache hit occurred on the primary key.\n\
                 Cleaning up orphan processes"
        );
    }

    /// The bug this split exists to fix: oxfmt's last line is stamped
    /// `40.153`, oxlint's whole window is the second `40`, and typecheck's runs
    /// to `43` while the tests start at `43.2`. A timestamp split puts the
    /// formatter's output in oxlint, leaves oxlint empty, and puts the test
    /// output in typecheck.
    #[test]
    fn steps_sharing_a_reported_second_still_get_their_own_output() {
        let steps = slice(&real_steps(), REAL_LOG);
        assert!(steps[4]
            .log
            .contains("All matched files use the correct format."));
        assert!(!steps[5].log.contains("correct format"));
        assert!(!steps[6].log.contains("Test Files"));
        assert_eq!(
            steps[5].log,
            "Finished in 245ms on 292 files with 128 rules."
        );
        assert!(steps[7].log.contains("Test Files  55 passed (55)"));
    }

    #[test]
    fn output_before_the_first_marker_joins_set_up_job() {
        // The runner's own setup lines carry no marker of their own, and they
        // precede every step, so they belong to `Set up job` rather than to the
        // first workflow step.
        let steps = slice(&real_steps(), REAL_LOG);
        assert!(steps[0]
            .log
            .starts_with("Current runner version: '2.337.0'"));
        assert!(!steps[1].log.contains("Current runner version"));
    }

    /// The `Post ...` and `Complete job` steps write no marker and all report
    /// the same second, so they collapse into one cleanup step holding the
    /// teardown, whatever number of them the workflow has.
    #[test]
    fn the_post_steps_collapse_into_one_cleanup_step() {
        let steps = slice(&real_steps(), REAL_LOG);
        let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
        assert_eq!(names.last(), Some(&CLEANUP_STEP_NAME));
        assert_eq!(
            steps
                .iter()
                .filter(|step| step.name == CLEANUP_STEP_NAME)
                .count(),
            1
        );
        assert!(!names.iter().any(|name| name.starts_with("Post Setup")));
        assert!(!names.contains(&"Complete job"));
        assert!(steps[8].log.starts_with("Post job cleanup."));
    }

    /// A run that never writes the teardown has nothing to collapse, and its
    /// last line is the runner's, which belongs to `Complete job`.
    #[test]
    fn without_a_teardown_the_last_line_goes_to_complete_job() {
        // One marker for one workflow step, so the marker opens the tests.
        let steps_list = vec![
            step("Set up job", "2026-09-26T19:12:43Z", "2026-09-26T19:12:43Z"),
            step(
                "Frontend tests",
                "2026-09-26T19:12:43Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Post Setup Node",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
            step(
                "Complete job",
                "2026-09-26T19:13:20Z",
                "2026-09-26T19:13:20Z",
            ),
        ];
        let log = concat!(
            "2026-09-26T19:12:43.2560593Z ##[group]Run npm test\n",
            "2026-09-26T19:12:43.4000000Z Test Files  55 passed (55)\n",
            "2026-09-26T19:13:20.5709221Z Cleaning up orphan processes\n",
        );
        let steps = slice(&steps_list, log);
        let names: Vec<&str> = steps.iter().map(|step| step.name.as_str()).collect();
        // Nothing to gather, so the runner's own steps are left as they are.
        assert_eq!(
            names,
            vec![
                "Set up job",
                "Frontend tests",
                "Post Setup Node",
                "Complete job"
            ]
        );
        assert!(!names.contains(&CLEANUP_STEP_NAME));

        assert_eq!(steps[1].log, "Test Files  55 passed (55)");
        assert_eq!(steps[2].log, "");
        assert_eq!(steps[3].log, "Cleaning up orphan processes");
    }

    #[test]
    fn a_blank_stamped_line_keeps_its_place_in_the_step() {
        // A stamp with no message is a blank line, not a line whose stamp went
        // missing. It sits between two markers, so it belongs to the step they
        // bound, and dropping it would reflow that step's output.
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
                "2026-09-26T19:12:25.0000000Z\n",
                "2026-09-26T19:12:38.9722213Z found 0 vulnerabilities\n",
            ),
        );
        // The only marker opens checkout, the first real step, so the lines
        // after it are that step's output.
        assert_eq!(steps[1].log, "\nfound 0 vulnerabilities");
        assert_eq!(steps[1].spans_by_line, vec![Vec::<u32>::new(), Vec::new()]);
    }

    #[test]
    fn an_unstamped_line_joins_the_step_it_falls_in() {
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:24.9518805Z ##[group]Run npm ci\n",
                "hint: names commonly chosen\n",
                "instead of 'master' are 'main'\n",
            ),
        );
        assert_eq!(
            steps[1].log,
            "hint: names commonly chosen\ninstead of 'master' are 'main'"
        );
    }

    #[test]
    fn the_runner_sections_and_an_action_internals_stay_with_their_step() {
        // `##[group]` also wraps the runner's own sections and an action's
        // internals, all nested inside a step. Only the `Run` title opens a
        // step, so these lines stay in the step they are nested in rather than
        // starting a bucket of their own. The titles themselves are hidden,
        // since they are the runner's headings rather than output.
        let steps = slice(
            &real_steps(),
            concat!(
                "2026-09-26T19:12:22.0354624Z ##[group]Run actions/checkout@v7\n",
                "2026-09-26T19:12:22.6990933Z ##[group]Checking out the ref\n",
                "2026-09-26T19:12:22.7477192Z   git switch -\n",
                "2026-09-26T19:12:22.7484497Z ##[endgroup]\n",
            ),
        );
        assert_eq!(steps[1].log, "  git switch -");
        assert_eq!(steps[2].log, "");
    }

    #[test]
    fn a_log_with_no_markers_lands_in_the_first_real_step() {
        // A run whose log carries no markers at all cannot be split, so the
        // text goes to the first real step rather than being dropped.
        let steps = vec![step("only", "", ""), step("other", "", "")];
        let log = "first line\nsecond line\n";
        let steps = slice(&steps, log);
        assert_eq!(steps[0].log, "first line\nsecond line");
        assert_eq!(steps[1].log, "");
    }

    #[test]
    fn a_run_with_only_runner_steps_writes_nothing() {
        // No step is a workflow step, so there is nowhere to put the log.
        let steps = vec![step("Set up job", "", ""), step("Complete job", "", "")];
        let steps = slice(
            &steps,
            "2026-09-26T19:12:21.3522162Z Current runner version\n",
        );
        assert_eq!(steps[0].log, "");
        assert_eq!(steps[1].log, "");
    }

    #[test]
    fn ansi_colour_is_split_out_of_the_log_text() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The install step's first line is the cyan `npm ci` echo. The style id
        // is resolved rather than hardcoded, since ids depend on the order
        // styles were interned in.
        let spans = steps[3].spans_by_line[0].clone();
        assert_eq!(spans.len(), 3);
        assert_eq!(&spans[0..2], [0, 6]);
        assert!(steps[3].log.starts_with("npm ci"));
        assert_eq!(steps[4].spans_by_line, vec![Vec::<u32>::new(); 2]);
    }

    #[test]
    fn a_command_line_is_shown_without_its_prefix_in_blue() {
        let steps = slice(&real_steps(), REAL_LOG);
        // The checkout step's last line is a command the runner echoed.
        let checkout = &steps[1];
        assert!(
            checkout.log.ends_with("/usr/bin/git log -1 --format=%H"),
            "the prefix was not stripped: {:?}",
            checkout.log
        );
        assert!(!checkout.log.contains("[command]"));
        let last = checkout.spans_by_line.last().expect("the line has spans");
        assert_eq!(last.len(), 3, "the command line is not styled");
        let command_len = checkout.log.lines().last().unwrap().chars().count() as u32;
        assert_eq!(&last[0..2], [0, command_len]);
    }

    #[test]
    fn the_leading_stamp_is_dropped_but_indentation_is_kept() {
        assert_eq!(split_log_line("2026-09-26T19:12:25.5Z hello").1, "hello");
        assert_eq!(split_log_line("  indented").1, "  indented");
        assert_eq!(split_log_line("not a stamp").1, "not a stamp");
    }
}
