import { useSyncExternalStore } from 'react';

import { ModernEventSource } from './sse.js';

export type Stream<T> = {
  initialValue: T;
  (): T;
};

export async function createStream<T>(url: URL | string) {
  let urlString = url.toString();

  let initialSource = new ModernEventSource<T>(urlString);
  let initialState: T;

  try {
    await initialSource.ready;
    initialState = await initialSource.event();
  } finally {
    initialSource.close();
  }

  let state = initialState;
  let listeners = new Set<() => void>();
  let eventSource: ModernEventSource<T> | null = null;

  async function consume(source: ModernEventSource<T>) {
    let closed = Symbol('closed');

    try {
      while (true) {
        let event = await Promise.race([
          source.event(),
          source.closed.then(() => closed),
        ]);

        if (event === closed) {
          return;
        }

        state = event as T;

        for (let listener of listeners) {
          listener();
        }
      }
    } catch {
      // Connection failed; keep the last known state.
    }
  }

  // The connection lives only while there is at least one subscriber, so
  // StrictMode's mount/unmount/mount just closes and reopens it.
  function subscribe(listener: () => void) {
    listeners.add(listener);

    if (listeners.size === 1) {
      eventSource = new ModernEventSource<T>(urlString);
      consume(eventSource);
    }

    return () => {
      listeners.delete(listener);

      if (listeners.size === 0) {
        eventSource?.close();
        eventSource = null;
      }
    };
  }

  function getSnapshot() {
    return state;
  }

  let stream = (() => useSyncExternalStore(subscribe, getSnapshot)) as Stream<T>;

  stream.initialValue = initialState;

  return stream;
}
