# Kairos — Product Requirements Document

**Version:** 1.0  
**Product Name:** Kairos  
**Greek Name:** καιρός  
**Tagline:** At your own pace.  
**Platform:** Windows 11  
**Application Type:** Native Desktop Application  
**Development Stack:** Tauri 2 + React + TypeScript + Rust + SQLite

---

# 1. Product Overview

## 1.1 Product Vision

Kairos is a minimalist personal desktop task management application.

It is designed to stay quietly on the user's desktop, helping them organize daily tasks, manage deadlines, maintain routines, and track progress.

Kairos is not designed to maximize productivity at all costs.

Its philosophy is:

**At your own pace.**

The application should help users organize their lives without creating unnecessary pressure.

## 1.2 Core Principles

- Minimalism: Keep the interface clean and uncluttered.
- Calmness: Avoid aggressive notifications and excessive visual stimulation.
- Reliability: Never lose user tasks or historical records.
- Flexibility: Allow users to customize categories, schedules, reminders, and layouts.
- Local-first: All core functionality must work offline.
- Personalization: Remember user preferences and interface settings.

## 1.3 Target Experience

The application should behave like a native desktop companion rather than a conventional web-based Todo application.

It should remain easily accessible while the user is studying, programming, browsing, or working.

---

# 2. Visual Identity and Design System

## 2.1 Brand Identity

Product name: Kairos

Tagline: At your own pace.

Brand personality:
- Calm
- Minimal
- Elegant
- Personal
- Reliable

Logo direction:
- Minimalist lettermark based on K.
- Monochrome design.
- Rounded square application icon.
- Avoid complex illustrations.

## 2.2 Visual Style

Apple-inspired minimalist design.

Reference design language:
- macOS Reminders
- macOS native floating panels
- Apple-style translucent materials

The application must not copy Apple's proprietary icons or assets.

## 2.3 Color Palette

Primary colors:
- Black: #171717
- Dark Gray: #454545
- Neutral Gray: #888888
- Light Gray: #D9D9D9
- Off White: #F5F5F5
- White: #FFFFFF

Priority colors:
- High: muted red
- Medium: muted orange
- Low: muted green

Priority colors should be used sparingly.

Avoid large saturated colored backgrounds.

## 2.4 Window Appearance

Implement a completely custom frameless window.

Requirements:
- No conventional Windows title bar.
- Custom Apple-inspired window controls.
- Rounded corners.
- Subtle shadows.
- Light frosted-glass background.
- Custom drag region.
- Custom resize handling.
- Smooth animations.

Window controls:
- Red: Close to system tray.
- Yellow: Minimize.
- Green: Expand / restore window.

## 2.5 Transparency

Provide four presets:

1. Solid
2. Light (default)
3. Medium
4. Transparent

Transparency must adapt to both light and dark themes.

Use Windows 11-compatible backdrop effects where possible.

If a particular backdrop effect is unsupported, provide a visually consistent fallback.

## 2.6 Theme

Three options:

- Light
- Dark
- Follow System (default)

Manual theme selection overrides system-following mode until the user selects Follow System again.

## 2.7 Language

Mixed Chinese-English interface.

Navigation and functional labels should primarily use English.

Examples:
- Tasks
- Today
- Upcoming
- No Date
- Completed
- Statistics
- Trash
- Settings

User-created task titles, notes, and categories must remain unchanged.

Date and time formats follow Windows regional settings.

---

# 3. Desktop Window and System Integration

## 3.1 Default Display

The application must remain visible on the desktop without requiring manual reopening.

Default position:
- Primary display
- Top-right corner

Requirements:
- Freely draggable.
- Remember previous position.
- Remember previous dimensions.
- Support multiple monitors.
- Remember separate layouts for each monitor.
- Automatically relocate when a monitor disconnects.
- Restore previous layouts when a monitor reconnects.

## 3.2 Window Size

Default: Medium.

Maximum height: 70% of available screen height.

Smart adaptive layout:
- Few tasks: compact window.
- More tasks: gradually increase height.
- Maximum height reached: enable internal scrolling.
- Header and quick-add area remain fixed.
- Window width should not automatically change based on task count.

Users can resize the window manually.

## 3.3 Window Modes

Support three modes:

