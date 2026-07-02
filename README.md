# QS Jobs

Internal Rails app for tracking QS projects, contributors, and project documents.

## Operational Notes

- Production document uploads use Active Storage with S3.
- Large PDFs are uploaded directly from the browser to S3 to avoid Heroku router timeouts.
- See [docs/s3_storage.md](docs/s3_storage.md) for the required production environment variables, bucket permissions, and CORS setup.

## Common Commands

```sh
bin/rails test
```
