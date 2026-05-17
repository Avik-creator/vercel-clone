-- Add github_access_token to users table for fetching repositories
ALTER TABLE users ADD COLUMN github_access_token TEXT;
