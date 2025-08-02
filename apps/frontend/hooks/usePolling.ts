import { useEffect, useRef, useState } from 'react';

interface UsePollingOptions {
  enabled: boolean;
  interval: number;
  callback: () => void | Promise<void>;
  immediate?: boolean;
}

interface UsePollingReturn {
  isPolling: boolean;
}

/**
 * A reusable hook for polling that calls a callback at regular intervals
 * while a condition is true.
 * 
 * @param options.enabled - Whether polling should be active
 * @param options.interval - Polling interval in milliseconds
 * @param options.callback - Function to call on each poll
 * @param options.immediate - Whether to call the callback immediately on mount (default: true)
 * @returns Object with isPolling state
 */
export function usePolling({
  enabled,
  interval,
  callback,
  immediate = true,
}: UsePollingOptions): UsePollingReturn {
  const intervalRef = useRef<NodeJS.Timeout | null>(null);
  const callbackRef = useRef(callback);
  const [isPolling, setIsPolling] = useState(false);

  // Keep the callback ref up to date
  useEffect(() => {
    callbackRef.current = callback;
  }, [callback]);

  // Safe wrapper for the callback that handles both sync and async errors
  const safeCallback = () => {
    try {
      const result = callbackRef.current();
      if (result instanceof Promise) {
        result.catch((error) => {
          console.error('Polling callback error:', error);
        });
      }
    } catch (error) {
      console.error('Polling callback error:', error);
    }
  };

  useEffect(() => {
    if (enabled) {
      // Call immediately if requested
      if (immediate) {
        safeCallback();
      }

      // Start polling
      setIsPolling(true);
      intervalRef.current = setInterval(() => {
        safeCallback();
      }, interval);

      // Cleanup function
      return () => {
        if (intervalRef.current) {
          clearInterval(intervalRef.current);
          intervalRef.current = null;
        }
        setIsPolling(false);
      };
    } else {
      // Clear interval if disabled
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
        intervalRef.current = null;
      }
      setIsPolling(false);
    }
  }, [enabled, interval, immediate]);

  return {
    isPolling,
  };
} 