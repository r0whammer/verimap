import { useEffect, useState } from 'react';
import { CoreBridge, type CoreStatus } from './bridge.ts';

/** Starts one core worker for the component's lifetime and tracks its status. */
export function useCore(): CoreStatus {
  const [status, setStatus] = useState<CoreStatus>({ state: 'starting' });
  useEffect(() => {
    const bridge = new CoreBridge();
    const unsubscribe = bridge.subscribe(setStatus);
    return () => {
      unsubscribe();
      bridge.dispose();
    };
  }, []);
  return status;
}
