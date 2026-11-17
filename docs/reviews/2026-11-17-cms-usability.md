# Independent CMS usability review

- Reviewer: inherited gpt-6.1-sol, reasoning effort unknown.
- Task: deleg_05f2205a / sa-0-ac5c3526; one independent read-only whole-feature review, no sub-reviewers.
- Backend: 521917979dfabbb225ec6be5319f10037612abab..ee80133f0d79d9eca3b561cad9b0a9f50fbec5f2.
- Frontend: f1bac5bf716bc0e72c01cc068437e5a269d7d675..45a08d427b95861d724561983ac789a4dbb91fdb.
- Original assessment: **With fixes**, 0 Critical /1 Important /2 Minor. Not an independent post-correction approval.

## Strengths reported

The reviewer inspected actual source/diffs and confirmed atomic release composition, bank-root revision invalidation for question edits, complete paginated review reads, conservative unknown-ack handling/publication readback, section metadata preservation and explicit round-trip validated WIB conversion.

## Original findings

### I1 — Previous enrollment answers retained under the next enrollment
Frontend `src/pages/TeacherResultsPage.tsx:15,47–55` keyed StudentDetail only by student. Changing enrollment_id for that same student reloaded result data but retained tab/attempt/offset and answer resources. Actual-component scratch reproduction observed the new enrollment heading with previous enrollment answers and a blank selected attempt.

Required correction: key by student+enrollment identity and prevent requesting/rendering an attempt absent from the current result; test both already-loaded and delayed answers across enrollment navigation.

### M1 — Restored question-report URL loses bank context
Frontend `src/pages/TeacherResultsPage.tsx:21,30,37–43`: question/version persist in URL, bank only in local state. A refreshed Quiz URL can show the first bank and lose available version history; bank switching clears restored question/version. Actual answers remain version-specific. Suggested future fix: persist/resolve bank identity and cover refreshed/shared archived/off-page Quiz reports.

### M2 — Latest activity omits attempt-start/section-visit timestamps
Backend `src/modules/reporting/infrastructure/students.rs:18`: GREATEST includes submissions, saved answers, lesson opening/completion and measurements, but not assessment_attempts.created_at or learning_section_visits.first_visited_at. Suggested future fix: include those events or narrow the definition/label, with projection regressions for each event.

## Reviewer evidence and limits

Reviewed required engineering/spec/plan/handoff documents, actual diffs/source and tests. Read prior logs reporting9 browser and1 native zoom test passed; did not claim fresh reviewer-owned gates. Both HEADs and working trees remained unchanged/clean. No production/secret access, DB tests, build or fixture regeneration. One scratch-only actual-component reproduction harness was created before a later no-more-writes instruction; no tracked checkout, index or HEAD changes. Screenshot/zoom metric paths were absent after default Playwright output cleanup during review; reviewer neither inferred they were never run nor claimed visual inspection. Parent subsequently regenerated/preserved durable evidence outside that output root.

## Declined-to-judge items (parent disposition required)

1. Owner-approved light sidebar/sans CMS supersedes prior green header/serif only in admin area.
2. Adolescent BMI classification separately deferred pending validated inputs/reference.
3. Lower-level publication APIs explicitly retained for compatibility.
4. Global-admin closure is the integration base, not stale main.
5. Native implementation/Luna routing limitation disclosed; process choice remains with owner.
6. WIB conversion correctness reviewed, owner timezone preference not judged.
7. Prior unrelated global-admin/deployment minors outside this diff.
8. Production deployment, images, operational config, backups/restore not reviewed/authorized.
9. No production-scale performance/load certification.
10. No independent visual/reflow acceptance from missing files; parent owns that verification.

## Parent correction status

I1 reproduced by two real-component regressions (loaded answers and delayed response): both RED with previous answer remaining visible. Parent changed the detail key to student+enrollment and gated answer reads/rendering on current-result attempt membership. Both tests GREEN; full frontend59 Vitest +6legacy and build passed. A fresh real-backend browser gate and final handoff are required before parent acceptance. M1/M2 remain deferred Minor findings, not silently fixed. No second independent review is claimed.