1. Desktop Mode
   - Display on desktop.
   - Do not force always-on-top behavior.

2. Always on Top
   - Keep the window above other windows.

3. Smart Mode (default)
   - Automatically manage window stacking based on the foreground application.
   - Support application whitelist.
   - Detect fullscreen applications and automatically avoid covering them.
   - Restore the configured behavior after fullscreen applications close.

## 3.4 Click-through Mode

Default: disabled.

Optional Click-through Mode:
- Mouse events pass through the window.
- The panel remains visible.
- A global shortcut or tray-menu action must restore normal interaction.
- Prevent users from getting permanently stuck in click-through mode.

## 3.5 Window Lifecycle

Clicking the close button must hide the window to the system tray instead of terminating the application.

System tray menu:
- Show Window
- Settings
- Exit

Only explicit Exit terminates the process.

When fully exited, background reminders are no longer guaranteed to run.

## 3.6 Startup

Automatically start after Windows login.

Default behavior:
- Launch the application.
- Display the task panel.
- Restore previous position, size, theme, and window mode.

Support disabling startup in Settings.

Prevent multiple application instances.

## 3.7 Global Shortcuts

Required shortcuts:

- Ctrl + N: New Task
- Ctrl + F: Search
- Ctrl + Shift + T: Show / Hide Window
- Ctrl + ,: Settings
- Enter: Confirm Quick Add
- Esc: Close current popover or dialog

Global shortcuts must be registered using Windows-compatible mechanisms.

Important: Ctrl + Shift + T conflicts with the Chrome shortcut for reopening closed tabs. Before finalizing the global shortcut, verify whether it should be replaced with a non-conflicting combination.

---

# 4. Main Interface

## 4.1 Layout

Use an extremely minimal main interface.

Do not use a permanent left sidebar.

Header:
- Current date
- Dynamic greeting
- Daily completion progress
- Search icon
- Filter icon
- More menu

Greeting:
- Good morning.
- Good afternoon.
- Good evening.

The greeting should follow local system time.

## 4.2 Daily Progress

Display:

- Completed parent tasks today.
- Total relevant parent tasks today.
- Completion percentage.
- Minimal progress bar.

Only parent tasks count toward progress.

Subtasks must never be counted separately.

Prevent division-by-zero errors when there are no tasks.

## 4.3 Main Sections

Three primary sections:

### Today

Contains:
- Tasks scheduled for today.
- Tasks carried forward from missed scheduled dates.
- Overdue tasks.

Overdue tasks must appear at the top.

### Upcoming

Contains tasks scheduled for future dates.

Sort by scheduled date.

### No Date

Contains tasks without a scheduled date.

Tasks without a scheduled date may still have a due date.

If their due date expires, move them to Today and mark them Overdue.

## 4.4 Overdue Logic

Distinguish:

Rescheduled:
- Scheduled Date has passed.
- Due Date has not expired.
- Automatically moves to Today.
- Display Rescheduled status.
- Preserve previous scheduled dates in task history.

Overdue:
- Due Date has expired.
- Task remains incomplete.
- Move to the top of Today.
- Display Overdue 1 day, Overdue 2 days, etc.

Due date expiration must use the actual deadline timestamp.

If only a Due Date is specified without a time, default to 23:59 local time.

## 4.5 Task Row

Default appearance:
- Only display circular checkbox and task title.

On hover:
- Reveal priority.
- Reveal category.
- Reveal scheduled date.
- Reveal deadline information.

Clicking the task opens its detail panel.

Avoid permanently displaying excessive metadata.

---

# 5. Task Management

## 5.1 Quick Add

Primary interaction:

- Click the + button.
- Display a compact input field.
- Enter task title.
- Press Enter to create.
- Press Esc to cancel.

New task defaults:
- Priority: Medium.
- Category: None.
- Scheduled Date: None.
- Due Date: None.
- Reminders: None unless inherited from a selected category.

New tasks without a Scheduled Date enter No Date.

Do not force users to complete a form when creating a task.

## 5.2 Task Details

Clicking a task opens a lightweight Apple-style detail panel.

Editable fields:

- Task Title
- Category
- Priority
- Scheduled Date
- Due Date
- Due Time
- Notes
- Subtasks
- Reminders
- Allow During DND
- Recurrence

