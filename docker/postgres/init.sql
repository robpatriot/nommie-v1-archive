-- Create the nommie schema
CREATE SCHEMA IF NOT EXISTS nommie;

-- Grant usage and create permissions on the nommie schema
GRANT USAGE ON SCHEMA nommie TO nommie_user;
GRANT CREATE ON SCHEMA nommie TO nommie_user;

-- Grant all privileges on all tables in the nommie schema
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA nommie TO nommie_user;

-- Grant all privileges on all sequences in the nommie schema
GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA nommie TO nommie_user;

-- Set default privileges for future tables and sequences
ALTER DEFAULT PRIVILEGES IN SCHEMA nommie GRANT ALL ON TABLES TO nommie_user;
ALTER DEFAULT PRIVILEGES IN SCHEMA nommie GRANT ALL ON SEQUENCES TO nommie_user;

-- Set the default search_path for nommie_user
ALTER USER nommie_user SET search_path TO nommie;