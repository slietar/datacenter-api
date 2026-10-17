export class ModernEventSource<T> {
  private eventSource: EventSource;
  private queue: T[] = [];
  private waiter: PromiseWithResolvers<void> | null = null;

  readonly ready: Promise<void>;

  constructor(url: URL | string) {
    this.eventSource = new EventSource(url.toString());

    let { promise, resolve, reject } = Promise.withResolvers<void>();

    this.ready = promise;
    this.eventSource.addEventListener('open', () => void resolve(), { once: true });
    this.eventSource.addEventListener('error', () => {
      reject(new Error('Failed to connect to EventSource'));
    }, { once: true });

    this.eventSource.addEventListener('message', (event) => {
      this.queue.push(JSON.parse(event.data) as T);

      if (this.waiter) {
        this.waiter.resolve();
        this.waiter = null;
      }
    });
  }

  close() {
    this.eventSource.close();
  }

  async event() {
    while (true) {
      if (this.queue.length > 0) {
        return this.queue.shift()!;
      } else {
        if (!this.waiter) {
          this.waiter = Promise.withResolvers();
        }

        await this.waiter.promise;
      }
    }
  }

  async * events() {
    while (true) {
      if (this.queue.length > 0) {
        yield this.queue.shift()!;
      } else {
        if (!this.waiter) {
          this.waiter = Promise.withResolvers();
        }

        await this.waiter.promise;
      }
    }
  }
}
