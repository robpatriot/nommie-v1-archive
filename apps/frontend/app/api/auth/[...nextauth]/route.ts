import NextAuth from 'next-auth';
import { config } from '@/lib/auth';

const { handlers } = NextAuth(config);

// Only export HTTP methods from a route module
export const { GET, POST } = handlers;