Changes must save automatically.

Support closing the panel by clicking outside or pressing Esc.

## 5.3 Priority

Three levels:

| Priority | Meaning |
|---|---|
| High | Important |
| Medium | Normal |
| Low | Optional |

Default: Medium.

Allow filtering and automatic sorting by priority.

## 5.4 Categories

Support custom categories.

Functions:
- Create
- Rename
- Change color
- Delete
- Filter tasks by category

A task may have one category or no category.

When deleting a category, allow reassignment or removal of the category from associated tasks.

Category names must not be hardcoded.

## 5.5 Scheduled Date and Due Date

These are independent fields.

Scheduled Date:
- Date only.
- Determines task section.
- Used for daily planning and Streak calculations.

Due Date:
- Date and optional time.
- Determines deadlines.
- Used for deadline reminders and overdue calculations.

If no due time is selected, use 23:59.

Date and time selection should resemble Apple Calendar:
- Native-feeling calendar popover.
- Month navigation.
- Today shortcut.
- Keyboard input.
- Time adjustment.
- Light and dark theme support.

## 5.6 Sorting

Support two modes.

Automatic:
1. Overdue tasks first.
2. Sort by deadline.
3. Higher priority first when deadlines are equal.
4. Tasks without deadlines appear after tasks with deadlines.
5. Upcoming tasks are primarily ordered by Scheduled Date.

Manual:
- Drag and drop.
- Remember ordering.
- New tasks inserted at the top of the relevant section.

Switching back to Automatic restores computed ordering.

---

# 6. Subtasks

Support one-level subtasks.

Functions:
- Add
- Edit
- Delete
- Reorder
- Check / Uncheck

Display progress, for example:

2 / 4

Completion rules:

1. Completing the final subtask automatically completes the parent.
2. Manually completing a parent automatically completes all subtasks.
3. Restoring a completed parent preserves existing subtask states.
4. Subtasks do not count separately in Statistics.

No multi-level nested subtasks.

---

# 7. Recurring Tasks

## 7.1 Recurrence Rules

Support:
- Daily
- Weekly
- Monthly
- Every N days
- Every N weeks
- Every N months
- Selected weekdays
- Specific day of month
- End date
- No end date

A recurring task must have a Scheduled Date as its recurrence anchor.

## 7.2 Generation

Only one active occurrence should be generated at a time.

Do not generate large numbers of future instances.

Generate the next occurrence after:
- Current occurrence is completed.
- Current occurrence is skipped.

If a task remains incomplete, keep it active and carry it forward.

Do not automatically discard incomplete recurring tasks.

The next occurrence must follow the recurrence rule and should be the next valid occurrence after the completion or skip date.

Preserve the relative offset between Scheduled Date and Due Date when generating subsequent instances.

## 7.3 Subtask Inheritance

Automatically copy subtasks into the next occurrence.

Reset all copied subtasks to incomplete.

Preserve:
- Subtask titles.
- Subtask ordering.

Do not copy previous completion timestamps.

## 7.4 Editing Recurring Tasks

When editing a recurring task, provide:

- Only This Task
- This and Future Tasks
- Entire Series

Previously completed timestamps and completion states must remain intact.

## 7.5 Deleting Recurring Tasks

Provide:

- Only This Task
- This and Future Tasks
- Entire Series

Completed historical occurrences remain in Completed.

Unfinished deleted occurrences move to Trash.

Deleting an entire series stops all future generation.

## 7.6 Skip This Occurrence

Provide an independent operation:

Skip This Occurrence.

Distinguish it from Complete and Delete.

When skipped:
- Mark occurrence as Skipped.
- Do not count it as completed.
- Do not trigger remaining reminders.
- Generate the next occurrence.
- Preserve skipped history.

---

# 8. Completion, History and Trash

## 8.1 Completed

When a task is completed:
- Remove it from active sections.
- Move it to Completed.
- Record completion timestamp.
- Preserve task information.
- Cancel remaining reminders.

Support:
- Search
- Date-based browsing
- Restore
- Individual deletion
- Batch deletion
- Clear history

Completed tasks are retained indefinitely by default.

## 8.2 Trash

Deleting an active task moves it to Trash.

