# Kairos Development Rules

Phase 0 requires a verified private GitHub repository on the main branch, a connected local clone, and an initial commit pushed to GitHub. Begin application development after this verification and report the repository URL and initialization status.

## Git and data safety

- Commit project initialization, each completed development phase, each passed two-phase quality gate, important bug fixes, and final release preparation.
- Use descriptive commit messages. After each passed two-phase gate, update progress and project documentation, commit the verified changes, push them to GitHub, and continue to the next group.
- Never force-push or overwrite remote history without explicit authorization.
- Keep credentials, API keys, access tokens, passwords, user databases, and private backups out of Git. Keep application development in this connected repository.

## Autonomous execution

Make routine technical decisions, fix bugs, and rerun tests independently. Interrupt the user when authentication or permissions are required, a destructive action needs authorization, a critical product decision cannot be inferred, an external dependency prevents progress, or Windows hardware or user interaction is needed for verification. Report what is verified, the exact blocker, and the minimum action required.

---

# Autonomous Development Workflow

## 1. Development Strategy

Implement the 8 development phases in 4 consecutive groups.

- Group 1: Phase 1 + Phase 2
- Group 2: Phase 3 + Phase 4
- Group 3: Phase 5 + Phase 6
- Group 4: Phase 7 + Phase 8

**Important: Complete two phases at a time. After each pair of phases, perform a comprehensive self-review and quality check before proceeding to the next group.**

Do not stop and wait for my approval after every phase.

If all checks pass, automatically continue to the next group.

## 2. Quality Gate After Every Two Phases

After completing each group, perform the following checks:

### A. Functional Verification
- Verify that all features specified in the two completed phases are actually implemented.
- Test all major user interactions.
- Verify that previously implemented features still work.
- Check edge cases and error handling.
- Ensure there are no unfinished critical features or placeholder implementations.

### B. Code Quality Review
- Review the overall project architecture.
- Check for duplicated or unnecessary code.
- Check TypeScript type safety.
- Check Rust error handling.
- Check database operations and transaction safety.
- Identify potential memory leaks and unnecessary background processes.
- Ensure the code remains modular and maintainable.

### C. Automated Testing
- Run all relevant unit tests.
- Run integration tests.
- Run frontend linting and type checking.
- Run Rust tests.
- Run the production build.
- Fix all discovered errors before proceeding.

### D. Regression Testing
- Verify that features from previous phases still work.
- Ensure new changes have not broken existing functionality.
- Verify database compatibility and data persistence.

### E. Documentation
After each group:
- Update README.md.
- Update development documentation.
- Update the implementation status.
- Document known limitations and unresolved issues.
- Record the tests performed and their results.

## 3. Pass / Fail Criteria

A group can be marked PASS only when:

1. All required features in the group are implemented.
2. All relevant automated tests pass.
3. Type checking and linting pass.
4. The application builds successfully.
5. No critical or high-severity bugs remain.
6. Previous functionality remains intact.
7. The completed features have been manually verified where necessary.

Do not mark a group as completed simply because its code has been written.

If any check fails:
- Identify the root cause.
- Fix the issue.
- Rerun the relevant tests.
- Repeat the quality gate until it passes.

If a requirement cannot be verified because of missing hardware, operating system capabilities, permissions, or unavailable test environments, explicitly document the limitation. Never report an unverified feature as PASS.

## 4. Automatic Continuation

After a group passes its quality gate:

1. Mark the group as PASS.
2. Update the project progress documentation.
3. Create a Git commit with a descriptive message, if the repository is under Git.
4. Automatically start the next group.
5. Continue until all 8 phases are completed.

Do not wait for additional instructions between groups unless:
- A critical architectural decision requires my input.
- A destructive or irreversible operation requires my approval.
- An external dependency makes further development impossible.
- A serious unresolved issue prevents safe continuation.

## 5. Progress Reporting

After every two phases, provide a concise report containing:

- Completed phases.
- Implemented features.
- Tests executed.
- Test results.
- Build status.
- Bugs discovered and fixed.
- Known limitations.
- Current Git commit.
- Next development group.

Use the following format:

GROUP 1 — PHASE 1 & 2

Status: PASS / FAIL

Implemented:
...

Tests:
...

Build:
...

Issues:
...

Next:
GROUP 2 — PHASE 3 & 4

## 6. Final Completion

After Phase 8:

- Perform a complete project-wide code review.
- Run all available automated tests.
- Verify the Windows production build.
- Test the installer and uninstaller.
- Verify data persistence and backup restoration.
- Verify core functionality on Windows 11.
- Prepare the final installation package.
- Produce a final development and testing report.

The final deliverable must be a working Windows desktop application, not merely a successful compilation.

**Development principle: Implement → Test → Review → Fix → Pass → Continue.**

Work autonomously and prioritize correctness, stability, and maintainability over development speed.