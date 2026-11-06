# Client hourly rate schedules

Clients use **Client Area → Hourly Rates** to maintain their own schedule. Each trade or role has optional Low, Mid and High amounts in dollars per hour. No default amounts are supplied. Blank means unset; zero is an explicit zero rate. Entered bands must be non-negative and in ascending order. Saving validates the entire schedule before changing any rates.

The starting catalogue has 47 active rows. Codes are unique, case-insensitive and stored in uppercase; they allow 1–20 letters, numbers, dots and hyphens (for example, `LABOUR.01-HIGH`). The initial defaults use three letters. The catalogue consolidates Formwork Carpenter, Stone Masonry, Wet Plastering (WPL) and Dry Plastering (DPL); Journeyman is removed because Low/Mid/High handles the rate bands. Structural Steel Welder (SSW) and Structural Steel Erector (SSE) remain separate roles.

Admins use **Rate Categories** in the navigation to add roles, change codes and names, choose a section and sort order, indent a role, or remove and restore categories. Edits apply to all client schedules. Removing archives a category; it does not delete entered rates. After removing a category, the separate **Delete** action under **Removed categories** permanently deletes an unused category after confirmation. Active categories cannot be deleted directly. Deletion is blocked when client rates reference the category, including archived rates. Removed codes remain reserved to prevent ambiguity. Restoring a category makes its existing rates available again. Internal keys are permanent, so code and name changes do not detach rates.

When consolidating defaults, the migration copies a removed role’s rates into the retained role only if the retained role has no amounts. Conflicting originals remain on the removed category and can be recovered by restoring it. New databases loaded from the schema must run `bin/rails db:seed` to install missing default categories; seeding preserves admin edits.

Admins review each builder’s schedule from their contributor page using **View Hourly Rates**. The rate page identifies the builder and links back to their contributor page; it has no schedule selector or links from the submissions list. Clients edit their own rates; admin review is read-only.

Rates are maintained at the client account level. Hire Items uses an hourly allowance.

The earlier single `users.hourly_rate` column is retained to preserve any existing values, but the interface now uses the categorized schedule. The old `/hourly_rate/edit` link redirects to the new page.

## Classification references

These are internal estimating resource codes, not official standard codes. Relevant Australian references classify different things:

- [ABS OSCA](https://www.abs.gov.au/statistics/classifications/osca-occupation-standard-classification-australia/2024-version-1-0): occupations.
- [NATSPEC National Classification System](https://natspec.com.au/resources/national-classification-system): specification worksections.
- [AIQS publications](https://www.aiqs.com.au/publications): cost-management and measurement references, including the Australian Cost Management Manual.

No official classification mapping is asserted or stored by this feature.

## Deletion protection

PostgreSQL foreign keys use `ON DELETE RESTRICT` from client rates to both their category and user. A referenced category or user cannot be physically deleted, including via direct SQL or Rails methods that bypass callbacks. Rails also refuses normal deletion of categories and users with rates instead of destroying the dependent rates. Removing a category in admin remains an archive operation.

Future iQs library tables must reference their source rate/category through restrictive foreign keys as well. No library relationships exist yet; these must be added alongside the library schema rather than relying on application callbacks or codes stored as plain text.

## Admin averages

Open **Rate Categories → Hourly Rate Averages**, or **All Client Averages** from a builder’s rate page. The admin-only report calculates the arithmetic mean separately for each active role and Low/Mid/High band across client users. Blank amounts are excluded, explicit zero amounts are included, and each band shows its own contributing-user count. Admin users’ schedules and archived categories are excluded. The report reflects current saved client rates; it is not an external market-rate benchmark.

## Section order

Use **Rate Categories → Section order** to set each section’s position. Lower numbers appear first; equal positions are ordered by section name. This controls the category list, section dropdown, client schedules and admin averages. A role’s own sort order controls its position within its section. Existing sections and amounts are preserved when the ordering feature is installed.
