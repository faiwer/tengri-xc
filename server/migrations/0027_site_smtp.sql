-- SMTP + mail templates on the site_settings singleton. Connection fields are
-- nullable (mail is optional). smtp_tls is an enum because implicit (465) and
-- STARTTLS (587) are different protocols.

CREATE TYPE smtp_tls AS ENUM ('implicit', 'starttls', 'none');

ALTER TABLE site_settings
    ADD COLUMN smtp_host      text,
    ADD COLUMN smtp_port      integer
        CHECK (smtp_port IS NULL OR smtp_port BETWEEN 1 AND 65535),
    ADD COLUMN smtp_tls       smtp_tls,
    ADD COLUMN smtp_username  text,
    ADD COLUMN smtp_password  text,
    ADD COLUMN from_address   text,
    ADD COLUMN title_template text NOT NULL DEFAULT '%title%',
    ADD COLUMN body_template  text NOT NULL DEFAULT '%body%';
