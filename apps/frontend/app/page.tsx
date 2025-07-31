'use client';

import { useState, useEffect } from 'react';
import { useSession } from 'next-auth/react';
import { LoginButton } from '@/components/auth/LoginButton';
import { ProtectedApiTest } from '@/components/auth/ProtectedApiTest';
import { BACKEND_URL } from '@/lib/config';

export default function Home() {
  const { data: session, status } = useSession();
  const [data, setData] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchBackendData = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const response = await fetch(`${BACKEND_URL}/`);
      
      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }
      
      const result = await response.text();
      setData(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'An error occurred');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchBackendData();
  }, []);

  return (
    <div className="min-h-screen relative">
      {/* Main content area - ready for real UI */}
      <div className="flex items-center justify-center p-8 min-h-screen">
        <div className="max-w-4xl w-full space-y-8">
          <div className="text-center">
            <h1 className="text-4xl font-bold mb-4">Welcome to Nommie</h1>
            <p className="text-xl text-gray-600 dark:text-gray-400">
              Your main application content goes here
            </p>
          </div>
          
          {/* Lobby Navigation - Only show if authenticated */}
          {status === 'authenticated' && (
            <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6 border border-gray-200 dark:border-gray-700">
              <h3 className="text-lg font-semibold mb-4">Game Lobby</h3>
              <p className="text-gray-600 dark:text-gray-400 mb-4">
                Join existing games or create new ones in the lobby.
              </p>
              <a
                href="/lobby"
                className="inline-block px-6 py-3 bg-blue-600 text-white rounded-lg hover:bg-blue-700 font-medium"
              >
                Go to Lobby
              </a>
            </div>
          )}
        </div>
      </div>

      {/* Floating backend test elements at the bottom */}
      <div className="fixed bottom-4 left-4 right-4 flex gap-4 z-50">
        {/* Backend Connection Test */}
        <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-4 border border-gray-200 dark:border-gray-700 flex-1 max-w-sm">
          <h3 className="text-sm font-semibold mb-2 text-center">Backend Connection</h3>
          
          {loading && (
            <div className="text-center">
              <div className="animate-spin rounded-full h-4 w-4 border-b-2 border-blue-600 mx-auto"></div>
              <p className="mt-1 text-xs text-gray-600 dark:text-gray-400">Connecting...</p>
            </div>
          )}
          
          {error && (
            <div className="text-center">
              <div className="text-red-600 dark:text-red-400 text-xs mb-1">❌ Error</div>
              <p className="text-xs text-gray-600 dark:text-gray-400 mb-2 truncate">{error}</p>
              <button
                onClick={fetchBackendData}
                className="bg-blue-600 hover:bg-blue-700 text-white px-2 py-1 rounded text-xs"
              >
                Retry
              </button>
            </div>
          )}
          
          {data && !loading && !error && (
            <div className="text-center">
              <div className="text-green-600 dark:text-green-400 text-xs mb-1">✅ Connected</div>
              <button
                onClick={fetchBackendData}
                className="bg-gray-600 hover:bg-gray-700 text-white px-2 py-1 rounded text-xs"
              >
                Refresh
              </button>
            </div>
          )}
        </div>

        {/* Authentication Test */}
        <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-4 border border-gray-200 dark:border-gray-700 flex-1 max-w-sm">
          <h3 className="text-sm font-semibold mb-2 text-center">Authentication</h3>
          <LoginButton />
        </div>

        {/* Protected API Test */}
        <div className="bg-white dark:bg-gray-800 shadow-lg rounded-lg p-4 border border-gray-200 dark:border-gray-700 flex-1 max-w-sm">
          <h3 className="text-sm font-semibold mb-2 text-center">Protected API</h3>
          <ProtectedApiTest />
        </div>
      </div>
    </div>
  );
}
