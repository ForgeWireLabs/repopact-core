# WI079 prospective comparative protocol — pre-registration

Date registered: 2026-11-22. Status: **planned, NOT RUN**. This is an exploratory validation of the optional helper, not evidence for the claim that one agent replaces an independent reviewer.

## Research question and competing hypotheses

For otherwise identical scoped changes, does a structured pre-completion self-audit with candidate freshness checking reduce escaped **known, independently seeded defects** relative to ordinary verification, without causing worse time, cost, or unnecessary code churn? Is it comparable to a separate reviewer?

- H0: Self-audit has no meaningful improvement in escaped-defect rate, or raises total cost/churn enough to offset any improvement.
- H1: Self-audit improves escaped-defect rate over baseline at a similar false-stop rate, with measured overhead.
- H2 (separate): Self-audit performs comparably to independent review across both routine and security-relevant tasks; reject or qualify this claim if meaningful gaps remain.

Do not treat negative results as failed implementation to be suppressed. A structural candidate match is a mechanism test, not a measure of model review quality.

## Registered conditions

A. **Baseline** — ordinary repository contract and actual tests; no WI079 self-audit instruction or helper.

B. **Self-audit** — identical tasks, tools and verification, plus a bounded self-audit prompt, no more than two correction cycles, and `self-audit capture/check` after the final changed candidate. Report honest failed/incomplete cases. A forged, unchecked, or stale report is not a pass.

C. **Independent reviewer** — identical implementing agent and verification plus one *fresh-context* reviewer (same model family if available) inspecting exactly the candidate the implementer produced, with one bounded fix/retest handoff. No shared chain-of-thought or hidden implementation context.

## Task construction and locks

Create 12 isolated task fixtures before seeing any experimental outcome: four routine edge-case/contract changes, four stateful/concurrent/resource-hygiene changes, and four security/authorization/failure-mode changes. Include both planted defects and known-clean tasks to measure false findings. Each fixture has a pinned baseline tree, fixed acceptance criteria, private golden tests and an external adjudication rubric. Record exact Git SHAs and checksums before running.

Prefer synthetic/local fixtures with no external services, sensitive data, production hosts or spend. Where models differ by availability, **do not compare their conditions as if matched**. Run each condition with the same selected model/version, toolset, resource ceiling, temperature/seed policy, and initial source tree; randomize condition order per fixture. Keep test-harness feedback and task instructions equal across arms except for the assigned review intervention. Reviewer independence must be declared honestly.

Preliminary 12-task/three-condition runs have limited statistical power. Treat conclusions as descriptive and exploratory; expand tasks/seeds under a separately authorized budget only after evaluating whether the pilot can falsify a useful effect.

## Primary outcome and measurement

Primary: `escaped_defects / seeded_defects` on independent golden checks after final candidate, with denominator, task identity, severity and platform recorded. Also report the number of clean tasks falsely flagged, new defects introduced by remediation, and task completion success with independent adjudication.

Secondary: elapsed time, steps, executed test count, tokens/inference cost if observable, max local resource use, final changed-line count, additional changed lines after initial implementation, and accepted vs rejected findings. Missing metrics are **NOT MEASURED**, never estimated as zero.

Define excessive churn as more than 20% extra modified lines versus the corresponding baseline fixture without a verified defect fix; this threshold is an analysis flag, not an automatic rejection of any useful refactor. Do not alter that flag after seeing outcomes.

## Falsification and stop criteria

- If self-audit does not lower observed escaped-defect rate relative to baseline or triggers materially more false stops and churn, do not claim it improves practical quality.
- If independent review catches defects the self-audit consistently misses, do not claim it replaces a second reviewer in that class.
- If the tool returns structural PASS on a knowingly stale candidate in a negative fixture, stop and record a mechanism defect before the comparative study.
- Abort a task on protected-surface alteration, unapproved external execution, leaked credential material, unbounded loops, or violation of resource caps.
- Preserve failures and inconclusive outcomes with exact task artifacts and timestamps.

## Planned evidence package

- `benchmark-manifest.json`: precommitted task ID, fixture SHA, gold-test SHA, condition/seed, model/version, prompt hashes, toolchain, and limits.
- `raw/<task>/<condition>/`: implementation candidate SHA or working-tree snapshot, self-review report (if any), reviewer notes, executed commands, outcomes, token/time telemetry and original tests.
- `results.json`: per-task scoring with denominators and missing-data flags.
- `analysis.md`: predeclared metric summaries, caveats, and adverse outcomes.

This preregistration does **not** authorize beginning model runs, account spend, or new production integrations. Reuse RepoPact Proving Grounds / PactBench tooling only where the fixture and instrumentation contracts match.
