# NextAuth.js Setup Guide

This project is configured with NextAuth.js using Google OAuth provider and JWT strategy.

## Environment Variables Required

Create a `.env` file in the `apps/frontend` directory with the following variables:

```env
# NextAuth.js Configuration
AUTH_SECRET=your-auth-secret-here

# Google OAuth Provider
GOOGLE_CLIENT_ID=your-google-client-id-here
GOOGLE_CLIENT_SECRET=your-google-client-secret-here

# NextAuth.js URL (for production, use your domain)
NEXTAUTH_URL=http://localhost:3000
```

## Getting Google OAuth Credentials

1. Go to the [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select an existing one
3. Enable the Google+ API
4. Go to "Credentials" and create an OAuth 2.0 Client ID
5. Set the authorized redirect URI to: `http://localhost:3000/api/auth/callback/google`
6. Copy the Client ID and Client Secret to your `.env` file

## Generating AUTH_SECRET

You can generate a secure secret using:

```bash
openssl rand -base64 32
```

Or use any secure random string generator.

## Usage

The authentication is now set up and ready to use. You can:

- Use the `LoginButton` component to sign in/out
- Use `useSession()` hook to access session data
- Use `signIn()` and `signOut()` functions for programmatic authentication

## Features

- ✅ Google OAuth provider
- ✅ JWT strategy for session management
- ✅ Secure token signing with AUTH_SECRET
- ✅ TypeScript support with proper type extensions
- ✅ Session provider wrapping the entire app 