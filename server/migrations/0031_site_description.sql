-- Short prose blurb for the site, used as the default meta/OG description.
-- Nullable: an unset description is the same as an empty one.

ALTER TABLE site_settings
    ADD COLUMN site_description text;
