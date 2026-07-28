# iQs Jobs Email Setup

Production email uses Action Mailer over SMTP. Client upload notifications are queued with Solid Queue, so production needs either the `worker` process running from the `Procfile` or `SOLID_QUEUE_IN_PUMA=true`.

Required config:

```bash
APP_HOST=your-live-domain.com
MAIL_FROM="iQs Jobs <no-reply@your-domain.com>"
CLIENT_SUBMISSIONS_EMAIL=office-notifications@your-domain.com
SMTP_ADDRESS=smtp.provider.com
SMTP_USERNAME=your-smtp-username
SMTP_PASSWORD=your-smtp-password
SMTP_PORT=587
SMTP_AUTHENTICATION=plain
SMTP_ENABLE_STARTTLS_AUTO=true
```

On Heroku:

```bash
heroku config:set APP_HOST=your-live-domain.com -a cd-projects
heroku config:set MAIL_FROM="iQs Jobs <no-reply@your-domain.com>" -a cd-projects
heroku config:set CLIENT_SUBMISSIONS_EMAIL=office-notifications@your-domain.com -a cd-projects
heroku config:set SMTP_ADDRESS=smtp.provider.com SMTP_USERNAME=your-smtp-username SMTP_PASSWORD=your-smtp-password -a cd-projects
heroku config:set SMTP_PORT=587 SMTP_AUTHENTICATION=plain SMTP_ENABLE_STARTTLS_AUTO=true -a cd-projects
heroku ps:scale worker=1 -a cd-projects
```

Emails currently sent:

- Portal access enabled: sent to the contributor login email when project upload access is enabled.
- Project uploaded: sent to the contributor login email and `CLIENT_SUBMISSIONS_EMAIL` when a client submits a project.
