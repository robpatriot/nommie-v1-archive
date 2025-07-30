'use client';

import { useState, useEffect } from 'react';
import { LoginButton } from '@/components/auth/LoginButton';
import { ProtectedApiTest } from '@/components/auth/ProtectedApiTest';

export default function Home() {
  const [data, setData] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchBackendData = async () => {
    setLoading(true);
    setError(null);
    
    try {
      const response = await fetch('http://localhost:8080/');
      
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
    <div className="min-h-screen flex items-center justify-center p-8 relative">
      {/* Top right box for ProtectedApiTest */}
      <div className="absolute top-6 right-6 w-80 bg-white dark:bg-gray-800 shadow-lg rounded-lg p-4 z-10 border border-gray-200 dark:border-gray-700">
        <h2 className="text-lg font-semibold mb-2 text-center">Protected API Test</h2>
        <ProtectedApiTest />
      </div>
      <div className="max-w-md w-full space-y-6">
        <h1 className="text-3xl font-bold text-center">Backend Connection Test</h1>
        
        {/* Authentication Section */}
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
          <h2 className="text-xl font-semibold mb-4 text-center">Authentication</h2>
          <LoginButton />
        </div>
        
        <div className="bg-white dark:bg-gray-800 shadow rounded-lg p-6">
          {loading && (
            <div className="text-center">
              <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-600 mx-auto"></div>
              <p className="mt-2 text-gray-600 dark:text-gray-400">Connecting to backend...</p>
            </div>
          )}
          
          {error && (
            <div className="text-center">
              <div className="text-red-600 dark:text-red-400 mb-2">❌ Error</div>
              <p className="text-sm text-gray-600 dark:text-gray-400 mb-4">{error}</p>
              <button
                onClick={fetchBackendData}
                className="bg-blue-600 hover:bg-blue-700 text-white px-4 py-2 rounded text-sm"
              >
                Retry
              </button>
            </div>
          )}
          
          {data && !loading && !error && (
            <div className="text-center">
              <div className="text-green-600 dark:text-green-400 mb-2">✅ Success</div>
              <p className="text-sm text-gray-600 dark:text-gray-400 mb-2">Backend response:</p>
              <pre className="bg-gray-100 dark:bg-gray-700 p-3 rounded text-sm overflow-x-auto">
                {data}
              </pre>
              <button
                onClick={fetchBackendData}
                className="mt-4 bg-gray-600 hover:bg-gray-700 text-white px-4 py-2 rounded text-sm"
              >
                Refresh
              </button>
            </div>
          )}
        </div>
        
        <p className="text-xs text-center text-gray-500 dark:text-gray-400">
          Testing connection to http://localhost:8080/
        </p>
      </div>
    </div>
  );
}
