# S3 Storage

Production Active Storage uploads are configured to use the `amazon` service in `config/storage.yml`.

## Required Environment Variables

Set these in the production host:

- `ACTIVE_STORAGE_SERVICE=amazon`
- `AWS_REGION`
- `AWS_BUCKET`
- `AWS_ACCESS_KEY_ID`
- `AWS_SECRET_ACCESS_KEY`

If the production host runs with an AWS IAM role that can access the bucket, `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` can be omitted.

## Bucket Permissions

The app should use a private bucket. Documents are served through Rails/Active Storage signed URLs rather than public S3 object URLs.

The app role or access key needs permission to:

- read objects
- write objects
- delete objects
- list bucket objects if operational tooling needs it

## Browser Direct Uploads

Project document uploads use Active Storage direct uploads. The browser asks Rails for a signed upload target, sends the file directly to S3, then submits the completed blob ID back to Rails with the document metadata. This avoids Heroku router timeouts for large drawing sets.

The S3 bucket must allow browser CORS requests from the production app host. A typical private-bucket CORS rule is:

```json
[
  {
    "AllowedHeaders": ["*"],
    "AllowedMethods": ["PUT"],
    "AllowedOrigins": ["https://cd-projects-392d0aa8a48a.herokuapp.com"],
    "ExposeHeaders": ["ETag"],
    "MaxAgeSeconds": 3000
  }
]
```

Add any custom production domain to `AllowedOrigins` before switching traffic to it.

## Notes

- Development and test still use local disk storage.
- Production defaults to `amazon`; set `ACTIVE_STORAGE_SERVICE=local` only for emergency fallback.
- Existing local uploads are not automatically copied to S3 by this config change.