Support:
- Restore
- Batch restore
- Individual permanent deletion
- Clear Trash

Require confirmation before permanent deletion.

Restoring a task should preserve its category, dates, priority, subtasks, and recurrence information.

---

# 9. Reminders and Notifications

## 9.1 Reminder Configuration

Support multiple reminders per task.

Preset options:
- 10 minutes before
- 30 minutes before
- 1 hour before
- 1 day before
- At deadline
- Custom date and time

Allow custom reminders independent of the deadline.

## 9.2 Category Defaults

Each category can define default reminder rules.

When a new task is assigned to a category, apply its default reminders if the task has no explicitly configured reminders.

Individual task reminders override category defaults.

Changing category defaults must not unexpectedly overwrite customized reminders.

## 9.3 Windows Notifications

Use native Windows notifications where feasible.

Notification content:
- Task title
- Deadline
- Reminder context

Actions:
- Complete
- Snooze
- Open Task

Snooze presets:
- 5 minutes
- 15 minutes
- 30 minutes
- 1 hour
- Custom time

Ensure an individual reminder does not fire repeatedly.

## 9.4 Missed Reminders

When Windows wakes from sleep or the application resumes:

- Check missed reminders.
- Ignore completed or deleted tasks.
- Aggregate multiple missed reminders for the same task into one notification.
- Include the number of missed reminders.
- Preserve future reminders.
- Avoid notification spam.

## 9.5 Notification Sounds

Provide:
- System Default
- Silent
- Custom Sound

Support sound preview.

Provide volume control if technically supported.

If Windows native toast notifications do not permit independent per-application volume control, clearly explain the limitation and implement an application-controlled audio fallback where feasible.

Do not pretend unsupported controls work.

## 9.6 Do Not Disturb

Support:
- Multiple scheduled DND periods.
- Manual activation.
- Manual deactivation.
- Silent-only mode.
- Complete notification suppression.
- Aggregated missed notifications after DND ends.

Each task has an Allow During DND switch.

Default: disabled.

This setting may bypass Kairos's own DND rules, but cannot guarantee bypassing Windows-level Focus Assist or system notification restrictions.

---

# 10. Statistics and Streak

## 10.1 Statistics Page

Accessible through More.

Show:
- Today's completed tasks.
- Today's total tasks.
- Today's completion rate.
- Weekly completed task chart.
- Weekly completed total.
- Current Streak.
- Best Streak.

Weekly period:
Monday to Sunday.

## 10.2 Counting Rules

Only parent tasks count.

Tasks with subtasks count as one task.

A parent task completed after all subtasks are completed counts as one completion.

Skipped tasks do not count as completed.

Historical statistics should be derived from persistent completion records.

## 10.3 Streak Rules

A day is successful when all its planned parent tasks are completed.

Additional rules:
- No scheduled tasks: automatically treated as a successful day.
- Custom rest day: neutral; does not increase or break Streak.
- Unfinished planned tasks: unsuccessful day.
- Skipped occurrences: neutral and excluded from completion requirements.
- If other planned tasks exist on a day with skipped occurrences, all non-skipped planned tasks must still be completed.

Allow users to configure multiple weekly rest days.

Preserve historical completion records when rest-day settings change.

---

# 11. Search and Filtering

## 11.1 Search

Global search across:
- Today
- Upcoming
- No Date

Search:
- Task titles
- Notes

Exclude:
- Completed
- Trash

Show search results grouped by their original sections.

## 11.2 Filters

Open through a top-level filter popover.

Filters:
- Category
- Priority
- Scheduled Date
- Due Date
- Overdue

Multiple filters use AND logic.

Example:

Category = Study  
Priority = High  
Scheduled Date = Today

Only tasks satisfying all selected conditions should appear.

Provide Clear All.

Search and filters must work simultaneously.

---

# 12. Navigation

No permanent sidebar.

Main screen contains:
- Search
- Filter
- More menu

More menu contains:
- Completed
- Statistics
- Trash
- Categories
- Settings

Keep the primary task interface minimal.

---

# 13. Data Architecture

## 13.1 Database

Use SQLite.

Database must be stored in the user's application data directory, not the installation directory.

Use Rust for database access.

Enable reliable transaction handling.

Use database migrations.

