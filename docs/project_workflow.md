# Current Projects

Administrators land on Current Projects after login. Clients continue to land in the Client Area.

The board has one column per project status, a count for each column, and cards linking to the full project. Use **Change status or schedule** on a card to move a job or change its scheduled due date. Use **Edit Statuses** to rename, reorder, or add stages; new projects start in the first stage.

Jobs are ordered within each column by scheduled due date, falling back to the earliest requested date from linked client submissions. Jobs without either date appear last. Equal dates are ordered by project code. Clearing a scheduled date returns the job to the client requested date. Client dates remain on the original submissions.

**Archive Project** moves a job into the Archived Projects register. **Restore Project** on the project detail page returns it to its previous board stage. Archiving preserves all documents, contributors, and financial details.

To remove a stage, open **Edit Statuses**, click **Delete** beside its **Save** button, and confirm deletion. If any projects use that stage, choose a replacement first. All assigned jobs, including archived jobs, move to that stage without changing their archive state or due dates. The final remaining status cannot be deleted.

## Rollout

Run `bin/rails db:migrate`. Existing projects are marked archived and stay in Archived Projects, so historical jobs do not appear as new enquiries. Restore any existing live work to the board and assign its stage. All new projects, including converted client uploads, start on the board.

For a fresh database loaded from the schema, run `bin/rails db:seed` to create the five default statuses. Seeding leaves existing customized statuses alone.

## Project duration and scheduling

Set **Duration (working days)** on the project form or its Current Projects card. Duration is an optional positive whole number and counts Monday–Friday; weekends are left free for catch-up. Public holidays are counted if they fall on a weekday.

Choose **Schedule from**:

- **Due date — calculate start**: enter a scheduled due date, or leave it blank to use the earliest linked client requested date. Saving a duration calculates the latest start date. A Saturday or Sunday deadline plans the work to finish on the preceding Friday, keeping the original deadline intact.
- **Start date — calculate due date**: enter a Monday–Friday start date and duration. Saving calculates and stores the scheduled due date, which determines the card order. Changing the start or duration recalculates that date.

The first day counts toward duration: a one-day job starts and finishes on the same weekday. A three-day job starting Thursday finishes Monday. Dates and duration appear on the cards and project details. To resume using the client deadline, choose scheduling from due date and clear the scheduled due date. No existing jobs receive a default duration.