Consider SQLite WAL mode for reliable concurrent access.

## 13.2 Suggested Tables

Design normalized database tables for:

- tasks
- subtasks
- categories
- recurrence_series
- reminders
- completion_history
- skipped_occurrences
- task_events
- app_settings
- dnd_schedules
- backup_records

Use stable unique identifiers.

Preserve relationships between recurring instances and their parent series.

## 13.3 Data Safety

All task changes must be persisted reliably.

Important operations must use transactions.

Avoid destructive migrations.

Support database schema versioning.

Prepare the data architecture for future cloud synchronization, but do not implement cloud services in version 1.

## 13.4 Backup

Automatic backup:
- Daily.
- Before application updates.
- Before important destructive operations.

Keep the latest 30 backups.

Allow custom backup directory.

Allow manual backup.

Display backup timestamps and file sizes.

Use a consistent SQLite backup mechanism rather than blindly copying an active database file.

## 13.5 Restore

Support two modes:

Replace:
- Restore the exact historical backup.

Merge:
- Preserve current records.
- Merge backup records.
- Avoid duplicates using stable IDs and series identifiers.

Before restoring:
- Automatically back up current data.
- Display the operation's impact.
- Require confirmation.

A failed restore must not destroy the current database.

---

# 14. Technical Architecture

## 14.1 Stack

Frontend:
- React
- TypeScript
- Vite

Desktop:
- Tauri 2

Backend:
- Rust

Database:
- SQLite

Platform:
- Windows 11

Packaging:
- Tauri Windows installer
- NSIS or another supported Windows installer target

## 14.2 Architecture Principles

Separate:
- UI components
- Business logic
- Database access
- Notification scheduling
- Windows integration
- Backup management

Use strongly typed interfaces.

Avoid putting business logic directly inside UI components.

Use a clear event-driven communication layer between React and Rust.

## 14.3 Windows Integration

Implement and test:
- Frameless window
- Custom drag and resize
- Always-on-top
- Click-through
- Multi-monitor positioning
- System tray
- Global shortcuts
- Startup registration
- Native notifications
- Fullscreen detection
- Foreground application detection

Research Tauri 2 plugin compatibility before implementation.

Use Windows APIs where existing Tauri plugins are insufficient.

---

# 15. Installer and Distribution

Generate a standalone Windows installer (.exe).

Requirements:
- Standard installation wizard.
- Start menu shortcut.
- Optional desktop shortcut.
- Startup registration.
- Normal uninstallation.
- User data must remain after uninstallation unless explicitly requested otherwise.

The installed application must work independently of VS Code, Codex, Node.js development tools, or Rust development tools.

---

# 16. Settings

Organize Settings into logical sections:

### Appearance
- Light / Dark / Follow System
- Transparency presets
- Window size
- Window position
- Smart window behavior

### Behavior
- Startup
- Window mode
- Click-through
- Default startup page
- Sorting mode

### Categories
- Category management
- Default reminders

### Notifications
- Reminder defaults
- Sound mode
- Sound volume where supported
- DND schedules
- DND exceptions

### Shortcuts
- Shortcut list
- Registration status
- Conflict handling

### Data
- Database location
- Backup directory
- Export
- Import
- Restore

### About
- Kairos
- Version
- Product tagline
- License information

---

# 17. Development Roadmap

Do not attempt to implement all features in one unverified step.

Build the application incrementally.

## Phase 1 — Project Foundation

- Initialize Tauri 2 project.
- Configure React and TypeScript.
- Set up Rust backend.
- Create SQLite database.
- Implement migrations.
- Build initial application shell.

Deliverable:
A runnable Windows desktop application.

## Phase 2 — Visual Interface

- Frameless window.
- Custom window controls.
- Apple-inspired monochrome design.
- Frosted-glass appearance.
- Light and dark themes.
- Adaptive sizing.
- Drag and resize.
- Multi-monitor support.

Deliverable:
A polished desktop panel.

## Phase 3 — Core Task Management

- Quick Add.
- Task details.
- Categories.
- Priority.
- Scheduled Date.
- Due Date.
- Today / Upcoming / No Date.
- Overdue handling.
- Automatic and manual sorting.

Deliverable:
Fully functional local Todo application.

## Phase 4 — Advanced Task Features

- Subtasks.
- Parent-child completion.
- Recurring tasks.
- Recurrence editing scopes.
- Skip occurrence.
- Completed history.
- Trash and restoration.

Deliverable:
Complete task management engine.

## Phase 5 — Notifications

- Reminder scheduling.
- Category default reminders.
- Multiple reminders.
- Windows Toast notifications.
- Snooze.
- Missed reminder recovery.
- DND.
- Sound configuration.

Deliverable:
Reliable background reminder system.

## Phase 6 — Statistics and Search

- Global search.
- AND-based filtering.
- Daily progress.
- Weekly statistics.
- Streak.
- Rest days.

Deliverable:
Complete statistics and discovery experience.

## Phase 7 — System Integration

- System tray.
- Startup.
- Global shortcuts.
- Smart always-on-top.
- Fullscreen detection.
- Click-through.
- Background behavior.

Deliverable:
Integrated Windows desktop companion.

## Phase 8 — Backup and Release

- Automatic backup.
- Replace and Merge restore.
- Data export and import.
- Error handling.
- Installer.
- Uninstaller.
- Release build.

Deliverable:
Standalone Windows installer.

---

# 18. Testing and Acceptance Criteria

The application must not be considered complete merely because the interface looks correct.

Verify actual behavior.

## Core Functionality

- Tasks can be created, edited, completed, restored, and deleted.
- Categories and priorities persist after restart.
- Scheduled Date and Due Date remain independent.
- Overdue tasks move to Today.
- Rescheduled tasks preserve their history.
- Subtask completion works correctly.
- Recurrence generates the correct next occurrence.
- Skip does not count as completion.
- Completed records remain available.

## Notification Reliability

- Reminders trigger at the correct local time.
- Multiple reminders work independently.
- Completed tasks cancel remaining reminders.
- Snooze works.
- Sleep recovery works.
- Missed reminders are aggregated.
- DND works.
- Task-level DND exceptions work within application-level limits.

## Window Behavior

- Close hides to tray.
- Tray restores the window.
- Full exit terminates the process.
- Startup works.
- Global shortcuts work.
- Click-through can always be disabled.
- Smart mode handles fullscreen applications.
- Multi-monitor positioning works.
- Window dimensions persist.

## Data Safety

- All data survives application restart.
- All data survives Windows restart.
- Database migrations preserve existing information.
- Backups are valid.
- Replace restoration works.
- Merge restoration works.
- Failed restore does not corrupt the active database.

## Performance

The application should:
- Start quickly.
- Remain responsive.
- Avoid unnecessary CPU usage while idle.
- Avoid unnecessary memory consumption.
- Avoid excessive background polling.
- Handle large task histories without significant UI slowdown.

## Final Release

A release is accepted only when:

1. The application builds successfully.
2. The installer installs correctly.
3. The application runs without development tools.
4. Core features pass testing.
5. Windows integration works.
6. Data persistence and backup are verified.
7. No critical known data-loss bugs remain.

---

# 19. Instructions to Codex

You are responsible for implementing Kairos as a real, usable Windows desktop application.

Important development instructions:

1. First inspect the existing repository and development environment.
2. Before implementation, provide a concise architecture plan.
3. Create a clear project structure.
4. Implement features incrementally according to the roadmap.
5. Build and run the application after every major phase.
6. Write automated tests for business logic and database operations.
7. Test Windows-specific behavior on Windows 11.
8. Do not simulate functionality that is not actually implemented.
9. Do not leave critical buttons or interactions as placeholders.
10. Do not silently remove previously implemented features when modifying the code.
11. Maintain documentation for database schema, migrations, and build instructions.
12. Keep all user data local in version 1.
13. Report technical limitations honestly.
14. Verify the final installer on a clean Windows environment.

For technically uncertain features, especially:
- Windows notification sound volume.
- Fullscreen detection.
- Smart always-on-top.
- Custom window transparency.
- Global shortcut registration.

Research the actual Windows and Tauri capabilities before committing to an implementation.

If a requirement cannot be implemented exactly, explain the limitation and propose the closest reliable alternative.

Do not prioritize visual appearance over functional correctness.

The final result must be a stable, usable, polished desktop application.

**Remember the product philosophy: At your own pace.**

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